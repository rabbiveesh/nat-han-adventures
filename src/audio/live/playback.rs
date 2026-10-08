//! Playing the engine through kira: a custom [`Sound`] that runs [`Engine`]s on the audio
//! thread, fed by a lock-free command queue.
//!
//! - **Threads.** kira calls [`Sound::process`] from its renderer, in chunks of its internal
//!   buffer (128 frames by default). On native that's cpal's audio thread; on the web, cpal's
//!   WebAudio host runs the same callback on the main thread (it schedules `AudioBuffer`s from
//!   `onended` callbacks), which is fine: a block costs microseconds. So there's one design for
//!   both, and no ring buffer of rendered audio: the engine renders just in time, a few
//!   milliseconds ahead of the speaker.
//! - **Rate.** The engine runs at [`ENGINE_RATE`] (32 kHz, like the offline renderer, so the
//!   output matches it sample for sample); each deck resamples to the device rate with 4-point
//!   Hermite interpolation, as kira does for static sounds.
//! - **In.** [`LiveHandle`] (main thread) sends [`Command`]s over an `rtrb` SPSC ring buffer:
//!   inputs, a new song (an [`Engine`] built on the main thread, crossfaded in), stop, volume.
//!   Engines that have faded out go back over a second ring buffer, so nothing is freed on the
//!   audio thread.
//! - **Out.** After each chunk the sound publishes the engine's [`EngineState`] and
//!   [`BeatClock`] into a mutex it only ever `try_lock`s (it never waits; the main thread
//!   holds it for a copy), with how far the engine has rendered ahead of the output.

use std::sync::{Arc, Mutex};

use kira::Frame;
use kira::info::Info;
use kira::sound::{Sound, SoundData};

use super::engine::{BeatClock, Engine, EngineState, Input};

/// The engine's sample rate.
pub const ENGINE_RATE: u32 = 32_000;
/// Frames the engine renders at a time.
const CHUNK: usize = 128;
/// Commands in flight.
const COMMANDS: usize = 1024;

/// Main thread → audio thread.
pub enum Command {
    Input(Input),
    /// Play this engine, fading it in over `fade_in` seconds while the current one fades out
    /// over `fade_out`.
    Play { engine: Box<Engine>, fade_in: f32, fade_out: f32 },
    /// Fade out and stop.
    Stop { fade: f32 },
    /// Master volume (linear gain), over `secs`.
    Volume { gain: f32, secs: f32 },
}

/// What the audio thread last published.
#[derive(Debug, Clone, Default)]
pub struct Published {
    pub state: EngineState,
    pub clock: BeatClock,
    /// Seconds of audio rendered but not output yet (the clock is that far ahead of the ear).
    pub ahead_secs: f64,
    /// A song is playing.
    pub playing: bool,
    /// Chunks processed so far.
    pub chunks: u64,
}

/// A linear gain ramp.
#[derive(Debug, Clone, Copy)]
struct Ramp {
    value: f32,
    target: f32,
    step: f32,
}

impl Ramp {
    fn new(value: f32) -> Self {
        Ramp { value, target: value, step: 0.0 }
    }

    fn to(&mut self, target: f32, secs: f32, rate: f32) {
        self.target = target;
        let n = (secs * rate).max(1.0);
        self.step = (target - self.value) / n;
    }

    #[inline]
    fn next(&mut self) -> f32 {
        if self.value != self.target {
            self.value += self.step;
            if (self.step > 0.0 && self.value >= self.target) || (self.step < 0.0 && self.value <= self.target) || self.step == 0.0 {
                self.value = self.target;
            }
        }
        self.value
    }
}

/// An engine with its resampler and fader.
struct Deck {
    engine: Box<Engine>,
    chunk: [Frame; CHUNK],
    pos: usize,
    hist: [Frame; 4],
    frac: f64,
    gain: Ramp,
}

impl Deck {
    fn new(engine: Box<Engine>) -> Self {
        Deck { engine, chunk: [Frame::ZERO; CHUNK], pos: CHUNK, hist: [Frame::ZERO; 4], frac: 0.0, gain: Ramp::new(0.0) }
    }

    #[inline]
    fn pull(&mut self) -> Frame {
        if self.pos == CHUNK {
            self.engine.fill(&mut self.chunk);
            self.pos = 0;
        }
        self.pos += 1;
        self.chunk[self.pos - 1]
    }

    /// The next output frame, `step` engine frames on.
    #[inline]
    fn next(&mut self, step: f64) -> Frame {
        let [x0, x1, x2, x3] = self.hist;
        let t = self.frac as f32;
        let out = hermite(x0, x1, x2, x3, t) * self.gain.next();
        self.frac += step;
        while self.frac >= 1.0 {
            self.frac -= 1.0;
            let [_, a, b, c] = self.hist;
            self.hist = [a, b, c, self.pull()];
        }
        out
    }

    /// Engine frames rendered but not output yet.
    fn ahead(&self) -> usize {
        CHUNK - self.pos + 3
    }
}

/// 4-point, 3rd-order Hermite between `x1` and `x2`.
#[inline]
fn hermite(x0: Frame, x1: Frame, x2: Frame, x3: Frame, t: f32) -> Frame {
    let c0 = x1;
    let c1 = (x2 - x0) * 0.5;
    let c2 = x0 - x1 * 2.5 + x2 * 2.0 - x3 * 0.5;
    let c3 = (x3 - x0) * 0.5 + (x1 - x2) * 1.5;
    ((c3 * t + c2) * t + c1) * t + c0
}

/// The kira sound.
pub struct LiveSound {
    commands: rtrb::Consumer<Command>,
    trash: rtrb::Producer<Box<Engine>>,
    shared: Arc<Mutex<Published>>,
    current: Option<Deck>,
    outgoing: Option<Deck>,
    volume: Ramp,
    stopping: bool,
    chunks: u64,
}

impl LiveSound {
    fn retire(&mut self, deck: Deck) {
        if let Err(rtrb::PushError::Full(e)) = self.trash.push(deck.engine) {
            drop(e);
        }
    }

    fn command(&mut self, c: Command, rate: f32) {
        match c {
            Command::Input(i) => {
                if let Some(d) = &mut self.current {
                    d.engine.post(i);
                }
            }
            Command::Play { engine, fade_in, fade_out } => {
                if let Some(old) = self.outgoing.take() {
                    self.retire(old);
                }
                if let Some(mut old) = self.current.take() {
                    old.gain.to(0.0, fade_out, rate);
                    self.outgoing = Some(old);
                }
                let mut d = Deck::new(engine);
                d.gain.to(1.0, fade_in, rate);
                self.current = Some(d);
                self.stopping = false;
            }
            Command::Stop { fade } => {
                if let Some(d) = &mut self.current {
                    d.gain.to(0.0, fade, rate);
                }
                self.stopping = true;
            }
            Command::Volume { gain, secs } => self.volume.to(gain, secs, rate),
        }
    }

    fn publish(&mut self) {
        self.chunks += 1;
        let Ok(mut p) = self.shared.try_lock() else { return };
        p.chunks = self.chunks;
        match &self.current {
            Some(d) => {
                d.engine.state_into(&mut p.state);
                p.clock = d.engine.beat_clock();
                p.ahead_secs = d.ahead() as f64 / ENGINE_RATE as f64;
                p.playing = !self.stopping;
            }
            None => p.playing = false,
        }
    }
}

impl Sound for LiveSound {
    fn process(&mut self, out: &mut [Frame], dt: f64, _info: &Info) {
        let rate = (1.0 / dt) as f32;
        while let Ok(c) = self.commands.pop() {
            self.command(c, rate);
        }
        let step = ENGINE_RATE as f64 * dt;
        for f in out.iter_mut() {
            let mut x = Frame::ZERO;
            if let Some(d) = &mut self.current {
                x += d.next(step);
            }
            if let Some(d) = &mut self.outgoing {
                x += d.next(step);
            }
            *f = x * self.volume.next();
        }
        if self.outgoing.as_ref().is_some_and(|d| d.gain.value == 0.0 && d.gain.target == 0.0) {
            let d = self.outgoing.take().unwrap();
            self.retire(d);
        }
        if self.stopping && self.current.as_ref().is_some_and(|d| d.gain.value == 0.0) {
            let d = self.current.take().unwrap();
            self.retire(d);
        }
        self.publish();
    }

    fn finished(&self) -> bool {
        self.commands.is_abandoned() && self.current.is_none()
    }
}

/// The main thread's side.
pub struct LiveHandle {
    commands: rtrb::Producer<Command>,
    trash: rtrb::Consumer<Box<Engine>>,
    shared: Arc<Mutex<Published>>,
}

impl LiveHandle {
    fn send(&mut self, c: Command) -> bool {
        self.collect_garbage();
        self.commands.push(c).is_ok()
    }

    /// Forward an input to the engine playing. False if the queue is full.
    pub fn post(&mut self, input: Input) -> bool {
        self.send(Command::Input(input))
    }

    /// Crossfade to a new engine.
    pub fn play(&mut self, engine: Engine, fade_in: f32, fade_out: f32) -> bool {
        self.send(Command::Play { engine: Box::new(engine), fade_in, fade_out })
    }

    pub fn stop(&mut self, fade: f32) -> bool {
        self.send(Command::Stop { fade })
    }

    /// Master volume in dB, over `secs`.
    pub fn set_volume_db(&mut self, db: f32, secs: f32) -> bool {
        self.send(Command::Volume { gain: 10f32.powf(db / 20.0), secs })
    }

    /// The last published state (a copy).
    pub fn published(&self) -> Published {
        self.shared.lock().map(|p| p.clone()).unwrap_or_default()
    }

    /// Drop the engines the audio thread is done with.
    pub fn collect_garbage(&mut self) {
        while self.trash.pop().is_ok() {}
    }
}

/// [`SoundData`] for [`kira::AudioManager::play`]: the sound, and its handle.
pub struct LiveSoundData {
    sound: LiveSound,
    handle: LiveHandle,
}

impl LiveSoundData {
    pub fn new() -> Self {
        let (tx, rx) = rtrb::RingBuffer::new(COMMANDS);
        let (trash_tx, trash_rx) = rtrb::RingBuffer::new(8);
        let shared = Arc::new(Mutex::new(Published::default()));
        LiveSoundData {
            sound: LiveSound {
                commands: rx,
                trash: trash_tx,
                shared: shared.clone(),
                current: None,
                outgoing: None,
                volume: Ramp::new(1.0),
                stopping: false,
                chunks: 0,
            },
            handle: LiveHandle { commands: tx, trash: trash_rx, shared },
        }
    }

    /// Split for use without a kira manager (tests, offline tools).
    pub fn split(self) -> (LiveSound, LiveHandle) {
        (self.sound, self.handle)
    }
}

impl Default for LiveSoundData {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundData for LiveSoundData {
    type Error = ();
    type Handle = LiveHandle;

    fn into_sound(self) -> Result<(Box<dyn Sound>, Self::Handle), Self::Error> {
        Ok((Box::new(self.sound), self.handle))
    }
}

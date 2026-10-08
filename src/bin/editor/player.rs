//! Sound: the game's own path. One kira manager plays a [`LiveSoundData`] (the live engine on
//! the audio thread, fed through its lock-free queue) and the sound effects, as
//! `audio::plugin` does; `NATHAN_AUDIO=headless` renders without a sound card.
//!
//! Every successful parse is hot-swapped in: a fresh [`Engine`] for the new text, started
//! where the playing one is ([`Engine::start_at`]) and given every dial again, crossfaded in
//! at the next bar line so the band carries on.

use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend};
use nat_han_adventures::audio::live::playback::{ENGINE_RATE, LiveHandle, LiveSound, LiveSoundData, Published};
use nat_han_adventures::audio::live::{BeatClock, Engine, EngineConfig, Input, SongFile};
use nat_han_adventures::audio::tuning::Tuning;
use nat_han_adventures::audio::{AudioOutput, Filters, Harmony, Sfx, sfx, synth, waltz};

/// Music level (dB), as in the game.
const MUSIC_DB: f32 = -4.0;

/// The dials the engine is given (again, for every new engine).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dials {
    pub force_harmony: Option<Harmony>,
    pub force_tuning: Option<Tuning>,
    /// lead, comp, bass, drums, dynamics.
    pub freedom: [f32; 5],
    /// Per channel level after mute / solo / volume.
    pub mix: [f32; 4],
}

impl Default for Dials {
    fn default() -> Self {
        Dials { force_harmony: None, force_tuning: None, freedom: [0.0; 5], mix: [1.0; 4] }
    }
}

impl Dials {
    pub fn freedom_input(&self) -> Input {
        let [lead, comp, bass, drums, dynamics] = self.freedom;
        Input::SetFreedom { lead, comp, bass, drums, dynamics }
    }

    pub fn inputs(&self) -> [Input; 4] {
        [Input::ForceHarmony(self.force_harmony), Input::ForceTuning(self.force_tuning), self.freedom_input(), Input::SetMix(self.mix)]
    }
}

/// What a new engine is told about the game so far.
#[derive(Debug, Clone, Copy, Default)]
pub struct Context {
    pub filters: Filters,
    pub stats: nat_han_adventures::audio::director::PlayStats,
}

/// A swap waiting for its bar line.
struct Pending {
    song: SongFile,
    /// Bars of the full song before the excerpt (when looping a selection).
    offset: usize,
}

pub struct Player {
    manager: Option<AudioManager<DefaultBackend>>,
    headless: Option<LiveSound>,
    handle: Option<LiveHandle>,
    sfx: Vec<(Sfx, StaticSoundData)>,
    pub published: Published,
    /// Chunks seen, and when (seconds).
    seen: (u64, f64),
    /// Playing (or about to).
    pub playing: bool,
    /// The song the engine plays starts this many bars into the edited song (loop selection).
    pub offset: usize,
    /// Bars, bar length and harmonies of the song playing.
    pub bars: usize,
    pub bar_beats: f64,
    pub can_play: [bool; 5],
    pending: Option<Pending>,
    pub sfx_on: bool,
    pub error: Option<String>,
}

fn to_static(r: synth::Rendered) -> StaticSoundData {
    StaticSoundData { sample_rate: r.sample_rate, frames: r.frames.into(), settings: StaticSoundSettings::new(), slice: None }
}

impl Player {
    pub fn new(output: AudioOutput) -> Player {
        let data = LiveSoundData::new();
        let (manager, headless, handle, error) = match output {
            AudioOutput::Headless => {
                let (sound, handle) = data.split();
                (None, Some(sound), Some(handle), None)
            }
            AudioOutput::Device => match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
                Ok(mut m) => {
                    let h = m.play(data).ok();
                    (Some(m), None, h, None)
                }
                Err(e) => (None, None, None, Some(format!("no audio output: {e:?}"))),
            },
        };
        let mut p = Player {
            manager,
            headless,
            handle,
            sfx: Sfx::ALL.into_iter().map(|s| (s, to_static(sfx::render(s)))).collect(),
            published: Published::default(),
            seen: (0, 0.0),
            playing: false,
            offset: 0,
            bars: 0,
            bar_beats: 4.0,
            can_play: [true, false, false, false, false],
            pending: None,
            sfx_on: true,
            error,
        };
        if let Some(h) = p.handle.as_mut() {
            h.set_volume_db(MUSIC_DB, 0.0);
        }
        p
    }

    /// Start `song` (bars `offset..` of the edited song) at song beat `beat`, fresh.
    pub fn play(&mut self, song: &SongFile, offset: usize, beat: f64, dials: &Dials, ctx: &Context) {
        match self.engine(song, beat, dials, ctx) {
            Ok(e) => {
                if let Some(h) = self.handle.as_mut() {
                    h.play(e, 0.02, 0.08);
                }
                self.offset = offset;
                self.playing = true;
                self.pending = None;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn engine(&mut self, song: &SongFile, beat: f64, dials: &Dials, ctx: &Context) -> Result<Engine, String> {
        let mut e = Engine::with_config(song, ENGINE_RATE, EngineConfig::default())?;
        e.start_at(e.shape().at(beat.max(0.0)));
        e.post(Input::SetStats(ctx.stats));
        e.post(Input::SetFilters(ctx.filters));
        for i in dials.inputs() {
            e.post(i);
        }
        self.bars = song.bars();
        self.bar_beats = song.bar_beats();
        self.can_play = Harmony::ALL.map(|h| e.can_play(h));
        Ok(e)
    }

    pub fn stop(&mut self) {
        if let Some(h) = self.handle.as_mut() {
            h.stop(0.08);
        }
        self.playing = false;
        self.pending = None;
    }

    /// Swap in `song` at the next bar line (see [`Player::update`]).
    pub fn swap(&mut self, song: SongFile, offset: usize) {
        if self.playing {
            self.pending = Some(Pending { song, offset });
        }
    }

    pub fn post(&mut self, i: Input) {
        if let Some(h) = self.handle.as_mut() {
            h.post(i);
        }
    }

    /// Play a rendered sound (an instrument preview) over the music.
    pub fn play_rendered(&mut self, r: synth::Rendered) {
        if let Some(m) = self.manager.as_mut() {
            let _ = m.play(to_static(r).volume(Decibels(MUSIC_DB)));
        }
    }

    pub fn play_sfx(&mut self, s: Sfx) {
        if !self.sfx_on {
            return;
        }
        let Some(d) = self.sfx.iter().find(|(k, _)| *k == s).map(|(_, d)| d.clone()) else { return };
        if let Some(m) = self.manager.as_mut() {
            let _ = m.play(d.volume(Decibels(-2.0)));
        }
    }

    /// Once a frame: render (headless), read what the audio thread published, and do a
    /// waiting swap if its bar line is near.
    pub fn update(&mut self, now: f64, dt: f64, dials: &Dials, ctx: &Context) {
        if let Some(sound) = self.headless.as_mut() {
            use kira::sound::Sound;
            const RATE: f64 = 48_000.0;
            let n = ((dt * RATE).round() as usize).min(48_000);
            let info = kira::info::MockInfoBuilder::new().build();
            let mut buf = [kira::Frame::ZERO; 128];
            let mut left = n;
            while left > 0 {
                let k = left.min(buf.len());
                sound.process(&mut buf[..k], 1.0 / RATE, &info);
                left -= k;
            }
        }
        if let Some(h) = self.handle.as_mut() {
            self.published = h.published();
        }
        if self.published.chunks != self.seen.0 {
            self.seen = (self.published.chunks, now);
        }
        let Some(p) = self.pending.as_ref() else { return };
        if !self.published.playing {
            let (song, offset) = (p.song.clone(), p.offset);
            self.play(&song, offset, 0.0, dials, ctx);
            return;
        }
        // Where the engine is now (not the ear: the new engine takes over from here).
        let c = self.published.clock.advanced(now - self.seen.1);
        let beat = self.canonical_beat(&c);
        let to_bar = (c.beats_per_bar - c.position.beat) * 60.0 / c.bpm.max(1.0) as f64;
        if to_bar > 0.15 {
            return;
        }
        let Pending { song, offset } = self.pending.take().expect("checked");
        // The beat in the new song (same bars, unless the selection moved).
        let shift = (self.offset as f64 - offset as f64) * song.bar_beats();
        let beat = (beat + shift).rem_euclid(song.beats().max(1.0));
        match self.engine(&song, beat, dials, ctx) {
            Ok(e) => {
                if let Some(h) = self.handle.as_mut() {
                    // The new engine is silent until the bar line; the old one bows out by it.
                    h.play(e, 0.01, (to_bar as f32).max(0.02));
                }
                self.offset = offset;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// The clock as heard (the engine runs `ahead_secs` ahead of the speaker).
    pub fn heard(&self, now: f64) -> BeatClock {
        let p = &self.published;
        let mut dt = now - self.seen.1 - p.ahead_secs;
        if dt < 0.0 {
            dt = dt.max(-p.clock.position.beat * 60.0 / p.clock.bpm.max(1.0) as f64);
        }
        p.clock.advanced(dt)
    }

    /// A clock's song beat in the 4/4 song (the waltz runs on its own 3/4 beats).
    pub fn canonical_beat(&self, c: &BeatClock) -> f64 {
        if (c.beats_per_bar - self.bar_beats).abs() > 1e-6 { waltz::unwarp(c.position.song_beat) } else { c.position.song_beat }
    }

    /// The heard song beat in the edited song (the selection's offset added).
    pub fn song_beat(&self, now: f64) -> f64 {
        self.canonical_beat(&self.heard(now)) + self.offset as f64 * self.bar_beats
    }

    pub fn has_output(&self) -> bool {
        self.handle.is_some()
    }

    pub fn headless(&self) -> bool {
        self.headless.is_some()
    }
}

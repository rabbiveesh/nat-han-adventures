//! Deterministic video capture (the `capture` feature, native dev builds only):
//! `NATHAN_CAPTURE=<dir> cargo run` plays an input timeline in game time and records every
//! frame, whatever the machine's load. `scripts/record-run --capture` drives it and muxes the
//! result into an MP4.
//!
//! - **Time.** Virtual time advances exactly one frame (`1/fps`, a whole number of 60 Hz
//!   simulation steps: [`frame_duration`]) per app update, however long the update takes
//!   (`TimeUpdateStrategy::ManualDuration`). The first update (Startup, time zero) is a
//!   warm-up and isn't recorded.
//! - **Video.** Each recorded update takes a [`Screenshot`] of the window; the next update
//!   waits (polling the GPU) until it has arrived before anything else runs, so every frame is
//!   saved once: `<dir>/frame_000000.png`, ... (encoded on worker threads). Pipelined
//!   rendering is off and pipelines compile synchronously ([`adjust_plugins`]), so frame N
//!   shows the world of update N, complete.
//! - **Audio.** The music + sfx render into `<dir>/audio.wav`, exactly `48000 / fps` samples
//!   per recorded frame ([`crate::audio::capture`]).
//! - **Input.** `NATHAN_CAPTURE_TIMELINE=<file>` ([`timeline`]) sends key presses as keyboard
//!   messages on exact frames; the capture ends (and the app quits) with the timeline, or after
//!   `NATHAN_CAPTURE_SECS` (default 5) without one. `NATHAN_CAPTURE_FPS` (default 30) must
//!   divide 60.
//! - `<dir>/capture.txt` sums it up (frames, fps, samples, wall time).

pub mod timeline;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::render::render_resource::PollType;
use bevy::render::renderer::RenderDevice;
use bevy::render::view::screenshot::{CapturedScreenshots, Screenshot};
use bevy::time::TimeUpdateStrategy;

use crate::audio::capture::{AudioCapture, CAPTURE_RATE, samples_per_frame};
use timeline::{Script, Timeline, TimelinePlayer};

/// Default frame rate.
pub const DEFAULT_FPS: u32 = 30;
/// Length without a timeline.
const DEFAULT_SECS: f64 = 5.0;
/// Give up on a screenshot after this long.
const FRAME_TIMEOUT: Duration = Duration::from_secs(60);
/// PNG encoders.
const WRITERS: usize = 4;

/// What to capture.
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub dir: PathBuf,
    pub fps: u32,
    pub timeline: Timeline,
}

impl CaptureConfig {
    /// From `NATHAN_CAPTURE` (and `_FPS`, `_TIMELINE`, `_SECS`); `None` when not capturing.
    pub fn from_env() -> Option<Result<Self, String>> {
        let dir = PathBuf::from(std::env::var_os("NATHAN_CAPTURE")?);
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        Some((|| {
            let fps = match var("NATHAN_CAPTURE_FPS") {
                Some(v) => v.parse().map_err(|_| format!("NATHAN_CAPTURE_FPS={v:?}"))?,
                None => DEFAULT_FPS,
            };
            let timeline = match var("NATHAN_CAPTURE_TIMELINE") {
                Some(path) => {
                    let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
                    Script::parse(&text).map_err(|e| format!("{path}: {e}"))?.compile(fps)
                }
                None => {
                    let secs = match var("NATHAN_CAPTURE_SECS") {
                        Some(v) => v.parse::<f64>().map_err(|_| format!("NATHAN_CAPTURE_SECS={v:?}"))?,
                        None => DEFAULT_SECS,
                    };
                    Timeline { events: Vec::new(), frames: (secs * fps as f64).round() as u64 }
                }
            };
            CaptureConfig::new(dir, fps, timeline)
        })())
    }

    /// Checks the frame rate; makes `dir`.
    pub fn new(dir: PathBuf, fps: u32, timeline: Timeline) -> Result<Self, String> {
        frame_duration(fps).ok_or_else(|| format!("capture fps {fps} must divide {}", crate::game::FIXED_HZ))?;
        samples_per_frame(CAPTURE_RATE, fps).ok_or_else(|| format!("capture fps {fps} must divide {CAPTURE_RATE}"))?;
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(CaptureConfig { dir, fps, timeline })
    }
}

/// One frame of game time at `fps`: exactly `FIXED_HZ / fps` simulation steps (so a whole
/// number of the fixed timestep, no rounding drift), or `None` if `fps` doesn't divide it.
pub fn frame_duration(fps: u32) -> Option<Duration> {
    let hz = crate::game::FIXED_HZ as u32;
    if fps == 0 || !hz.is_multiple_of(fps) {
        return None;
    }
    let step = Time::<Fixed>::from_hz(crate::game::FIXED_HZ).timestep();
    Some(step * (hz / fps))
}

/// The window for a capture: a fixed 1280x720, no vsync wait.
pub fn capture_window(window: Window) -> Window {
    Window {
        resolution: bevy::window::WindowResolution::new(1280, 720),
        resizable: false,
        present_mode: bevy::window::PresentMode::AutoNoVsync,
        ..window
    }
}

/// The default plugins for a capture: no pipelined rendering (frame N renders within update
/// N), pipelines compiled before the frame that needs them (no missing sprites).
pub fn adjust_plugins(plugins: bevy::app::PluginGroupBuilder) -> bevy::app::PluginGroupBuilder {
    plugins
        .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>()
        .set(bevy::render::RenderPlugin { synchronous_pipeline_compilation: true, ..default() })
}

/// Add after the game's plugins (it overrides the time strategy; its `AudioCapture` makes the
/// audio plugin render into the WAV).
pub struct CapturePlugin(pub CaptureConfig);

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        let c = &self.0;
        let audio = AudioCapture::new(&c.dir.join("audio.wav"), c.fps).expect("capture audio file");
        app.insert_resource(TimeUpdateStrategy::ManualDuration(frame_duration(c.fps).unwrap()))
            .insert_resource(audio)
            .insert_resource(TimelinePlayer::new(c.timeline.clone()))
            .insert_resource(CaptureState::new(c))
            .add_systems(First, collect_frame)
            .add_systems(PreUpdate, timeline::play_timeline.before(bevy::input::InputSystems).run_if(recording))
            .add_systems(Last, request_frame.run_if(recording));
        info!("capture: {} frames at {} fps into {}", c.timeline.frames, c.fps, c.dir.display());
    }
}

/// The capture's bookkeeping.
#[derive(Resource)]
pub struct CaptureState {
    dir: PathBuf,
    fps: u32,
    /// Frames to record.
    total: u64,
    /// Updates run.
    updates: u64,
    /// Frames recorded (screenshots requested).
    frames: u64,
    recording: bool,
    finished: bool,
    /// The screenshot of the last recorded frame, not saved yet.
    pending: Option<(Entity, u64)>,
    writer: Option<FrameWriter>,
    started: Instant,
}

impl CaptureState {
    fn new(c: &CaptureConfig) -> Self {
        CaptureState {
            dir: c.dir.clone(),
            fps: c.fps,
            total: c.timeline.frames,
            updates: 0,
            frames: 0,
            recording: false,
            finished: false,
            pending: None,
            writer: Some(FrameWriter::new(&c.dir)),
            started: Instant::now(),
        }
    }
}

/// This update is a recorded frame.
pub fn recording(state: Res<CaptureState>) -> bool {
    state.recording
}

/// Start of every update: save the last frame's screenshot (waiting for it), then decide
/// whether this update is recorded, or the capture is over.
fn collect_frame(world: &mut World) {
    if world.resource::<CaptureState>().finished {
        return;
    }
    if let Some((entity, frame)) = world.resource_mut::<CaptureState>().pending.take() {
        match wait_for_screenshot(world, entity) {
            Some(image) => {
                if let Some(w) = world.resource::<CaptureState>().writer.as_ref() {
                    w.send(frame, image);
                }
            }
            None => {
                error!("capture: frame {frame} never arrived");
                finish(world, false);
                return;
            }
        }
        if let Ok(e) = world.get_entity_mut(entity) {
            e.despawn();
        }
    }
    let mut s = world.resource_mut::<CaptureState>();
    let warm_up = s.updates == 0;
    s.updates += 1;
    if warm_up {
        return;
    }
    if s.frames < s.total {
        s.recording = true;
        world.resource_mut::<AudioCapture>().recording = true;
    } else {
        finish(world, true);
    }
}

/// Block until the screenshot taken for `entity` comes back from the GPU.
fn wait_for_screenshot(world: &mut World, entity: Entity) -> Option<Image> {
    let rx = world.resource::<CapturedScreenshots>().0.clone();
    let device = world.get_resource::<RenderDevice>().cloned();
    let start = Instant::now();
    let rx = rx.lock().ok()?;
    while start.elapsed() < FRAME_TIMEOUT {
        if let Some(d) = &device {
            let _ = d.poll(PollType::Wait { submission_index: None, timeout: Some(Duration::from_millis(100)) });
        }
        match rx.recv_timeout(Duration::from_millis(5)) {
            Ok((e, image)) if e == entity => return Some(image),
            Ok((e, _)) => warn!("capture: a stray screenshot ({e})"),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return None,
        }
    }
    None
}

/// End of a recorded update: ask for its screenshot.
fn request_frame(mut commands: Commands, mut state: ResMut<CaptureState>) {
    let e = commands.spawn(Screenshot::primary_window()).id();
    let frame = state.frames;
    state.pending = Some((e, frame));
    state.frames += 1;
    state.recording = false;
}

/// Stop recording, flush the frames and the WAV, write the summary, quit.
fn finish(world: &mut World, ok: bool) {
    let samples = {
        let mut audio = world.resource_mut::<AudioCapture>();
        let samples = audio.blocks * audio.block() as u64;
        if let Err(e) = audio.finish() {
            error!("capture audio: {e}");
        }
        samples
    };
    let mut s = world.resource_mut::<CaptureState>();
    s.finished = true;
    s.recording = false;
    let written = s.writer.take().map_or(0, FrameWriter::finish);
    let wall = s.started.elapsed().as_secs_f64();
    let summary = format!(
        "frames {}\nfps {}\nsamples {samples}\nsample_rate {CAPTURE_RATE}\nseconds {:.3}\nwall_seconds {wall:.1}\nok {ok}\n",
        written,
        s.fps,
        s.frames as f64 / s.fps as f64
    );
    if let Err(e) = std::fs::write(s.dir.join("capture.txt"), &summary) {
        error!("capture: {e}");
    }
    info!("capture done: {}", summary.replace('\n', " "));
    world.write_message(if ok { AppExit::Success } else { AppExit::error() });
}

/// PNG encoding on worker threads, in any order (the names carry the frame numbers).
struct FrameWriter {
    tx: Option<SyncSender<(u64, Image)>>,
    workers: Vec<JoinHandle<u64>>,
}

impl FrameWriter {
    fn new(dir: &Path) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel::<(u64, Image)>(WRITERS * 2);
        let rx: Arc<Mutex<Receiver<(u64, Image)>>> = Arc::new(Mutex::new(rx));
        let workers = (0..WRITERS)
            .map(|_| {
                let (rx, dir) = (rx.clone(), dir.to_owned());
                std::thread::spawn(move || {
                    let mut n = 0;
                    loop {
                        let Ok((frame, image)) = rx.lock().unwrap().recv() else { return n };
                        let path = dir.join(format!("frame_{frame:06}.png"));
                        match image.try_into_dynamic() {
                            Ok(img) => match img.to_rgb8().save(&path) {
                                Ok(()) => n += 1,
                                Err(e) => error!("capture: {}: {e}", path.display()),
                            },
                            Err(e) => error!("capture: frame {frame}: {e}"),
                        }
                    }
                })
            })
            .collect();
        FrameWriter { tx: Some(tx), workers }
    }

    fn send(&self, frame: u64, image: Image) {
        if let Some(tx) = &self.tx {
            let _ = tx.send((frame, image));
        }
    }

    /// Wait for every frame to be written; how many were.
    fn finish(mut self) -> u64 {
        self.tx.take();
        self.workers.drain(..).map(|w| w.join().unwrap_or(0)).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_whole_simulation_steps() {
        let step = Time::<Fixed>::from_hz(crate::game::FIXED_HZ).timestep();
        assert_eq!(frame_duration(60), Some(step));
        assert_eq!(frame_duration(30), Some(step * 2));
        assert_eq!(frame_duration(20), Some(step * 3));
        assert_eq!(frame_duration(24), None);
        assert_eq!(frame_duration(0), None);
    }

    /// The frame-stepping clock: whatever the wall time, each update is exactly 60/fps fixed
    /// steps, and virtual time is frames/fps.
    #[test]
    fn each_update_runs_exactly_its_fixed_steps() {
        for fps in [60, 30, 20, 15] {
            #[derive(Resource, Default)]
            struct Steps(u64);
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .insert_resource(Time::<Fixed>::from_hz(crate::game::FIXED_HZ))
                .insert_resource(TimeUpdateStrategy::ManualDuration(frame_duration(fps).unwrap()))
                .init_resource::<Steps>()
                .add_systems(FixedUpdate, |mut s: ResMut<Steps>| s.0 += 1);
            app.update(); // the warm-up: time zero
            let per = 60 / fps as u64;
            for frame in 1..=600u64 {
                if frame % 97 == 0 {
                    std::thread::sleep(Duration::from_millis(3)); // a slow frame changes nothing
                }
                app.update();
                assert_eq!(app.world().resource::<Steps>().0, frame * per, "fps {fps} frame {frame}");
            }
            let t = app.world().resource::<Time<Virtual>>().elapsed_secs_f64();
            assert!((t - 600.0 / fps as f64).abs() < 1e-5, "fps {fps}: {t}");
        }
    }
}

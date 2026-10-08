//! The live engine in the game, for A/B listening: `NATHAN_LIVE_MUSIC=1 cargo run` (native).
//! Not part of the default build's plugin set (see `main.rs`).
//!
//! It mirrors the old plugin ([`crate::audio::plugin`]): the music follows the [`AppState`]
//! ([`desired_music`]), the director runs on the same messages (a copy of its bookkeeping;
//! decisions go to the engine as [`Input::SetFilters`]), `NATHAN_MUSIC` still forces filters,
//! and pause ducks. Every gameplay message is also forwarded as an [`Input`]. What's sounding
//! comes back from the audio thread: [`Groove`] (and [`NowPlaying`], [`MusicChanged`]) follow
//! it at the bar line where a new harmony starts, and [`LiveClock`] carries the beat for
//! physics.
//!
//! The old plugin keeps playing the sound effects; its music is silenced by swapping in silent
//! songs ([`SongSource`]). The engine plays through its own kira manager (bevy_kira_audio only
//! plays static sounds), so there are two output streams while both plugins run.

use bevy::prelude::*;
use kira::{AudioManager, AudioManagerSettings, DefaultBackend};

use crate::audio::director::{self, PlayStats};
use crate::audio::tuning::Tuning;
use crate::audio::{Filters, Music, MusicChanged, MusicOverride, NowPlaying, Song, SongSource, desired_music, songs};
use crate::events::{CheckpointReached, Jumped, Landed, NuggetCollected, PlayerDied};
use crate::game::{Groove, LevelRun, RestartLevel};
use crate::level::Levels;
use crate::state::{AppState, CurrentLevel, PlayState};

use super::engine::{BeatClock, Engine, Input};
use super::library;
use super::playback::{ENGINE_RATE, LiveHandle, LiveSound, LiveSoundData};

/// Music volume (dB) and the pause duck, as the old plugin.
const MUSIC_DB: f32 = -4.0;
const PAUSE_DUCK_DB: f32 = -12.0;

/// `NATHAN_LIVE_MUSIC=1` (native only).
pub fn enabled() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("NATHAN_LIVE_MUSIC").is_ok_and(|v| v == "1")
    }
    #[cfg(target_arch = "wasm32")]
    false
}

/// Where the engine's audio goes.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LiveOutput {
    /// The sound card (its own kira manager).
    #[default]
    Device,
    /// Nowhere: a system renders as much as real time passes (headless tests).
    Headless,
}

/// The audio side: the kira manager (kept alive) or a headless sound, and the handle.
struct LiveAudio {
    _manager: Option<AudioManager<DefaultBackend>>,
    headless: Option<LiveSound>,
    handle: Option<LiveHandle>,
}

/// The engine's beat, as heard (extrapolated between audio callbacks).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct LiveClock {
    pub clock: BeatClock,
    /// Filters sounding.
    pub filters: Filters,
    pub tuning: Tuning,
}

/// The player's bookkeeping.
#[derive(Resource, Default)]
pub struct LivePlayer {
    music: Option<Music>,
    /// What's sounding (from the audio thread), once known.
    sounding: Option<(Filters, Tuning)>,
    /// The last decision and its reason (for the toast when it lands).
    decided: Option<(Filters, &'static str)>,
    ducked: Option<bool>,
    /// Last published chunk count, and when we saw it change.
    seen: (u64, f64),
    director: DirectorState,
}

/// A copy of the old plugin's director bookkeeping.
#[derive(Default)]
struct DirectorState {
    stats: PlayStats,
    window: director::Window,
    level_start: f32,
    next_check: f32,
    was_playing: bool,
}

pub fn plugin(app: &mut App) {
    app.insert_resource(SongSource(silent))
        .init_resource::<LiveOutput>()
        .init_resource::<LivePlayer>()
        .init_resource::<LiveClock>()
        .init_resource::<NowPlaying>()
        .init_resource::<MusicOverride>()
        .add_systems(Startup, setup)
        .add_systems(Update, (follow_state, forward, direct, duck).chain())
        .add_systems(PostUpdate, (pump, sync).chain());
}

/// The old plugin's songs, silenced (titles kept for its "now playing").
fn silent(m: Music) -> Song {
    Song { pulse1: "r1", pulse2: "", triangle: "", noise: "", chords: "", ..songs::song(m) }
}

fn setup(world: &mut World) {
    let output = *world.resource::<LiveOutput>();
    let data = LiveSoundData::new();
    let audio = match output {
        LiveOutput::Headless => {
            let (sound, handle) = data.split();
            LiveAudio { _manager: None, headless: Some(sound), handle: Some(handle) }
        }
        LiveOutput::Device => match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
            Ok(mut manager) => {
                let handle = manager.play(data).ok();
                info!("live music engine on");
                LiveAudio { _manager: Some(manager), headless: None, handle }
            }
            Err(e) => {
                warn!("live music: no audio output ({e:?})");
                LiveAudio { _manager: None, headless: None, handle: None }
            }
        },
    };
    world.insert_non_send(audio);
}

/// The override as filters for a looping song.
fn initial_filters(overrides: &MusicOverride, music: Music) -> Filters {
    match overrides.0 {
        Some(f) if music != Music::LevelClear => f,
        _ => Filters::default(),
    }
}

#[allow(clippy::too_many_arguments)]
fn follow_state(
    state: Res<State<AppState>>,
    levels: Option<Res<Levels>>,
    current_level: Res<CurrentLevel>,
    overrides: Res<MusicOverride>,
    mut player: ResMut<LivePlayer>,
    mut audio: NonSendMut<LiveAudio>,
) {
    let want = desired_music(*state.get(), levels.as_deref(), *current_level);
    if player.music == Some(want) {
        return;
    }
    player.music = Some(want);
    player.sounding = None;
    player.ducked = None;
    let Some(handle) = audio.handle.as_mut() else { return };
    let engine = library::load(library::stem(want)).and_then(|f| Engine::new(&f, ENGINE_RATE));
    let mut engine = match engine {
        Ok(e) => e,
        Err(e) => {
            error!("live music: {e}");
            return;
        }
    };
    engine.post(Input::SetFilters(initial_filters(&overrides, want)));
    let (fade_in, fade_out) = match want {
        Music::LevelClear => (0.01, 0.12),
        _ => (0.25, 0.45),
    };
    handle.play(engine, fade_in, fade_out);
}

/// Every gameplay message, as an input.
#[allow(clippy::too_many_arguments)]
fn forward(
    mut audio: NonSendMut<LiveAudio>,
    mut jumped: MessageReader<Jumped>,
    mut landed: MessageReader<Landed>,
    mut nuggets: MessageReader<NuggetCollected>,
    mut died: MessageReader<PlayerDied>,
    mut checkpoints: MessageReader<CheckpointReached>,
    mut restart: MessageReader<RestartLevel>,
) {
    let Some(h) = audio.handle.as_mut() else {
        // Keep the readers drained.
        let _ = (jumped.read().count(), landed.read().count(), nuggets.read().count());
        let _ = (died.read().count(), checkpoints.read().count(), restart.read().count());
        return;
    };
    for j in jumped.read() {
        h.post(if j.double { Input::Toot } else { Input::Jump { on_ground: true } });
    }
    for l in landed.read() {
        h.post(Input::Land { speed: l.speed });
    }
    for _ in nuggets.read() {
        h.post(Input::Nugget);
    }
    for _ in died.read() {
        h.post(Input::Death);
    }
    for _ in checkpoints.read() {
        h.post(Input::Checkpoint);
    }
    for _ in restart.read() {
        h.post(Input::Restart);
    }
}

/// The old plugin's `direct`, sending its decisions to the engine.
#[allow(clippy::too_many_arguments)]
fn direct(
    state: Res<State<AppState>>,
    run: Option<Res<LevelRun>>,
    overrides: Res<MusicOverride>,
    mut player: ResMut<LivePlayer>,
    mut audio: NonSendMut<LiveAudio>,
    mut restart: MessageReader<RestartLevel>,
    mut jumped: MessageReader<Jumped>,
    mut nuggets: MessageReader<NuggetCollected>,
    mut died: MessageReader<PlayerDied>,
    mut checkpoints: MessageReader<CheckpointReached>,
) {
    let playing = *state.get() == AppState::Playing;
    let restarted = restart.read().count() > 0;
    let toots = jumped.read().filter(|j| j.double).count() as u32;
    let got = nuggets.read().count() as u32;
    let deaths = died.read().count() as u32;
    let cps = checkpoints.read().count();
    let now = run.as_ref().map_or(0.0, |r| r.time);
    let player = &mut *player;
    let d = &mut player.director;
    let mut decision = None;
    let mut before = d.stats;
    if toots + got > 0 {
        d.window.fill(&mut before, now, d.level_start);
    }
    let mut level_start = false;
    if playing && (!d.was_playing || restarted) {
        d.stats = PlayStats::default();
        d.window = director::Window::default();
        d.level_start = now;
        d.next_check = now + director::MUSIC_CHECK_SECS;
        decision = Some((Filters::default(), ""));
        level_start = !restarted;
    }
    d.was_playing = playing;
    if !playing {
        return;
    }
    d.window.record(now, toots, got, deaths);
    d.stats.level_deaths += deaths;
    d.stats.checkpoint_deaths += deaths;
    d.window.fill(&mut d.stats, now, d.level_start);
    let summoned = toots + got > 0 && director::summoned(&before, &d.stats);
    if deaths > 0 || cps > 0 || now >= d.next_check || summoned {
        decision = Some(director::choose_filters(&d.stats));
        d.next_check = now + director::MUSIC_CHECK_SECS;
        if cps > 0 {
            d.stats.checkpoint_deaths = 0;
        }
    }
    let stats = d.stats;
    let Some(h) = audio.handle.as_mut() else { return };
    if level_start {
        h.post(Input::LevelStart);
    }
    if stats != before || decision.is_some() {
        h.post(Input::SetStats(stats));
    }
    if let Some((filters, reason)) = decision {
        let decided = match overrides.0 {
            Some(f) => (f, "NATHAN_MUSIC"),
            None => (filters, reason),
        };
        player.decided = Some(decided);
        h.post(Input::SetFilters(decided.0));
    }
}

fn duck(play_state: Option<Res<State<PlayState>>>, mut player: ResMut<LivePlayer>, mut audio: NonSendMut<LiveAudio>) {
    let duck = play_state.is_some_and(|s| *s.get() == PlayState::Paused);
    if player.ducked == Some(duck) {
        return;
    }
    let Some(h) = audio.handle.as_mut() else { return };
    let db = if duck { MUSIC_DB + PAUSE_DUCK_DB } else { MUSIC_DB };
    if h.set_volume_db(db, 0.2) {
        player.ducked = Some(duck);
    }
}

/// Headless output: render as much audio as real time has passed (48 kHz "device").
fn pump(time: Res<Time<Real>>, mut audio: NonSendMut<LiveAudio>) {
    use kira::sound::Sound;
    const RATE: f64 = 48_000.0;
    let Some(sound) = audio.headless.as_mut() else { return };
    let n = ((time.delta_secs_f64() * RATE).round() as usize).min(48_000);
    let info = kira::info::MockInfoBuilder::new().build();
    let mut buf = [kira::Frame::ZERO; 128];
    let mut left = n;
    while left > 0 {
        let k = left.min(buf.len());
        sound.process(&mut buf[..k], 1.0 / RATE, &info);
        left -= k;
    }
}

/// What's sounding → Groove, NowPlaying, MusicChanged, LiveClock.
#[allow(clippy::too_many_arguments)]
fn sync(
    time: Res<Time<Real>>,
    audio: NonSend<LiveAudio>,
    mut player: ResMut<LivePlayer>,
    mut clock: ResMut<LiveClock>,
    mut groove: Option<ResMut<Groove>>,
    mut now_playing: ResMut<NowPlaying>,
    mut changed: MessageWriter<MusicChanged>,
) {
    let Some(h) = audio.handle.as_ref() else { return };
    let p = h.published();
    let now = time.elapsed_secs_f64();
    if p.chunks != player.seen.0 {
        player.seen = (p.chunks, now);
    }
    if !p.playing {
        return;
    }
    let filters = Filters { harmony: p.state.harmony, just_intonation: p.state.tuning == Tuning::Medley };
    // The clock as heard: the engine is `ahead_secs` ahead of the output; time has passed since.
    let heard = p.clock.advanced(now - player.seen.1 - p.ahead_secs);
    *clock = LiveClock { clock: heard, filters, tuning: p.state.tuning };
    if let Some(g) = groove.as_deref_mut() {
        let mut want = Groove::new(filters);
        want.tuning = p.state.tuning;
        if *g != want {
            *g = want;
        }
    }
    let sounding = (filters, p.state.tuning);
    if player.sounding != Some(sounding) {
        let first = player.sounding.is_none();
        player.sounding = Some(sounding);
        let reason = player.decided.filter(|(f, _)| *f == filters).map_or("", |(_, r)| r);
        if now_playing.filters != filters || now_playing.reason != reason {
            now_playing.filters = filters;
            now_playing.reason = reason;
            if !first {
                let secs = heard.position.song_beat * 60.0 / heard.bpm as f64;
                changed.write(MusicChanged { now: now_playing.clone(), at_secs: secs });
            }
        }
    }
}

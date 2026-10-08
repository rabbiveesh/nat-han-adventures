//! The Bevy plugin: the live engine plays the game's music, and the sound effects play on the
//! same kira manager (one output stream).
//!
//! - **Music** follows the [`AppState`] ([`desired_music`]): each new piece is a fresh
//!   [`Engine`] (built on the main thread, a few ms), crossfaded in on the audio thread
//!   ([`super::live::playback`]). [`MusicStarted`] fires when one starts.
//! - **The director** ([`super::director::Band`]) hears the gameplay messages and decides; its
//!   decisions go to the engine as [`Input::SetFilters`] (`NATHAN_MUSIC` overrides them), and
//!   the engine plays them from the next bar line it hasn't committed yet. Every gameplay
//!   message is forwarded as an [`Input`] too (the band answers toots, crashes after a death),
//!   and a jump in threes as [`Input::WaltzStep`].
//! - **What's sounding** comes back from the audio thread every frame: [`NowPlaying`] (and
//!   [`MusicChanged`], with the bar line it came in at) and [`Groove`]'s physics follow the
//!   moment a new harmony starts sounding; [`Groove::clock`] and [`LiveClock`] carry the beat
//!   (in the waltz, its 3/4 bars) for the world on the beat; [`NowPlaying::tuning_now`] is the
//!   laughing band's tuning of the phrase playing.
//! - **Pause** ducks the music.
//! - **Sound effects** are rendered at startup ([`super::sfx`]) and played as kira static
//!   sounds on the same manager.
//!
//! [`AudioOutput::Headless`] runs it all without a sound card: a system renders as much music
//! as real time has passed (tests; sfx are only counted). With an `AudioCapture` resource
//! (the `capture` feature, video recording) it runs the same manager on a deviceless backend and
//! renders exactly one video frame of music + sfx per frame into a WAV (`super::capture`).
//!
//! On the web the manager's cpal backend makes the page's one `AudioContext` at startup,
//! through `window.AudioContext`, which `index.html` wraps to resume it on the first gesture
//! (autoplay policy); until then nothing renders and the music starts from the top when it
//! unlocks. The page also reports how far ahead of the ear the browser's audio runs, so the
//! beat clock steps back by the browser's buffering and output latency (`web_output_lead`).

use bevy::prelude::*;
use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, PlaybackRate};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::events::{BandFreedom, CheckpointReached, HanSays, Jumped, Landed, LevelCompleted, NuggetCollected, PlaySfx, PlayerDied};
use crate::game::{self, GeneratedLevel, Groove, LevelRun, RestartLevel};
use crate::level::Levels;
use crate::state::{AppState, CurrentLevel, PlayState};

use super::live::engine::{BeatClock, Engine, EngineConfig, Input};
use super::live::library;
use super::live::playback::{ENGINE_RATE, LiveHandle, LiveSound, LiveSoundData};
use super::tuning::{Tuning, Wobble};
use super::{Filters, Harmony, Music, Sfx, director, sfx, synth};

/// Music volume (dB). Sfx play at their own levels around 0 dB.
const MUSIC_DB: f32 = -4.0;
/// Extra music attenuation while paused.
const PAUSE_DUCK_DB: f32 = -12.0;
/// Max fall speed in px/s ([`crate::game::tuning::MAX_FALL`]), for scaling the landing thud.
const LAND_FULL_SPEED: f32 = 420.0;

pub fn plugin(app: &mut App) {
    app.insert_resource(AudioOutput::from_env())
        .init_resource::<LivePlayer>()
        .init_resource::<LiveClock>()
        .init_resource::<NowPlaying>()
        .init_resource::<Director>()
        .init_resource::<HanBabble>()
        .init_resource::<SfxCount>()
        .init_resource::<SfxRng>()
        .insert_resource(MusicOverride(music_override()))
        .add_systems(Startup, setup)
        .add_systems(Update, (follow_state, forward, direct, duck, play_sfx, babble).chain())
        .add_systems(PostUpdate, (pump, sync).chain());
}

/// Where the audio goes.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioOutput {
    /// The sound card.
    #[default]
    Device,
    /// Nowhere: a system renders the music as fast as real time passes; sfx are counted.
    Headless,
}

impl AudioOutput {
    /// [`AudioOutput::Device`], unless `NATHAN_AUDIO=headless` (native; `scripts/headless-run`
    /// sets it: a null sound card would pull the music far faster than real time).
    pub fn from_env() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        if std::env::var("NATHAN_AUDIO").is_ok_and(|v| v == "headless") {
            return AudioOutput::Headless;
        }
        AudioOutput::Device
    }
}

/// `NATHAN_MUSIC` (native dev builds): forced filters for every looping song.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct MusicOverride(pub Option<Filters>);

fn music_override() -> Option<Filters> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let v = std::env::var("NATHAN_MUSIC").ok()?;
        let f = Filters::parse(&v);
        if f.is_none() {
            warn!("NATHAN_MUSIC={v:?} not understood (try coltrane, quartal, melodic, waltz, original, +ji)");
        }
        f
    }
    #[cfg(target_arch = "wasm32")]
    None
}

/// The world of the level being played: free play's generated level, else the story level.
pub fn playing_world(levels: Option<&Levels>, current: CurrentLevel, generated: Option<&GeneratedLevel>) -> u8 {
    match generated {
        Some(g) => g.0.world,
        None => levels.and_then(|l| l.0.get(current.0)).map_or(1, |l| l.world),
    }
}

/// Which music goes with an app state (`world`: the level being played's, [`playing_world`]).
pub fn desired_music(state: AppState, world: u8) -> Music {
    match state {
        AppState::Title | AppState::LevelSelect | AppState::FreePlaySetup => Music::Title,
        AppState::Playing => Music::World(world),
        AppState::LevelComplete => Music::LevelClear,
        AppState::Victory => Music::Victory,
    }
}

/// What's playing, for the UI ("now playing" toasts, the band badge).
#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct NowPlaying {
    pub music: Music,
    pub title: &'static str,
    /// The filters sounding.
    pub filters: Filters,
    /// Why the band plays it this way ("GIANT STEPS!"), or "".
    pub reason: &'static str,
    /// The tuning of the phrase sounding when the laughing band plays (the medley's pick:
    /// [`Tuning::Just`], [`Tuning::Tet7`], ...); `None` otherwise. Follows the phrases as they
    /// change (read it every frame; it never fires a message).
    pub tuning_now: Option<Tuning>,
    /// What the band's arranging beyond the tune, else "": a feel ("BOSSA NOVA"), a chorus
    /// ("STOP-TIME"), the intro, an ending ([`crate::audio::live::band::BandPlan::label`]).
    /// The band's own choice: music only.
    pub band_now: &'static str,
}

impl Default for NowPlaying {
    fn default() -> Self {
        NowPlaying { music: Music::Title, title: "", filters: Filters::default(), reason: "", tuning_now: None, band_now: "" }
    }
}

impl NowPlaying {
    /// E.g. "GIANT STEPS! - COLTRANE CHANGES"; just the label without a reason; "" when plain.
    pub fn toast(&self) -> String {
        let label = self.filters.label();
        match (self.reason, label.as_str()) {
            (_, "") => String::new(),
            ("", l) => l.to_string(),
            (r, l) => format!("{r} - {l}"),
        }
    }
}

/// A track started from the top.
#[derive(Message, Debug, Clone, PartialEq)]
pub struct MusicStarted(pub NowPlaying);

/// The playing track switched filters mid-song (at a bar line).
#[derive(Message, Debug, Clone, PartialEq)]
pub struct MusicChanged {
    pub now: NowPlaying,
    /// Song position (seconds, in the version now playing) of the bar line where it came in.
    pub at_secs: f64,
}

/// The engine's beat, as heard (extrapolated between audio callbacks).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct LiveClock {
    pub clock: BeatClock,
    /// Filters sounding.
    pub filters: Filters,
    pub tuning: Tuning,
}

/// The music's bookkeeping.
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
    /// The band's freedom, as last decided by the adaptive engine (re-sent to each new level
    /// engine).
    freedom: Option<BandFreedom>,
}

impl LivePlayer {
    /// The piece playing.
    pub fn now_playing(&self) -> Option<Music> {
        self.music
    }

    /// The filters sounding (once the audio thread has said).
    pub fn filters(&self) -> Option<Filters> {
        self.sounding.map(|s| s.0)
    }

    /// The freedom last posted to the engine ([`BandFreedom`], from the adaptive engine).
    pub fn freedom(&self) -> Option<BandFreedom> {
        self.freedom
    }

    /// Filters decided but not sounding yet (they come in at the next bar line).
    pub fn pending_filters(&self) -> Option<Filters> {
        let (f, _) = self.decided?;
        (self.filters() != Some(f)).then_some(f)
    }
}

/// The director's bookkeeping between frames.
#[derive(Resource, Debug, Default)]
pub struct Director {
    /// The band's memory of this level: stats, rolling window, the held summon.
    pub band: director::Band,
    was_playing: bool,
}

/// Sound effects played (or, headless, that would have been).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SfxCount(pub u32);

/// The kira manager: on the sound card, or (capture) pulled a frame at a time. (One lives for
/// the whole run: the size difference doesn't matter.)
#[allow(clippy::large_enum_variant)]
enum Manager {
    Device(AudioManager<DefaultBackend>),
    #[cfg(feature = "capture")]
    Capture(AudioManager<super::capture::CaptureBackend>),
}

/// The sfx.s dice (Han.s babble). Fixed seed when capturing, so takes repeat exactly.
#[derive(Resource)]
struct SfxRng(StdRng);

impl Default for SfxRng {
    fn default() -> Self {
        SfxRng(StdRng::from_rng(&mut rand::rng()))
    }
}

/// The audio side (main thread only): the kira manager or the headless sound, the music's
/// handle, the rendered sfx.
struct Audio {
    manager: Option<Manager>,
    headless: Option<LiveSound>,
    handle: Option<LiveHandle>,
    sfx: Vec<(Sfx, StaticSoundData)>,
    han: Vec<StaticSoundData>,
}

impl Audio {
    fn sfx(&self, s: Sfx) -> Option<&StaticSoundData> {
        self.sfx.iter().find(|(k, _)| *k == s).map(|(_, d)| d)
    }

    /// Play a sound effect at `db` (and `rate`).
    fn play(&mut self, data: Option<StaticSoundData>, db: f32, rate: f64, count: &mut SfxCount) {
        let Some(data) = data else { return };
        count.0 += 1;
        let data = data.volume(Decibels(db)).playback_rate(PlaybackRate(rate));
        let played = match self.manager.as_mut() {
            Some(Manager::Device(m)) => m.play(data).map(drop),
            #[cfg(feature = "capture")]
            Some(Manager::Capture(m)) => m.play(data).map(drop),
            None => Ok(()),
        };
        if let Err(e) = played {
            debug!("sfx: {e}");
        }
    }
}

/// Rendered frames as a kira static sound.
fn to_static(r: synth::Rendered) -> StaticSoundData {
    StaticSoundData { sample_rate: r.sample_rate, frames: r.frames.into(), settings: StaticSoundSettings::new(), slice: None }
}

fn setup(world: &mut World) {
    let started = bevy::platform::time::Instant::now();
    let output = *world.resource::<AudioOutput>();
    let sfx = Sfx::ALL.into_iter().map(|s| (s, to_static(sfx::render(s)))).collect();
    let han = (0..sfx::HAN_VARIANTS).map(|v| to_static(sfx::han_blip(v))).collect();
    let data = LiveSoundData::new();
    #[cfg(feature = "capture")]
    if world.contains_resource::<super::capture::AudioCapture>() {
        // Capturing: the device path's mix, pulled a frame at a time; Han's babble seeded.
        let mut manager = AudioManager::<super::capture::CaptureBackend>::new(AudioManagerSettings::default())
            .expect("the capture backend never fails");
        let handle = manager.play(data).ok();
        world.insert_resource(SfxRng(StdRng::seed_from_u64(0x7007)));
        world.insert_non_send(Audio { manager: Some(Manager::Capture(manager)), headless: None, handle, sfx, han });
        return;
    }
    let (manager, headless, handle) = match output {
        AudioOutput::Headless => {
            let (sound, handle) = data.split();
            (None, Some(sound), Some(handle))
        }
        AudioOutput::Device => match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
            Ok(mut manager) => {
                let handle = manager.play(data).ok();
                (Some(Manager::Device(manager)), None, handle)
            }
            Err(e) => {
                warn!("no audio output ({e:?}): playing silently");
                (None, None, None)
            }
        },
    };
    world.insert_non_send(Audio { manager, headless, handle, sfx, han });
    debug!("audio startup: {:.0}ms", started.elapsed().as_secs_f32() * 1000.0);
}

/// A fresh engine for a piece, its title, and the filters it starts with (the override, if
/// the song can take it).
fn start_engine(music: Music, overrides: &MusicOverride) -> Result<(Engine, &'static str, Filters), String> {
    let (title, file) = library::song(music)?;
    // A level's tune starts with an intro (when the band's loose enough: see `chorus`).
    let config = EngineConfig { intro: matches!(music, Music::World(_)), ..EngineConfig::default() };
    let engine = Engine::with_config(file, ENGINE_RATE, config)?;
    let mut filters = match overrides.0 {
        Some(f) if music != Music::LevelClear => f,
        _ => Filters::default(),
    };
    if !engine.can_play(filters.harmony) {
        filters.harmony = Harmony::Original;
    }
    Ok((engine, title, filters))
}

#[allow(clippy::too_many_arguments)]
fn follow_state(
    state: Res<State<AppState>>,
    levels: Option<Res<Levels>>,
    current_level: Res<CurrentLevel>,
    generated: Option<Res<GeneratedLevel>>,
    overrides: Res<MusicOverride>,
    mut player: ResMut<LivePlayer>,
    mut now_playing: ResMut<NowPlaying>,
    mut groove: Option<ResMut<Groove>>,
    mut started: MessageWriter<MusicStarted>,
    mut audio: NonSendMut<Audio>,
) {
    let world = playing_world(levels.as_deref(), *current_level, generated.as_deref());
    let want = desired_music(*state.get(), world);
    if player.music == Some(want) {
        return;
    }
    player.music = Some(want);
    player.sounding = None;
    player.decided = None;
    player.ducked = None;
    let (mut engine, title, filters) = match start_engine(want, &overrides) {
        Ok(x) => x,
        Err(e) => {
            error!("music: {e}");
            return;
        }
    };
    engine.post(Input::SetFilters(filters));
    // The band plays the levels' tunes, and the level-clear jingle (a Basie ending on it,
    // if it's loose).
    if let (Music::World(_) | Music::LevelClear, Some(f)) = (want, player.freedom) {
        engine.post(set_freedom(f));
    }
    // The fanfare cuts in quickly; everything else crossfades.
    let (fade_in, fade_out) = match want {
        Music::LevelClear => (0.01, 0.12),
        _ => (0.25, 0.45),
    };
    if let Some(h) = audio.handle.as_mut() {
        h.play(engine, fade_in, fade_out);
    }
    let reason = if overrides.0.is_some() && filters != Filters::default() { "NATHAN_MUSIC" } else { "" };
    *now_playing = NowPlaying { music: want, title, filters, reason, tuning_now: None, band_now: "" };
    // The physics follow the music the moment it starts.
    if let Some(g) = groove.as_deref_mut() {
        set_groove(g, filters);
    }
    started.write(MusicStarted(now_playing.clone()));
}

/// The physics that go with `filters` (the clock carries on: [`sync`] sets it).
fn set_groove(g: &mut Groove, filters: Filters) {
    let new = Groove { nervous: g.nervous, ..Groove::new(filters) };
    if *g != new {
        *g = Groove { clock: g.clock, ..new };
    }
}

fn set_freedom(f: BandFreedom) -> Input {
    Input::SetFreedom { lead: f.lead, comp: f.comp, bass: f.bass, drums: f.drums, dynamics: f.dynamics }
}

/// Every gameplay message, as an input.
#[allow(clippy::too_many_arguments)]
fn forward(
    mut audio: NonSendMut<Audio>,
    mut player: ResMut<LivePlayer>,
    mut freedom: MessageReader<BandFreedom>,
    mut jumped: MessageReader<Jumped>,
    mut landed: MessageReader<Landed>,
    mut nuggets: MessageReader<NuggetCollected>,
    mut died: MessageReader<PlayerDied>,
    mut checkpoints: MessageReader<CheckpointReached>,
    mut restart: MessageReader<RestartLevel>,
) {
    let mut inputs = Vec::new();
    inputs.extend(jumped.read().map(|j| if j.double { Input::Toot } else { Input::Jump { on_ground: true } }));
    inputs.extend(landed.read().map(|l| Input::Land { speed: l.speed }));
    inputs.extend(nuggets.read().map(|_| Input::Nugget));
    inputs.extend(died.read().map(|_| Input::Death));
    inputs.extend(checkpoints.read().map(|_| Input::Checkpoint));
    inputs.extend(restart.read().map(|_| Input::Restart));
    if let Some(f) = freedom.read().last().copied() {
        player.freedom = Some(f);
        inputs.push(set_freedom(f));
    }
    if let Some(h) = audio.handle.as_mut() {
        for i in inputs {
            h.post(i);
        }
    }
}

/// Feed the director what happened this frame; it decides at level start, death, checkpoints,
/// summons and every [`director::MUSIC_CHECK_SECS`] of play.
#[allow(clippy::too_many_arguments)]
fn direct(
    state: Res<State<AppState>>,
    run: Option<Res<LevelRun>>,
    overrides: Res<MusicOverride>,
    mut director: ResMut<Director>,
    mut player: ResMut<LivePlayer>,
    mut audio: NonSendMut<Audio>,
    mut restart: MessageReader<RestartLevel>,
    mut jumped: MessageReader<Jumped>,
    mut nuggets: MessageReader<NuggetCollected>,
    mut died: MessageReader<PlayerDied>,
    mut checkpoints: MessageReader<CheckpointReached>,
    mut groove_for_grip: Option<ResMut<Groove>>,
) {
    let playing = *state.get() == AppState::Playing;
    let restarted = restart.read().count() > 0;
    let mut ev = director::Events::default();
    for j in jumped.read() {
        if j.double {
            ev.toots += 1;
        } else {
            ev.ground_jumps += 1;
        }
    }
    ev.nuggets = nuggets.read().count() as u32;
    ev.deaths = died.read().count() as u32;
    ev.checkpoints = checkpoints.read().count() as u32;
    // Play time (pause excluded): the director's clock.
    let now = run.as_ref().map_or(0.0, |r| r.time);
    let d = &mut *director;
    let mut decision = None;
    let mut level_start = false;
    if playing && (!d.was_playing || restarted) {
        // Level start: everything resets, the band plays it straight.
        decision = Some(d.band.start(now));
        level_start = !restarted;
    }
    d.was_playing = playing;
    if !playing {
        return;
    }
    let (before, steps) = (d.band.stats, d.band.steps_taken);
    if let Some(decided) = d.band.step(now, ev) {
        decision = Some(decided);
    }
    let stats = d.band.stats;
    if let Some(g) = groove_for_grip.as_deref_mut() {
        let nervous = stats.level_deaths >= director::NERVOUS_DEATHS;
        if g.nervous != nervous {
            g.nervous = nervous;
        }
    }
    let new_steps = d.band.steps_taken.saturating_sub(steps);
    let decided = decision.map(|(filters, reason)| match overrides.0 {
        Some(f) => (f, "NATHAN_MUSIC"),
        None => (filters, reason),
    });
    if let Some(dec) = decided {
        player.decided = Some(dec);
    }
    let Some(h) = audio.handle.as_mut() else { return };
    if level_start {
        h.post(Input::LevelStart);
    }
    for _ in 0..new_steps {
        h.post(Input::WaltzStep);
    }
    if stats != before || decided.is_some() {
        h.post(Input::SetStats(stats));
    }
    if let Some((filters, _)) = decided {
        h.post(Input::SetFilters(filters));
    }
}

fn duck(play_state: Option<Res<State<PlayState>>>, mut player: ResMut<LivePlayer>, mut audio: NonSendMut<Audio>) {
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

/// Headless output: render as much audio as real time has passed (a 48 kHz "device").
/// Capture: exactly one video frame's audio.
fn pump(
    time: Res<Time<Real>>,
    mut audio: NonSendMut<Audio>,
    #[cfg(feature = "capture")] capture: Option<ResMut<super::capture::AudioCapture>>,
) {
    use kira::sound::Sound;
    #[cfg(feature = "capture")]
    if let Some(Manager::Capture(m)) = audio.manager.as_mut() {
        if let Some(mut c) = capture {
            c.pump(m);
        }
        return;
    }
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

/// What's sounding → NowPlaying, MusicChanged, Groove (and its clock), LiveClock.
#[allow(clippy::too_many_arguments)]
fn sync(
    time: Res<Time<Real>>,
    audio: NonSend<Audio>,
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
    // The clock as heard: the engine is `ahead_secs` ahead of the output, and time has passed
    // since it published (on the web, the browser says how far off the last sample it was
    // handed is: [`web_output_lead`]). Stepping back stops at the playhead's bar line, so the
    // beat and the physics turn together when a new meter comes in.
    let mut dt = match web_output_lead() {
        Some(lead) => -(lead + p.ahead_secs),
        None => now - player.seen.1 - p.ahead_secs,
    };
    if dt < 0.0 {
        dt = dt.max(-p.clock.position.beat * 60.0 / p.clock.bpm.max(1.0) as f64);
    }
    let heard = p.clock.advanced(dt);
    *clock = LiveClock { clock: heard, filters, tuning: p.state.tuning };
    let tuning_now = p.state.medley_phrase;
    if now_playing.tuning_now != tuning_now {
        now_playing.tuning_now = tuning_now;
    }
    let band_now = p.state.label;
    if now_playing.band_now != band_now {
        now_playing.band_now = band_now;
    }
    let sounding = (filters, p.state.tuning);
    if player.sounding != Some(sounding) {
        let first = player.sounding.is_none();
        player.sounding = Some(sounding);
        let reason = player.decided.filter(|(f, _)| *f == filters).map_or("", |(_, r)| r);
        if now_playing.filters != filters || (!first && now_playing.reason != reason) {
            now_playing.filters = filters;
            now_playing.reason = reason;
            if let Some(g) = groove.as_deref_mut() {
                set_groove(g, filters);
            }
            if !first {
                // The bar line it came in at (the playhead's bar: this frame or the last).
                let c = &p.clock;
                let secs = c.position.song_bar as f64 * c.beats_per_bar * 60.0 / c.bpm as f64;
                changed.write(MusicChanged { now: now_playing.clone(), at_secs: secs });
            }
        }
    }
    if let Some(mut g) = groove {
        let beat_secs = 60.0 / heard.bpm.max(1.0) as f64;
        g.clock = game::BeatClock::at(heard.position.song_beat, beat_secs, heard.beats_per_bar.round() as u32);
        // The laughing band's phrase and its wobble, for the phrase's nudge. The voices'
        // wobble runs on song time fitted to the loop (`Medley::new`): so does this.
        g.phrase = tuning_now;
        g.sway = if tuning_now.is_some() {
            let loop_secs = heard.loop_beats * beat_secs;
            let wobble = if loop_secs > 0.0 { Wobble::MEDLEY.fitted(loop_secs) } else { Wobble::MEDLEY };
            (std::f64::consts::TAU * wobble.hz * heard.position.song_beat * beat_secs).sin() as f32
        } else {
            0.0
        };
    }
}

/// On the web: seconds from now until the last sample the game handed the browser reaches the
/// ear. cpal's WebAudio host renders a whole buffer (2048 frames, ~43 ms) per callback and
/// schedules it a buffer ahead of the context's clock, and the browser adds its own pipeline
/// (`baseLatency`) and the device's (`outputLatency`, often tens of ms, Bluetooth far more):
/// about 0.1 s that a native stream doesn't have. `index.html` tracks when the last scheduled
/// buffer ends and answers `nathanAudioLead()`; the publish and that buffer's scheduling happen
/// in the same callback on the main thread, so the lead lines up with what was published.
/// `None` natively, or when the page doesn't know (no audio yet).
#[cfg(target_arch = "wasm32")]
fn web_output_lead() -> Option<f64> {
    use wasm_bindgen::JsCast;
    let w = web_sys::window()?;
    let f = js_sys::Reflect::get(w.as_ref(), &"nathanAudioLead".into()).ok()?.dyn_into::<js_sys::Function>().ok()?;
    let lead = f.call0(w.as_ref()).ok()?.as_f64()?;
    // Never trust a wild value (a suspended context, a clock hiccup) with the physics.
    lead.is_finite().then(|| lead.clamp(0.0, 1.0))
}

#[cfg(not(target_arch = "wasm32"))]
fn web_output_lead() -> Option<f64> {
    None
}

/// Pending Han blips: seconds until each plays.
#[derive(Resource, Default)]
struct HanBabble(Vec<f32>);

#[allow(clippy::too_many_arguments)]
fn play_sfx(
    mut audio: NonSendMut<Audio>,
    mut count: ResMut<SfxCount>,
    mut babble: ResMut<HanBabble>,
    mut jumped: MessageReader<Jumped>,
    mut landed: MessageReader<Landed>,
    mut nuggets: MessageReader<NuggetCollected>,
    mut died: MessageReader<PlayerDied>,
    mut checkpoints: MessageReader<CheckpointReached>,
    mut completed: MessageReader<LevelCompleted>,
    mut han: MessageReader<HanSays>,
    mut requests: MessageReader<PlaySfx>,
    mut rng: ResMut<SfxRng>,
) {
    let mut queue: Vec<(Sfx, f32)> = Vec::new();
    for j in jumped.read() {
        queue.push((if j.double { Sfx::Toot } else { Sfx::Jump }, -2.0));
    }
    for l in landed.read() {
        if let Some(db) = land_db(l.speed) {
            queue.push((Sfx::Land, db));
        }
    }
    queue.extend(nuggets.read().map(|_| (Sfx::Nugget, -3.0)));
    queue.extend(died.read().map(|_| (Sfx::Splat, 0.0)));
    queue.extend(checkpoints.read().map(|_| (Sfx::Checkpoint, -2.0)));
    queue.extend(completed.read().map(|_| (Sfx::Flush, 0.0)));
    queue.extend(requests.read().map(|PlaySfx(s)| (*s, -2.0)));
    for (s, db) in queue {
        let data = audio.sfx(s).cloned();
        audio.play(data, db, 1.0, &mut count);
    }
    let rng = &mut rng.0;
    for line in han.read() {
        // A few syllables over ~0.4s, roughly following how much he says.
        let syllables = line.text.split_whitespace().count().clamp(2, 4);
        let mut t = 0.0;
        for _ in 0..syllables {
            babble.0.push(t);
            t += 0.4 / syllables as f32 * rng.random_range(0.8..1.2);
        }
    }
}

/// Landing thud volume (dB) for a fall speed in px/s; `None` for tiny hops.
pub fn land_db(speed: f32) -> Option<f32> {
    let k = speed / LAND_FULL_SPEED;
    (k >= 0.15).then(|| 20.0 * (0.2 + 0.8 * k.min(1.0)).log10() - 1.0)
}

fn babble(
    time: Res<Time>,
    mut audio: NonSendMut<Audio>,
    mut count: ResMut<SfxCount>,
    mut pending: ResMut<HanBabble>,
    mut rng: ResMut<SfxRng>,
) {
    if pending.0.is_empty() || audio.han.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    let rng = &mut rng.0;
    let mut last = None;
    let mut due = Vec::new();
    pending.0.retain_mut(|t| {
        *t -= dt;
        if *t > 0.0 {
            return true;
        }
        due.push(());
        false
    });
    for () in due {
        // Never the same syllable twice in a row.
        let mut v = rng.random_range(0..audio.han.len());
        if Some(v) == last {
            v = (v + 1) % audio.han.len();
        }
        last = Some(v);
        let data = audio.han[v].clone();
        audio.play(Some(data), -4.0, rng.random_range(0.92..1.12), &mut count);
    }
}

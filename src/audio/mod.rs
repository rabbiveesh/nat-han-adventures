//! Chiptune audio: a tiny NES-style synth (2 pulse voices, triangle bass, noise drums) renders
//! songs written in MML to PCM at startup, and hands them to bevy_kira_audio. No audio files.
//!
//! Modules:
//! - [`mml`]: parses the MML dialect below into note events.
//! - [`synth`]: renders a [`Song`] / sound effect to stereo frames.
//! - [`songs`]: the soundtrack — chunky 8-bit takes on public-domain (pre-1931) jazz standards.
//! - [`sfx`]: sound effects (toot, splat, nugget, flush, ...).
//! - [`chart`]: chord charts ([`Song::chords`]); [`theory`]: just intonation, Coltrane changes,
//!   melodic-minor and quartal harmony; [`accomp`]: generated comping + bass for the
//!   reharmonizing [`Filters`]; [`director`]: picks the filters from how the player is doing;
//!   [`demo`]: a ii-V-I exercise for hearing the filters.
//!
//! # MML dialect
//! Whitespace and `|` (bar lines) are ignored.
//! - Notes `c d e f g a b`, optional accidental `+`/`#` (sharp) or `-` (flat), optional length
//!   (`1 2 4 8 16 32`, whole..32nd; default from `l`), optional `.` (dotted, x1.5).
//!   e.g. `c4 e-8 g+16. a`
//! - `r` rest, with the same length rules.
//! - `o<n>` set octave (o4 contains middle-C = c), `>` octave up, `<` octave down.
//! - `l<n>` default note length. `t<bpm>` is NOT used: tempo is [`Song::bpm`].
//! - `v<0-15>` volume. `@<0-3>` pulse duty: 12.5%, 25%, 50%, 75% (pulse channels only).
//! - `&` between two notes ties them (no re-attack), e.g. `c4&c16`.
//! - `[ ... ]<n>` repeats the bracketed part n times (nestable).
//! - Noise channel (drums) uses drum letters instead of notes: `k` kick, `s` snare, `h` closed hat,
//!   `H` open hat, `r` rest — same length rules, e.g. `k8 h8 s8 h8`.
//!
//! Swing ([`Song::swing`]): 0.0 = straight; 0.33 ≈ triplet swing. Off-beat 8th notes are
//! delayed by `swing * (an 8th)` and the preceding on-beat 8th lengthened to match.
//!
//! ## Dialect details (decisions where the above leaves room)
//! - Lengths: any `n` in `1..=96` means a 1/n note (so `l12` / `c12` are 8th-note triplets),
//!   and more than one dot is allowed (`c4..` = 4 + 8 + 16). A dot needs an explicit length.
//! - Octaves range over `o0..=o8`; `c-` / `b+` cross into the neighbouring octave.
//! - `v` scales every channel, the triangle and drums included (the real NES triangle has no
//!   volume, but it's handy for balancing). `@` is accepted but ignored on triangle/noise, and so
//!   are `o < >` on the noise channel.
//! - `&`: tying to the *same* pitch makes one longer note; tying to a *different* pitch is a slur
//!   (pitch changes, no re-attack). Commands may sit between `&` and the note (`b4& >c4`).
//!   `r4&r8` extends a rest. `&` before a rest/drum after a note is an error.
//! - `[ ... ]` without a count repeats twice. State changes inside a repeat (octave, volume, ...)
//!   carry over exactly as if the body were written out n times.
//! - Swing (see [`synth::apply_swing`]) moves only 8th notes/rests that start on an off-beat 8th
//!   position; the event just before is lengthened to meet them. 16ths, dotted rhythms and
//!   downbeats never move, and each event is placed from its own unswung time (no drift).
//! - Notes play their full written length (with a ~2ms attack and ~8ms release inside it).
//!   Drums ring for their natural length regardless of the written length (open hats are choked
//!   by the next hit). A looping song's length is the longest track's length (trailing rests
//!   count), and anything ringing past the loop point wraps around to the start.
//! - Melodic channels are case-sensitive: notes are lowercase only; `t` is rejected.
//!
//! # Playback
//! [`plugin`] renders all sfx (and the level-clear jingle) at startup and each song when it
//! starts. Music follows [`AppState`]; see [`desired_music`]. A song (re)starts plain
//! ([`Filters::default`]); during a level, the [`director`] picks new [`Filters`] from how the
//! player is doing (after a death, on reaching a checkpoint; back to plain on level start).
//! A new decision doesn't restart the song: the new version is rendered incrementally (a few
//! ms per frame, see [`synth::RenderJob`]) and swapped in at the next bar line, at the same song
//! position, with a short crossfade — the band changing its mind on the fly. Only the playing
//! render and the one being prepared are kept in memory. [`NowPlaying`] says what's on;
//! [`MusicStarted`] / [`MusicChanged`] fire when a track starts / switches filters.
//!
//! Dev override (native only): `NATHAN_MUSIC=coltrane|quartal|melodic|original[+ji]` (or just
//! `ji`) forces the filters of every looping song.

pub mod accomp;
pub mod chart;
pub mod demo;
pub mod director;
pub mod mml;
pub mod sfx;
pub mod songs;
pub mod synth;
pub mod theory;
pub mod tuning;

use std::time::Duration;

use bevy::{platform::collections::HashMap, prelude::*};
use bevy_kira_audio::prelude::{
    AudioApp, AudioChannel, AudioControl, AudioEasing, AudioInstance, AudioPlugin, AudioSource, AudioTween,
    PlaybackState, StaticSoundData, StaticSoundSettings,
};
use rand::Rng;

use crate::{
    events::{CheckpointReached, HanSays, Jumped, Landed, LevelCompleted, NuggetCollected, PlaySfx, PlayerDied},
    game::{LevelRun, RestartLevel},
    level::Levels,
    state::{AppState, CurrentLevel, PlayState},
};

use director::PlayStats;

/// Music volume (dB). Sfx play at 0 dB on their own channel.
const MUSIC_DB: f32 = -4.0;
/// Extra music attenuation while paused.
const PAUSE_DUCK_DB: f32 = -12.0;
const FADE_OUT: Duration = Duration::from_millis(450);
const FADE_IN: Duration = Duration::from_millis(250);
/// Crossfade when the filters switch mid-song.
const SWITCH_FADE: Duration = Duration::from_millis(150);
/// Per-frame time budget for rendering a new version of the playing song.
const RENDER_BUDGET: Duration = Duration::from_micros(3500);
/// A bar line closer than this (seconds) is too close to switch on; take the next one.
const SWITCH_MARGIN: f64 = 0.03;
/// Max fall speed in px/s ([`crate::game::tuning::MAX_FALL`]), for scaling the landing thud.
const LAND_FULL_SPEED: f32 = 420.0;

/// bevy_kira_audio channel for music.
#[derive(Resource)]
pub struct MusicChannel;

/// bevy_kira_audio channel for sound effects.
#[derive(Resource)]
pub struct SfxChannel;

pub fn plugin(app: &mut App) {
    if !app.is_plugin_added::<AudioPlugin>() {
        app.add_plugins(AudioPlugin);
    }
    app.add_audio_channel::<MusicChannel>()
        .add_audio_channel::<SfxChannel>()
        .init_resource::<MusicPlayer>()
        .init_resource::<NowPlaying>()
        .init_resource::<SongSource>()
        .init_resource::<Director>()
        .init_resource::<HanBabble>()
        .insert_resource(MusicOverride(music_override()))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (follow_state, direct, prepare_switch, switch, duck_on_pause, play_sfx, babble).chain(),
        );
}

/// Where songs come from: [`songs::song`], unless a test swaps in its own.
#[derive(Resource, Clone, Copy)]
pub struct SongSource(pub fn(Music) -> Song);

impl Default for SongSource {
    fn default() -> Self {
        SongSource(songs::song)
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
            warn!("NATHAN_MUSIC={v:?} not understood (try coltrane, quartal, melodic, original, +ji)");
        }
        f
    }
    #[cfg(target_arch = "wasm32")]
    None
}

/// Rendered sound effects (and the short level-clear jingle), as kira assets.
#[derive(Resource)]
pub struct AudioBank {
    sfx: HashMap<Sfx, Handle<AudioSource>>,
    han: Vec<Handle<AudioSource>>,
    jingle: Option<Handle<AudioSource>>,
}

impl AudioBank {
    pub fn sfx(&self, sfx: Sfx) -> Handle<AudioSource> {
        self.sfx[&sfx].clone()
    }
}

/// Wrap rendered frames as a kira sound (looping over the whole thing if it loops).
pub fn to_source(r: synth::Rendered) -> AudioSource {
    let mut settings = StaticSoundSettings::new();
    if r.looping {
        settings = settings.loop_region(..);
    }
    AudioSource {
        sound: StaticSoundData { sample_rate: r.sample_rate, frames: r.frames.into(), settings, slice: None },
    }
}

fn setup(mut commands: Commands, mut assets: ResMut<Assets<AudioSource>>, source: Res<SongSource>) {
    let started = bevy::platform::time::Instant::now();
    let sfx = Sfx::ALL.into_iter().map(|s| (s, assets.add(to_source(sfx::render(s))))).collect();
    let han = (0..sfx::HAN_VARIANTS).map(|v| assets.add(to_source(sfx::han_blip(v)))).collect();
    let jingle = match synth::render_song(&(source.0)(Music::LevelClear)) {
        Ok(r) => Some(assets.add(to_source(r))),
        Err(e) => {
            error!("{e}");
            None
        }
    };
    commands.insert_resource(AudioBank { sfx, han, jingle });
    debug!("audio startup render: {:.0}ms", started.elapsed().as_secs_f32() * 1000.0);
}

/// Which music goes with an app state.
pub fn desired_music(state: AppState, levels: Option<&Levels>, current: CurrentLevel) -> Music {
    match state {
        AppState::Title | AppState::LevelSelect => Music::Title,
        AppState::Playing => {
            let world = levels.and_then(|l| l.0.get(current.0)).map_or(1, |l| l.world);
            Music::World(world)
        }
        AppState::LevelComplete => Music::LevelClear,
        AppState::Victory => Music::Victory,
    }
}

/// What's playing, for the UI ("now playing" toasts).
#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct NowPlaying {
    pub music: Music,
    pub title: &'static str,
    pub filters: Filters,
    /// Why the band plays it this way ("GIANT STEPS!"), or "".
    pub reason: &'static str,
}

impl Default for NowPlaying {
    fn default() -> Self {
        NowPlaying { music: Music::Title, title: "", filters: Filters::default(), reason: "" }
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
    /// Song position (seconds) of the bar line where the new version came in.
    pub at_secs: f64,
}

/// The music channel's state.
#[derive(Resource, Default)]
pub struct MusicPlayer {
    current: Option<Current>,
    /// A new version of the current song, being rendered / waiting for its bar line.
    pending: Option<Pending>,
    /// Filters the director asked for (applied by [`prepare_switch`]).
    wanted: Option<(Filters, &'static str)>,
    /// Duck state last applied to the current instance (`None`: not yet applied).
    ducked: Option<bool>,
}

struct Current {
    music: Music,
    song: Song,
    filters: Filters,
    instance: Handle<AudioInstance>,
    /// Kept alive while it plays.
    _source: Option<Handle<AudioSource>>,
    /// Real time (s) at which song position 0 was (or would have been) playing.
    origin: f64,
    /// Loop length (s); 0 for one-shots.
    len_secs: f64,
}

struct Pending {
    filters: Filters,
    reason: &'static str,
    job: Option<synth::RenderJob>,
    ready: Option<Handle<AudioSource>>,
    /// Real time of the switch, and the song position (a bar line) it lands on.
    at: Option<(f64, f64)>,
}

impl MusicPlayer {
    pub fn now_playing(&self) -> Option<Music> {
        self.current.as_ref().map(|c| c.music)
    }

    /// Filters of the version that's playing.
    pub fn filters(&self) -> Option<Filters> {
        self.current.as_ref().map(|c| c.filters)
    }

    /// Filters of a version being prepared, if any.
    pub fn pending_filters(&self) -> Option<Filters> {
        self.pending.as_ref().map(|p| p.filters)
    }
}

/// The filters a song can actually take: no reharmonizing without a chart.
fn effective(filters: Filters, song: &Song) -> Filters {
    if song.chords.trim().is_empty() { Filters { harmony: Harmony::Original, ..filters } } else { filters }
}

/// Start a render job for `filters` (falling back to the original harmony if the chart is
/// unusable; errors are logged).
fn start_job(song: &Song, filters: Filters) -> Option<(synth::RenderJob, Filters)> {
    let seed = rand::rng().random::<u64>();
    match synth::RenderJob::new(song, filters, seed) {
        Ok(j) => Some((j, filters)),
        Err(e) if filters.harmony != Harmony::Original => {
            error!("{e}");
            let plain = Filters { harmony: Harmony::Original, ..filters };
            synth::RenderJob::new(song, plain, seed).ok().map(|j| (j, plain))
        }
        Err(e) => {
            error!("{e}");
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn follow_state(
    state: Res<State<AppState>>,
    levels: Option<Res<Levels>>,
    current_level: Res<CurrentLevel>,
    time: Res<Time<Real>>,
    bank: Option<Res<AudioBank>>,
    source: Res<SongSource>,
    overrides: Res<MusicOverride>,
    mut player: ResMut<MusicPlayer>,
    mut now_playing: ResMut<NowPlaying>,
    mut started: MessageWriter<MusicStarted>,
    mut sources: ResMut<Assets<AudioSource>>,
    mut instances: ResMut<Assets<AudioInstance>>,
    channel: Res<AudioChannel<MusicChannel>>,
) {
    let want = desired_music(*state.get(), levels.as_deref(), *current_level);
    if player.current.as_ref().is_some_and(|c| c.music == want) {
        return;
    }
    // The fanfare cuts in quickly; everything else crossfades.
    let (fade_out, fade_in) = match want {
        Music::LevelClear => (Duration::from_millis(120), Duration::from_millis(10)),
        _ => (FADE_OUT, FADE_IN),
    };
    if let Some(old) = player.current.take()
        && let Some(mut instance) = instances.get_mut(&old.instance)
    {
        instance.stop(AudioTween::new(fade_out, AudioEasing::OutPowi(2)));
    }
    player.pending = None;
    player.wanted = None;
    player.ducked = None;
    let song = (source.0)(want);
    let mut filters = Filters::default();
    let handle = if want == Music::LevelClear {
        bank.and_then(|b| b.jingle.clone())
    } else {
        if let Some(f) = overrides.0 {
            filters = effective(f, &song);
        }
        let t = bevy::platform::time::Instant::now();
        let job = start_job(&song, filters).and_then(|(mut job, f)| {
            filters = f;
            while !job.step(Duration::MAX) {}
            job.into_rendered()
        });
        debug!("rendered {want:?} {filters:?} in {:.0}ms", t.elapsed().as_secs_f32() * 1000.0);
        job.map(|r| sources.add(to_source(r)))
    };
    let now = time.elapsed_secs_f64();
    let len_secs = if song.looping { song_secs(&song) } else { 0.0 };
    let instance = match &handle {
        Some(h) => channel.play(h.clone()).with_volume(MUSIC_DB).fade_in(AudioTween::linear(fade_in)).handle(),
        // Unplayable song: remember it anyway so we don't retry every frame.
        None => Handle::default(),
    };
    player.current = Some(Current { music: want, song: song.clone(), filters, instance, _source: handle, origin: now, len_secs });
    let reason = if overrides.0.is_some() && filters != Filters::default() { "NATHAN_MUSIC" } else { "" };
    *now_playing = NowPlaying { music: want, title: song.title, filters, reason };
    started.write(MusicStarted(now_playing.clone()));
}

/// Length of one loop of a song, in seconds (the longest track).
fn song_secs(song: &Song) -> f64 {
    let beats = [
        (song.pulse1, mml::Channel::Melodic),
        (song.pulse2, mml::Channel::Melodic),
        (song.triangle, mml::Channel::Melodic),
        (song.noise, mml::Channel::Drums),
    ]
    .iter()
    .filter_map(|(src, ch)| mml::parse(src, *ch).ok())
    .map(|t| t.length)
    .fold(0.0, f64::max);
    // The renderer rounds to whole samples.
    (beats * synth::SAMPLE_RATE as f64 * 60.0 / song.bpm as f64).round() / synth::SAMPLE_RATE as f64
}

/// The director's bookkeeping between frames.
#[derive(Resource, Debug, Default)]
pub struct Director {
    /// Level-long counters (the rolling-window fields are filled in at each decision).
    pub stats: PlayStats,
    /// What happened in the last [`director::MUSIC_CHECK_SECS`] of play.
    pub window: director::Window,
    /// [`LevelRun::time`] when the level started, and of the next periodic check.
    level_start: f32,
    next_check: f32,
    was_playing: bool,
}

/// Track play stats and make decisions at level start, death, checkpoints and every
/// [`director::MUSIC_CHECK_SECS`] of play.
#[allow(clippy::too_many_arguments)]
fn direct(
    state: Res<State<AppState>>,
    run: Option<Res<LevelRun>>,
    overrides: Res<MusicOverride>,
    mut director: ResMut<Director>,
    mut player: ResMut<MusicPlayer>,
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
    // Play time (pause excluded): the director's clock.
    let now = run.as_ref().map_or(0.0, |r| r.time);
    let d = &mut *director;
    let mut decision = None;
    if playing && (!d.was_playing || restarted) {
        // Level start: everything resets, the band plays it straight.
        d.stats = PlayStats::default();
        d.window = director::Window::default();
        d.level_start = now;
        d.next_check = now + director::MUSIC_CHECK_SECS;
        decision = Some((Filters::default(), ""));
    }
    d.was_playing = playing;
    if !playing {
        return;
    }
    d.window.record(now, toots, got, deaths);
    d.stats.level_deaths += deaths;
    d.stats.checkpoint_deaths += deaths;
    if deaths > 0 || cps > 0 || now >= d.next_check {
        d.window.fill(&mut d.stats, now, d.level_start);
        decision = Some(director::choose_filters(&d.stats));
        d.next_check = now + director::MUSIC_CHECK_SECS;
        if cps > 0 {
            d.stats.checkpoint_deaths = 0;
        }
    }
    if let Some((filters, reason)) = decision {
        let decided = match overrides.0 {
            Some(f) => (f, "NATHAN_MUSIC"),
            None => (filters, reason),
        };
        player.wanted = Some(decided);
    }
}

/// Turn a director decision into a render job; step the job within the frame budget; once
/// rendered, schedule the switch at the next bar line.
fn prepare_switch(
    time: Res<Time<Real>>,
    mut player: ResMut<MusicPlayer>,
    mut sources: ResMut<Assets<AudioSource>>,
    instances: Res<Assets<AudioInstance>>,
) {
    let player = &mut *player;
    let Some(cur) = &player.current else {
        player.wanted = None;
        return;
    };
    if let Some((wanted, reason)) = player.wanted.take()
        && cur.len_secs > 0.0
    {
        let want = effective(wanted, &cur.song);
        match &mut player.pending {
            Some(p) if p.filters == want => p.reason = reason,
            _ if cur.filters == want => player.pending = None,
            _ => {
                // A newer decision replaces whatever was being prepared.
                player.pending = start_job(&cur.song, want).map(|(job, filters)| Pending {
                    filters,
                    reason,
                    job: Some(job),
                    ready: None,
                    at: None,
                });
            }
        }
    }
    let Some(p) = &mut player.pending else { return };
    if let Some(job) = &mut p.job {
        if !job.step(RENDER_BUDGET) {
            return;
        }
        let rendered = p.job.take().and_then(|j| j.into_rendered()).expect("job finished");
        p.ready = Some(sources.add(to_source(rendered)));
    }
    if p.at.is_none() {
        let now = time.elapsed_secs_f64();
        let pos = position(cur, now, &instances);
        let bar = 4.0 * 60.0 / cur.song.bpm as f64;
        let next = ((pos + SWITCH_MARGIN) / bar).ceil() * bar;
        p.at = Some((now + (next - pos), next.rem_euclid(cur.len_secs)));
    }
}

/// Song position (s) of the current track: kira's when it reports one, else our own clock.
fn position(cur: &Current, now: f64, instances: &Assets<AudioInstance>) -> f64 {
    let ours = (now - cur.origin).rem_euclid(cur.len_secs.max(1e-9));
    match instances.get(&cur.instance).map(|i| i.state()) {
        Some(PlaybackState::Playing { position }) => position.rem_euclid(cur.len_secs.max(1e-9)),
        _ => ours,
    }
}

/// At the scheduled bar line: crossfade to the new version at the same song position.
#[allow(clippy::too_many_arguments)]
fn switch(
    time: Res<Time<Real>>,
    mut player: ResMut<MusicPlayer>,
    mut now_playing: ResMut<NowPlaying>,
    mut changed: MessageWriter<MusicChanged>,
    mut instances: ResMut<Assets<AudioInstance>>,
    channel: Res<AudioChannel<MusicChannel>>,
) {
    let now = time.elapsed_secs_f64();
    let due = player.pending.as_ref().is_some_and(|p| p.ready.is_some() && p.at.is_some_and(|(t, _)| now >= t));
    if !due {
        return;
    }
    let p = player.pending.take().expect("checked");
    let (at, bar_pos) = p.at.expect("checked");
    let source = p.ready.expect("checked");
    let Some(cur) = &mut player.current else { return };
    if let Some(mut old) = instances.get_mut(&cur.instance) {
        old.stop(AudioTween::new(SWITCH_FADE, AudioEasing::OutPowi(2)));
    }
    // We're a little past the bar line (frames are discrete): start that far into it.
    let pos = (bar_pos + (now - at)).rem_euclid(cur.len_secs);
    cur.instance = channel
        .play(source.clone())
        .start_from(pos)
        .with_volume(MUSIC_DB)
        .fade_in(AudioTween::linear(SWITCH_FADE))
        .handle();
    cur._source = Some(source);
    cur.filters = p.filters;
    cur.origin = now - pos;
    *now_playing = NowPlaying { music: cur.music, title: cur.song.title, filters: p.filters, reason: p.reason };
    player.ducked = None;
    changed.write(MusicChanged { now: now_playing.clone(), at_secs: bar_pos });
}

fn duck_on_pause(
    play_state: Option<Res<State<PlayState>>>,
    mut player: ResMut<MusicPlayer>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    let duck = play_state.is_some_and(|s| *s.get() == PlayState::Paused);
    if player.ducked == Some(duck) {
        return;
    }
    let Some(cur) = &player.current else { return };
    // The instance appears once bevy_kira_audio has processed the play command; retry until then.
    let Some(mut instance) = instances.get_mut(&cur.instance) else { return };
    let db = if duck { MUSIC_DB + PAUSE_DUCK_DB } else { MUSIC_DB };
    instance.set_decibels(db, AudioTween::linear(Duration::from_millis(200)));
    player.ducked = Some(duck);
}

/// Pending Han blips: seconds until each plays.
#[derive(Resource, Default)]
struct HanBabble(Vec<f32>);

#[allow(clippy::too_many_arguments)]
fn play_sfx(
    bank: Res<AudioBank>,
    channel: Res<AudioChannel<SfxChannel>>,
    mut babble: ResMut<HanBabble>,
    mut jumped: MessageReader<Jumped>,
    mut landed: MessageReader<Landed>,
    mut nuggets: MessageReader<NuggetCollected>,
    mut died: MessageReader<PlayerDied>,
    mut checkpoints: MessageReader<CheckpointReached>,
    mut completed: MessageReader<LevelCompleted>,
    mut han: MessageReader<HanSays>,
    mut requests: MessageReader<PlaySfx>,
) {
    let play = |s: Sfx, db: f32| {
        channel.play(bank.sfx(s)).with_volume(db);
    };
    for j in jumped.read() {
        play(if j.double { Sfx::Toot } else { Sfx::Jump }, -2.0);
    }
    for l in landed.read() {
        if let Some(db) = land_db(l.speed) {
            play(Sfx::Land, db);
        }
    }
    for _ in nuggets.read() {
        play(Sfx::Nugget, -3.0);
    }
    for _ in died.read() {
        play(Sfx::Splat, 0.0);
    }
    for _ in checkpoints.read() {
        play(Sfx::Checkpoint, -2.0);
    }
    for _ in completed.read() {
        play(Sfx::Flush, 0.0);
    }
    for PlaySfx(s) in requests.read() {
        play(*s, -2.0);
    }
    let mut rng = rand::rng();
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
    bank: Res<AudioBank>,
    channel: Res<AudioChannel<SfxChannel>>,
    mut pending: ResMut<HanBabble>,
) {
    if pending.0.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    let mut rng = rand::rng();
    let mut last = None;
    pending.0.retain_mut(|t| {
        *t -= dt;
        if *t > 0.0 {
            return true;
        }
        // Never the same syllable twice in a row.
        let mut v = rng.random_range(0..bank.han.len());
        if Some(v) == last {
            v = (v + 1) % bank.han.len();
        }
        last = Some(v);
        channel
            .play(bank.han[v].clone())
            .with_volume(-4.0)
            .with_playback_rate(rng.random_range(0.92..1.12));
        false
    });
}

/// Which piece of music to play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum Music {
    Title,
    /// Level music for world 1..=5.
    World(u8),
    /// Short fanfare on reaching the goal (doesn't loop).
    LevelClear,
    /// After level 10: credits music.
    Victory,
}

impl Music {
    pub const ALL: [Music; 8] = [
        Music::Title,
        Music::World(1),
        Music::World(2),
        Music::World(3),
        Music::World(4),
        Music::World(5),
        Music::LevelClear,
        Music::Victory,
    ];
}

/// A song in MML. Tracks may differ in length; the song loops (if `looping`) at the end of the
/// longest one.
#[derive(Debug, Clone)]
pub struct Song {
    /// Title shown in credits, e.g. "Sweet Georgia Brown (1925)".
    pub title: &'static str,
    pub bpm: f32,
    pub swing: f32,
    pub looping: bool,
    /// Lead melody (pulse 1).
    pub pulse1: &'static str,
    /// Harmony / comping (pulse 2).
    pub pulse2: &'static str,
    /// Walking bass (triangle).
    pub triangle: &'static str,
    /// Drums (noise).
    pub noise: &'static str,
    /// Home key's tonic as a pitch class (0 = C, 1 = C#/Db, ... 11 = B). Just intonation tunes
    /// every note relative to it.
    pub key: u8,
    /// Chord chart for the whole song (see "Chord charts" below). Reharmonizing filters build new
    /// bass + comping from it. Empty = the song can't be reharmonized (only [`Harmony::Original`]).
    pub chords: &'static str,
}

/// # Chord charts
/// One entry per 4/4 bar, bars separated by `|` (leading/trailing `|` and whitespace ignored),
/// covering the song from its first bar to its loop point — exactly `length_in_beats / 4` bars.
/// A bar holds 1, 2 or 4 space-separated chord tokens that split it evenly (4, 2+2, 1+1+1+1 beats).
/// `%` repeats the previous chord (as a whole bar `| % |` or inside one, `C % F C/E`).
/// Chord token: root `A`–`G` with optional `#`/`b`, then a quality:
/// `` (major triad), `6`, `maj7`, `7`, `9`, `7b9`, `7#9`, `7#5`, `7sus4`, `m`, `m6`, `m7`,
/// `mMaj7`, `m7b5`, `dim7`, `aug`; optionally `/<note>` for a slash bass. E.g.
/// `| D7 | % | G7 | % | C7 | % | F6 | Am7b5 D7 |`. Parsed by [`chart::parse`]; errors name
/// the bar and token. For harmonic analysis `6`/`maj7`/triads are all "major" (tonics are
/// usually written `F6`), and the dominant family is `7 9 7b9 7#9 7#5 7sus4`.
///
/// "Filters" chosen by the [`director`] from how the player is doing, to make the music
/// clunkier and stranger. See [`theory`] for the music theory, [`accomp`] for the generated
/// accompaniment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub struct Filters {
    pub harmony: Harmony,
    /// Retune everything to 5-limit just intonation relative to [`Song::key`].
    pub just_intonation: bool,
}

/// How the accompaniment (pulse 2 + triangle) is harmonized. The melody and drums never change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub enum Harmony {
    /// As written.
    #[default]
    Original,
    /// Coltrane changes: ii-V-I / V-I resolutions become Giant Steps cycles through major thirds.
    Coltrane,
    /// McCoy Tyner: quartal voicings (stacked fourths), pounding root-fifth left hand.
    Quartal,
    /// Every chord replaced by a melodic minor sonority (altered, lydian dominant, mMaj7, ...).
    MelodicMinor,
}

impl Harmony {
    pub const ALL: [Harmony; 4] = [Harmony::Original, Harmony::Coltrane, Harmony::Quartal, Harmony::MelodicMinor];

    /// Short upper-case label ("" for the original).
    pub fn label(self) -> &'static str {
        match self {
            Harmony::Original => "",
            Harmony::Coltrane => "COLTRANE CHANGES",
            Harmony::Quartal => "QUARTAL",
            Harmony::MelodicMinor => "MELODIC MINOR",
        }
    }

    /// Lower-case name for files and the `NATHAN_MUSIC` override.
    pub fn slug(self) -> &'static str {
        match self {
            Harmony::Original => "original",
            Harmony::Coltrane => "coltrane",
            Harmony::Quartal => "quartal",
            Harmony::MelodicMinor => "melodic",
        }
    }
}

impl Filters {
    /// "COLTRANE CHANGES", "JUST INTONATION", "QUARTAL + JUST INTONATION", or "" when plain.
    pub fn label(&self) -> String {
        match (self.harmony.label(), self.just_intonation) {
            (h, false) => h.to_string(),
            ("", true) => "JUST INTONATION".to_string(),
            (h, true) => format!("{h} + JUST INTONATION"),
        }
    }

    /// Parse `coltrane`, `quartal`, `melodic`, `original`, each optionally `+ji`, or just `ji`
    /// (case-insensitive). `None` if not understood.
    pub fn parse(s: &str) -> Option<Filters> {
        let mut f = Filters::default();
        for part in s.to_ascii_lowercase().split('+').map(str::trim).filter(|p| !p.is_empty()) {
            match part {
                "ji" => f.just_intonation = true,
                p => f.harmony = *Harmony::ALL.iter().find(|h| h.slug() == p || (p == "melodicminor" && **h == Harmony::MelodicMinor))?,
            }
        }
        Some(f)
    }
}

/// Sound effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum Sfx {
    Jump,
    /// Double jump: a short comedic toot.
    Toot,
    Land,
    Nugget,
    Splat,
    Checkpoint,
    /// Toilet flush on reaching the goal.
    Flush,
    MenuMove,
    MenuSelect,
    /// Han talking: a little "blip blip" babble.
    HanBlip,
}

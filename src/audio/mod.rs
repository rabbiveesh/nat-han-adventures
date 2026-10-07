//! Chiptune audio: a tiny NES-style synth (2 pulse voices, triangle bass, noise drums) renders
//! songs written in MML to PCM at startup, and hands them to bevy_kira_audio. No audio files.
//!
//! Modules:
//! - [`mml`]: parses the MML dialect below into note events.
//! - [`synth`]: renders a [`Song`] / sound effect to stereo frames.
//! - [`songs`]: the soundtrack — chunky 8-bit takes on public-domain (pre-1931) jazz standards.
//! - [`sfx`]: sound effects (toot, splat, nugget, flush, ...).
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
//! [`plugin`] renders all sfx at startup and each song lazily, the first time it's needed
//! (only one world song is kept in memory at a time). Music follows [`AppState`]; see
//! [`desired_music`].

pub mod mml;
pub mod sfx;
pub mod songs;
pub mod synth;

use std::time::Duration;

use bevy::{platform::collections::HashMap, prelude::*};
use bevy_kira_audio::prelude::{
    AudioApp, AudioChannel, AudioControl, AudioEasing, AudioInstance, AudioPlugin, AudioSource, AudioTween,
    StaticSoundData, StaticSoundSettings,
};
use rand::Rng;

use crate::{
    events::{CheckpointReached, GusSays, Jumped, Landed, LevelCompleted, NuggetCollected, PlaySfx, PlayerDied},
    level::Levels,
    state::{AppState, CurrentLevel, PlayState},
};

/// Music volume (dB). Sfx play at 0 dB on their own channel.
const MUSIC_DB: f32 = -4.0;
/// Extra music attenuation while paused.
const PAUSE_DUCK_DB: f32 = -12.0;
const FADE_OUT: Duration = Duration::from_millis(450);
const FADE_IN: Duration = Duration::from_millis(250);
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
        .init_resource::<GusBabble>()
        .add_systems(Startup, setup)
        .add_systems(Update, (follow_state, duck_on_pause, play_sfx, babble).chain());
}

/// Rendered audio, as kira assets. Music is rendered on first use.
#[derive(Resource)]
pub struct AudioBank {
    sfx: HashMap<Sfx, Handle<AudioSource>>,
    gus: Vec<Handle<AudioSource>>,
    music: HashMap<Music, Handle<AudioSource>>,
}

impl AudioBank {
    pub fn sfx(&self, sfx: Sfx) -> Handle<AudioSource> {
        self.sfx[&sfx].clone()
    }

    /// The music asset, rendering it now if needed. Only one world song is cached at a time
    /// (they're the big ones); the others are dropped when a new world's song is rendered.
    /// `None` (and an error log) if the song's MML doesn't parse.
    pub fn music(&mut self, music: Music, assets: &mut Assets<AudioSource>) -> Option<Handle<AudioSource>> {
        if let Some(h) = self.music.get(&music) {
            return Some(h.clone());
        }
        let started = bevy::platform::time::Instant::now();
        let rendered = match synth::render_song(&songs::song(music)) {
            Ok(r) => r,
            Err(e) => {
                error!("{e}");
                return None;
            }
        };
        debug!(
            "rendered {music:?}: {:.1}s of audio in {:.0}ms",
            rendered.duration_secs(),
            started.elapsed().as_secs_f32() * 1000.0
        );
        if matches!(music, Music::World(_)) {
            self.music.retain(|m, _| !matches!(m, Music::World(_)));
        }
        let h = assets.add(to_source(rendered));
        self.music.insert(music, h.clone());
        Some(h)
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

fn setup(mut commands: Commands, mut assets: ResMut<Assets<AudioSource>>) {
    let started = bevy::platform::time::Instant::now();
    let sfx = Sfx::ALL.into_iter().map(|s| (s, assets.add(to_source(sfx::render(s))))).collect();
    let gus = (0..sfx::GUS_VARIANTS).map(|v| assets.add(to_source(sfx::gus_blip(v)))).collect();
    let mut bank = AudioBank { sfx, gus, music: HashMap::default() };
    // The title song is needed straight away.
    bank.music(Music::Title, &mut assets);
    debug!("audio startup render: {:.0}ms", started.elapsed().as_secs_f32() * 1000.0);
    commands.insert_resource(bank);
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

/// What the music channel is playing.
#[derive(Resource, Default)]
pub struct MusicPlayer {
    current: Option<(Music, Handle<AudioInstance>)>,
    /// Duck state last applied to the current instance (`None`: not yet applied).
    ducked: Option<bool>,
}

impl MusicPlayer {
    pub fn now_playing(&self) -> Option<Music> {
        self.current.as_ref().map(|(m, _)| *m)
    }
}

#[allow(clippy::too_many_arguments)]
fn follow_state(
    state: Res<State<AppState>>,
    levels: Option<Res<Levels>>,
    current_level: Res<CurrentLevel>,
    mut player: ResMut<MusicPlayer>,
    mut bank: ResMut<AudioBank>,
    mut sources: ResMut<Assets<AudioSource>>,
    mut instances: ResMut<Assets<AudioInstance>>,
    channel: Res<AudioChannel<MusicChannel>>,
) {
    let want = desired_music(*state.get(), levels.as_deref(), *current_level);
    if player.current.as_ref().is_some_and(|(m, _)| *m == want) {
        return;
    }
    // The fanfare cuts in quickly; everything else crossfades.
    let (fade_out, fade_in) = match want {
        Music::LevelClear => (Duration::from_millis(120), Duration::from_millis(10)),
        _ => (FADE_OUT, FADE_IN),
    };
    if let Some((_, old)) = player.current.take()
        && let Some(mut instance) = instances.get_mut(&old)
    {
        instance.stop(AudioTween::new(fade_out, AudioEasing::OutPowi(2)));
    }
    player.ducked = None;
    let Some(source) = bank.music(want, &mut sources) else {
        // Unplayable song: remember it anyway so we don't retry every frame.
        player.current = Some((want, Handle::default()));
        return;
    };
    let instance = channel.play(source).with_volume(MUSIC_DB).fade_in(AudioTween::linear(fade_in)).handle();
    player.current = Some((want, instance));
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
    let Some((_, handle)) = &player.current else { return };
    // The instance appears once bevy_kira_audio has processed the play command; retry until then.
    let Some(mut instance) = instances.get_mut(handle) else { return };
    let db = if duck { MUSIC_DB + PAUSE_DUCK_DB } else { MUSIC_DB };
    instance.set_decibels(db, AudioTween::linear(Duration::from_millis(200)));
    player.ducked = Some(duck);
}

/// Pending Gus blips: seconds until each plays.
#[derive(Resource, Default)]
struct GusBabble(Vec<f32>);

#[allow(clippy::too_many_arguments)]
fn play_sfx(
    bank: Res<AudioBank>,
    channel: Res<AudioChannel<SfxChannel>>,
    mut babble: ResMut<GusBabble>,
    mut jumped: MessageReader<Jumped>,
    mut landed: MessageReader<Landed>,
    mut nuggets: MessageReader<NuggetCollected>,
    mut died: MessageReader<PlayerDied>,
    mut checkpoints: MessageReader<CheckpointReached>,
    mut completed: MessageReader<LevelCompleted>,
    mut gus: MessageReader<GusSays>,
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
    for line in gus.read() {
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
    mut pending: ResMut<GusBabble>,
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
        let mut v = rng.random_range(0..bank.gus.len());
        if Some(v) == last {
            v = (v + 1) % bank.gus.len();
        }
        last = Some(v);
        channel
            .play(bank.gus[v].clone())
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
    /// Gus talking: a little "blip blip" babble.
    GusBlip,
}

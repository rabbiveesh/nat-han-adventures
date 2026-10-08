//! The groove: the music that's playing bends the physics.
//!
//! The band director (`crate::audio::director`) reharmonizes the soundtrack from how the player
//! plays; the audio plugin writes [`Groove`] at the exact moment a new version starts sounding
//! (when a track starts, and at the bar line where a reharmonization switches in), so what you
//! hear and how Nat moves are always in sync. Level start and restart reset it to normal.
//! Every frame it also writes where the music is ([`Groove::clock`]: bar, beat, phase), which
//! the waltzing world dances to. Headless tests set both directly.
//!
//! | music | trigger | physics | gates |
//! |---|---|---|---|
//! | original | – | normal | – |
//! | Coltrane ("GIANT STEPS!") | 5 toots in 20s | gravity ×[`GIANT_STEPS_GRAVITY`], run ×[`GIANT_STEPS_SPEED`] | giant walls (6 tiles tall) |
//! | quartal ("FIRED UP") | 4 quick nuggets | run ×[`FIRED_UP_SPEED`] | long gaps (11 tiles) |
//! | waltz ("THE BAND WALTZES") | 3 evenly spaced ground jumps | the world dances in 3 (below); jump on ONE ×[`WALTZ_ONE_BOOST`] | waltz rows (spray cans) |
//! | melodic minor ("NERVOUS") | 3 deaths | game time ×[`NERVOUS_TIME`] (slow motion) | none: an assist |
//! | + laughing band (tuning medley) | 2 deaths at one checkpoint | landings bounce | none: comedy |
//!
//! Giant Steps slows the run so its longer air time doesn't also clear long gaps: each gate
//! opens in exactly one mode (`tests/levels.rs` checks it).
//!
//! # The waltz: the world dances in 3
//! The band plays 3/4 at a fixed tempo (`crate::audio::waltz::WALTZ_BPM`: 0.5 s a beat, 1.5 s
//! a bar), and the world follows the music's clock:
//! - **Spray cans** all fire on *the big ONE* — the downbeat of every other bar, where the
//!   4/4 downbeat lands and the drummer kicks hardest — for one beat, then hold off for the
//!   other five ([`Groove::waltz_spray_on`]): 0.5 s on, 2.5 s off, against the normal 1.0 s on,
//!   1.5 s off. That long breath is what gets you through a *waltz row* (a run of cans under a
//!   low ceiling, too long to cross in 1.5 s even fired up: see `tests/levels.rs`).
//! - **Moving platforms** glide on ONE and hold on 2–3 ([`Groove::waltz_glide_rate`]): their
//!   clock runs three times as fast (eased) during beat ONE and stops for the rest of the bar,
//!   so the path and the average speed stay the same.
//! - **Flies** circle once per bar.
//! - **Jump on ONE**: a ground jump within ±[`WALTZ_ONE_WINDOW`] s of any downbeat gets
//!   ×[`WALTZ_ONE_BOOST`] jump speed and a golden sparkle, and every
//!   [`WALTZ_ONE_LINE_EVERY`]rd one Han calls "ONE-two-three!". The step is a twirl: it spends
//!   the toot (no double jump out of it), so a ONE jump never reaches higher than a normal
//!   double jump and the giant walls stay Giant Steps' alone.
//!
//! Adding a mode: give [`Groove`] the new knob, set it in [`Groove::new`] from the music, and
//! read it in the physics; nothing else needs to know. [`Groove::tuning`] carries the laughing
//! band's tuning so physics can later follow the medley phrase by phrase.

use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::audio::{Filters, Harmony, tuning::Tuning};

/// Gravity multiplier under Giant Steps (jumps ~1.5x higher: a single jump ~4.8 tiles, a
/// quick 0.1s/0.1s double-tap ~6.2 tiles, a perfect double jump ~8.3 tiles).
pub const GIANT_STEPS_GRAVITY: f32 = 0.65;
/// Run speed multiplier under Giant Steps: keeps the horizontal reach of a jump about normal.
pub const GIANT_STEPS_SPEED: f32 = 0.65;
/// Run speed multiplier while the band is fired up (quartal).
pub const FIRED_UP_SPEED: f32 = 1.35;
/// Game speed while the band is nervous (melodic minor).
pub const NERVOUS_TIME: f32 = 0.8;
/// Upward speed of the laughing-band landing bounce, at most (~1 tile).
pub const BOUNCE_SPEED: f32 = 210.0;
/// Bounce speed per landing speed (each bounce is lower until it dies out).
pub const BOUNCE_RESTITUTION: f32 = 0.6;
/// Landings slower than this don't bounce.
pub const BOUNCE_MIN_SPEED: f32 = 150.0;
/// Waltz: jump speed multiplier of a ground jump on ONE (apex ~4.3 tiles instead of ~3.2).
pub const WALTZ_ONE_BOOST: f32 = 1.15;
/// Waltz: how close (seconds, either side) to a downbeat a jump counts as on ONE.
pub const WALTZ_ONE_WINDOW: f32 = 0.12;
/// Waltz: every this many ONE jumps, Han calls the step.
pub const WALTZ_ONE_LINE_EVERY: u32 = 3;
pub const WALTZ_ONE_LINE: &str = "ONE-two-three!";
/// Waltz: spray cans fire on the big ONE, the downbeat of every this many bars.
pub const WALTZ_SPRAY_BARS: u32 = 2;

/// Where the music is: bar (counted from the start of the version playing), beat in the bar,
/// phase in the beat. Written every frame by the audio plugin; headless tests set it with
/// [`BeatClock::at`].
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct BeatClock {
    pub bar: u32,
    /// 0-based: 0 is ONE.
    pub beat: u8,
    /// 0..1 through the beat.
    pub phase: f32,
    /// Length of a beat (s).
    pub beat_secs: f32,
    pub beats_per_bar: u8,
}

impl Default for BeatClock {
    fn default() -> Self {
        BeatClock { bar: 0, beat: 0, phase: 0.0, beat_secs: 0.5, beats_per_bar: 4 }
    }
}

impl BeatClock {
    /// The clock `beats` beats into a song with beats of `beat_secs` seconds, `beats_per_bar`
    /// to the bar.
    pub fn at(beats: f64, beat_secs: f64, beats_per_bar: u32) -> Self {
        let beats = beats.max(0.0);
        let bpb = beats_per_bar.max(1) as f64;
        let whole = beats.floor();
        BeatClock {
            bar: (whole / bpb).floor() as u32,
            beat: (whole % bpb) as u8,
            phase: (beats - whole) as f32,
            beat_secs: beat_secs as f32,
            beats_per_bar: bpb as u8,
        }
    }

    pub fn bar_secs(&self) -> f32 {
        self.beat_secs * self.beats_per_bar as f32
    }

    /// Seconds to the nearest downbeat (the one just gone or the next one).
    pub fn secs_from_downbeat(&self) -> f32 {
        let into = (self.beat as f32 + self.phase) * self.beat_secs;
        into.min(self.bar_secs() - into)
    }
}

/// How the music currently playing bends the physics.
#[derive(Resource, Debug, Clone, Copy, Reflect)]
#[reflect(Resource)]
pub struct Groove {
    /// The harmony the band is playing (for the UI and effects).
    pub harmony: Harmony,
    /// Gravity multiplier; max fall speed scales by its square root (floaty, not just slow).
    pub gravity_scale: f32,
    /// Run speed multiplier (ground and air acceleration scale with it).
    pub speed_scale: f32,
    /// Game time multiplier (slow motion below 1). The music keeps real time.
    pub time_scale: f32,
    /// Landings spring Nat back up a little.
    pub bounce: bool,
    /// The tuning the band plays in ([`Tuning::Medley`] for the laughing band). Physics don't
    /// read it yet.
    #[reflect(ignore)]
    pub tuning: Tuning,
    /// Where the music is. Not part of equality: it says when, not how, the physics bend.
    pub clock: BeatClock,
}

impl PartialEq for Groove {
    fn eq(&self, o: &Self) -> bool {
        (self.harmony, self.gravity_scale, self.speed_scale, self.time_scale, self.bounce, self.tuning)
            == (o.harmony, o.gravity_scale, o.speed_scale, o.time_scale, o.bounce, o.tuning)
    }
}

impl Default for Groove {
    fn default() -> Self {
        Self::new(Filters::default())
    }
}

impl Groove {
    /// The physics that go with the music `filters`.
    pub fn new(filters: Filters) -> Self {
        let plain = Groove {
            harmony: filters.harmony,
            gravity_scale: 1.0,
            speed_scale: 1.0,
            time_scale: 1.0,
            bounce: filters.just_intonation,
            tuning: if filters.just_intonation { Tuning::Medley } else { Tuning::Equal },
            clock: BeatClock::default(),
        };
        match filters.harmony {
            Harmony::Original | Harmony::Waltz => plain,
            Harmony::Coltrane => {
                Groove { gravity_scale: GIANT_STEPS_GRAVITY, speed_scale: GIANT_STEPS_SPEED, ..plain }
            }
            Harmony::Quartal => Groove { speed_scale: FIRED_UP_SPEED, ..plain },
            Harmony::MelodicMinor => Groove { time_scale: NERVOUS_TIME, ..plain },
        }
    }

    /// The physics of a harmony, without the laughing band.
    pub fn of(harmony: Harmony) -> Self {
        Self::new(Filters { harmony, just_intonation: false })
    }

    /// The same groove with the music at `clock`.
    pub fn at(self, clock: BeatClock) -> Self {
        Groove { clock, ..self }
    }

    pub fn giant_steps(&self) -> bool {
        self.harmony == Harmony::Coltrane
    }

    pub fn waltz(&self) -> bool {
        self.harmony == Harmony::Waltz
    }

    /// Max fall speed multiplier.
    pub fn fall_scale(&self) -> f32 {
        self.gravity_scale.sqrt()
    }

    /// Waltz: are the spray cans firing (beat ONE of the big ONE's bar)?
    pub fn waltz_spray_on(&self) -> bool {
        self.clock.beat == 0 && self.clock.bar.is_multiple_of(WALTZ_SPRAY_BARS)
    }

    /// Waltz: how fast the moving platforms' clock runs (×real time): an eased glide on ONE
    /// that covers the whole bar's worth of path (3 × smoothstep′), standing still on 2–3.
    pub fn waltz_glide_rate(&self) -> f32 {
        if self.clock.beat != 0 {
            return 0.0;
        }
        let p = self.clock.phase.clamp(0.0, 1.0);
        self.clock.beats_per_bar as f32 * 6.0 * p * (1.0 - p)
    }

    /// Waltz: does a ground jump right now land on ONE?
    pub fn on_the_one(&self) -> bool {
        self.waltz() && self.clock.secs_from_downbeat() <= WALTZ_ONE_WINDOW
    }

    /// How fast the moving platforms' clock runs (×game time).
    pub fn platform_rate(&self) -> f32 {
        if self.waltz() { self.waltz_glide_rate() } else { 1.0 }
    }

    /// Fly orbits per second (`normal_period`: seconds per orbit outside the waltz).
    pub fn fly_rate(&self, normal_period: f32) -> f32 {
        if self.waltz() { 1.0 / self.clock.bar_secs().max(0.1) } else { 1.0 / normal_period }
    }
}

/// Fly orbit angle (radians) after `turns` orbits, for a fly with phase offset `phase` (turns).
pub fn fly_angle(turns: f32, phase: f32) -> f32 {
    TAU * (turns + phase)
}

/// A ground jump on ONE: the golden sparkle (and Han's call) follow it.
#[derive(Message, Debug, Clone, Copy)]
pub struct JumpedOnOne {
    pub pos: Vec2,
}

/// ONE jumps this level (Han calls every [`WALTZ_ONE_LINE_EVERY`]rd).
#[derive(Resource, Debug, Default)]
pub struct OneJumps(pub u32);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Groove>()
        .init_resource::<OneJumps>()
        .add_message::<JumpedOnOne>()
        .add_systems(Update, apply_time_scale)
        .add_systems(FixedUpdate, call_the_step.in_set(super::GameSet::Interact));
}

/// Slow motion: the game's virtual clock follows [`Groove::time_scale`].
fn apply_time_scale(groove: Res<Groove>, mut time: ResMut<Time<Virtual>>) {
    if time.relative_speed() != groove.time_scale {
        time.set_relative_speed(groove.time_scale);
    }
}

/// Han counts the waltz: "ONE-two-three!" on every [`WALTZ_ONE_LINE_EVERY`]rd ONE jump.
fn call_the_step(
    mut ones: MessageReader<JumpedOnOne>,
    mut count: ResMut<OneJumps>,
    mut says: MessageWriter<crate::events::HanSays>,
) {
    for _ in ones.read() {
        count.0 += 1;
        if count.0.is_multiple_of(WALTZ_ONE_LINE_EVERY) {
            says.write(crate::events::HanSays { text: WALTZ_ONE_LINE.to_string() });
        }
    }
}

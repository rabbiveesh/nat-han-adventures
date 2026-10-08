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
//! | melodic minor ("NERVOUS") | 3 deaths | game time ×[`NERVOUS_TIME`] (slow motion); sweaty grip: brakes and jumps on grease ([`Groove::grip`]) | grease chutes |
//! | + laughing band (tuning medley) | 2 deaths at one checkpoint | landings bounce; each phrase's tuning nudges (below) | none: comedy |
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
//!   [`WALTZ_ONE_LINE_EVERY`]rd one Han calls "ONE-two-three!". The step is a twirl: its toot
//!   is a *weak* one ([`WALTZ_ONE_TOOT_SPEED`] instead of `DOUBLE_JUMP_SPEED`), so a ONE jump
//!   plus toot tops out at ~87 px (ONE apex 437²/2800 ≈ 68 px + 230²/2800 ≈ 19 px, with the
//!   toot at the very apex), safely under a 6-tile (96 px) giant wall: those stay Giant Steps'.
//!
//! # The laughing band: a nudge per phrase
//! The laughing band plays a medley of tunings, a different one every 4-bar phrase
//! (`crate::audio::tuning::Medley`), always a bit drunk on top. The audio plugin writes the
//! phrase sounding into [`Groove::phrase`] (the same pick as `NowPlaying::tuning_now`) and the
//! drunk pitch wobble into [`Groove::sway`] every frame, and each phrase nudges the physics a
//! little ([`Groove::nudge`], named on the HUD's groove badge):
//!
//! | phrase | badge | nudge |
//! |---|---|---|
//! | just intonation | SOBER FOR A SEC | no bounce, no sway, the camera stops giggling |
//! | harmonic series | OVERTONES! | the toot is up to ×11/8 ([`Groove::toot_speed`]) |
//! | 7-TET | SEASICK | slippery landings ([`SEASICK_DECEL`]), the camera rolls |
//! | Carlos alpha | MELTING | Nat shrinks to [`MELTING_SIZE`], jumps ×[`MELTING_JUMP`] |
//! | Bohlen–Pierce | ALIEN | gravity pulses in threes ([`Groove::gravity_now`]) |
//! | (all but just) | | run speed staggers down to ×(1 − [`DRUNK_SWAY`]) with the pitch wobble |
//!
//! Small, and never unfair: none of them opens a band gate or closes a way through.
//! - The harmonic toot only ever reaches the apex a perfectly timed normal toot would from
//!   the same moment, sooner, so it lands sooner: no higher, no farther (this module's tests).
//! - The drunk sway only slows Nat (any faster and a fired-up dash gets through a waltz row),
//!   and its slowest is still above the validator's human margin (90% of top speed).
//! - The melting jump and the alien gravity only take reach away (anything that adds some
//!   opens a gate somewhere: the margins are a few px), and every level stays beatable with
//!   each of them in force all along ([`crate::level::validate::Physics::laughing`],
//!   `tests/levels.rs`). They're a phrase long anyway.
//! - Grip wins over the slippery landings, as it does over the bounce.
//!
//! Adding a mode: give [`Groove`] the new knob, set it in [`Groove::new`] from the music, and
//! read it in the physics; nothing else needs to know.

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
/// Waltz: upward speed of the weak toot after a jump on ONE (adds ≤ ~19 px). The ONE jump's
/// apex (×[`WALTZ_ONE_BOOST`]: 437 px/s → ~68 px) plus this stays ≤ ~87 px even with ideal
/// timing: ≥ 9 px under a 6-tile giant wall (96 px).
pub const WALTZ_ONE_TOOT_SPEED: f32 = 230.0;
/// Waltz: how close (seconds, either side) to a downbeat a jump counts as on ONE.
pub const WALTZ_ONE_WINDOW: f32 = 0.12;
/// Waltz: every this many ONE jumps, Han calls the step.
pub const WALTZ_ONE_LINE_EVERY: u32 = 3;
pub const WALTZ_ONE_LINE: &str = "ONE-two-three!";
/// Waltz: spray cans fire on the big ONE, the downbeat of every this many bars.
pub const WALTZ_SPRAY_BARS: u32 = 2;
/// Laughing band, harmonic-series phrase ("OVERTONES!"): the toot's most, ×its own speed.
pub const OVERTONE_TOOT: f32 = 11.0 / 8.0;
/// Laughing band, 7-TET phrase ("SEASICK"): ground braking ×this just after a landing...
pub const SEASICK_DECEL: f32 = 0.5;
/// ...for this long (s).
pub const SEASICK_SECS: f32 = 0.25;
/// 7-TET: the camera rolls this much (radians, either way) at [`SEASICK_ROLL_HZ`].
pub const SEASICK_ROLL: f32 = 0.026;
pub const SEASICK_ROLL_HZ: f32 = 0.35;
/// Laughing band, Carlos alpha phrase ("MELTING"): Nat's size (drawn)...
pub const MELTING_SIZE: f32 = 0.85;
/// ...and ground jump speed ×this (apex ×0.9).
pub const MELTING_JUMP: f32 = 0.95;
/// Laughing band, Bohlen–Pierce phrase ("ALIEN"): gravity is up to ×(1 + this) on every
/// third beat of the phrase, easing off through the beat. (Much more, and a validated jump
/// falls short: ×1.08 closes three levels.)
pub const ALIEN_PULSE: f32 = 0.05;
/// Beats per alien gravity pulse (the tritave's 3).
pub const ALIEN_EVERY: u32 = 3;
/// Laughing band (all but the sober phrase): run speed staggers down to ×(1 − this) and back
/// with the pitch wobble (never faster than sober).
pub const DRUNK_SWAY: f32 = 0.08;

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
    /// The tuning the band plays in ([`Tuning::Medley`] for the laughing band).
    #[reflect(ignore)]
    pub tuning: Tuning,
    /// The laughing band's phrase: the medley's tuning sounding now ([`Tuning::Just`], ...),
    /// `None` when the band isn't laughing. Written every frame like [`Groove::clock`] (and,
    /// like it, not part of equality). See [`Groove::nudge`].
    #[reflect(ignore)]
    pub phrase: Option<Tuning>,
    /// The medley's drunk pitch wobble now, -1..1 (sharpest at 1). Written every frame.
    pub sway: f32,
    /// Where the music is. Not part of equality: it says when, not how, the physics bend.
    pub clock: BeatClock,
    /// Sweaty grip from the death count (3+ deaths this level), whatever the band plays: a
    /// layer like the laughing band, so a held summon (or ghost nuggets refiring one) can never
    /// keep grip away in a grease chute. Set by the audio plugin from the director. Grip wins
    /// over the laughing band's bounce.
    pub nervous: bool,
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
            phrase: None,
            sway: 0.0,
            clock: BeatClock::default(),
            nervous: false,
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

    /// Sweaty grip: the nervous band (melodic minor, the 3+ deaths mood) lets Nat brake and
    /// jump on grease.
    pub fn grip(&self) -> bool {
        self.nervous || self.harmony == Harmony::MelodicMinor
    }

    /// Max fall speed multiplier.
    pub fn fall_scale(&self) -> f32 {
        self.gravity_scale.sqrt()
    }

    /// The same groove in the laughing band's `phrase` (tests; the audio plugin writes it).
    pub fn in_phrase(self, phrase: Tuning) -> Self {
        Groove { phrase: Some(phrase), ..self }
    }

    /// The laughing band's phrase nudging the physics now, if any (see the module docs).
    pub fn nudge(&self) -> Option<Nudge> {
        if !self.bounce {
            return None;
        }
        Some(match self.phrase? {
            Tuning::Just => Nudge::Sober,
            Tuning::Harmonic => Nudge::Overtones,
            Tuning::Tet7 => Nudge::Seasick,
            Tuning::CarlosAlpha => Nudge::Melting,
            Tuning::BohlenPierce => Nudge::Alien,
            Tuning::Equal | Tuning::Drunk | Tuning::Medley => return None,
        })
    }

    /// Landings bounce: the laughing band, unless it's sobered up for a phrase.
    pub fn bouncy(&self) -> bool {
        self.bounce && self.nudge() != Some(Nudge::Sober)
    }

    /// Drunk: the run speed sways with the pitch wobble (the laughing band, not sober).
    pub fn drunk(&self) -> bool {
        self.bouncy()
    }

    /// Nat's run speed multiplier now: [`Groove::speed_scale`] and the drunk sway.
    pub fn run_scale(&self) -> f32 {
        let sway = if self.drunk() { 1.0 - DRUNK_SWAY * (1.0 - self.sway.clamp(-1.0, 1.0)) / 2.0 } else { 1.0 };
        self.speed_scale * sway
    }

    /// Nat's gravity multiplier now: [`Groove::gravity_scale`] and the alien pulse (on every
    /// [`ALIEN_EVERY`]rd beat of the phrase, heaviest on the beat, easing off through it).
    pub fn gravity_now(&self) -> f32 {
        if self.nudge() != Some(Nudge::Alien) {
            return self.gravity_scale;
        }
        let beat = self.clock.bar * self.clock.beats_per_bar as u32 + self.clock.beat as u32;
        let phrase_beats = crate::audio::tuning::MEDLEY_PHRASE_BEATS as u32;
        let pulse = if (beat % phrase_beats).is_multiple_of(ALIEN_EVERY) {
            ALIEN_PULSE * (1.0 - self.clock.phase.clamp(0.0, 1.0))
        } else {
            0.0
        };
        self.gravity_scale * (1.0 + pulse)
    }

    /// Nat's ground jump speed multiplier (melting: lower).
    pub fn jump_scale(&self) -> f32 {
        if self.nudge() == Some(Nudge::Melting) { MELTING_JUMP } else { 1.0 }
    }

    /// Nat's drawn size (melting: smaller).
    pub fn nat_size(&self) -> f32 {
        if self.nudge() == Some(Nudge::Melting) { MELTING_SIZE } else { 1.0 }
    }

    /// Ground braking multiplier `since_landing` seconds after Nat landed (seasick: slippery
    /// for a moment; grip wins).
    pub fn landing_decel(&self, since_landing: f32) -> f32 {
        if self.nudge() == Some(Nudge::Seasick) && !self.grip() && since_landing < SEASICK_SECS {
            SEASICK_DECEL
        } else {
            1.0
        }
    }

    /// Upward speed of a toot of speed `base` (the toot or the waltz's weak one) by Nat rising
    /// at `vy` (px/s, up; the physics pass the rise he'd have had without the jump cut that
    /// letting go to toot makes: holding on, he'd also be higher, so the bound below holds). Normally `base`; in the overtones phrase up to ×[`OVERTONE_TOOT`],
    /// but never past the apex a toot timed at the top of the rise would reach
    /// (`vy² + base²` of kinetic energy, give or take a step): it gets there sooner, so it
    /// also lands sooner. A
    /// falling Nat (or one whose rise is spent) gets `base`.
    pub fn toot_speed(&self, base: f32, vy: f32) -> f32 {
        if self.nudge() != Some(Nudge::Overtones) {
            return base;
        }
        // (Less a step's worth of gravity: a real toot lands on a 60 Hz step, a step short of
        // the exact top at best.)
        let step = crate::game::tuning::GRAVITY * self.gravity_now() / 60.0;
        let rise = (vy - step).max(0.0);
        (rise * rise + base * base).sqrt().min(base * OVERTONE_TOOT).max(base)
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

/// What the laughing band's phrase does to the physics ([`Groove::nudge`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nudge {
    /// Just intonation: no bounce, no sway.
    Sober,
    /// The harmonic series: a stronger toot.
    Overtones,
    /// 7-TET: slippery landings, the camera rolls.
    Seasick,
    /// Carlos alpha: Nat shrinks, lower jumps.
    Melting,
    /// Bohlen–Pierce: gravity pulses in threes.
    Alien,
}

impl Nudge {
    /// The HUD's groove badge.
    pub fn label(self) -> &'static str {
        match self {
            Nudge::Sober => "SOBER FOR A SEC",
            Nudge::Overtones => "OVERTONES!",
            Nudge::Seasick => "SEASICK",
            Nudge::Melting => "MELTING",
            Nudge::Alien => "ALIEN",
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tuning::{DOUBLE_JUMP_SPEED, GRAVITY, JUMP_SPEED};

    fn laughing(phrase: Tuning) -> Groove {
        Groove::new(Filters { harmony: Harmony::Original, just_intonation: true }).in_phrase(phrase)
    }

    #[test]
    fn each_phrase_has_its_nudge() {
        assert_eq!(Groove::default().in_phrase(Tuning::Tet7).nudge(), None, "not laughing: no nudge");
        assert_eq!(Groove::new(Filters { harmony: Harmony::Original, just_intonation: true }).nudge(), None);
        let nudges: Vec<_> =
            crate::audio::tuning::MEDLEY_TUNINGS.iter().map(|&t| laughing(t).nudge().map(Nudge::label)).collect();
        assert_eq!(
            nudges,
            [Some("SOBER FOR A SEC"), Some("OVERTONES!"), Some("SEASICK"), Some("MELTING"), Some("ALIEN")]
        );
        // Sober: no bounce, no sway. The rest bounce and sway.
        let sober = Groove { sway: 1.0, ..laughing(Tuning::Just) };
        assert!(!sober.bouncy() && sober.run_scale() == 1.0);
        let drunk = Groove { sway: 1.0, ..laughing(Tuning::Tet7) };
        assert!(drunk.bouncy());
        assert_eq!(drunk.run_scale(), 1.0, "never faster than sober");
        assert!((Groove { sway: -1.0, ..drunk }.run_scale() - (1.0 - DRUNK_SWAY)).abs() < 1e-6);
        // Melting: smaller, lower jumps.
        assert_eq!((laughing(Tuning::CarlosAlpha).nat_size(), laughing(Tuning::CarlosAlpha).jump_scale()), (MELTING_SIZE, MELTING_JUMP));
        // Seasick: slippery for a moment after landing; grip wins.
        let sea = laughing(Tuning::Tet7);
        assert_eq!((sea.landing_decel(0.1), sea.landing_decel(SEASICK_SECS)), (SEASICK_DECEL, 1.0));
        assert_eq!(Groove { nervous: true, ..sea }.landing_decel(0.1), 1.0);
        assert_eq!(laughing(Tuning::Harmonic).landing_decel(0.1), 1.0);
    }

    #[test]
    fn alien_gravity_pulses_in_threes() {
        let g = laughing(Tuning::BohlenPierce);
        let at = |beat: f64| g.at(BeatClock::at(beat, 0.5, 4)).gravity_now();
        let pulses: Vec<bool> = (0..20).map(|b| at(b as f64) > 1.0).collect();
        let want: Vec<bool> = (0..20).map(|b: u32| (b % 16).is_multiple_of(ALIEN_EVERY)).collect();
        assert_eq!(pulses, want, "every third beat of the phrase (a new phrase starts the count)");
        assert!((at(3.0) - (1.0 + ALIEN_PULSE)).abs() < 1e-6, "heaviest on the beat");
        assert!(at(3.5) < at(3.0) && at(3.5) > 1.0, "easing off through it");
        assert_eq!(laughing(Tuning::Harmonic).at(BeatClock::at(3.0, 0.5, 4)).gravity_now(), 1.0);
        let gs = Groove::new(Filters { harmony: Harmony::Coltrane, just_intonation: true }).in_phrase(Tuning::BohlenPierce);
        assert!((gs.gravity_now() - GIANT_STEPS_GRAVITY * (1.0 + ALIEN_PULSE)).abs() < 1e-6, "on top of the mode's");
    }

    /// One ground jump (jump held) with a toot at frame `toot` (Nat's own step order: the toot
    /// sets the speed, then gravity, then the move): the height every frame.
    fn arc(g: &Groove, gravity: f32, toot: usize) -> Vec<f32> {
        let dt = 1.0 / 60.0;
        let (mut y, mut vy) = (0.0f32, JUMP_SPEED);
        let mut out = Vec::new();
        for f in 0..240 {
            if f == toot {
                vy = g.toot_speed(DOUBLE_JUMP_SPEED, vy);
            }
            vy -= gravity * dt;
            y += vy * dt;
            out.push(y);
            if y < -400.0 {
                break;
            }
        }
        out
    }

    /// The overtones' toot: stronger, but it never reaches anything a normal toot can't. For
    /// every toot time and every height, it's no higher than the best normal toot's apex and
    /// it's never still at a height later than some normal toot is: no higher, no farther
    /// (horizontal reach is the time spent at or above a height, running).
    #[test]
    fn overtones_toot_reaches_nothing_new() {
        let over = laughing(Tuning::Harmonic);
        for gravity in [GRAVITY, GRAVITY * GIANT_STEPS_GRAVITY] {
            let normal: Vec<Vec<f32>> = (0..90).map(|t| arc(&Groove::default(), gravity, t)).collect();
            let harmonic: Vec<Vec<f32>> = (0..90).map(|t| arc(&over, gravity, t)).collect();
            let apex = |arcs: &[Vec<f32>]| arcs.iter().flatten().fold(f32::MIN, |a, &b| a.max(b));
            assert!(apex(&harmonic) <= apex(&normal) + 0.5, "{} > {}", apex(&harmonic), apex(&normal));
            // Latest frame at or above each height.
            let last = |arcs: &[Vec<f32>], h: f32| arcs.iter().filter_map(|a| a.iter().rposition(|&y| y >= h)).max();
            let mut h = -64.0;
            while h < apex(&normal) {
                assert!(last(&harmonic, h) <= last(&normal, h), "at {h}px: {:?} > {:?}", last(&harmonic, h), last(&normal, h));
                h += 1.0;
            }
            // It is stronger: an early toot climbs well above an early normal toot.
            let early = |arcs: &[Vec<f32>]| arcs[2].iter().fold(f32::MIN, |a, &b| a.max(b));
            assert!(early(&harmonic) > early(&normal) + 16.0, "{} vs {}", early(&harmonic), early(&normal));
        }
        // Falling: just a toot. Never weaker.
        assert_eq!(over.toot_speed(DOUBLE_JUMP_SPEED, -200.0), DOUBLE_JUMP_SPEED);
        assert_eq!(over.toot_speed(DOUBLE_JUMP_SPEED, 1000.0), DOUBLE_JUMP_SPEED * OVERTONE_TOOT);
    }
}

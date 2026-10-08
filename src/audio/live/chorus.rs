//! Arranging: what each chorus (loop pass) is, how a tune starts and how it ends. Music only:
//! no physics, no director.
//!
//! # Choruses
//! Every loop pass is one [`Chorus`], decided for the whole band at once in its shared plan
//! ([`super::band::BandPlan::chorus`]), as a pure function of the seed, the pass and the dials
//! ([`auto`]), so all four play the same chorus and the plans can show it ahead of time:
//! - the dial is the band's mean freedom ([`dial`]); below [`MIN_DIAL`] every chorus is the
//!   head as written (so freedom 0 is untouched), and arranged choruses fade in above it;
//! - the first pass is the head (a two-feel head once the band loosens up: the bass in two,
//!   the walking comes later); then the passes run in arcs of four ([`ARC`]): two middle
//!   choruses, a climax (a shout chorus, with a key-up on a loose night) and the head again;
//! - a middle chorus is the head, or one the dial allows ([`menu`]): two-feel, stop-time,
//!   breaks, riff backgrounds (from the start of the tier), blowing, strolling, soli (looser);
//!   it never repeats the chorus the pass before it rolled;
//! - [`super::engine::Input::ForceChorus`] (the editor's chips) overrides it: every pass
//!   plays that chorus from the next bar committed, at any freedom.
//!
//! What each player does in each chorus (see [`Chorus`]):
//! | Chorus | Lead | Comp | Bass | Drums |
//! |---|---|---|---|---|
//! | Head | the tune | as it rolls | as it rolls | as it rolls |
//! | Two-feel | the tune | Charleston, softer | in two | lighter |
//! | Blowing | solos | comps | walks | time, trades |
//! | Stop-time | the tune (solos when loose) | hits on ONE | hits on ONE | kick on ONE, hats on 2 and 4 |
//! | Breaks | the tune, a solo break every section | time, then out | time, then out | time, then out |
//! | Riffs | solos (the tune when calm) | a riff behind | walks | time |
//! | Strolling | solos | lays out | walks | lighter |
//! | Soli | the tune exactly | the tune a third below | walks | time |
//! | Shout | the tune up an octave, punchy | stabs with the lead | walks | crashes, fills, louder |
//!
//! Stop-time and breaks work section by section (8 bars): stop-time hits on the ONE of every
//! bar but the last two, which swing back into the next section; a break stops the band on
//! the ONE of the section's second-last bar and leaves the lead alone for two bars. Feels
//! ([`super::feel`]) only happen in the head and the solo choruses (blowing, riffs, strolling:
//! a bossa stop-time or soli is another tune); trading fours only in blowing choruses and
//! heads.
//!
//! # Intros
//! With [`super::engine::EngineConfig::intro`] (the game's levels), a tune that starts from the
//! top with the band loose enough ([`INTRO_DIAL`]) gets a four-bar intro before the head
//! ([`Intro`]): a vamp on I–VI–ii–V, or a ii–V over a dominant pedal. The engine plays it as
//! the song's last four bars (a partial pass: the head is still pass 0) with the harmony
//! replaced ([`harmony`]); the lead lays out, then picks up into the head.
//!
//! # Endings
//! [`super::engine::Input::End`] ends a song from the next bar committed with an [`Ending`];
//! a one-shot (the level-clear jingle) that the band is loose enough to play with gets a Basie
//! ending after its last bar. Then the engine is finished.
//! - **Basie**: a ii–V, the piano's three quiet plinks with the band out, the band's last chord
//!   (a one-shot's: just the plinks and the chord);
//! - **Tag**: the I–VI–ii–V turnaround three times, then the last chord;
//! - **Vamp-out**: a ii–V vamped four times, quieter each time, then a soft last chord;
//! - **Plain** (freedom 0): just the last chord.

use crate::audio::chart::{Chord, Quality};

use super::band::{self, ramp};

/// What a loop pass is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Chorus {
    /// The tune as written (as ornamented as the dials say).
    #[default]
    Head,
    /// The head in two: the bass on 1 and 3, a softer comp and drums.
    TwoFeel,
    /// A solo chorus: the lead improvises, the bass walks.
    Blowing,
    /// The band hits on ONE and lays out; the lead plays on.
    StopTime,
    /// Two-bar solo breaks at the end of every section.
    Breaks,
    /// The comp repeats a riff behind the lead.
    Riffs,
    /// The comp lays out: lead, bass and drums.
    Strolling,
    /// Lead and comp play the tune together, in harmony.
    Soli,
    /// The climax: the whole band riffs, loud.
    Shout,
}

impl Chorus {
    pub const ALL: [Chorus; 9] =
        [Chorus::Head, Chorus::TwoFeel, Chorus::Blowing, Chorus::StopTime, Chorus::Breaks, Chorus::Riffs, Chorus::Strolling, Chorus::Soli, Chorus::Shout];

    /// For the HUD's band readout ("" for the head).
    pub fn label(self) -> &'static str {
        match self {
            Chorus::Head => "",
            Chorus::TwoFeel => "TWO-FEEL",
            Chorus::Blowing => "BLOWING",
            Chorus::StopTime => "STOP-TIME",
            Chorus::Breaks => "BREAKS",
            Chorus::Riffs => "RIFFS",
            Chorus::Strolling => "STROLLING",
            Chorus::Soli => "SOLI",
            Chorus::Shout => "SHOUT CHORUS",
        }
    }

    /// Its name on the command line and in the editor.
    pub fn slug(self) -> &'static str {
        match self {
            Chorus::Head => "head",
            Chorus::TwoFeel => "two-feel",
            Chorus::Blowing => "blowing",
            Chorus::StopTime => "stop-time",
            Chorus::Breaks => "breaks",
            Chorus::Riffs => "riffs",
            Chorus::Strolling => "strolling",
            Chorus::Soli => "soli",
            Chorus::Shout => "shout",
        }
    }

    pub fn parse(s: &str) -> Option<Chorus> {
        Chorus::ALL.into_iter().find(|c| c.slug() == s)
    }

    /// May the band pick a feel in it (the solo choruses: the feel's comping takes the riff's
    /// place)?
    pub fn allows_feel(self) -> bool {
        matches!(self, Chorus::Head | Chorus::Blowing | Chorus::Riffs | Chorus::Strolling)
    }

    /// May the band trade fours in it (a head only after the first pass)?
    pub fn allows_trades(self) -> bool {
        matches!(self, Chorus::Head | Chorus::Blowing)
    }

    /// Does the lead solo through it, at lead freedom `lead`?
    pub fn lead_solos(self, lead: f32) -> bool {
        match self {
            Chorus::Blowing | Chorus::Strolling => true,
            Chorus::Riffs => lead >= 0.45,
            Chorus::StopTime => lead >= 0.55,
            _ => false,
        }
    }

    /// Does the bass walk through it (unless a bar says otherwise)?
    pub fn walks(self) -> bool {
        matches!(self, Chorus::Blowing | Chorus::Riffs | Chorus::Strolling | Chorus::Soli | Chorus::Shout | Chorus::Breaks)
    }
}

/// No arranged choruses below this dial.
pub const MIN_DIAL: f32 = 0.25;
/// Passes per arc after the first: two middles, a climax, the head.
pub const ARC: u64 = 4;
/// An intro needs at least this dial.
pub const INTRO_DIAL: f32 = 0.1;
/// Endings other than the plain chord need at least this dial.
pub const END_DIAL: f32 = 0.05;

/// The arranger's dial: the band's mean freedom.
pub fn dial(freedom: [f32; 4]) -> f32 {
    freedom.iter().sum::<f32>() / 4.0
}

/// The middle choruses the dial allows, with their weights.
pub fn menu(dial: f32) -> [(Chorus, f64); 7] {
    let lo = ramp(dial, MIN_DIAL, 0.35);
    [
        (Chorus::TwoFeel, 1.0 * lo),
        (Chorus::StopTime, 2.0 * lo),
        (Chorus::Breaks, 2.0 * lo),
        (Chorus::Riffs, 1.5 * lo),
        (Chorus::Blowing, 2.0 * ramp(dial, 0.45, 0.6)),
        (Chorus::Soli, 1.5 * ramp(dial, 0.45, 0.65)),
        (Chorus::Strolling, 1.0 * ramp(dial, 0.5, 0.7)),
    ]
}

/// The chance a middle chorus is arranged (not the head).
pub fn chance(dial: f32) -> f64 {
    0.95 * ramp(dial, MIN_DIAL, 0.55)
}

/// A middle chorus's raw roll for pass `pass` (before the no-repeat rule).
fn roll(seed: u64, pass: u64, dial: f32) -> Chorus {
    let mut r = band::rng(seed, 8, pass, 0);
    let coin = r.f();
    if coin >= chance(dial) {
        return Chorus::Head;
    }
    let m = menu(dial);
    let total: f64 = m.iter().map(|x| x.1).sum();
    if total <= 0.0 {
        return Chorus::Head;
    }
    let mut x = r.f() * total;
    for (c, w) in m {
        if x < w {
            return c;
        }
        x -= w;
    }
    m.iter().rev().find(|x| x.1 > 0.0).map_or(Chorus::Head, |x| x.0)
}

/// A middle chorus: [`roll`], never the same arranged chorus as the pass before (looking back
/// at most an arc: the pass before a middle is a middle, the head or the climax).
fn middle(seed: u64, pass: u64, dial: f32) -> Chorus {
    let c = roll(seed, pass, dial);
    if c == Chorus::Head || pass == 0 || auto(seed, pass - 1, dial).chorus != c {
        return c;
    }
    // The next one on the menu the dial allows.
    let m = menu(dial);
    let i = m.iter().position(|x| x.0 == c).unwrap_or(0);
    (1..m.len()).map(|j| m[(i + j) % m.len()]).find(|x| x.1 > 0.0).map_or(Chorus::Head, |x| x.0)
}

/// A pass's chorus, and whether it's played a half step up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Call {
    pub chorus: Chorus,
    pub key_up: bool,
}

/// The chorus of loop pass `pass` (the head's first pass is 0) at `dial`: see the module docs.
pub fn auto(seed: u64, pass: u64, dial: f32) -> Call {
    let head = Call::default();
    if dial < MIN_DIAL {
        return head;
    }
    if pass == 0 {
        let two = band::rng(seed, 8, 0, 1).chance(0.6 * ramp(dial, 0.3, 0.5));
        return Call { chorus: if two { Chorus::TwoFeel } else { Chorus::Head }, key_up: false };
    }
    match (pass - 1) % ARC {
        0 | 1 => Call { chorus: middle(seed, pass, dial), key_up: false },
        2 => {
            let mut r = band::rng(seed, 8, pass, 2);
            if r.chance(ramp(dial, 0.5, 0.7)) {
                Call { chorus: Chorus::Shout, key_up: r.chance(0.6 * ramp(dial, 0.6, 0.85)) }
            } else {
                Call { chorus: middle(seed, pass, dial), key_up: false }
            }
        }
        _ => head,
    }
}

/// Where a bar sits in its 8-bar section: its index, and the section's length (the song's
/// odd tail is shorter).
pub fn in_section(song_bar: usize, bars: usize) -> (usize, usize) {
    let start = song_bar - song_bar % 8;
    (song_bar - start, (bars - start).min(8))
}

/// A stop-time bar: the band hits on ONE (every bar of a section but the last two).
pub fn stop_bar(song_bar: usize, bars: usize) -> bool {
    let (k, n) = in_section(song_bar, bars);
    n >= 4 && k + 2 < n
}

/// A break: the section's last two bars (the band stops on the ONE of the first, rests in the
/// second); `Some(0)` or `Some(1)`.
pub fn break_bar(song_bar: usize, bars: usize) -> Option<usize> {
    let (k, n) = in_section(song_bar, bars);
    (n >= 4 && k + 2 >= n).then(|| k + 2 - n)
}

// --- intros ---------------------------------------------------------------------------------

/// How a tune starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntroKind {
    /// I–VI | ii–V, twice.
    Vamp,
    /// ii–V in every bar over the dominant in the bass.
    Pedal,
}

/// An intro bar: its kind, and which of how many.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Intro {
    pub kind: IntroKind,
    pub k: u8,
    pub n: u8,
}

impl Intro {
    pub fn last(&self) -> bool {
        self.k + 1 == self.n
    }
}

/// Bars of intro.
pub const INTRO_BARS: u8 = 4;

/// The intro a tune gets (`None` below [`INTRO_DIAL`]).
pub fn intro_for(seed: u64, dial: f32) -> Option<IntroKind> {
    if dial < INTRO_DIAL {
        return None;
    }
    Some(if band::rng(seed, 8, 0, 3).chance(0.5) { IntroKind::Vamp } else { IntroKind::Pedal })
}

// --- endings --------------------------------------------------------------------------------

/// How a tune ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EndKind {
    /// Just the last chord.
    Plain,
    /// A ii–V, three plinks, the last chord.
    Basie,
    /// The I–VI–ii–V turnaround three times, then the last chord.
    Tag,
    /// A ii–V vamped four times, quieter and quieter, then a soft last chord.
    VampOut,
}

impl EndKind {
    pub fn label(self) -> &'static str {
        match self {
            EndKind::Plain => "",
            EndKind::Basie => "BASIE ENDING",
            EndKind::Tag => "TAG",
            EndKind::VampOut => "VAMP-OUT",
        }
    }
}

/// What the band plays in a bar of an ending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndStep {
    /// Time, on the ending's changes.
    Time,
    /// The piano alone: three quiet plinks.
    Plinks,
    /// The last chord, held.
    Final,
}

/// An ending bar: the kind, which of how many.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndBar {
    pub kind: EndKind,
    pub k: u8,
    pub n: u8,
    /// A one-shot's own ending (after its last bar: no ii–V first).
    pub tail: bool,
}

impl EndBar {
    pub fn step(&self) -> EndStep {
        if self.k + 1 == self.n {
            EndStep::Final
        } else if self.kind == EndKind::Basie && self.k + 2 == self.n {
            EndStep::Plinks
        } else {
            EndStep::Time
        }
    }

    /// How much quieter the bar is (the vamp-out fades).
    pub fn fade(&self) -> f32 {
        match self.kind {
            EndKind::VampOut => 0.13 * self.k as f32,
            _ => 0.0,
        }
    }
}

/// Bars of an ending (`tail`: a one-shot's, after its last bar).
pub fn end_bars(kind: EndKind, tail: bool) -> u8 {
    match (kind, tail) {
        (EndKind::Plain, _) => 1,
        (EndKind::Basie, true) => 2,
        (EndKind::Basie, false) => 3,
        (EndKind::Tag, _) => 7,
        (EndKind::VampOut, _) => 5,
    }
}

/// The ending the band picks when told to end (at `dial`).
pub fn ending_for(seed: u64, bar: u64, dial: f32) -> EndKind {
    if dial < END_DIAL {
        return EndKind::Plain;
    }
    [EndKind::Basie, EndKind::Tag, EndKind::VampOut][band::rng(seed, 8, bar, 4).below(3)]
}

// --- the arranged harmony -------------------------------------------------------------------

/// The chords of an intro or ending bar in key `key` (a bar of `bb` beats): up to two, the
/// second from beat `half`.
pub fn harmony(key: u8, bb: f64, intro: Option<Intro>, end: Option<EndBar>) -> Option<([Chord; 2], f64)> {
    let ch = |deg: u8, q: Quality| Chord::new(key + deg, q);
    let half = (bb / 2.0).floor().max(1.0);
    let turnaround = |k: u8| {
        if k.is_multiple_of(2) { [ch(0, Quality::Six), ch(9, Quality::Dom7)] } else { [ch(2, Quality::Minor7), ch(7, Quality::Dom7)] }
    };
    let two_five = [ch(2, Quality::Minor7), ch(7, Quality::Dom7)];
    let tonic = ch(0, Quality::Six);
    if let Some(i) = intro {
        return Some((if i.kind == IntroKind::Vamp { turnaround(i.k) } else { two_five }, half));
    }
    let e = end?;
    let chords = match (e.step(), e.kind) {
        (EndStep::Final | EndStep::Plinks, _) => [tonic, tonic],
        (EndStep::Time, EndKind::Tag) => turnaround(e.k),
        (EndStep::Time, _) => two_five,
    };
    Some((chords, half))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choruses_arrange_themselves() {
        // Below the dial: always the head. The first pass is a head (in two or not); the
        // fourth of every arc the head again; a loose band shouts.
        for seed in 0..40 {
            assert!((0..20).all(|p| auto(seed, p, 0.2) == Call::default()));
            for d in [0.3, 0.5, 0.8] {
                assert!(matches!(auto(seed, 0, d).chorus, Chorus::Head | Chorus::TwoFeel));
                for p in 1..20 {
                    let c = auto(seed, p, d);
                    if (p - 1) % ARC == 3 {
                        assert_eq!(c, Call::default());
                    }
                    // An arranged chorus never repeats the pass before.
                    if c.chorus != Chorus::Head {
                        assert_ne!(c.chorus, auto(seed, p - 1, d).chorus, "seed {seed} pass {p}");
                    }
                    assert!(!c.key_up || c.chorus == Chorus::Shout);
                }
            }
        }
        let count = |d: f32, c: Chorus| (0..60).flat_map(|s| (0..12).map(move |p| auto(s, p, d))).filter(|x| x.chorus == c).count();
        // Calm: the tune-keeping ones only; loose: everything, the shout, the key-up.
        for c in [Chorus::Blowing, Chorus::Soli, Chorus::Strolling, Chorus::Shout] {
            assert_eq!(count(0.35, c), 0, "{c:?}");
        }
        for c in Chorus::ALL {
            assert!(count(0.8, c) > 0, "{c:?}");
        }
        assert!((0..60).any(|s| (0..12).any(|p| auto(s, p, 0.9).key_up)));
        // More dial, fewer heads.
        assert!(count(0.3, Chorus::Head) > count(0.6, Chorus::Head));
    }

    #[test]
    fn sections_stop_and_break() {
        // 8-bar sections: stop-time on bars 0-5, breaks on 6 and 7; a 4-bar tail likewise.
        let stops: Vec<bool> = (0..12).map(|b| stop_bar(b, 12)).collect();
        assert_eq!(stops, [true, true, true, true, true, true, false, false, true, true, false, false]);
        let breaks: Vec<Option<usize>> = (0..12).map(|b| break_bar(b, 12)).collect();
        assert_eq!(&breaks[5..8], [None, Some(0), Some(1)]);
        assert_eq!(&breaks[9..12], [None, Some(0), Some(1)]);
        // Too short a tail: neither.
        assert!(!stop_bar(8, 10) && break_bar(9, 10).is_none());
    }

    #[test]
    fn endings_end_on_the_tonic() {
        for kind in [EndKind::Plain, EndKind::Basie, EndKind::Tag, EndKind::VampOut] {
            for tail in [false, true] {
                let n = end_bars(kind, tail);
                let bars: Vec<EndBar> = (0..n).map(|k| EndBar { kind, k, n, tail }).collect();
                assert_eq!(bars.last().unwrap().step(), EndStep::Final);
                assert_eq!(bars.iter().filter(|b| b.step() == EndStep::Plinks).count(), (kind == EndKind::Basie) as usize);
                let (c, _) = harmony(5, 4.0, None, bars.last().copied()).unwrap();
                assert_eq!((c[0].root, c[0].quality), (5, Quality::Six));
            }
        }
        // The tag's turnaround: F6 D7 | Gm7 C7.
        let tag = |k| harmony(5, 4.0, None, Some(EndBar { kind: EndKind::Tag, k, n: 7, tail: false })).unwrap().0;
        assert_eq!(tag(0).map(|c| c.root), [5, 2]);
        assert_eq!(tag(1).map(|c| c.root), [7, 0]);
        // A vamp-out fades.
        assert!(EndBar { kind: EndKind::VampOut, k: 3, n: 5, tail: false }.fade() > 0.3);
    }
}

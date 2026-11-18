//! Taste: a critic over the arranger's choices.
//!
//! The arranger's big choices (which chorus a pass is, whether the climax is a shout and goes up
//! a key, whether a section drops into a feel and which) are weighted dice ([`super::chorus`],
//! [`super::feel`]): the weights say how often, but not *when*. A [`Taste`] scores each
//! candidate against the moment (the tune's tempo, how busy its melody is, its form, the chorus
//! before, how far into the night) and the dice are reweighted by it: a candidate's weight is
//! its old weight times `exp(score)`. So the band still surprises, but leans towards what suits
//! the moment.
//!
//! The scores are linear: a weight per candidate and per feature of the moment ([`Context`]),
//! learned from listening (A/B pairs: which of two takes, the same but for one choice, sounds
//! better; fitted as a Bradley-Terry model by `examples/fit_taste.rs`) and kept as text
//! ([`Taste::parse`], `music/taste.critic`). [`Taste::NEUTRAL`] (all zeros) scores everything
//! the same, and then the dice are rolled exactly as before: the same seeds play the same
//! nights.

use super::chorus::{Call, Chorus};
use super::feel::Feel;
use super::soloist::Moment;
use crate::chart::Family;

/// Features of the moment a choice is made in.
pub const FEATURES: usize = 10;

/// Their names, as columns in the text.
pub const FEATURE_NAMES: [&str; FEATURES] = ["bias", "slow", "fast", "busy", "sparse", "long", "after-head", "after-out", "after-solo", "late"];

/// Below this tempo a tune is slow, from this one fast (the library's tunes run 132 to 192
/// bpm: `cargo run --example taste_tunes`).
pub const SLOW_BPM: f32 = 150.0;
pub const FAST_BPM: f32 = 185.0;
/// A melody with at least this many notes a beat is busy (the rags: 1.2 to 1.3), with at most
/// this many sparse (the ballads and riff tunes: 0.5 to 0.7).
pub const BUSY: f32 = 1.1;
pub const SPARSE: f32 = 0.72;
/// A form longer than this (a 36- or 40-bar tune) is long.
pub const LONG_BARS: usize = 32;
/// From this pass on, it's late in the night.
pub const LATE_PASS: u64 = 5;

/// What the critic knows about the tune.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Tune {
    pub bpm: f32,
    /// The melody's notes per beat, over the whole head.
    pub density: f32,
    /// Bars in the form.
    pub bars: usize,
}

/// A moment's features: 0 or 1, the first always 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Context(pub [f32; FEATURES]);

impl Context {
    /// The moment pass `pass` of `tune` is decided in, after `before` (the chorus the pass
    /// before played; for a feel, the pass's own).
    pub fn new(tune: &Tune, pass: u64, before: Option<Chorus>) -> Context {
        let b = |x: bool| if x { 1.0 } else { 0.0 };
        let known = tune.bpm > 0.0;
        let c = before.unwrap_or(Chorus::Head);
        Context([
            1.0,
            b(known && tune.bpm < SLOW_BPM),
            b(known && tune.bpm >= FAST_BPM),
            b(known && tune.density >= BUSY),
            b(known && tune.density <= SPARSE),
            b(known && tune.bars > LONG_BARS),
            b(before.is_some() && matches!(c, Chorus::Head | Chorus::TwoFeel)),
            b(matches!(c, Chorus::StopTime | Chorus::Breaks)),
            b(matches!(c, Chorus::Blowing | Chorus::Riffs | Chorus::Strolling)),
            b(pass >= LATE_PASS),
        ])
    }
}

/// The soloist's features of the moment: bias; slow / fast (as for the arranger); its first
/// chorus / its third or later; the chord a dominant / a minor; the solo hot / cool (its
/// chorus shape's heat, [`super::soloist::Arc::heat`]); a dominant resolving to minor.
pub const SOLO_FEATURE_NAMES: [&str; FEATURES] = ["bias", "slow", "fast", "first", "later", "dominant", "minor", "hot", "cool", "to-minor"];

impl Context {
    /// The moment a soloist's choice is made in.
    pub fn solo(m: &Moment) -> Context {
        let b = |x: bool| if x { 1.0 } else { 0.0 };
        let known = m.bpm > 0.0;
        Context([
            1.0,
            b(known && m.bpm < SLOW_BPM),
            b(known && m.bpm >= FAST_BPM),
            b(m.chorus == 1),
            b(m.chorus >= 3),
            b(m.family == Some(Family::Dominant)),
            b(matches!(m.family, Some(Family::Minor | Family::HalfDim))),
            b(m.heat.is_some_and(|h| h > 0.66)),
            b(m.heat.is_some_and(|h| h < 0.33)),
            b(m.resolves_minor),
        ])
    }
}

/// The features of a big band section's moment, past the bias: the tempo's band; the chorus's
/// first or last 8-bar section; a head (the tune, or two-feel); behind a solo; a climax (shout,
/// soli, riffs); the first pass; a late one.
pub const SECTION_FEATURE_NAMES: [&str; FEATURES] = ["bias", "slow", "fast", "first", "last", "head", "solo", "climax", "opening", "late"];

impl Context {
    /// The moment a big band choice is made in: section `s` of `n` in pass `pass` of a
    /// `chorus`, `solo` when a soloist is up.
    pub fn section(bpm: f32, s: usize, n: usize, chorus: Chorus, solo: bool, pass: u64) -> Context {
        let b = |x: bool| if x { 1.0 } else { 0.0 };
        Context([
            1.0,
            b(bpm > 0.0 && bpm < SLOW_BPM),
            b(bpm >= FAST_BPM),
            b(s == 0),
            b(s + 1 >= n),
            b(matches!(chorus, Chorus::Head | Chorus::TwoFeel)),
            b(solo),
            b(matches!(chorus, Chorus::Shout | Chorus::Soli | Chorus::Riffs)),
            b(pass == 0),
            b(pass >= LATE_PASS),
        ])
    }
}

/// The soloist's choices a [`Taste`] leans, each a table of rows (candidates).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Table {
    /// A chorus's shape ([`super::soloist::Arc`]).
    Arc,
    /// Whether a phrase picks up the comp, plays a lick, carries a cell (each against a new
    /// idea; [`super::soloist::Idea`]).
    Idea,
    /// A phrase's mode ([`super::soloist::Mode`]).
    Mode,
    /// How a line gets into a target ([`super::soloist::Device`]).
    Device,
    /// A target's degree (`target_weights`' order).
    Target,
    /// The big band's ([`super::arranger`], [`super::horns`]; scored in a
    /// [`Context::section`]): what the upper sections play behind a line in an 8-bar section.
    Upper,
    /// Who takes the tune in a head's section (the tutti row: the last section's chance of
    /// everyone on it).
    Take,
    /// What the low horns play in a section ([`super::horns::LowRole`]).
    Low,
    /// How a section's pads (and a chorus's riffs' long notes) open inside the chord.
    Pad,
    /// How a tutti phrase's last long note ends.
    Ending,
    /// A chorus's riff: the theme row the chance it's the tune's own motif, the unison row its
    /// fast notes in octaves.
    Riff,
    /// Where an answer comes from: the line's figure upside down or as heard, the tune's
    /// motif, a stock answer.
    Answer,
}

impl Table {
    pub const ALL: [Table; 12] = [Table::Arc, Table::Idea, Table::Mode, Table::Device, Table::Target, Table::Upper, Table::Take, Table::Low, Table::Pad, Table::Ending, Table::Riff, Table::Answer];

    /// The big band's tables (scored in a [`Context::section`], the rest in a
    /// [`Context::solo`]).
    pub fn is_section(self) -> bool {
        Table::ALL.iter().position(|&t| t == self) >= Table::ALL.iter().position(|&t| t == Table::Upper)
    }

    pub fn slug(self) -> &'static str {
        match self {
            Table::Arc => "arc",
            Table::Idea => "idea",
            Table::Mode => "mode",
            Table::Device => "device",
            Table::Target => "target",
            Table::Upper => "upper",
            Table::Take => "take",
            Table::Low => "low",
            Table::Pad => "pad",
            Table::Ending => "ending",
            Table::Riff => "riff",
            Table::Answer => "answer",
        }
    }

    /// Its rows' names in the text.
    pub fn rows(self) -> &'static [&'static str] {
        match self {
            Table::Arc => &["arch", "builds", "front", "flat"],
            Table::Idea => &["echo", "lick", "carry"],
            Table::Mode => &["changes", "scalar", "blues"],
            Table::Device => &["direct", "chromatic", "enclosure", "scale", "blues", "arpeggio"],
            Table::Target => &["R", "3", "5", "7", "9", "b9", "+9", "+11", "13", "b13"],
            Table::Upper => &["tacet", "pads", "answers", "riff", "punches"],
            Table::Take => &["lead", "soli", "trumpet", "tenor", "alto", "tutti"],
            Table::Low => &["tacet", "unison", "octave", "pedal", "pads", "hits", "alone"],
            Table::Pad => &["plain", "bloom", "pyramid", "cliche", "suspension"],
            Table::Ending => &["plain", "wedge", "bloom"],
            Table::Riff => &["theme", "unison"],
            Table::Answer => &["inverted", "echo", "motif", "stock"],
        }
    }

    /// Its first row among [`Taste::solo`]'s.
    fn at(self) -> usize {
        Table::ALL.iter().take_while(|&&t| t != self).map(|t| t.rows().len()).sum()
    }

    pub fn parse(s: &str) -> Option<Table> {
        Table::ALL.into_iter().find(|t| t.slug() == s)
    }
}

/// Rows of the soloist's and the big band's tables, all told.
pub const SOLO_ROWS: usize = 4 + 3 + 3 + 6 + 10 + (5 + 6 + 7 + 5 + 3 + 2 + 4);

/// Rows of the chorus table: one per chorus, then the key-up.
pub const CHORUS_ROWS: usize = 10;
/// The key-up's row.
pub const KEY_UP_ROW: usize = 9;

/// The critic: a weight per candidate (row) and feature (column).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Taste {
    /// Rows indexed like [`Chorus::ALL`], then [`KEY_UP_ROW`].
    pub chorus: [[f32; FEATURES]; CHORUS_ROWS],
    /// Rows indexed like [`Feel::ALL`].
    pub feel: [[f32; FEATURES]; 5],
    /// The soloist's tables ([`Table`]), one after another.
    pub solo: [[f32; FEATURES]; SOLO_ROWS],
}

impl Default for Taste {
    fn default() -> Self {
        Taste::NEUTRAL
    }
}

fn dot(w: &[f32; FEATURES], c: &Context) -> f32 {
    w.iter().zip(c.0).map(|(a, b)| a * b).sum()
}

fn chorus_row(c: Chorus) -> usize {
    Chorus::ALL.iter().position(|&x| x == c).unwrap_or(0)
}

fn feel_row(f: Feel) -> usize {
    Feel::ALL.iter().position(|&x| x == f).unwrap_or(0)
}

impl Taste {
    /// No opinions: the dice as they were.
    pub const NEUTRAL: Taste = Taste { chorus: [[0.0; FEATURES]; CHORUS_ROWS], feel: [[0.0; FEATURES]; 5], solo: [[0.0; FEATURES]; SOLO_ROWS] };

    pub fn is_neutral(&self) -> bool {
        *self == Taste::NEUTRAL
    }

    /// A chorus's score in a moment (the key-up's added for a key-up).
    pub fn chorus_score(&self, call: Call, ctx: &Context) -> f32 {
        let s = dot(&self.chorus[chorus_row(call.chorus)], ctx);
        if call.key_up { s + dot(&self.chorus[KEY_UP_ROW], ctx) } else { s }
    }

    /// A feel's score in a moment.
    pub fn feel_score(&self, feel: Feel, ctx: &Context) -> f32 {
        dot(&self.feel[feel_row(feel)], ctx)
    }

    /// Row `i` of the soloist's `table`, scored in a moment.
    pub fn solo_score(&self, table: Table, i: usize, ctx: &Context) -> f32 {
        dot(&self.solo[table.at() + i], ctx)
    }

    /// Lean a soloist's weights `w` (one per row of `table`): each times `exp(score)`. A neutral
    /// taste leaves them exactly as they were.
    pub fn lean(&self, table: Table, w: &mut [f32], ctx: &Context) {
        for (i, x) in w.iter_mut().enumerate().take(table.rows().len()) {
            let s = self.solo_score(table, i, ctx);
            if s != 0.0 {
                *x *= s.exp();
            }
        }
    }

    /// Lean a big band pick's options (each with its row in `table`): each weight times
    /// `exp(score)`. A neutral taste leaves them exactly as they were.
    pub fn lean_opts<T: Copy>(&self, table: Table, opts: &[(T, f64)], row: impl Fn(T) -> usize, ctx: &Context) -> Vec<(T, f64)> {
        opts.iter()
            .map(|&(t, w)| {
                let s = self.solo_score(table, row(t), ctx);
                (t, if s != 0.0 { w * (s as f64).exp() } else { w })
            })
            .collect()
    }

    /// Whether any row of `table` has an opinion.
    pub fn has(&self, table: Table) -> bool {
        self.solo[table.at()..table.at() + table.rows().len()].iter().any(|r| r.iter().any(|&x| x != 0.0))
    }

    /// A row of the soloist's `table` by name.
    pub fn solo_row_mut(&mut self, table: Table, row: &str) -> Option<&mut [f32; FEATURES]> {
        let i = table.rows().iter().position(|&r| r == row)?;
        Some(&mut self.solo[table.at() + i])
    }

    /// The features a soloist's row lights up, flattened like [`Taste::weights`].
    pub fn solo_features(table: Table, i: usize, ctx: &Context) -> [f32; WEIGHTS] {
        let mut out = [0.0; WEIGHTS];
        let at = (CHORUS_ROWS + 5 + table.at() + i) * FEATURES;
        out[at..at + FEATURES].copy_from_slice(&ctx.0);
        out
    }

    /// The features a chorus candidate lights up, flattened like [`Taste::weights`] (for
    /// fitting: a pair's difference of these is what its preference is about).
    pub fn chorus_features(call: Call, ctx: &Context) -> [f32; WEIGHTS] {
        let mut out = [0.0; WEIGHTS];
        let r = chorus_row(call.chorus);
        out[r * FEATURES..(r + 1) * FEATURES].copy_from_slice(&ctx.0);
        if call.key_up {
            out[KEY_UP_ROW * FEATURES..(KEY_UP_ROW + 1) * FEATURES].copy_from_slice(&ctx.0);
        }
        out
    }

    /// The same for a feel.
    pub fn feel_features(feel: Feel, ctx: &Context) -> [f32; WEIGHTS] {
        let mut out = [0.0; WEIGHTS];
        let at = (CHORUS_ROWS + feel_row(feel)) * FEATURES;
        out[at..at + FEATURES].copy_from_slice(&ctx.0);
        out
    }

    /// Every weight, flattened: the chorus rows, then the feel rows.
    pub fn weights(&self) -> [f32; WEIGHTS] {
        let mut out = [0.0; WEIGHTS];
        for (i, row) in self.chorus.iter().chain(self.feel.iter()).chain(self.solo.iter()).enumerate() {
            out[i * FEATURES..(i + 1) * FEATURES].copy_from_slice(row);
        }
        out
    }

    pub fn from_weights(w: &[f32; WEIGHTS]) -> Taste {
        let mut t = Taste::NEUTRAL;
        for (i, row) in t.chorus.iter_mut().chain(t.feel.iter_mut()).chain(t.solo.iter_mut()).enumerate() {
            row.copy_from_slice(&w[i * FEATURES..(i + 1) * FEATURES]);
        }
        t
    }

    /// Read the text form:
    /// ```text
    /// # comments
    /// [chorus]
    /// breaks   0.4 0 -0.3 0 0 0.2 0 -0.5 0 0     (a row: a name, then a weight per feature)
    /// key-up   ...
    /// [feel]
    /// bossa    ...
    /// ```
    /// Rows left out are zeros; a row may stop early (the rest are zeros).
    pub fn parse(text: &str) -> Result<Taste, String> {
        let mut t = Taste::NEUTRAL;
        let mut section = "";
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if let Some(s) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                section = match s.trim() {
                    "chorus" => "chorus",
                    "feel" => "feel",
                    other => Table::parse(other).map(Table::slug).ok_or_else(|| format!("line {}: unknown section [{other}]", n + 1))?,
                };
                continue;
            }
            let mut words = line.split_whitespace();
            let name = words.next().unwrap_or("");
            let row: &mut [f32; FEATURES] = match section {
                "chorus" if name == "key-up" => &mut t.chorus[KEY_UP_ROW],
                "chorus" => &mut t.chorus[chorus_row(Chorus::parse(name).ok_or_else(|| format!("line {}: no chorus \"{name}\"", n + 1))?)],
                "feel" => &mut t.feel[feel_row(Feel::parse(name).ok_or_else(|| format!("line {}: no feel \"{name}\"", n + 1))?)],
                "" => return Err(format!("line {}: a row before any [section]", n + 1)),
                table => {
                    let table = Table::parse(table).unwrap_or(Table::Arc);
                    t.solo_row_mut(table, name).ok_or_else(|| format!("line {}: no {} \"{name}\"", n + 1, table.slug()))?
                }
            };
            for (i, w) in words.enumerate() {
                if i >= FEATURES {
                    return Err(format!("line {}: more than {FEATURES} weights", n + 1));
                }
                row[i] = w.parse().map_err(|_| format!("line {}: \"{w}\" isn't a number", n + 1))?;
            }
        }
        Ok(t)
    }

    /// The text form ([`Taste::parse`] reads it back).
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let head = format!("# {:<10}{}\n", "", FEATURE_NAMES.map(|f| format!("{f:>11}")).join(""));
        let row = |s: &mut String, name: &str, w: &[f32; FEATURES]| {
            s.push_str(&format!("{name:<12}"));
            for x in w {
                s.push_str(&format!("{x:>11.3}"));
            }
            s.push('\n');
        };
        s.push_str("[chorus]\n");
        s.push_str(&head);
        for (c, w) in Chorus::ALL.iter().zip(&self.chorus) {
            row(&mut s, c.slug(), w);
        }
        row(&mut s, "key-up", &self.chorus[KEY_UP_ROW]);
        s.push_str("\n[feel]\n");
        s.push_str(&head);
        for (f, w) in Feel::ALL.iter().zip(&self.feel) {
            row(&mut s, f.slug(), w);
        }
        let solo_head = format!("# {:<10}{}\n", "", SOLO_FEATURE_NAMES.map(|f| format!("{f:>11}")).join(""));
        let section_head = format!("# {:<10}{}\n", "", SECTION_FEATURE_NAMES.map(|f| format!("{f:>11}")).join(""));
        for table in Table::ALL {
            s.push_str(&format!("\n[{}]\n", table.slug()));
            s.push_str(if table.is_section() { &section_head } else { &solo_head });
            for (i, name) in table.rows().iter().enumerate() {
                row(&mut s, name, &self.solo[table.at() + i]);
            }
        }
        s
    }
}

/// Weights in a [`Taste`].
pub const WEIGHTS: usize = (CHORUS_ROWS + 5 + SOLO_ROWS) * FEATURES;

/// The taste the band plays with by default (`music/taste.critic`).
pub fn shipped() -> Taste {
    Taste::parse(include_str!("../../music/taste.critic")).expect("music/taste.critic")
}

/// `p` leaned by a score: `p e^s / (p e^s + 1 - p)` (0 stays 0, 1 stays 1, and it rises with
/// `p`, so a looser dial still keeps whatever a tighter one had).
pub fn lean(p: f64, s: f32) -> f64 {
    if s == 0.0 || p <= 0.0 || p >= 1.0 {
        return p;
    }
    let e = p * (s as f64).exp();
    e / (e + 1.0 - p)
}

/// An index into `w` by weight, from a draw `x` in 0..1 (the last positive one if rounding runs
/// off the end); `None` if every weight is 0.
pub fn pick(w: &[f64], x: f64) -> Option<usize> {
    let total: f64 = w.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let mut x = x * total;
    for (i, &v) in w.iter().enumerate() {
        if x < v {
            return Some(i);
        }
        x -= v;
    }
    w.iter().rposition(|&v| v > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_round_trips() {
        let mut t = Taste::NEUTRAL;
        t.chorus[3][1] = 0.5;
        t.chorus[KEY_UP_ROW][9] = -1.25;
        t.feel[2][0] = 2.0;
        t.solo_row_mut(Table::Device, "enclosure").unwrap()[3] = 0.75;
        t.solo_row_mut(Table::Target, "+11").unwrap()[5] = -0.5;
        let back = Taste::parse(&t.to_text()).unwrap();
        assert_eq!(back, t);
        assert!(Taste::parse("[chorus]\nnope 1").is_err());
        assert!(shipped().chorus.len() == CHORUS_ROWS);
    }

    #[test]
    fn features_match_scores() {
        let mut t = Taste::NEUTRAL;
        for (i, w) in t.chorus.iter_mut().flatten().chain(t.feel.iter_mut().flatten()).chain(t.solo.iter_mut().flatten()).enumerate() {
            *w = (i as f32 * 0.37).sin();
        }
        let ctx = Context::new(&Tune { bpm: 220.0, density: 0.5, bars: 12 }, 6, Some(Chorus::Breaks));
        let w = t.weights();
        let dotw = |f: [f32; WEIGHTS]| f.iter().zip(&w).map(|(a, b)| a * b).sum::<f32>();
        for c in Chorus::ALL {
            for key_up in [false, true] {
                let call = Call { chorus: c, key_up };
                assert!((dotw(Taste::chorus_features(call, &ctx)) - t.chorus_score(call, &ctx)).abs() < 1e-5);
            }
        }
        for f in Feel::ALL {
            assert!((dotw(Taste::feel_features(f, &ctx)) - t.feel_score(f, &ctx)).abs() < 1e-5);
        }
        for table in Table::ALL {
            for i in 0..table.rows().len() {
                assert!((dotw(Taste::solo_features(table, i, &ctx)) - t.solo_score(table, i, &ctx)).abs() < 1e-5);
            }
        }
        assert_eq!(Taste::from_weights(&w), t);
    }

    #[test]
    fn leaning_keeps_order() {
        assert_eq!(lean(0.3, 0.0), 0.3);
        assert!(lean(0.3, 1.0) > 0.3 && lean(0.3, -1.0) < 0.3);
        assert!(lean(0.2, 0.7) < lean(0.4, 0.7));
        assert_eq!(lean(0.0, 3.0), 0.0);
    }
}

#[cfg(test)]
mod leaning {
    use super::super::chorus::{self, ARC};
    use super::super::feel;
    use super::*;

    const TUNE: Tune = Tune { bpm: 192.0, density: 0.7, bars: 32 };

    #[test]
    fn a_neutral_taste_rolls_the_old_dice() {
        for seed in 0..40 {
            for pass in 0..12 {
                for dial in [0.0, 0.3, 0.5, 0.7, 1.0] {
                    assert_eq!(chorus::auto_with(seed, pass, dial, &Taste::NEUTRAL, &TUNE), chorus::auto(seed, pass, dial));
                    for bar in [0, 8, 16, 24] {
                        let ctx = Context::new(&TUNE, pass, Some(Chorus::Blowing));
                        assert_eq!(feel::auto_with(seed, pass, bar, 32, dial, &Taste::NEUTRAL, &ctx), feel::auto(seed, pass, bar, 32, dial));
                    }
                }
            }
        }
    }

    /// How often each chorus comes up in middle passes over many nights.
    fn middles(taste: &Taste, tune: &Tune) -> [usize; 9] {
        let mut n = [0; 9];
        for seed in 0..400 {
            for pass in [1, 2, 5, 6] {
                let c = chorus::auto_with(seed, pass, 0.8, taste, tune).chorus;
                n[Chorus::ALL.iter().position(|&x| x == c).unwrap()] += 1;
            }
        }
        n
    }

    #[test]
    fn a_taste_leans_the_choruses_and_keeps_the_rules() {
        let breaks = Chorus::ALL.iter().position(|&c| c == Chorus::Breaks).unwrap();
        let before = middles(&Taste::NEUTRAL, &TUNE);
        let mut t = Taste::NEUTRAL;
        // Loves breaks on sparse tunes only.
        t.chorus[breaks][4] = 3.0;
        let after = middles(&t, &TUNE);
        assert!(after[breaks] > 2 * before[breaks], "{before:?} -> {after:?}");
        let busy = Tune { density: 1.3, ..TUNE };
        let other = middles(&t, &busy);
        assert!(other[breaks] < after[breaks] / 2, "{other:?}");
        // Still never the same arranged chorus twice running, still the head to close each arc.
        for seed in 0..200 {
            for pass in 1..12 {
                let (a, b) = (chorus::auto_with(seed, pass, 0.8, &t, &TUNE), chorus::auto_with(seed, pass + 1, 0.8, &t, &TUNE));
                if (pass % ARC) != 0 && (pass + 1 - 1) % ARC < 2 && a.chorus != Chorus::Head {
                    assert_ne!(a.chorus, b.chorus, "seed {seed} pass {pass}");
                }
                if pass % ARC == 0 {
                    assert_eq!(a.chorus, Chorus::Head);
                }
            }
        }
    }

    #[test]
    fn a_taste_leans_the_feels_and_stays_monotone() {
        let mut t = Taste::NEUTRAL;
        let bossa = 1;
        t.feel[bossa][0] = 2.5;
        let ctx = Context::new(&TUNE, 1, Some(Chorus::Blowing));
        let count = |t: &Taste, dial: f32, f: Feel| {
            (0..300u64).flat_map(|seed| (0..4u64).flat_map(move |pass| (0..4).map(move |s| (seed, pass, s * 8))))
                .filter(|&(seed, pass, bar)| feel::auto_with(seed, pass, bar, 32, dial, t, &ctx) == f)
                .count()
        };
        let (lean, plain) = (count(&t, 0.7, Feel::Bossa), count(&Taste::NEUTRAL, 0.7, Feel::Bossa));
        // (Less than the odds alone: a slot never repeats the feel the slot before picked.)
        assert!(2 * lean > 3 * plain, "{plain} -> {lean}");
        // A looser dial keeps every feel a tighter one had.
        for seed in 0..100 {
            for pass in 0..4 {
                for bar in [8, 16, 24] {
                    let lo = feel::auto_with(seed, pass, bar, 32, 0.5, &t, &ctx);
                    if lo != Feel::Swing {
                        assert_eq!(feel::auto_with(seed, pass, bar, 32, 0.8, &t, &ctx), lo);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod planning {
    use super::super::band::PassPlan;
    use super::super::chorus::{Call, Chorus};
    use super::super::engine::{Engine, EngineConfig, Input};
    use super::super::feel::Feel;
    use super::super::library;
    use crate::Frame;

    /// A listening test's take: pass 0 and pass 1 arranged ahead, played as planned.
    #[test]
    fn planned_passes_play_as_planned() {
        let file = library::load("tiger_rag").unwrap();
        let mut e = Engine::with_config(&file, 16_000, EngineConfig::default()).unwrap();
        e.post(Input::SetFreedom { lead: 0.7, comp: 0.7, bass: 0.7, drums: 0.7, dynamics: 0.5 });
        e.post(Input::PlanPass(PassPlan { pass: 0, chorus: Some(Call { chorus: Chorus::StopTime, key_up: false }), feel: Some(Feel::Swing) }));
        e.post(Input::PlanPass(PassPlan { pass: 1, chorus: Some(Call { chorus: Chorus::Blowing, key_up: false }), feel: Some(Feel::Bossa) }));
        let bars = e.shape().bars;
        e.start_at(e.shape().bar_starts[bars - 2]);
        let mut seen = Vec::new();
        let mut buf = [Frame::ZERO; 256];
        for _ in 0..2000 {
            e.fill(&mut buf);
            let s = e.state();
            seen.push((s.position.pass, s.chorus.chorus, s.feel));
        }
        assert!(seen.iter().any(|&(p, c, f)| p == 0 && c == Chorus::StopTime && f == Feel::Swing), "{:?}", &seen[..4]);
        assert!(seen.iter().any(|&(p, c, f)| p == 1 && c == Chorus::Blowing && f == Feel::Bossa), "{:?}", seen.last());
        assert!(!seen.iter().any(|&(p, c, _)| p == 1 && c != Chorus::Blowing));
    }
}

#[cfg(test)]
mod soloing {
    use super::super::band::PassPlan;
    use super::super::chorus::{Call, Chorus};
    use super::super::engine::{Engine, EngineConfig, Input};
    use super::super::feel::Feel;
    use super::super::library;
    use super::super::soloist::{Mode, Soloist};
    use super::*;
    use crate::Frame;

    /// Phrase modes and chorus shapes a Trane chorus over Tiger Rag plays with `taste`.
    fn played(taste: Taste) -> (Vec<Mode>, Vec<super::super::soloist::Arc>) {
        let file = library::load("tiger_rag").unwrap();
        let mut e = Engine::with_config(&file, 16_000, EngineConfig { seed: 3, taste, soloist: Some(Soloist::Trane), ..EngineConfig::default() }).unwrap();
        e.post(Input::SetFreedom { lead: 0.8, comp: 0.8, bass: 0.8, drums: 0.8, dynamics: 0.5 });
        for pass in 0..3 {
            e.post(Input::PlanPass(PassPlan { pass, chorus: Some(Call { chorus: Chorus::Blowing, key_up: false }), feel: Some(Feel::Swing) }));
        }
        let (mut modes, mut arcs, mut seen) = (Vec::new(), Vec::new(), std::collections::HashSet::new());
        let mut buf = [Frame::ZERO; 512];
        for _ in 0..3000 {
            e.fill(&mut buf);
            for b in &e.state().upcoming {
                if let Some(t) = b.thought.filter(|_| seen.insert(b.slot.index)) {
                    arcs.push(t.arc);
                    modes.extend(t.phrases[..t.nphrases as usize].iter().map(|p| p.mode));
                }
            }
        }
        (modes, arcs)
    }

    #[test]
    fn a_taste_leans_the_soloist() {
        let (plain, plain_arcs) = played(Taste::NEUTRAL);
        assert_eq!(played(Taste::NEUTRAL), (plain.clone(), plain_arcs.clone()), "a neutral taste plays the same solo");
        let mut t = Taste::NEUTRAL;
        t.solo_row_mut(Table::Mode, "scalar").unwrap()[0] = 6.0;
        t.solo_row_mut(Table::Arc, "flat").unwrap()[0] = 8.0;
        let (leaned, arcs) = played(t);
        let share = |m: &[Mode]| m.iter().filter(|&&x| x == Mode::Scalar).count() as f32 / m.len().max(1) as f32;
        assert!(!plain.is_empty() && share(&leaned) > share(&plain) + 0.2, "{} -> {}", share(&plain), share(&leaned));
        assert!(arcs.iter().all(|&a| a == super::super::soloist::Arc::Flat), "{arcs:?}");
    }

    /// The big band's sections (upper role, low role) through head and blowing passes.
    fn sections(taste: Taste) -> Vec<(Option<super::super::horns::UpperRole>, Option<super::super::horns::LowRole>)> {
        let file = library::load("tiger_rag").unwrap();
        let mut e = Engine::with_config(&file, 16_000, EngineConfig { seed: 5, taste, horns: super::super::horns::Horns::BIG_BAND, ..EngineConfig::default() }).unwrap();
        e.post(Input::SetFreedom { lead: 0.8, comp: 0.8, bass: 0.8, drums: 0.8, dynamics: 0.5 });
        for pass in 0..8 {
            let chorus = if pass % 2 == 0 { Chorus::Head } else { Chorus::Blowing };
            e.post(Input::PlanPass(PassPlan { pass, chorus: Some(Call { chorus, key_up: false }), feel: Some(Feel::Swing) }));
        }
        let (mut out, mut seen) = (Vec::new(), std::collections::HashSet::new());
        let mut buf = [Frame::ZERO; 512];
        for _ in 0..9000 {
            e.fill(&mut buf);
            for b in &e.state().upcoming {
                if seen.insert(b.slot.index) && b.slot.song_bar % super::super::horns::SECTION == 0 {
                    out.push((b.upper, b.low));
                }
            }
        }
        out
    }

    #[test]
    fn a_taste_leans_the_big_band() {
        use super::super::horns::{LowRole, UpperRole};
        let plain = sections(Taste::NEUTRAL);
        assert_eq!(sections(Taste::NEUTRAL), plain, "a neutral taste plays the same chart");
        let mut t = Taste::NEUTRAL;
        t.solo_row_mut(Table::Upper, "answers").unwrap()[0] = 8.0;
        t.solo_row_mut(Table::Low, "pedal").unwrap()[0] = 8.0;
        let leaned = sections(t);
        let share = |v: &[(Option<UpperRole>, Option<LowRole>)], f: &dyn Fn(&(Option<UpperRole>, Option<LowRole>)) -> bool| v.iter().filter(|x| f(x)).count() as f32 / v.len().max(1) as f32;
        let answers = |x: &(Option<UpperRole>, Option<LowRole>)| x.0 == Some(UpperRole::Answers);
        let pedal = |x: &(Option<UpperRole>, Option<LowRole>)| x.1 == Some(LowRole::Pedal);
        assert!(plain.len() > 20, "{}", plain.len());
        assert!(share(&leaned, &answers) > share(&plain, &answers) + 0.2, "answers {} -> {}", share(&plain, &answers), share(&leaned, &answers));
        assert!(share(&leaned, &pedal) > share(&plain, &pedal) + 0.3, "pedal {} -> {}", share(&plain, &pedal), share(&leaned, &pedal));
    }
}

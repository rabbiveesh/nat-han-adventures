//! The band's shared plan for each bar ([`BandPlan`]): what the musicians must agree on, decided
//! once per bar (by the engine, when the bar is committed) and read by all four as they commit
//! it, so it lines up by construction:
//! - **hits**: 16th positions the comp stabs, the bass hits (a root, then rests) and the drums
//!   kick together: the comp's anticipation of the next bar (low freedom) and stop-time /
//!   ending figures at phrase ends (mid);
//! - **sub**: the band's own reharmonization of part of the bar (tritone subs, backdoor ii-V):
//!   the comp voices it, the bass plays its roots, the lead's ornaments use it;
//! - **trade**: trading fours: the lead solos four bars, then lays out while the drums solo;
//! - **fill / crash**: the drums' fills (every 4 and 8 bars, phrase ends, press rolls into
//!   sections) and the crash after one;
//! - **flourish**: a big moment from the game (a summon switching the harmony: crash and fill;
//!   a checkpoint: a short fill; a death: the lead's wah-wah).
//!
//! Every choice is a pure function of the seed, the bar, the dials and the cues, so it's
//! deterministic, and the musicians' own plans can show the same choices ahead of time.
//!
//! # Extension point: feels
//! A later *feel* (bossa, samba, rock, funk) belongs here: a per-bar [`BandPlan::feel`] that the
//! drums read for their pattern, the comp and bass for their rhythm transforms, and every
//! musician for its instrument ([`super::instrument::Instruments::palette`]); decided like the
//! trade (per section, from the dials), so the band switches together.

use crate::audio::accomp::Rng;
use crate::audio::chart::{Chord, Family, Quality};
use crate::audio::tuning;

use super::musician::{BarSlot, PhrasePlan, Target};
use super::ornament::{self, Harm};

/// Who's soloing in a trading-fours stretch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Trade {
    #[default]
    None,
    /// The lead's four: it solos, the drums keep time.
    Lead,
    /// The drums' four: the lead and the comp lay out, the bass keeps roots.
    Drums,
}

/// The drums' fill in this bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Fill {
    #[default]
    None,
    /// The last beat.
    Short,
    /// The last two beats.
    Full,
    /// A snare press roll over the last two beats, into a section.
    PressRoll,
}

/// A big moment, cued by the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flourish {
    #[default]
    None,
    /// The harmony switched here (a summon): crash on the one, a fill at the end.
    Summon,
    /// A checkpoint: a short fill.
    Checkpoint,
    /// A death: the lead's comic wah-wah fall.
    Death,
}

/// How a hit figure ends the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HitKind {
    #[default]
    None,
    /// A stab on the last off-beat 8th, anticipating the next bar's chord.
    Anticipation,
    /// Stop-time / ending figure: hits, then the band rests (the lead plays on).
    Ending,
}

/// The band's reharmonization of part of a bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sub {
    /// Beats from the bar line.
    pub from: f64,
    pub to: f64,
    pub chord: Chord,
    pub kind: SubKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubKind {
    /// A dominant replaced by the one a tritone away.
    Tritone,
    /// A dominant into the tonic replaced by the backdoor ii-V (iv-7, bVII7).
    Backdoor,
}

/// The shared plan of one bar.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BandPlan {
    /// The absolute bar.
    pub bar: u64,
    /// Hit positions, bit `k` = the `k`-th 16th of the bar.
    pub hits: u16,
    pub hit_kind: HitKind,
    /// Up to two substitutions (the backdoor's ii and V).
    pub subs: [Option<Sub>; 2],
    pub trade: Trade,
    pub fill: Fill,
    /// A crash on the downbeat (after a fill, or a summon).
    pub crash: bool,
    pub flourish: Flourish,
    /// The previous bar anticipated this one's downbeat (comp and bass don't re-attack it).
    pub anticipated: bool,
    /// Reserved for feels (see the module docs): 0 = the song's own.
    pub feel: u8,
}

impl BandPlan {
    /// The substitute chord at `beat` (from the bar line), if any.
    pub fn sub_at(&self, beat: f64) -> Option<&Sub> {
        self.subs.iter().flatten().find(|s| beat >= s.from - 1e-9 && beat < s.to - 1e-9)
    }

    /// Hit positions as beats from the bar line.
    pub fn hit_beats(&self) -> impl Iterator<Item = f64> + '_ {
        (0..16).filter(move |k| self.hits & (1 << k) != 0).map(|k| k as f64 * 0.25)
    }
}

/// What the plan is decided from.
pub struct BandInput<'a> {
    pub seed: u64,
    pub slot: BarSlot,
    /// The bar's length in beats.
    pub bar_beats: f64,
    /// Bars in the song (this shape).
    pub bars: usize,
    /// The phrase this bar belongs to (its shape: where it ends).
    pub phrase: PhrasePlan,
    /// Freedom of the lead, comp, bass, drums.
    pub freedom: [f32; 4],
    /// The harmony at a beat of this bar (from the bar line); `None` without a chart.
    pub harm: &'a dyn Fn(f64) -> Option<Harm>,
    /// The home key's tonic.
    pub key: u8,
    /// The harmony switched at this bar (a summon).
    pub switched: bool,
    /// Cues from the game for this bar.
    pub checkpoint: bool,
    pub death: bool,
    /// The previous bar's plan (fills crash into this one; anticipations tie over).
    pub prev: &'a BandPlan,
}

/// 0 below `lo`, 1 above `hi`, linear between: how far a dial is into a tier.
pub fn ramp(f: f32, lo: f32, hi: f32) -> f64 {
    ((f - lo) / (hi - lo)).clamp(0.0, 1.0) as f64
}

/// The tiers of the ornament table: low (0.1-0.3), mid (0.3-0.6), high (0.6-1).
pub fn low(f: f32) -> f64 {
    if f <= 0.0 { 0.0 } else { ramp(f, 0.04, 0.25) }
}
pub fn mid(f: f32) -> f64 {
    ramp(f, 0.3, 0.6)
}
pub fn high(f: f32) -> f64 {
    ramp(f, 0.6, 0.9)
}

/// A deterministic RNG for decision `what` about bar `bar`.
pub fn rng(seed: u64, who: usize, bar: u64, what: u64) -> Rng {
    Rng::new(tuning::salt(seed ^ 0xBA4D, who, bar as usize, what as usize))
}

/// The drums' fill for a bar, from its place in the song (used by the plan and shown ahead in
/// the drummer's own plan).
pub fn fill_for(seed: u64, bar: u64, song_bar: usize, bars: usize, phrase_last: bool, drums: f32) -> Fill {
    if drums <= 0.0 {
        return Fill::None;
    }
    let mut r = rng(seed, 3, bar, 1);
    let next = song_bar + 1;
    let section = next % 8 == 0 || next == bars;
    let four = next % 4 == 0;
    let roll = r.f();
    if section && roll < 0.5 * high(drums) {
        return Fill::PressRoll;
    }
    let p = if section {
        0.3 + 0.5 * low(drums) * (0.5 + 0.5 * mid(drums))
    } else if four {
        0.08 + 0.5 * mid(drums)
    } else if phrase_last {
        0.25 * mid(drums)
    } else {
        0.0
    };
    if roll < p {
        if mid(drums) > 0.0 && r.chance(0.5 + 0.5 * mid(drums)) || section { Fill::Full } else { Fill::Short }
    } else {
        Fill::None
    }
}

/// Is this bar in a trading stretch, and whose four is it? Decided per 8-bar section.
pub fn trade_for(seed: u64, pass: u64, song_bar: usize, bars: usize, lead: f32, drums: f32) -> Trade {
    let section = song_bar / 8;
    if (section + 1) * 8 > bars {
        return Trade::None;
    }
    let p = 0.45 * high(lead.min(drums));
    let mut r = rng(seed, 4, pass * 1000 + section as u64, 2);
    if p <= 0.0 || !r.chance(p) {
        return Trade::None;
    }
    if song_bar % 8 < 4 { Trade::Lead } else { Trade::Drums }
}

impl BandPlan {
    /// Decide the plan of `input.slot`.
    pub fn decide(input: &BandInput) -> BandPlan {
        let [lead, comp, bass, drums] = input.freedom;
        let slot = input.slot;
        let mut plan = BandPlan { bar: slot.index, anticipated: input.prev.hit_kind == HitKind::Anticipation && input.prev.bar + 1 == slot.index, ..BandPlan::default() };
        if input.freedom.iter().all(|f| *f <= 0.0) {
            return plan;
        }
        let mut r = rng(input.seed, 5, slot.index, 0);
        let bb = input.bar_beats;
        let sixteenths = (bb * 4.0).round() as u32;
        let phrase_last = input.phrase.last_bar() == slot.index;
        let loop_end = slot.song_bar + 1 == input.bars;

        plan.trade = trade_for(input.seed, slot.pass, slot.song_bar, input.bars, lead, drums);

        // Fills, and the crash after one.
        plan.fill = fill_for(input.seed, slot.index, slot.song_bar, input.bars, phrase_last, drums);
        plan.crash = drums > 0.0 && input.prev.bar + 1 == slot.index && input.prev.fill != Fill::None && mid(drums) + low(drums) > 0.5;
        if plan.trade == Trade::Drums {
            plan.fill = Fill::Full;
        }

        // Flourishes.
        if input.switched && drums > 0.0 {
            plan.flourish = Flourish::Summon;
            plan.crash = true;
            plan.fill = Fill::Full;
        } else if input.death && lead > 0.0 {
            plan.flourish = Flourish::Death;
        } else if input.checkpoint && drums > 0.0 {
            plan.flourish = Flourish::Checkpoint;
            if plan.fill == Fill::None {
                plan.fill = Fill::Short;
            }
        }

        // Hits: the comp's anticipation (the bass and drums join it), or an ending figure.
        let harm_now = (input.harm)(0.0);
        let harm_next = (input.harm)(bb);
        let changes = match (harm_now, harm_next) {
            (Some(a), Some(b)) => a.chord != b.chord,
            _ => false,
        };
        if comp > 0.0 && plan.trade != Trade::Drums && !loop_end {
            let ending = phrase_last && matches!(input.phrase.target, Target::SectionEnd | Target::Cadence);
            let p_end = 0.55 * mid(comp.min(bass.max(drums)));
            let p_ant = 0.3 * low(comp) * if changes { 1.0 } else { 0.3 };
            let big_fill = matches!(plan.fill, Fill::Full | Fill::PressRoll);
            if ending && r.f() < p_end && plan.fill != Fill::PressRoll {
                // Charleston hits (1, the "and" of 2), or 3 and the "and" of 4.
                plan.hits = if r.chance(0.5) || sixteenths < 16 { 1 | 1 << 6 } else { 1 << 8 | 1 << 14 };
                plan.hit_kind = HitKind::Ending;
                // The figure is the fill.
                if plan.flourish != Flourish::Summon {
                    plan.fill = Fill::None;
                }
            } else if harm_next.is_some() && !big_fill && r.f() < p_ant {
                plan.hits = 1 << (sixteenths - 2);
                plan.hit_kind = HitKind::Anticipation;
            }
        }

        // The band's reharmonization: comp and bass together, high freedom.
        if comp >= 0.6 && bass >= 0.3 && plan.trade != Trade::Drums {
            let p = 0.5 * high(comp);
            if let Some(h) = harm_now
                && h.chord.family() == Family::Dominant
                && r.chance(p)
            {
                // The span of this dominant inside the bar.
                let mut to = bb;
                for k in 1..(bb * 2.0).round() as usize {
                    let b = k as f64 * 0.5;
                    if (input.harm)(b).is_some_and(|x| x.chord != h.chord) {
                        to = b;
                        break;
                    }
                }
                let resolves_home = harm_next.is_some_and(|x| x.chord.root == input.key && x.chord.family() == Family::Major);
                if resolves_home && to >= bb - 1e-9 && r.chance(0.4) {
                    let (ii, v) = ornament::backdoor(input.key);
                    let half = (bb / 2.0).floor();
                    plan.subs = [
                        Some(Sub { from: 0.0, to: half, chord: ii, kind: SubKind::Backdoor }),
                        Some(Sub { from: half, to: bb, chord: v, kind: SubKind::Backdoor }),
                    ];
                } else {
                    let mut c = ornament::tritone_sub(&h.chord);
                    if h.chord.quality == Quality::Dom9 {
                        c.quality = Quality::Dom9;
                    }
                    plan.subs[0] = Some(Sub { from: 0.0, to, chord: c, kind: SubKind::Tritone });
                }
            }
        }
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_ramp_in_order() {
        assert_eq!((low(0.0), mid(0.0), high(0.0)), (0.0, 0.0, 0.0));
        assert!(low(0.2) > 0.5 && mid(0.2) == 0.0);
        assert!(low(0.35) == 1.0 && mid(0.35) > 0.0 && high(0.35) == 0.0);
        assert!(mid(0.7) == 1.0 && high(0.7) > 0.0);
    }

    #[test]
    fn trades_cover_whole_sections() {
        let mut seen = false;
        for seed in 0..40 {
            let t: Vec<Trade> = (0..16).map(|b| trade_for(seed, 0, b, 32, 1.0, 1.0)).collect();
            if t[0] != Trade::None {
                seen = true;
                assert!(t[..4].iter().all(|x| *x == Trade::Lead) && t[4..8].iter().all(|x| *x == Trade::Drums), "{t:?}");
            }
            assert!((0..32).all(|b| trade_for(seed, 0, b, 32, 0.5, 1.0) == Trade::None));
        }
        assert!(seen);
    }
}

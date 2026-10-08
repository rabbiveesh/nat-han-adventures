//! Feels: the band, loose enough, plays a section in another groove (bossa nova, samba, rock,
//! funk) and comes back to the tune's own (swing, or the waltz's). Music only: no physics, no
//! director, nothing the player summons.
//!
//! # Who chooses, and when
//! The band itself, in its shared plan ([`super::band::BandPlan::feel`]), decided bar by bar
//! like the trading fours, as a pure function of the seed, the bar and the dials ([`auto`]):
//! - the dial is the rhythm section's mean freedom ([`dial`]: comp, bass, drums): no feels up
//!   to [`MIN_DIAL`] (0.4), so freedom 0 and the game's neutral 0.35 never change;
//! - the timeline is cut into 8-bar sections, counted across loop passes, and those into slots
//!   of three ([`SLOT_SECTIONS`]): a slot's first section is always the tune's own feel (so the
//!   very first section is, and there's a section of it between two feels); with probability
//!   [`chance`] (0.36 at 0.6, 0.9 at 0.9) one of the other two sections, or both (8 or 16
//!   bars), plays a feel. A slot never repeats the feel the slot before it rolled. Sections
//!   that aren't whole (a song's odd tail) never feel;
//! - a feel only starts at its first bar (a feel the band missed the start of, e.g. after a
//!   waltz or a jump into the song, waits for the next), and the drums announce it: a full fill
//!   in the bar before ([`super::band::Fill::Full`]) and a crash into it, a fill back out and a
//!   crash home again (no trading, no ending figures in those bars);
//! - [`super::engine::Input::ForceFeel`] (the editor's FORCE FEEL chips) overrides it:
//!   `Some(Swing)` never feels, `Some(feel)` plays it from the next bar committed, at any
//!   freedom (at freedom 0 each player realizes it plainly).
//!
//! # The waltz
//! No feels in the waltz: a 3/4 bossa is a different tune. A bar in the waltz's shape is always
//! [`Feel::Swing`] (the waltz's own feel); a waltz that comes in mid-feel ends it at its bar
//! line (the waltz's own summon fill and crash announce the change), and a feel the waltz
//! interrupted doesn't resume when it ends: the band waits for the next one.
//!
//! # How each player realizes it
//! Each feel is a rhythm transform of the bar per player (like the comp's Charleston or the
//! bass's walking: one bar at a time), on instruments from the feel's palette, with the harmony
//! coloured where the style wants it. All four are straight: [`super::musician::Ctx::swing8`]
//! stops swinging in a feel, so every ornament (pickups, fills, solos) goes straight with it,
//! and the lead's written line is un-swung ([`super::musician::Ctx::straighten`]).
//! - **Bossa nova**: the drums play the 3-2 bossa clave on the rim ([`BOSSA_CLAVE`], a two-bar
//!   pattern), a soft shaker on the 8ths and a light kick (1, the "and" of 2, 3, the "and" of
//!   4); the bass a root-fifth two-feel (root on 1, fifth on 3, the next bar's root anticipated
//!   on the "and" of 4 and tied over); the comp João Gilberto's syncopated batida
//!   ([`BOSSA_COMP`]) in maj9 / 6/9 / m9 / 7(13) voicings ([`voicing`]) on a soft nylon pluck;
//!   the lead on a softer instrument; the whole band a notch quieter.
//! - **Samba**: the same harmony at double the energy, the bar subdivided in 16ths: the ganzá
//!   (hats) on every 16th with accents, the tamborim's teleco-teco ([`SAMBA_TAMBORIM`]), the
//!   kick heavy on 2 and 4 and a cuíca's "oo-EE" now and then (two pitched hits, two kits);
//!   the bass a surdo: a light root on 1 and 3 with a 16th pickup, the big hit on 2 and 4; the
//!   comp the partido-alto ([`PARTIDO_ALTO`]) on a bright cavaquinho; the lead brassier.
//! - **Rock**: straight 8ths, kick on 1 and 3, a hard backbeat on 2 and 4, 8th hats, a crash at
//!   section starts; the bass pumps 8th-note roots; the comp power chords (root, fifth,
//!   octave: every chord simplified to its root's, [`power`]) palm-muted on a gritty 12.5%
//!   duty with the odd ringing chord; the lead overdriven (narrow duty, vibrato, a scoop), with
//!   bluesy bends ([`super::ornament::Orn::Bend`]).
//! - **Funk**: a straight 16th grid: 16th hats with accents and the odd open hat, a ghost-note
//!   snare around the backbeat, a syncopated kick ([`FUNK_KICKS`]); the bass locked to the
//!   kick, popping octaves with dead notes (muted triangle blips) between; the comp short clav
//!   stabs on the 12.5% duty, 16th syncopation ([`FUNK_CLAV`]), dominants as 7#9 or 9; the lead
//!   punchy (short notes) with horn-section stabs at its phrase ends.
//!
//! Everything still runs through the band's plan on top: the reharmonization (the comp voices
//! the substitute plainly, the bass plays its roots), hits, fills, the filters' harmony
//! (Coltrane's changes get bossa'd too) and the tunings (the voices tune whatever comes out).
//!
//! # Instruments
//! A feel plays its own instruments: each song may list them per feel and channel in its
//! `[instruments]` (`bossa.pulse1 = clarinet flute`, see [`super::instrument`]); a channel a
//! song doesn't list plays the shared built-in set ([`BUILTIN`], [`DEFAULTS`]), added to the
//! engine's table by [`equip`] (a song's own instrument of the same name wins, so a song can
//! re-voice `nylon`). The players' extra sounds (the rock comp's mute and ring, the funk bass's
//! dead notes, the cuíca) are looked up by name the same way ([`Extra`]).

use crate::audio::accomp::Rng;
use crate::audio::chart::{Chord, Family, Quality};
use crate::audio::mml::Arp;

use super::band::{self, BandInput};
use super::instrument::{Def, Instruments};

/// A band-wide groove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Feel {
    /// The tune's own feel: swing (or, in the waltz, the waltz's).
    #[default]
    Swing,
    Bossa,
    Samba,
    Rock,
    Funk,
}

impl Feel {
    pub const ALL: [Feel; 5] = [Feel::Swing, Feel::Bossa, Feel::Samba, Feel::Rock, Feel::Funk];
    /// The feels the band can switch to.
    pub const OTHERS: [Feel; 4] = [Feel::Bossa, Feel::Samba, Feel::Rock, Feel::Funk];

    /// For the HUD's band readout ("" for the tune's own).
    pub fn label(self) -> &'static str {
        match self {
            Feel::Swing => "",
            Feel::Bossa => "BOSSA NOVA",
            Feel::Samba => "SAMBA",
            Feel::Rock => "ROCK",
            Feel::Funk => "FUNK",
        }
    }

    /// Its name in `.song` files and on the command line.
    pub fn slug(self) -> &'static str {
        match self {
            Feel::Swing => "swing",
            Feel::Bossa => "bossa",
            Feel::Samba => "samba",
            Feel::Rock => "rock",
            Feel::Funk => "funk",
        }
    }

    pub fn parse(s: &str) -> Option<Feel> {
        Feel::ALL.into_iter().find(|f| f.slug() == s)
    }

    /// Index into [`Feel::OTHERS`] (`None` for the tune's own).
    pub fn other_index(self) -> Option<usize> {
        (self as usize).checked_sub(1)
    }
}

/// No feels at or below this dial.
pub const MIN_DIAL: f32 = 0.4;
/// Sections (8 bars) per slot: the first always the tune's own feel.
pub const SLOT_SECTIONS: u64 = 3;

/// The band's dial for feels: the rhythm section's mean freedom (comp, bass, drums).
pub fn dial(freedom: [f32; 4]) -> f32 {
    (freedom[1] + freedom[2] + freedom[3]) / 3.0
}

/// The chance a slot (three sections) gets a feel: none to [`MIN_DIAL`], then rising.
pub fn chance(dial: f32) -> f64 {
    0.9 * band::ramp(dial, MIN_DIAL, 0.9)
}

/// A slot's roll: its coin, whether the feel is 16 bars or 8 (and then which section), which
/// feel (never the one the slot before rolled).
fn roll(seed: u64, slot: u64) -> (f64, bool, bool, Feel) {
    let pick = |s: u64| {
        let mut r = band::rng(seed, 6, s, 0);
        let (coin, long, late) = (r.f(), r.chance(0.5), r.chance(0.5));
        (coin, long, late, r.below(4), r)
    };
    let (coin, long, late, mut k, mut r) = pick(slot);
    if slot > 0 && pick(slot - 1).3 == k {
        k = (k + 1 + r.below(3)) % 4;
    }
    (coin, long, late, Feel::OTHERS[k])
}

/// The feel the band picks for itself in bar `song_bar` of loop pass `pass` (a song of `bars`
/// bars), at `dial`: see the module docs. Pure; monotone in the dial (a higher dial keeps every
/// feel a lower one had).
pub fn auto(seed: u64, pass: u64, song_bar: usize, bars: usize, dial: f32) -> Feel {
    let p = chance(dial);
    let section = song_bar / 8;
    if p <= 0.0 || (section + 1) * 8 > bars {
        return Feel::Swing;
    }
    let abs = pass * bars.div_ceil(8) as u64 + section as u64;
    let (slot, k) = (abs / SLOT_SECTIONS, abs % SLOT_SECTIONS);
    if k == 0 {
        return Feel::Swing;
    }
    let (coin, long, late, feel) = roll(seed, slot);
    let on = coin < p && (long || k == if late { 2 } else { 1 });
    if on { feel } else { Feel::Swing }
}

/// A bar's feel, decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Call {
    pub feel: Feel,
    /// The bar it started (this one, for the tune's own).
    pub since: u64,
    /// The next bar's (as it stands): a different one means a fill.
    pub next: Feel,
}

/// The feel of `input.slot` (see the module docs: a feel starts only at its first bar, never in
/// the waltz; a force overrides the band).
pub fn decide(input: &BandInput) -> Call {
    let s = input.slot;
    if input.waltz {
        return Call { feel: Feel::Swing, since: s.index, next: Feel::Swing };
    }
    let d = dial(input.freedom);
    let want = |pass: u64, bar: usize| input.force_feel.unwrap_or_else(|| auto(input.seed, pass, bar, input.bars, d));
    let now = want(s.pass, s.song_bar);
    let before = if s.song_bar > 0 {
        Some(want(s.pass, s.song_bar - 1))
    } else if s.pass > 0 {
        Some(want(s.pass - 1, input.bars - 1))
    } else {
        None
    };
    let contiguous = input.prev.bar + 1 == s.index;
    let carried = contiguous && input.prev.feel == now;
    let starts = input.force_feel.is_some() || before != Some(now);
    let feel = if now == Feel::Swing || carried || starts { now } else { Feel::Swing };
    let since = if carried && feel != Feel::Swing { input.prev.feel_since } else { s.index };
    let mut next = if s.song_bar + 1 < input.bars {
        want(s.pass, s.song_bar + 1)
    } else if input.looping {
        want(s.pass + 1, 0)
    } else {
        Feel::Swing
    };
    // A feel missed at its start doesn't start next bar either.
    if feel != now && next == now {
        next = feel;
    }
    Call { feel, since, next }
}

// --- the patterns -----------------------------------------------------------------------

/// The 3-2 bossa clave, beats from the bar line: the first bar of a pair, and the second.
pub const BOSSA_CLAVE: [&[f64]; 2] = [&[0.0, 1.5, 3.0], &[1.0, 2.5]];
/// The bossa kick: 1 and 3 (a pair's first bar: 1, the "and" of 2, 3).
pub const BOSSA_KICK: [&[f64]; 2] = [&[0.0, 1.5, 2.0], &[0.0, 2.0]];
/// João Gilberto's batida (the guitar's chords; the thumb is the bass), a two-bar pattern.
pub const BOSSA_COMP: [&[f64]; 2] = [&[0.0, 1.0, 2.5, 3.5], &[0.5, 2.0, 3.0]];
/// The tamborim's teleco-teco, 16ths of the bar; at a fast tempo ([`fast`]) thinned out.
pub const SAMBA_TAMBORIM: [u8; 7] = [0, 2, 5, 7, 9, 12, 14];
pub const SAMBA_TAMBORIM_FAST: [u8; 4] = [0, 5, 9, 14];
/// The partido-alto (the cavaquinho's chords), 16ths of the bar.
pub const PARTIDO_ALTO: [u8; 6] = [1, 4, 6, 8, 11, 14];
/// The funk vamps: two-bar riffs (16ths of each bar), one per feel (fixed while it lasts, so
/// it grooves): the kick, which the bass doubles (the one long and hard, rests between), the
/// clav's few stabs, the ghost note (the second bar only) and the bass's octave pop.
pub struct Vamp {
    pub kick: [&'static [u8]; 2],
    pub clav: [&'static [u8]; 2],
    pub ghost: u8,
    pub pop: [Option<u8>; 2],
}

pub const FUNK_VAMPS: [Vamp; 3] = [
    Vamp { kick: [&[0, 10], &[0, 7, 10]], clav: [&[6, 14], &[3, 6, 11]], ghost: 9, pop: [Some(14), None] },
    Vamp { kick: [&[0, 6], &[0, 3, 10]], clav: [&[3, 11], &[6, 14]], ghost: 7, pop: [Some(10), Some(14)] },
    Vamp { kick: [&[0, 3, 8], &[0, 10]], clav: [&[6, 12], &[2, 6, 14]], ghost: 15, pop: [None, Some(7)] },
];
/// The funk snare's backbeat (16ths 4 and 12).
pub const FUNK_BACKBEAT: [u8; 2] = [4, 12];

/// The funk vamp for a feel that started at bar `since`, and which of its two bars `bar` is.
pub fn vamp(seed: u64, bar: u64, since: u64) -> (&'static Vamp, usize) {
    (&FUNK_VAMPS[pattern(seed, since, FUNK_VAMPS.len())], !first_of_pair(bar, since) as usize)
}

/// A fast tune (bossa goes half-time, the samba thins out, so they stay relaxed).
pub fn fast(bpm: f32) -> bool {
    bpm >= 160.0
}

/// The hits of a two-bar pattern (`pat`, beats of each bar) in bar `bar` of a feel that started
/// at `since`; `half`: half-time, the pattern spread over four bars (twice as slow).
pub fn two_bar(pat: [&[f64]; 2], bar: u64, since: u64, half: bool) -> ([f64; 8], usize) {
    let mut out = [0.0; 8];
    let mut n = 0;
    let j = bar - since.min(bar);
    let (which, from) = if half { (((j / 2) % 2) as usize, (j % 2) as f64 * 4.0) } else { ((j % 2) as usize, 0.0) };
    let k = if half { 2.0 } else { 1.0 };
    for &b in pat[which] {
        let x = b * k - from;
        if (0.0..4.0 - 1e-9).contains(&x) && n < 8 {
            out[n] = x;
            n += 1;
        }
    }
    (out, n)
}

/// Which pattern of a list bar `bar` plays (the same for every player: they lock together).
pub fn pattern(seed: u64, bar: u64, n: usize) -> usize {
    band::rng(seed, 7, bar, 0).below(n)
}

/// Is `song_bar` the first of a two-bar pattern pair (the clave's "3" side)? Counted from the
/// feel's start, so a feel always opens on it.
pub fn first_of_pair(bar: u64, since: u64) -> bool {
    (bar - since.min(bar)).is_multiple_of(2)
}

// --- harmony ------------------------------------------------------------------------------

/// The intervals (above the root) a feel colours a chord with: bossa's maj9, 6/9, m9 and
/// 7(13); funk's 7#9 or 9 (`alt` picks), m9, 6/9. `None`: the chord as it is.
fn colour(chord: &Chord, feel: Feel, alt: bool) -> Option<&'static [u8]> {
    use Quality::*;
    let q = chord.quality;
    Some(match (feel, q) {
        (Feel::Bossa, Major | Six) => &[4, 7, 9, 14],
        (Feel::Bossa, Maj7) => &[4, 7, 11, 14],
        (Feel::Bossa, Dom7 | Dom9) => &[4, 10, 14, 21],
        (Feel::Bossa, Dom7b9) => &[4, 10, 13, 21],
        (Feel::Bossa, Sus4) => &[5, 7, 10, 14],
        (Feel::Bossa, Minor | Minor7) => &[3, 7, 10, 14],
        (Feel::Bossa, Minor6) => &[3, 7, 9, 14],
        (Feel::Bossa, MinMaj7) => &[3, 7, 11, 14],
        (Feel::Funk, Dom7 | Dom9) if alt => &[4, 10, 15],
        (Feel::Funk, Dom7 | Dom9) => &[4, 10, 14, 19],
        (Feel::Funk, Dom7s9) => &[4, 10, 15],
        (Feel::Funk, Dom7b9) => &[4, 10, 13],
        (Feel::Funk, Sus4) => &[5, 10, 14],
        (Feel::Funk, Minor | Minor7 | Minor6) => &[3, 10, 14],
        (Feel::Funk, Major | Six | Maj7) => &[4, 9, 14],
        _ => return None,
    })
}

/// A chord voiced for a feel around `center` (close position, lowest first): coloured for bossa
/// and funk, a power chord for rock, else [`super::ornament::voice`].
pub fn voicing(chord: &Chord, feel: Feel, center: u8, alt: bool) -> Arp {
    if feel == Feel::Rock {
        return power(chord, center);
    }
    let Some(iv) = colour(chord, feel, alt) else { return super::ornament::voice(chord, center, 0) };
    let lo = center as i32 - 7;
    let mut notes = [0u8; 6];
    for (x, i) in notes.iter_mut().zip(iv) {
        let pc = (chord.root + i) as i32 % 12;
        *x = (lo + (pc - lo).rem_euclid(12)).clamp(0, 127) as u8;
    }
    let n = iv.len().min(6);
    notes[..n].sort_unstable();
    Arp::new(&notes[..n])
}

/// The rock simplification: the root's power chord (root, fifth, octave) a little below
/// `center`; a diminished or augmented chord keeps just its root in octaves (its fifth isn't
/// perfect).
pub fn power(chord: &Chord, center: u8) -> Arp {
    let lo = center as i32 - 14;
    let r = (lo + (chord.root as i32 - lo).rem_euclid(12)).clamp(0, 115) as u8;
    match chord.family() {
        Family::HalfDim | Family::Dim | Family::Aug => Arp::new(&[r, r + 12]),
        _ => Arp::new(&[r, r + 7, r + 12]),
    }
}

/// Pitch classes a feel's comp may sound over `chord` (the chord's, its colours, and for a
/// dominant the altered 9ths): for the tests and the editor.
pub fn allowed_pcs(chord: &Chord, feel: Feel) -> Vec<u8> {
    let mut v: Vec<u8> = chord.pitch_classes().collect();
    if feel == Feel::Rock {
        v = vec![chord.root, (chord.root + 7) % 12];
        return v;
    }
    for alt in [false, true] {
        if let Some(iv) = colour(chord, feel, alt) {
            v.extend(iv.iter().map(|i| (chord.root + i) % 12));
        }
    }
    v
}

// --- instruments --------------------------------------------------------------------------

/// The built-in feel instruments (the same text a song's `[instruments]` takes).
pub const BUILTIN: &str = "\
flute     : tri | vol 7 10 12 12 11 | vib delay=14 depth=14 speed=5      ; bossa: a soft flute (the triangle's wave)
nylon     : vol 12 10 8 7 6 5 4 4 3 | duty 1 2                         ; bossa: a nylon-string pluck
softbass  : vol 13 12 10 9 8 7 7 6                                      ; bossa: a round upright
bossakit  : kick hz=52 decay=4 click=0.05 | snare noise=short period=1 decay=0.5 tone=1150 body=0.7 | hat decay=0.8 period=1 | ohat decay=2.5 period=1 | crash decay=10 ; bossa: rim click, shaker
brass     : vol 9 12 14 15 15 14 13 | duty 0 1 2 | pitch -1 -0.5 -0.25 0 | vib delay=14 depth=14 speed=5.5 ; samba: a trumpet
cavaco    : vol 15 11 8 5 4 3 2 1 | duty 2 1                            ; samba: a bright cavaquinho
surdo     : vol 15 14 12 10 8 7 6 5 | pitch -0.5 -0.25 0                 ; samba: the surdo's thump
sambakit  : kick pitch=-14 hz=44 decay=7 click=0.1 | snare noise=short period=1 decay=1 tone=880 body=1.2 | hat decay=0.7 period=0 | ohat decay=3 period=0 | crash decay=14 ; samba: tamborim, ganza
cuica_lo  : snare noise=short period=0 decay=0.5 tone=420 body=5        ; samba: the cuica's oo
cuica_hi  : snare noise=short period=0 decay=0.5 tone=700 body=4        ; samba: and its EE
drive     : vol 14 15 15 15 14 14 13 | duty 0 | pitch -0.5 -0.25 0 | vib delay=10 depth=24 speed=6.5 ; rock: an overdriven lead
crunch    : vol 15 13 12 11 10 9 9 8 | duty 0                           ; rock: a power chord's crunch
mute      : vol 15 9 5 2 0 | duty 0                                     ; rock: palm-muted
ring      : vol 15 14 13 12 12 11 10 10 9 | duty 0 1 0 | vib delay=24 depth=6 speed=5 ; rock: a ringing chord
rockbass  : vol 15 13 12 11 10 10 9                                     ; rock: a picked bass
rockkit   : kick pitch=-30 decay=5 click=0.6 | snare period=3 decay=9 tone=175 body=3 | hat decay=0.6 | ohat decay=4 | crash decay=20 ; rock: a hard-hitting kit
horn      : vol 11 15 15 13 12 11 | duty 1 2 | pitch -0.5 0             ; funk: a punchy horn
clav      : vol 15 11 7 4 2 0 | duty 0                                  ; funk: a clav stab
slap      : vol 15 13 10 8 7 6 5                                        ; funk: a slapped bass
dead      : vol 12 4 0                                                  ; funk: a dead note
funkkit   : kick pitch=-28 decay=3.5 click=0.5 | snare noise=short period=2 decay=4 tone=210 body=1.5 | hat decay=0.5 period=0 | ohat decay=3 | crash decay=16 ; funk: a tight kit
";

/// Each feel's default palette per channel (pulse 1, pulse 2, triangle, noise), by name.
pub const DEFAULTS: [[&str; 4]; 4] = [
    ["flute", "nylon", "softbass", "bossakit"],
    ["brass", "cavaco", "surdo", "sambakit"],
    ["drive", "crunch", "rockbass", "rockkit"],
    ["horn", "clav", "slap", "funkkit"],
];

/// The players' extra sounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Extra {
    /// The rock comp's palm mute, and its ringing chord.
    Mute,
    Ring,
    /// The funk bass's dead notes.
    Dead,
    /// The cuíca's two pitches (kits).
    CuicaLo,
    CuicaHi,
}

impl Extra {
    pub const ALL: [Extra; 5] = [Extra::Mute, Extra::Ring, Extra::Dead, Extra::CuicaLo, Extra::CuicaHi];

    pub fn name(self) -> &'static str {
        match self {
            Extra::Mute => "mute",
            Extra::Ring => "ring",
            Extra::Dead => "dead",
            Extra::CuicaLo => "cuica_lo",
            Extra::CuicaHi => "cuica_hi",
        }
    }
}

/// A song's instruments with the feels' added: every feel and channel the song doesn't give a
/// palette gets the built-in one ([`DEFAULTS`]), and the [`Extra`]s are resolved. Built-ins
/// are appended (a song's own numbers don't move) unless the song has an instrument of the
/// same name and kind, which is used instead.
pub fn equip(song: &Instruments) -> Instruments {
    let builtin = Instruments::parse(BUILTIN).expect("the built-in feel instruments parse");
    let mut out = song.clone();
    let need = |out: &mut Instruments, name: &str| -> u8 {
        let k = builtin.index(name).expect("a built-in");
        let def = builtin.defs[k as usize - 1];
        let kit = matches!(def, Def::Kit(_));
        if let Some(i) = out.index(name).filter(|&i| i > 0 && out.is_kit(i) == kit) {
            return i;
        }
        // Not the song's: add it (under a name of its own if the song's means something else).
        let mut n = name.to_string();
        while out.index(&n).is_some() {
            n.insert(0, '_');
        }
        out.names.push(n);
        out.defs.push(def);
        out.names.len() as u8
    };
    for (f, names) in DEFAULTS.iter().enumerate() {
        for (ch, name) in names.iter().enumerate() {
            if out.feel_palettes[f][ch].is_empty() {
                let i = need(&mut out, name);
                out.feel_palettes[f][ch].push(i);
            }
        }
    }
    for x in Extra::ALL {
        out.feel_extras[x as usize] = need(&mut out, x.name());
    }
    out
}

/// A deterministic pick of `n` for the feel that started at bar `since` on channel `ch`.
pub fn pick(seed: u64, ch: usize, since: u64, n: usize) -> usize {
    Rng::new(crate::audio::tuning::salt(seed ^ 0xFEE1, ch, since as usize, 3)).below(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::chart::parse_chord;

    #[test]
    fn feels_choose_themselves_deterministically() {
        // The first section, and each slot's first, never feel; more dial, more feels (a
        // superset); none at all to 0.4.
        let bars = 32;
        let count = |seed: u64, d: f32| (0..8).flat_map(|pass| (0..bars).map(move |b| (pass, b))).filter(|&(p, b)| auto(seed, p, b, bars, d) != Feel::Swing).count();
        for seed in 0..30 {
            assert!((0..8).all(|b| auto(seed, 0, b, bars, 1.0) == Feel::Swing));
            assert_eq!(count(seed, 0.4), 0);
            assert_eq!(count(seed, 0.35), 0);
            for (pass, b) in (0..8).flat_map(|pass| (0..bars).map(move |b| (pass, b))) {
                let lo = auto(seed, pass, b, bars, 0.6);
                if lo != Feel::Swing {
                    assert_eq!(auto(seed, pass, b, bars, 0.9), lo, "seed {seed}: monotone");
                }
                // Whole sections.
                assert_eq!(auto(seed, pass, b, bars, 0.8), auto(seed, pass, b - b % 8, bars, 0.8));
            }
        }
        let total = |d: f32| (0..60).map(|s| count(s, d)).sum::<usize>();
        let (lo, mid, hi) = (total(0.5), total(0.6), total(0.9));
        assert!(lo < mid && mid < hi, "{lo} {mid} {hi}");
        // At 0.9 a good share of the time is in a feel, at 0.6 some (of 60 seeds x 8 passes).
        let all = 60 * 8 * bars as usize;
        assert!(hi * 4 > all && mid * 20 > all && mid * 4 < all, "{mid} {hi} of {all}");
        // Every feel turns up.
        for f in Feel::OTHERS {
            assert!((0..60).any(|s| (0..8).any(|p| (0..bars).any(|b| auto(s, p, b, bars, 0.9) == f))), "{f:?}");
        }
        // A partial section never feels.
        assert!((0..60).all(|s| (0..8).all(|p| (32..36).all(|b| auto(s, p, b, 36, 1.0) == Feel::Swing))));
    }

    #[test]
    fn voicings_colour_the_chord() {
        let c = |s: &str| parse_chord(s).unwrap();
        let pcs = |a: Arp| a.notes().iter().map(|n| n % 12).collect::<Vec<_>>();
        // Cmaj7 in bossa: E G B D.
        let mut v = pcs(voicing(&c("Cmaj7"), Feel::Bossa, 62, false));
        v.sort();
        assert_eq!(v, [2, 4, 7, 11]);
        // G7 in bossa: B F A E (3, b7, 9, 13).
        let mut v = pcs(voicing(&c("G7"), Feel::Bossa, 62, false));
        v.sort();
        assert_eq!(v, [4, 5, 9, 11]);
        // G7 in funk: B F A# (7#9).
        let mut v = pcs(voicing(&c("G7"), Feel::Funk, 62, true));
        v.sort();
        assert_eq!(v, [5, 10, 11]);
        // Rock: power chords.
        assert_eq!(power(&c("Dm7"), 62).notes(), [50, 57, 62]);
        assert_eq!(power(&c("Bdim7"), 62).notes(), [59, 71]);
        for name in ["C", "C6", "Cmaj7", "C7", "C9", "C7b9", "C7#9", "C7sus4", "Cm", "Cm7", "Cm6", "CmMaj7", "Cm7b5", "Cdim7", "Caug", "C7#5"] {
            for f in Feel::OTHERS {
                for alt in [false, true] {
                    let ch = c(name);
                    let a = voicing(&ch, f, 64, alt);
                    assert!(a.notes().windows(2).all(|w| w[0] < w[1]), "{name} {f:?}");
                    let ok = allowed_pcs(&ch, f);
                    assert!(a.notes().iter().all(|n| ok.contains(&(n % 12))), "{name} {f:?}: {:?}", a.notes());
                }
            }
        }
    }

    #[test]
    fn the_built_ins_equip_every_song() {
        let song = Instruments::parse("brass : vol 15\nnylon : kick decay=3\npulse1 = default brass\nbossa.pulse1 = brass").unwrap();
        let e = equip(&song);
        // The song's numbers don't move; its brass is the samba lead; its kit named nylon isn't
        // a tone, so the built-in nylon comes in under another name.
        assert_eq!(&e.names[..2], ["brass", "nylon"]);
        assert_eq!(e.palette(0), [0, 1]);
        assert_eq!(e.feel_palette(Feel::Bossa, 0), [1]);
        assert_eq!(e.feel_palette(Feel::Samba, 0), [1]);
        let nylon = e.feel_palette(Feel::Bossa, 1)[0];
        assert_eq!(e.name(nylon), "_nylon");
        assert!(!e.is_kit(nylon));
        assert!(e.is_kit(e.feel_palette(Feel::Funk, 3)[0]));
        assert!(e.is_kit(e.feel_extras[Extra::CuicaHi as usize]) && !e.is_kit(e.feel_extras[Extra::Dead as usize]));
        assert!(e.feel_palette(Feel::Swing, 0).is_empty());
    }
}

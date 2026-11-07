//! The crutch pit (`game::crutches`): the WET FLOOR sign at its bottom, Han's plunger on its
//! rope, Nat's crutches and leg casts (drawn over him), and Nat on crutches for the hospital card.

use super::palette::*;

const CRUTCH_PAL: Palette = &[
    ('k', hex(0x2b1608)), // outline (Nat's own: the hospital card keeps a small palette)
    ('m', hex(0xb8c0cc)), // aluminium
    ('M', hex(0x7d8794)), // shade
    ('g', hex(0x3a3a44)), // rubber pads and grips
    ('w', hex(0xffffff)), // plaster (shaded with the aluminium's shade)
];
/// Two frames: planted, and swung out a pixel at the foot.
const CRUTCHES: [&[&str]; 2] = [
    &[
        "................",
        "................",
        "................",
        "................",
        "ggg..........ggg",
        ".m............m.",
        ".m............m.",
        ".M............M.",
        "gm............mg",
        ".m............m.",
        ".m............m.",
        ".M............M.",
        ".m...kkk.kkk..m.",
        ".m..kwwMk.kwwMk.",
        ".m..kwwMk.kwwMk.",
        ".g...kkk.kkk..g.",
    ],
    &[
        "................",
        "................",
        "................",
        "................",
        "ggg..........ggg",
        ".m............m.",
        ".m............m.",
        ".M............M.",
        "gm............mg",
        ".m............m.",
        "m..............m",
        "M..............M",
        "m....kkk.kkk...m",
        "m...kwwMk.kwwMkm",
        "m...kwwMk.kwwMkm",
        "g....kkk.kkk...g",
    ],
];

pub fn crutches() -> Vec<Pixels> {
    CRUTCHES.iter().map(|g| grid(g, CRUTCH_PAL)).collect()
}

/// Nat on crutches, for the hospital card.
pub fn nat_on_crutches() -> Vec<Pixels> {
    let mut p = super::characters::poo_idle().remove(0);
    p.blit(&grid(CRUTCHES[0], CRUTCH_PAL), 0, 0);
    vec![p]
}

const SIGN_PAL: Palette = &[('k', hex(0x1d1a10)), ('y', hex(0xffd21f)), ('Y', hex(0xd9a400))];
const WET_FLOOR: &[&str] = &[
    "................",
    "................",
    "......kkkk......",
    ".....kyyyyk.....",
    ".....kyykyk.....",
    "....kyyykyyk....",
    "....kyyykyyk....",
    "....kyyykyyk....",
    "...kyyyykyyyk...",
    "...kyyyyyyyyk...",
    "...kyyyykyyyk...",
    "..kyyyyyyyyyyk..",
    "..kYYYYYYYYYYk..",
    "..kYk......kYk..",
    "..kYk......kYk..",
    "..kkk......kkk..",
];

pub fn wet_floor() -> Vec<Pixels> {
    vec![grid(WET_FLOOR, SIGN_PAL)]
}

const PLUNGER_PAL: Palette = &[
    ('k', hex(0x2a1208)),
    ('h', hex(0xc8873f)), // wooden handle
    ('H', hex(0x8f5a24)),
    ('r', hex(0xe0303a)), // the rubber cup
    ('R', hex(0x8e1622)),
    ('l', hex(0xff8a8a)),
];
/// The cup at the bottom (its rim on the bottom row), the handle up to the top (the rope goes on
/// from there).
const PLUNGER: &[&str] = &[
    ".......kk.......",
    "......khHk......",
    "......khHk......",
    "......khHk......",
    "......khHk......",
    "......khHk......",
    "......khHk......",
    "......khHk......",
    ".....kkkkkk.....",
    "....krlrrrRk....",
    "...krlrrrrrRk...",
    "..krlrrrrrrrRk..",
    "..krrrrrrrrrRk..",
    ".krrrrrrrrrrrRk.",
    ".kRRRRRRRRRRRRk.",
    ".kkkkkkkkkkkkkk.",
];

pub fn plunger() -> Vec<Pixels> {
    vec![grid(PLUNGER, PLUNGER_PAL)]
}

#[cfg(test)]
pub fn grids() -> Vec<(&'static str, Vec<&'static str>, Palette)> {
    let mut v: Vec<(&'static str, Vec<&'static str>, Palette)> =
        vec![("WET_FLOOR", WET_FLOOR.to_vec(), SIGN_PAL), ("PLUNGER", PLUNGER.to_vec(), PLUNGER_PAL)];
    for g in CRUTCHES {
        v.push(("CRUTCHES", g.to_vec(), CRUTCH_PAL));
    }
    v
}

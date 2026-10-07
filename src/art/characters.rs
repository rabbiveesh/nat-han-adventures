//! The hero (a brave little poo) and Gus the plumber. Both face right; feet on the bottom row.

use super::palette::*;

// ---------------------------------------------------------------- the poo

pub const POO_PAL: Palette = &[
    ('k', hex(0x2b1608)), // outline
    ('B', hex(0x6b3e1c)), // dark brown (creases, shade)
    ('b', hex(0x9a5b2a)), // brown
    ('l', hex(0xc98a4b)), // highlight
    ('w', hex(0xffffff)), // eyes
    ('r', hex(0xe0303a)), // cape
    ('R', hex(0x8e1622)), // cape shade
    ('p', hex(0xf27a8a)), // tongue
];

/// Body without cape or pupils: tip, top tier (with the eyes), bottom tier (with the smile).
/// Row 0 is left empty so the body can bob up a pixel.
const POO_BODY: &[&str] = &[
    "................",
    ".........kk.....",
    "........kblk....",
    ".......kblbBk...",
    "....kkblbbbbBkk.",
    "...kblbbwbbbwbBk",
    "...kbbbwwwbwwwBk",
    "...kbbbwwwbwwwBk",
    "...kbbbbwbbbwbBk",
    "..kkBBBBBBBBBBkk",
    ".kblbbbbbbbbbbBk",
    ".kbllbbkbbbbkbBk",
    ".kblbbbbkppkbbBk",
    ".kbbbbbbbkkbbbBk",
    "..kBbbbbbbbbbBk.",
    "...kkkkkkkkkkk..",
];
/// Where the eyes are (left column of each 3-wide eye, and the eyes' top row).
const EYE_X: [i32; 2] = [7, 11];
const EYE_Y: i32 = 5;

/// Cape poses, drawn behind the body.
const CAPE_HANG: &[&str] = &[
    "....",
    "....",
    "....",
    "...r",
    "..rr",
    ".rrR",
    ".rRR",
    "rrRR",
    "rRR.",
    "rRR.",
    "rR..",
    "R...",
];
const CAPE_BLOW: &[&str] = &[
    "....",
    "....",
    "....",
    "..rr",
    "rrrr",
    "rRRR",
    "RrRR",
    ".RR.",
];
const CAPE_BLOW2: &[&str] = &[
    "....",
    "....",
    "....",
    "...r",
    ".rrr",
    "rrRR",
    "RRRR",
    "..R.",
];
/// Cape flying upward (falling).
const CAPE_UP: &[&str] = &[
    "....",
    "R...",
    "rR..",
    "rrR.",
    ".rrr",
    "..rr",
];

/// Little feet that poke out under the body while running / jumping.
const FEET_A: &[&str] = &["...kBk....kBk..."];
const FEET_B: &[&str] = &["......kBkkBk...."];
const FEET_C: &[&str] = &[".....kBk....kBk."];
const FEET_D: &[&str] = &["..kBk....kBk...."];

/// Open "O" mouth (jumping / falling), drawn over the smile at (6, 11).
const MOUTH_O: &[&str] = &["bkkkkb", "kppppk", "bkkkkb"];
const BLINK: &[&str] = &["bbb", "bbb", "kkk", "bbb"];

const SPLAT_1: &[&str] = &[
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "........kk......",
    ".......kblk.....",
    "....kkkBBBBkkk..",
    "..kkblbbbbbbbBkk",
    ".kblkkkbbkkkbbBk",
    "kBBBBBBBBBBBBBBk",
    "kbllbbbkkkkbbbBk",
    "kBbbbbbbbbbbbBBk",
    ".kkkkkkkkkkkkkk.",
];
const SPLAT_2: &[&str] = &[
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "..b..........b..",
    "................",
    "................",
    "................",
    "................",
    ".......kk.......",
    "...kkkkblkkkkk..",
    ".kbllkkbbbkkbbk.",
    "kBbbbbbkkkbbbBBk",
    ".kkkkkkkkkkkkkk.",
];
const SPLAT_3: &[&str] = &[
    "................",
    "................",
    "................",
    "B..............B",
    "................",
    "..B..........B..",
    "................",
    "....B......B....",
    "................",
    "................",
    "......kkkk......",
    "...kkkbllbbkkk..",
    "..kbkbkbbkbkbBk.",
    ".kbbbkbbbbkbbbBk",
    "kbbbkbkbbkbkbbBk",
    ".kkkkkkkkkkkkkk.",
];

#[derive(Clone, Copy)]
enum Eyes {
    Right,
    Up,
    Blink,
}

#[derive(Clone, Copy)]
struct PooPose {
    bob: i32,
    cape: &'static [&'static str],
    eyes: Eyes,
    feet: Option<&'static [&'static str]>,
    open_mouth: bool,
    /// Breathe: squash the body down a pixel (drop a bottom-tier row).
    squash: bool,
}

const BASE_POSE: PooPose =
    PooPose { bob: 0, cape: CAPE_HANG, eyes: Eyes::Right, feet: None, open_mouth: false, squash: false };

fn poo(p: PooPose) -> Pixels {
    // Body with face, unshifted.
    let mut body = Pixels::new(16, 16);
    body.blit(&grid(p.cape, POO_PAL), 0, 0);
    body.blit(&grid(POO_BODY, POO_PAL), 0, 0);
    let k = hex(0x2b1608);
    for ex in EYE_X {
        match p.eyes {
            Eyes::Right => body.fill_rect(ex + 1, EYE_Y + 1, 2, 2, k),
            Eyes::Up => body.fill_rect(ex + 1, EYE_Y, 2, 2, k),
            Eyes::Blink => body.blit(&grid(BLINK, POO_PAL), ex, EYE_Y),
        }
    }
    if p.open_mouth {
        body.blit(&grid(MOUTH_O, POO_PAL), 7, 11);
    }
    let mut out = Pixels::new(16, 16);
    for y in 0..16 {
        // Squash: rows above 13 slide down one, row 13 disappears.
        let src = if p.squash && y <= 13 { y - 1 } else { y };
        for x in 0..16 {
            out.set(x, y - p.bob, body.get(x, src));
        }
    }
    if let Some(f) = p.feet {
        out.blit(&grid(f, POO_PAL), 0, 15);
    }
    out
}

pub fn poo_idle() -> Vec<Pixels> {
    vec![poo(BASE_POSE), poo(PooPose { squash: true, ..BASE_POSE })]
}

/// A blink frame, for anyone who wants to sprinkle it into the idle loop.
#[allow(dead_code)]
pub fn poo_blink() -> Pixels {
    poo(PooPose { eyes: Eyes::Blink, ..BASE_POSE })
}

pub fn poo_run() -> Vec<Pixels> {
    vec![
        poo(PooPose { bob: 1, cape: CAPE_BLOW, feet: Some(FEET_A), ..BASE_POSE }),
        poo(PooPose { bob: 1, cape: CAPE_BLOW2, feet: Some(FEET_B), ..BASE_POSE }),
        poo(PooPose { bob: 1, cape: CAPE_BLOW, feet: Some(FEET_C), ..BASE_POSE }),
        poo(PooPose { bob: 1, cape: CAPE_BLOW2, feet: Some(FEET_D), ..BASE_POSE }),
    ]
}

pub fn poo_jump() -> Vec<Pixels> {
    vec![poo(PooPose { bob: 1, cape: CAPE_HANG, eyes: Eyes::Up, feet: Some(FEET_B), ..BASE_POSE })]
}

pub fn poo_fall() -> Vec<Pixels> {
    vec![poo(PooPose { bob: 1, cape: CAPE_UP, eyes: Eyes::Up, feet: Some(FEET_A), open_mouth: true, ..BASE_POSE })]
}

pub fn poo_splat() -> Vec<Pixels> {
    vec![grid(SPLAT_1, POO_PAL), grid(SPLAT_2, POO_PAL), grid(SPLAT_3, POO_PAL)]
}

// ---------------------------------------------------------------- Gus

pub const GUS_PAL: Palette = &[
    ('k', hex(0x1b1420)), // outline, mustache
    ('r', hex(0xe0303a)), // cap, shirt
    ('R', hex(0x7a1a3a)), // plunger cup (maroon, distinct from the cap)
    ('t', hex(0xd8a868)), // plunger handle
    ('s', hex(0xf5b98a)), // skin
    ('S', hex(0xd0805a)), // skin shade (nose)
    ('w', hex(0xffffff)), // eye
    ('u', hex(0x3a6ae0)), // overalls
    ('U', hex(0x203a90)), // overalls shade
    ('y', hex(0xffd94a)), // buttons
    ('n', hex(0x6a3a1a)), // boots
];

/// Head and torso, rows 0..=12 (row 0 empty so he can bob).
const GUS_TOP: &[&str] = &[
    "................",
    "......kkkkk.....",
    ".kkk.krrrrrk....",
    "kRRRkkrrrrrrkk..",
    "kRRRkkrrrrrrrrrk",
    ".ktk.kksssswksk.",
    ".ktk.kkssssssSSk",
    ".ktk..kskkkkkkkk",
    ".ktk...kskkkkk..",
    ".sssrkruuuuurrk.",
    ".ktk.kuuyuuyuusk",
    ".ktk.kuuuuuuuuk.",
    ".ktk.kUuuuuuuUk.",
];
const GUS_LEGS_IDLE: &[&str] = &["..k..kuUk.kUuk..", ".....knnk.knnnk.", ".....kkkk.kkkkk."];
const GUS_LEGS_RUN: [&[&str]; 4] = [
    &["..k.kuUk..kUuk..", "...knnk....knnnk", "...kkkk....kkkkk"],
    &["..k...kuUuk.....", ".......knnnk....", ".......kkkkk...."],
    &["..k.kUuk...kuUk.", "..knnnk....knnk.", "..kkkkk....kkkk."],
    &["..k...kUuuk.....", "......knnnnk....", "......kkkkkk...."],
];
const GUS_LEGS_JUMP: &[&str] = &["..k.kuUk.kUuk...", "...knnk..knnk...", "................"];

/// Assemble Gus: torso + legs; `up` shifts the torso up a pixel (stretching the overalls).
fn gus(legs: &[&str], up: bool) -> Pixels {
    let mut rows: Vec<&str> = Vec::with_capacity(16);
    if up {
        rows.extend_from_slice(&GUS_TOP[1..]);
        rows.push(GUS_TOP[12]);
    } else {
        rows.extend_from_slice(GUS_TOP);
    }
    rows.extend_from_slice(legs);
    grid(&rows, GUS_PAL)
}

pub fn gus_idle() -> Vec<Pixels> {
    // Breathe: second frame squashes the torso down a pixel.
    let a = gus(GUS_LEGS_IDLE, false);
    let mut rows: Vec<&str> = vec![GUS_TOP[0]];
    rows.extend_from_slice(&GUS_TOP[..11]);
    rows.push(GUS_TOP[12]);
    rows.extend_from_slice(GUS_LEGS_IDLE);
    vec![a, grid(&rows, GUS_PAL)]
}

pub fn gus_run() -> Vec<Pixels> {
    GUS_LEGS_RUN.iter().enumerate().map(|(i, l)| gus(l, i % 2 == 1)).collect()
}

pub fn gus_jump() -> Vec<Pixels> {
    vec![gus(GUS_LEGS_JUMP, true)]
}

/// Every hand-authored grid in this module, for validation tests.
#[cfg(test)]
pub fn grids() -> Vec<(&'static str, Vec<&'static str>, Palette)> {
    let mut v: Vec<(&'static str, Vec<&'static str>, Palette)> = vec![
        ("POO_BODY", POO_BODY.to_vec(), POO_PAL),
        ("CAPE_HANG", CAPE_HANG.to_vec(), POO_PAL),
        ("CAPE_BLOW", CAPE_BLOW.to_vec(), POO_PAL),
        ("CAPE_BLOW2", CAPE_BLOW2.to_vec(), POO_PAL),
        ("CAPE_UP", CAPE_UP.to_vec(), POO_PAL),
        ("MOUTH_O", MOUTH_O.to_vec(), POO_PAL),
        ("BLINK", BLINK.to_vec(), POO_PAL),
        ("SPLAT_1", SPLAT_1.to_vec(), POO_PAL),
        ("SPLAT_2", SPLAT_2.to_vec(), POO_PAL),
        ("SPLAT_3", SPLAT_3.to_vec(), POO_PAL),
        ("GUS_LEGS_IDLE", GUS_LEGS_IDLE.to_vec(), GUS_PAL),
        ("GUS_LEGS_JUMP", GUS_LEGS_JUMP.to_vec(), GUS_PAL),
    ];
    for f in [FEET_A, FEET_B, FEET_C, FEET_D] {
        v.push(("FEET", f.to_vec(), POO_PAL));
    }
    let mut top = GUS_TOP.to_vec();
    top.extend_from_slice(GUS_LEGS_IDLE);
    v.push(("GUS_TOP", top, GUS_PAL));
    for l in GUS_LEGS_RUN {
        v.push(("GUS_LEGS_RUN", l.to_vec(), GUS_PAL));
    }
    v
}

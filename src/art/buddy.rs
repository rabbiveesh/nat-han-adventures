//! Han the buddy's poses (so players can read his AI) and the gate markers. Han faces right;
//! feet on the bottom row. Poses are composed from parts: his body (head and torso, no
//! plunger), his plunger (upright, level or as a parachute), arms and legs.

use super::palette::*;

/// Han's palette plus sweat.
pub const BUDDY_PAL: Palette = &[
    ('k', hex(0x1b1420)), // outline, mustache
    ('r', hex(0xe0303a)), // cap, shirt
    ('R', hex(0x7a1a3a)), // plunger cup
    ('t', hex(0xd8a868)), // plunger handle
    ('s', hex(0xf5b98a)), // skin
    ('S', hex(0xd0805a)), // skin shade
    ('w', hex(0xffffff)), // eye
    ('u', hex(0x3a6ae0)), // overalls
    ('U', hex(0x203a90)), // overalls shade
    ('y', hex(0xffd94a)), // buttons
    ('n', hex(0x6a3a1a)), // boots
    ('b', hex(0x9fd8ff)), // sweat
];

/// Head and torso (11 wide, 12 tall): cap, eye, nose, mustache, shirt, overalls.
const BODY: &[&str] = &[
    ".kkkkk.....",
    "krrrrrk....",
    "krrrrrrkk..",
    "krrrrrrrrrk",
    "kksssswksk.",
    "kkssssssSSk",
    ".kskkkkkkkk",
    "..kskkkkk..",
    "kruuuuurrk.",
    "kuuyuuyuusk",
    "kuuuuuuuuk.",
    "kUuuuuuuUk.",
];
/// ...open-mouthed (huffing, yelling).
const BODY_OPEN: &[&str] = &[
    ".kkkkk.....",
    "krrrrrk....",
    "krrrrrrkk..",
    "krrrrrrrrrk",
    "kksssswksk.",
    "kkssssssSSk",
    ".kskkkkkkkk",
    "..kskkwkk..",
    "kruuuuurrk.",
    "kuuyuuyuusk",
    "kuuuuuuuuk.",
    "kUuuuuuuUk.",
];

/// The plunger upright (5 wide): cup on top, then the handle.
const CUP: &[&str] = &[".kkk.", "kRRRk", "kRRRk"];
const HANDLE: &str = ".ktk.";
/// Level, cup forward (7 x 5).
const PLUNGER_LEVEL: &[&str] = &[".....kk", "kkkkkRk", "ttttkRk", "kkkkkRk", ".....kk"];

/// Legs (16 wide, 3 tall), placed under a body at x = 5 (shift them with the body).
const LEGS_WIDE: &[&str] = &["....kuUk...kUuk.", "...knnk....knnnk", "...kkkk....kkkkk"];
const LEGS_AIR: &[&str] = &["....kuUk.kUuk...", "...knnk..knnk...", "................"];
const LEGS_MARCH: [&[&str]; 4] = [
    &["....kuUk..kUuk..", "...knnk....knnnk", "...kkkk....kkkkk"],
    &["......kuUuk.....", ".......knnnk....", ".......kkkkk...."],
    &["....kUuk...kuUk.", "...knnnk...knnk.", "...kkkkk...kkkk."],
    &["......kUuuk.....", "......knnnnk....", "......kkkkkk...."],
];
const LEGS_CROUCH: &[&str] = &["....kuuUkUuuk...", "...knnnk.knnnk..", "...kkkkk.kkkkk.."];

/// The parachute: the plunger cup grown big, strings down to his hands on the handle (16 x 9).
const CANOPY: &[&str] = &[
    "....kkkkkkkk....",
    "..kkRRRRRRRRkk..",
    ".kRRRRRRRRRRRRk.",
    "kRRRRRRRRRRRRRRk",
    "kkkkkkkkkkkkkkkk",
    "k.....ktk......k",
    ".k....ktk.....k.",
    "..k...ktk....k..",
    "...k.sktks..k...",
];

/// Han splatted into a big raft: three 16 x 16 segments, art in the top rows (the surface).
const RAFT_L: &[&str] = &[
    "...kkkkkkkkkkkkk",
    "..kuuuuuuuuuuuuu",
    ".knnkuuyuuuuuuuu",
    ".knnkUuuuuuuuuuu",
    "..kkkUUUUUUUUUUU",
    "....kkkkkkkkkkkk",
];
const RAFT_M: &[&str] = &[
    "kkkkkkkkkkkkkkkk",
    "uuuuuukkkkkuuuuu",
    "uuuuukrrrrrkkuuu",
    "uuyuukrrrrrrrkyu",
    "UUUUUUkkkkkkkkUU",
    "kkkkkkkkkkkkkkkk",
];
const RAFT_R: &[&str] = &[
    "kkkkkkkkkkkkk...",
    "uuuuuuuuuuuuuk..",
    "uuuuuuuuyuuksk..",
    "uuuuuuuuuuuksk..",
    "UUUUUUUUUUUkk...",
    "kkkkkkkkkkkk....",
];

fn body(open: bool) -> Pixels {
    grid(if open { BODY_OPEN } else { BODY }, BUDDY_PAL)
}

/// An upright plunger with a handle `len` pixels long.
fn plunger(len: usize) -> Pixels {
    let mut rows: Vec<&str> = CUP.to_vec();
    rows.extend(std::iter::repeat_n(HANDLE, len));
    grid(&rows, BUDDY_PAL)
}

fn legs(rows: &[&str]) -> Pixels {
    grid(rows, BUDDY_PAL)
}

fn px(p: &mut Pixels, x: i32, y: i32, c: char) {
    let rgba = BUDDY_PAL.iter().find(|(k, _)| *k == c).map(|(_, v)| *v).expect("palette char");
    p.set(x, y, rgba);
}

/// Braced, plunger up and feet planted wide: come on down, Nat! 16 x 20 (two frames: strain).
pub fn han_braced() -> Vec<Pixels> {
    (0..2)
        .map(|k| {
            let mut p = Pixels::new(16, 20);
            p.blit(&body(k == 1), 5, 5 + k);
            p.blit(&plunger(12), 0, k);
            // Arm up to the handle.
            for y in 9..13 {
                px(&mut p, 4, y + k, 'r');
            }
            for x in 1..4 {
                px(&mut p, x, 8 + k, 's');
            }
            p.blit(&legs(LEGS_WIDE), 0, 17);
            p
        })
        .collect()
}

/// The plunger cup down (5 wide), for plunging: its rim at the bottom.
const CUP_DOWN: &[&str] = &["kRRRk", "kRRRk", "kkkkk"];
/// ...squashed flat on the bottom of a plunge (7 wide).
const CUP_SQUASHED: &[&str] = &["kRRRRRk", "kkkkkkk"];

/// Plunging (Side B's encore, on the throne): both hands on the handle, cup down in front of
/// him. 16 x 20, two frames: raised, then pushed down (crouched, the cup squashed).
pub fn han_plunge() -> Vec<Pixels> {
    (0..2)
        .map(|k| {
            let mut p = Pixels::new(16, 20);
            let down = 2 * k;
            p.blit(&body(k == 1), 1, 4 + down);
            let mut rows: Vec<&str> = vec![HANDLE; 11];
            if k == 0 {
                rows.extend(CUP_DOWN);
                p.blit(&grid(&rows, BUDDY_PAL), 10, 2);
            } else {
                p.blit(&grid(&rows, BUDDY_PAL), 10, 5);
                p.blit(&grid(CUP_SQUASHED, BUDDY_PAL), 9, 16);
            }
            // Hands on the handle, an arm out to it.
            px(&mut p, 11, 9 + down, 'r');
            px(&mut p, 12, 8 + down, 's');
            px(&mut p, 12, 10 + down, 's');
            p.blit(&legs(if k == 0 { LEGS_WIDE } else { LEGS_CROUCH }), -4, 17);
            p
        })
        .collect()
}

/// The plunger yanked out (Side B's encore): held up high in front of him, cup up (Nat stuck
/// to it, drawn apart), mouth open. 16 x 20, one frame.
pub fn han_yank() -> Vec<Pixels> {
    let mut p = Pixels::new(16, 20);
    p.blit(&body(true), 1, 5);
    p.blit(&plunger(8), 10, 0);
    for y in 7..10 {
        px(&mut p, 11, y, 'r');
    }
    px(&mut p, 12, 6, 's');
    px(&mut p, 12, 8, 's');
    p.blit(&legs(LEGS_WIDE), -4, 17);
    vec![p]
}

/// Intercepting: arms out like a goalkeeper. Two frames (arms up, arms level).
pub fn han_intercept() -> Vec<Pixels> {
    (0..2)
        .map(|k| {
            let mut p = Pixels::new(16, 16);
            p.blit(&body(true), 3, 1);
            let hy = 7 + 2 * k;
            px(&mut p, 2, 9, 'r');
            px(&mut p, 1, hy + 1, 'r');
            px(&mut p, 0, hy, 's');
            px(&mut p, 1, hy, 's');
            px(&mut p, 14, 9, 'r');
            px(&mut p, 15, hy, 's');
            px(&mut p, 14, hy, 's');
            p.blit(&legs(LEGS_WIDE), -2, 13);
            p
        })
        .collect()
}

/// Going ahead: a determined march, plunger levelled like a lance. Four frames.
pub fn han_march() -> Vec<Pixels> {
    LEGS_MARCH
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let mut p = Pixels::new(16, 16);
            let bob = (i % 2) as i32;
            p.blit(&body(false), 1, 1 + bob);
            p.blit(&grid(PLUNGER_LEVEL, BUDDY_PAL), 9, 7 + bob);
            px(&mut p, 10, 9 + bob, 's');
            p.blit(&legs(l), -3, 13);
            p
        })
        .collect()
}

/// Floating down on his plunger. 16 x 24 (two frames: a little sway).
pub fn han_parachute() -> Vec<Pixels> {
    (0..2)
        .map(|k| {
            let mut p = Pixels::new(16, 24);
            p.blit(&grid(CANOPY, BUDDY_PAL), 0, 0);
            p.blit(&body(false), 3 + k, 9);
            p.blit(&legs(LEGS_AIR), -2 + k, 21);
            p
        })
        .collect()
}

/// Winded: bent over, leaning on his plunger, sweating. Two frames (puffing).
pub fn han_winded() -> Vec<Pixels> {
    (0..2)
        .map(|k| {
            let mut p = Pixels::new(16, 16);
            p.blit(&body(true), 5, 3 + k);
            // Plunger as a walking stick: cup down on the floor.
            for y in 6..13 {
                p.blit(&grid(&[HANDLE], BUDDY_PAL), 0, y);
            }
            p.blit(&grid(&["kRRRk", "kRRRk", ".kkk."], BUDDY_PAL), 0, 13);
            for x in 1..4 {
                px(&mut p, x, 8 + k, 's');
            }
            px(&mut p, 4, 9 + k, 'r');
            p.blit(&legs(LEGS_CROUCH), 1, 13);
            px(&mut p, 15, 1 + 2 * k, 'b');
            px(&mut p, 14, 2 + 2 * k, 'b');
            p
        })
        .collect()
}

/// Splatted in the sewage: sinking, a hand waving, then just the cap. Three frames.
pub fn han_splat() -> Vec<Pixels> {
    (0..3)
        .map(|k| {
            let mut p = Pixels::new(16, 16);
            let y = 4 + 4 * k as i32;
            p.blit(&body(true), 3, y);
            if k < 2 {
                px(&mut p, 1, y + 6, 'r');
                px(&mut p, 0, y + 4, 's');
                px(&mut p, 0, y + 5, 's');
                px(&mut p, 1, y + 4, 's');
            }
            p
        })
        .collect()
}

/// Giant Steps: paddling the air. Two frames.
pub fn han_paddle() -> Vec<Pixels> {
    (0..2)
        .map(|k| {
            let mut p = Pixels::new(16, 16);
            p.blit(&body(false), 5, 0);
            p.blit(&plunger(5), 0, 1);
            let hy = if k == 0 { 5 } else { 10 };
            px(&mut p, 15, hy, 's');
            px(&mut p, 14, (hy + 9) / 2, 'r');
            p.blit(&legs(LEGS_AIR), 0, 12);
            if k == 1 {
                p.blit(&legs(LEGS_AIR), 1, 12);
            }
            p
        })
        .collect()
}

/// The laughing band: rolled up in a ball, tumbling. Four frames (a quarter turn each).
pub fn han_roll() -> Vec<Pixels> {
    const BALL: &[&str] = &[
        "....kkkkk.......",
        "..kkrrrrrkk.....",
        ".krrrrrrrrrk....",
        ".kuuussssuuk....",
        "kuuuswsswsuuk...",
        "kuuussssssuuk...",
        "kuuuskkkksuuk...",
        "kuyuuuuuuuuyk...",
        ".kuuuuuuuuuk....",
        ".knnuuuuuunnk...",
        "..kknnUUnnkk....",
        "....kkkkkk......",
    ];
    let ball = grid(BALL, BUDDY_PAL);
    (0..4)
        .map(|q| {
            let mut p = Pixels::new(16, 16);
            for y in 0..12 {
                for x in 0..13 {
                    // Rotate the 13 x 12 ball about its middle by q quarter turns.
                    let (cx, cy) = (x - 6, y - 6);
                    let (sx, sy) = match q {
                        0 => (cx, cy),
                        1 => (cy, -cx),
                        2 => (-cx, -cy),
                        _ => (-cy, cx),
                    };
                    let c = ball.get(sx + 6, sy + 6);
                    if c[3] > 0 {
                        p.set(x + 2, y + 4, c);
                    }
                }
            }
            p
        })
        .collect()
}

/// Han's raft: left, middle and right segments (frames 0, 1, 2).
pub fn han_raft() -> Vec<Pixels> {
    [RAFT_L, RAFT_M, RAFT_R]
        .iter()
        .map(|rows| {
            let mut p = Pixels::new(16, 16);
            p.blit(&grid(rows, BUDDY_PAL), 0, 1);
            p
        })
        .collect()
}

// ---------------------------------------------------------------- gate markers

pub const MARK_PAL: Palette = &[
    ('k', hex(0x1b1420)),
    ('g', hex(0xffd94a)), // gold
    ('G', hex(0xc08a1a)), // dark gold
    ('r', hex(0xe0303a)), // plunger-handle red
    ('R', hex(0x8e1622)),
    ('t', hex(0xd8a868)),
    ('y', hex(0xffe14a)), // tape
    ('Y', hex(0xd0a020)),
    ('w', hex(0xfff6e0)), // sign board
    ('b', hex(0x8a5a2a)), // sign frame
    ('B', hex(0x5a3a1a)),
];

/// Giant wall trim: gold music-staff lines on the wall's face (overlay, 16 x 16).
pub fn giant_trim() -> Vec<Pixels> {
    let mut p = Pixels::new(16, 16);
    let gold = MARK_PAL[1].1;
    let dark = MARK_PAL[2].1;
    for (i, y) in [2, 5, 8, 11, 14].into_iter().enumerate() {
        for x in 0..16 {
            p.set(x, y, if (x + i as i32) % 5 == 0 { dark } else { gold });
        }
    }
    // A bar line.
    for y in 2..15 {
        p.set(15, y, dark);
    }
    vec![p]
}

/// The note emblem on a giant wall: a gold eighth note on a dark plaque (16 x 16).
pub fn giant_emblem() -> Vec<Pixels> {
    const EMBLEM: &[&str] = &[
        "..kkkkkkkkkkkk..",
        ".kGGGGGGGGGGGGk.",
        "kGGGGGGGgGGGGGGk",
        "kGGGGGGGggGGGGGk",
        "kGGGGGGGgGgGGGGk",
        "kGGGGGGGgGGgGGGk",
        "kGGGGGGGgGGGgGGk",
        "kGGGGGGGgGGGgGGk",
        "kGGGGGGGgGGgGGGk",
        "kGGGGGGGgGGGGGGk",
        "kGGGGgggggGGGGGk",
        "kGGGgggggGGGGGGk",
        "kGGGgggggGGGGGGk",
        "kGGGGgggGGGGGGGk",
        ".kGGGGGGGGGGGGk.",
        "..kkkkkkkkkkkk..",
    ];
    vec![grid(EMBLEM, MARK_PAL)]
}

/// The waltz emblem at each end of a glide launch: a gold 3/4 time signature on a dark plaque
/// (16 x 16).
pub fn waltz_emblem() -> Vec<Pixels> {
    const EMBLEM: &[&str] = &[
        "..kkkkkkkkkkkk..",
        ".kGGGGGGGGGGGGk.",
        "kGGGGGggggGGGGGk",
        "kGGGGGGGGgGGGGGk",
        "kGGGGGGgggGGGGGk",
        "kGGGGGGGGgGGGGGk",
        "kGGGGGggggGGGGGk",
        "kGGGGGGGGGGGGGGk",
        "kGGGGGGGgGGGGGGk",
        "kGGGGGGggGGGGGGk",
        "kGGGGGgGgGGGGGGk",
        "kGGGGggggggGGGGk",
        "kGGGGGGGgGGGGGGk",
        "kGGGGGGGgGGGGGGk",
        ".kGGGGGGGGGGGGk.",
        "..kkkkkkkkkkkk..",
    ];
    vec![grid(EMBLEM, MARK_PAL)]
}

/// Mega wall face: red plunger-handle notches (overlay, 16 x 16).
pub fn ledge_notches() -> Vec<Pixels> {
    const NOTCH: &[&str] = &[
        "................",
        "................",
        "..krrrrrrrrrrk..",
        "..kRRRRRRRRRRk..",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "..krrrrrrrrrrk..",
        "..kRRRRRRRRRRk..",
        "................",
        "................",
        "................",
        "................",
    ];
    vec![grid(NOTCH, MARK_PAL)]
}

/// Han's yellow plumber's tape, wound along a mega wall's top edge (overlay, 16 x 16).
pub fn plumber_tape() -> Vec<Pixels> {
    const TAPE: &[&str] = &[
        "yyYyyyYyyyYyyyYy",
        "yYyyyYyyyYyyyYyy",
        "YyyyYyyyYyyyYyyy",
        "kkkkkkkkkkkkkkkk",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
    ];
    vec![grid(TAPE, MARK_PAL)]
}

/// Every hand-authored grid in this module, for validation tests.
#[cfg(test)]
pub fn grids() -> Vec<(&'static str, Vec<&'static str>, Palette)> {
    let mut v: Vec<(&'static str, Vec<&'static str>, Palette)> = vec![
        ("BODY", BODY.to_vec(), BUDDY_PAL),
        ("BODY_OPEN", BODY_OPEN.to_vec(), BUDDY_PAL),
        ("CUP", CUP.to_vec(), BUDDY_PAL),
        ("PLUNGER_LEVEL", PLUNGER_LEVEL.to_vec(), BUDDY_PAL),
        ("LEGS_WIDE", LEGS_WIDE.to_vec(), BUDDY_PAL),
        ("LEGS_AIR", LEGS_AIR.to_vec(), BUDDY_PAL),
        ("LEGS_CROUCH", LEGS_CROUCH.to_vec(), BUDDY_PAL),
        ("CANOPY", CANOPY.to_vec(), BUDDY_PAL),
        ("RAFT_L", RAFT_L.to_vec(), BUDDY_PAL),
        ("RAFT_M", RAFT_M.to_vec(), BUDDY_PAL),
        ("RAFT_R", RAFT_R.to_vec(), BUDDY_PAL),
    ];
    for l in LEGS_MARCH {
        v.push(("LEGS_MARCH", l.to_vec(), BUDDY_PAL));
    }
    v
}

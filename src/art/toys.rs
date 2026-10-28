//! The secret playground's toys: the "helper" (a shifty guy in a bowler hat, all grin and
//! mustache), the blast button between its speakers, a stop sign, a little house (window dark,
//! or lit with the chef at the stove), and Nat the schnitzel (breaded, with a lemon).

use super::items::OUTLINE;
use super::palette::*;

// ---------------------------------------------------------------- the helper

const HELPER_PAL: Palette = &[
    ('k', OUTLINE),
    ('h', hex(0x3a3448)), // bowler hat
    ('H', hex(0x5c5470)),
    ('s', hex(0xf0b890)), // skin
    ('S', hex(0xc88a68)),
    ('m', hex(0x4a2a18)), // mustache
    ('w', hex(0xffffff)),
    ('t', hex(0x50a050)), // striped shirt
    ('T', hex(0xe8e0c0)),
    ('p', hex(0x6a5a8a)), // trousers
];

/// Idle (his hand out right, holding the rope), calling (waving), snickering.
const HELPER: [&[&str]; 3] = [
    &[
        "................",
        ".....kkkkk......",
        "....khHhhhk.....",
        "...kkkkkkkkkk...",
        "....ksssssk.....",
        "....kswkswk.....",
        "....ksssssSk....",
        "...kmmmmmmmk....",
        "....kswwwsk.....",
        ".....kkkkk......",
        "....ktTtTtkkkkk.",
        "...ktTtTtTtssk..",
        "...ksktTtTkkk...",
        "....kpppppk.....",
        "....kpk.kpk.....",
        "....kkk.kkk.....",
    ],
    &[
        "..........kk....",
        ".....kkkkksk....",
        "....khHhhhksk...",
        "...kkkkkkkkksk..",
        "....ksssssk.tk..",
        "....kswkswk.tk..",
        "....ksssssSktk..",
        "...kmmmmmmmktk..",
        "....kskkksktk...",
        ".....kskkk.tk...",
        "....ktTtTtkTk...",
        "...ktTtTtTtk....",
        "...ksktTtTk.....",
        "....kpppppk.....",
        "....kpk.kpk.....",
        "....kkk.kkk.....",
    ],
    &[
        "................",
        ".....kkkkk......",
        "....khHhhhk.....",
        "...kkkkkkkkkk...",
        "....ksssssk.....",
        "....kkkskkk.....",
        "....ksssssSk....",
        "...kmmmmmmmk....",
        "....ksskkssk....",
        ".....kssssk.....",
        "....ktTtTsskk...",
        "...ktTtTtTtk....",
        "...ksktTtTk.....",
        "....kpppppk.....",
        "....kpk.kpk.....",
        "....kkk.kkk.....",
    ],
];

pub fn helper() -> Vec<Pixels> {
    HELPER.iter().map(|g| grid(g, HELPER_PAL)).collect()
}

// ---------------------------------------------------------------- the blast button

const BLASTER_PAL: Palette = &[
    ('k', OUTLINE),
    ('b', hex(0x3a3448)), // speaker box
    ('B', hex(0x5c5470)),
    ('c', hex(0x24202c)), // cone
    ('C', hex(0x8a84a0)),
    ('r', hex(0xff4040)), // button
    ('R', hex(0xa81c28)),
    ('w', hex(0xffb0b0)),
    ('g', hex(0x9a9aa8)), // base
    ('G', hex(0x60606c)),
];

/// A speaker stack either side of a big red button: up, and pressed (the cones out, BOOM).
/// 48x16, the floor under the bottom row.
const BLASTER: [&[&str]; 2] = [
    &[
        "kkkkkkkkkk............................kkkkkkkkkk",
        "kBBBBBBBBk............................kBBBBBBBBk",
        "kbbkkkkbbk............................kbbkkkkbbk",
        "kbkCCCCkbk............................kbkCCCCkbk",
        "kbkCcccCkbk..........................kbkCcccCkbk",
        "kbkCcccCkbk..........................kbkCcccCkbk",
        "kbkCCCCkbk............................kbkCCCCkbk",
        "kbbkkkkbbk..........kkkkkkkk..........kbbkkkkbbk",
        "kbbkkkkbbk........kkwwrrrrrrkk........kbbkkkkbbk",
        "kbkCCCCkbk.......kwwrrrrrrrrRRk.......kbkCCCCkbk",
        "kbkCcccCkbk......krrrrrrrrrrRRk......kbkCcccCkbk",
        "kbkCcccCkbk......kRrrrrrrrrRRRk......kbkCcccCkbk",
        "kbkCCCCkbk.......kkRRRRRRRRRRkk.......kbkCCCCkbk",
        "kbbkkkkbbk.....kkggggggggggggggkk.....kbbkkkkbbk",
        "kBBBBBBBBk.....kgGGGGGGGGGGGGGGGk.....kBBBBBBBBk",
        "kkkkkkkkkk.....kkkkkkkkkkkkkkkkkk.....kkkkkkkkkk",
    ],
    &[
        "kkkkkkkkkk............................kkkkkkkkkk",
        "kBBBBBBBBk............................kBBBBBBBBk",
        "kbbkkkkbbk............................kbbkkkkbbk",
        "kbkCCCCCCk............................kCCCCCCkbk",
        "kbkCccccCCk..........................kCCccccCkbk",
        "kbkCccccCCk..........................kCCccccCkbk",
        "kbkCCCCCCk............................kCCCCCCkbk",
        "kbbkkkkbbk............................kbbkkkkbbk",
        "kbbkkkkbbk............................kbbkkkkbbk",
        "kbkCCCCCCk............................kCCCCCCkbk",
        "kbkCccccCCk.........kkkkkkkk.........kCCccccCkbk",
        "kbkCccccCCk......kkkwwrrrrrRRkkk.....kCCccccCkbk",
        "kbkCCCCCCk.......kRRRRRRRRRRRRRk......kCCCCCCkbk",
        "kbbkkkkbbk.....kkggggggggggggggkk.....kbbkkkkbbk",
        "kBBBBBBBBk.....kgGGGGGGGGGGGGGGGk.....kBBBBBBBBk",
        "kkkkkkkkkk.....kkkkkkkkkkkkkkkkkk.....kkkkkkkkkk",
    ],
];

pub fn blaster() -> Vec<Pixels> {
    BLASTER.iter().map(|g| grid(g, BLASTER_PAL)).collect()
}

// ---------------------------------------------------------------- the stop sign

const SIGN_PAL: Palette = &[('k', OUTLINE), ('r', hex(0xe02828)), ('R', hex(0x9a1420)), ('w', hex(0xffffff)), ('g', hex(0xa0a0b0)), ('G', hex(0x686878))];

/// An octagon on a pole, STOP across it (as near as 12 px get). 16x32.
const SIGN: &[&str] = &[
    "................",
    "....kkkkkkkk....",
    "...krrrrrrrrk...",
    "..krrrrrrrrrrk..",
    ".krrrrrrrrrrrrk.",
    "krrrrrrrrrrrrrRk",
    "krwwrwwwrwwrwwRk",
    "krwrrrwrrwrrwrwk",
    "krwwrrwrrwrrwwwk",
    "krrwrrwrrwrrwrRk",
    "krwwrrwrrwwrwrRk",
    "krrrrrrrrrrrrrRk",
    ".krrrrrrrrrrrRk.",
    "..kRrrrrrrrrRk..",
    "...kRRRRRRRRk...",
    "....kkkgGkkk....",
    ".......gG.......",
    ".......gG.......",
    ".......gG.......",
    ".......gG.......",
    ".......gG.......",
    ".......gG.......",
    ".......gG.......",
    ".......gG.......",
    ".......gG.......",
    ".......gG.......",
    ".......gG.......",
    ".......gG.......",
    ".......gG.......",
    "......kgGk......",
    ".....kgggGk.....",
    ".....kkkkkk.....",
];

pub fn stop_sign() -> Vec<Pixels> {
    vec![grid(SIGN, SIGN_PAL)]
}

// ---------------------------------------------------------------- the house

const HOUSE_PAL: Palette = &[
    ('k', OUTLINE),
    ('r', hex(0xc0483c)), // roof
    ('R', hex(0x84282a)),
    ('w', hex(0xf0dcb0)), // walls
    ('W', hex(0xc8b088)),
    ('d', hex(0x8a5428)), // door
    ('D', hex(0x5a3418)),
    ('y', hex(0xffd860)), // knob, the lit window
    ('n', hex(0x2a3a5a)), // a dark window
    ('c', hex(0x7a7a88)), // chimney
    ('C', hex(0x50505c)),
    ('s', hex(0xf0b890)), // the chef
    ('p', hex(0x24202c)), // the pan
];

/// Nobody home (the window dark), and the chef in (the window lit: the chef at the stove, his
/// pan up). 48x48: the door's floor 6 px up from the bottom, its middle 3 px right of the
/// picture's; the chimney over the door's
/// right ([`crate::game::houses::CHIMNEY`]).
const HOUSE: [&[&str]; 2] = [
    &[
        "................................................",
        "................................................",
        "............................kkkkkk..............",
        "............................kccccCk.............",
        "............................kccccCk.............",
        "............................kccccCk.............",
        "......................kkkkk.kccccCk.............",
        "....................kkrrrrrkkccccCk.............",
        "..................kkrrrrrrrrkccccCk.............",
        "................kkrrrrrrrrrrrkkkkk..............",
        "..............kkrrrrrrrrrrrrrrrkk...............",
        "............kkrrrrrrrrrrrrrrrrrrrkk.............",
        "..........kkrrrrrrrrrrrrrrrrrrrrrrrkk...........",
        "........kkrrrrrrrrrrrrrrrrrrrrrrrrrrrkk.........",
        "......kkrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrkk.......",
        "....kkRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRkk.....",
        "...kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk....",
        ".....kwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwWk.....",
        ".....kwwkkkkkkkkkkwwwwwwwwwwwwwwwwwwwwwwwWk.....",
        ".....kwwknnnnknnnkwwwwwwwwwwwwwwwwwwwwwwwWk.....",
        ".....kwwknnnnknnnkwwwwwwwwwwwwwwwwwwwwwwwWk.....",
        ".....kwwknnnnknnnkwwwwwwwwwwwwwwwwwwwwwwwWk.....",
        ".....kwwkkkkkkkkkkwwwwwwwwwwwwwwwwwwwwwwwWk.....",
        ".....kwwknnnnknnnkwwwwwwwwwwwwwwwwwwwwwwwWk.....",
        ".....kwwknnnnknnnkwwwwwkkkkkkkkwwwwwwwwwwWk.....",
        ".....kwwknnnnknnnkwwwwkddddddddkwwwwwwwwwWk.....",
        ".....kwwkkkkkkkkkkwwwwkdDddddDdkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkdDddddDdkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkdDddddDdkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkdDddddDdkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkddddddddkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkddddddydkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkddddddydkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkddddddddkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkdDddddDdkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkdDddddDdkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkdDddddDdkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkdDddddDdkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkddddddddkwwwwwwwwwWk.....",
        ".....kWWWWWWWWWWWWWWWWkddddddddkWWWWWWWWWWk.....",
        ".....kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk.....",
        "................................................",
        "................................................",
        "................................................",
        "................................................",
        "................................................",
        "................................................",
    ],
    &[
        "................................................",
        "................................................",
        "............................kkkkkk..............",
        "............................kccccCk.............",
        "............................kccccCk.............",
        "............................kccccCk.............",
        "......................kkkkk.kccccCk.............",
        "....................kkrrrrrkkccccCk.............",
        "..................kkrrrrrrrrkccccCk.............",
        "................kkrrrrrrrrrrrkkkkk..............",
        "..............kkrrrrrrrrrrrrrrrkk...............",
        "............kkrrrrrrrrrrrrrrrrrrrkk.............",
        "..........kkrrrrrrrrrrrrrrrrrrrrrrrkk...........",
        "........kkrrrrrrrrrrrrrrrrrrrrrrrrrrrkk.........",
        "......kkrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrkk.......",
        "....kkRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRRkk.....",
        "...kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk....",
        ".....kwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwWk.....",
        ".....kwwkkkkkkkkkkwwwwwwwwwwwwwwwwwwwwwwwWk.....",
        ".....kwwkywwwkyyykwwwwwwwwwwwwwwwwwwwwwwwWk.....",
        ".....kwwkwwwwkyyykwwwwwwwwwwwwwwwwwwwwwwwWk.....",
        ".....kwwkyssykyppkwwwwwwwwwwwwwwwwwwwwwwwWk.....",
        ".....kwwkkkkkkkkkkwwwwwwwwwwwwwwwwwwwwwwwWk.....",
        ".....kwwkysssksppkwwwwwwwwwwwwwwwwwwwwwwwWk.....",
        ".....kwwkywwwkyyykwwwwwkkkkkkkkwwwwwwwwwwWk.....",
        ".....kwwkywwwkyyykwwwwkddddddddkwwwwwwwwwWk.....",
        ".....kwwkkkkkkkkkkwwwwkdDddddDdkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkdDddddDdkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkdDddddDdkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkdDddddDdkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkddddddddkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkddddddydkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkddddddydkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkddddddddkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkdDddddDdkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkdDddddDdkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkdDddddDdkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkdDddddDdkwwwwwwwwwWk.....",
        ".....kwwwwwwwwwwwwwwwwkddddddddkwwwwwwwwwWk.....",
        ".....kWWWWWWWWWWWWWWWWkddddddddkWWWWWWWWWWk.....",
        ".....kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk.....",
        "................................................",
        "................................................",
        "................................................",
        "................................................",
        "................................................",
        "................................................",
    ],
];

pub fn house() -> Vec<Pixels> {
    HOUSE.iter().map(|g| grid(g, HOUSE_PAL)).collect()
}

// ---------------------------------------------------------------- Nat the schnitzel

const SCHNITZEL_PAL: Palette = &[
    ('k', hex(0x5a3010)),
    ('g', hex(0xe8a840)), // crumbs
    ('G', hex(0xb87420)),
    ('l', hex(0xffd878)),
    ('w', hex(0xffffff)),
    ('y', hex(0xfff060)), // lemon
    ('Y', hex(0xd0b020)),
];

/// Breaded, golden, dizzy (x eyes), a lemon wedge on top. 16x16.
const SCHNITZEL: &[&str] = &[
    "................",
    "..........kkk...",
    ".........kyyyk..",
    "........kyyYk...",
    "....kkkkkkkkkk..",
    "..kkglgGglggGkk.",
    ".kglgwkwgggwkwGk",
    ".kgggkwkggGkwkgk",
    "kglggwkwgglwkwgG",
    "kgGgggggggggggGk",
    "kgggglgkkkkgggGk",
    "kglgggggggglggGk",
    ".kGgglgggGgggGk.",
    "..kkGGGGGGGGkk..",
    "....kkkkkkkk....",
    "................",
];

pub fn schnitzel() -> Vec<Pixels> {
    vec![grid(SCHNITZEL, SCHNITZEL_PAL)]
}

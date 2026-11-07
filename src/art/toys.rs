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

// ---------------------------------------------------------------- the policeman

const POLICE_PAL: Palette = &[
    ('k', OUTLINE),
    ('b', hex(0x2a3a8a)), // uniform
    ('B', hex(0x18245a)),
    ('y', hex(0xffd040)), // badge, buttons
    ('s', hex(0xf0b890)), // skin
    ('S', hex(0xc88a68)),
    ('w', hex(0xffffff)),
    ('m', hex(0x4a2a18)), // mustache
    ('g', hex(0xb0b0c0)), // whistle
];

/// Walking (two steps), "STOP!" (his hand up, whistle in), "THIS WAY!" (his arm out right,
/// whistle in). 16x24, his feet on the bottom row.
const POLICE: [&[&str]; 4] = [
    &[
        "................",
        ".....kkkkkk.....",
        "....kbbbbbbk....",
        "...kbbbybbbbk...",
        "...kkkkkkkkkkk..",
        "....kssssssk....",
        "....kswksswk....",
        "....kssssssSk...",
        "....kmmmmmmk....",
        "....kssssssk....",
        ".....kkkkkk.....",
        "....kbbbbbbk....",
        "...kbbbbybbbk...",
        "...kbkbbbbkbk...",
        "...kbkbbybkbk...",
        "...kskbbbbksk...",
        "....kbbbbbbk....",
        "....kBBBBBBk....",
        "....kbbkkbbk....",
        "....kbbk.kbk....",
        "...kbbk..kbk....",
        "...kbk...kbbk...",
        "..kkkk...kkkk...",
        "................",
    ],
    &[
        "................",
        ".....kkkkkk.....",
        "....kbbbbbbk....",
        "...kbbbybbbbk...",
        "...kkkkkkkkkkk..",
        "....kssssssk....",
        "....kswksswk....",
        "....kssssssSk...",
        "....kmmmmmmk....",
        "....kssssssk....",
        ".....kkkkkk.....",
        "....kbbbbbbk....",
        "...kbbbbybbbk...",
        "...kbkbbbbkbk...",
        "...kbkbbybkbk...",
        "...kskbbbbksk...",
        "....kbbbbbbk....",
        "....kBBBBBBk....",
        "....kbbkkbbk....",
        "....kbk.kbbk....",
        "....kbk..kbbk...",
        "...kbbk...kbk...",
        "...kkkk...kkkk..",
        "................",
    ],
    &[
        "..........kkk...",
        ".....kkkkkskk...",
        "....kbbbbbkssk..",
        "...kbbbybbbkssk.",
        "...kkkkkkkkkskk.",
        "....kssssssk.kbk",
        "....kswksswk.kbk",
        "....kssssssSkkbk",
        "....kmmmmmmkkbk.",
        "....ksskggggkbk.",
        ".....kkkkkkkbbk.",
        "....kbbbbbbbbk..",
        "...kbbbbybbbk...",
        "...kbkbbbbbk....",
        "...kbkbbybbk....",
        "...kskbbbbbk....",
        "....kbbbbbbk....",
        "....kBBBBBBk....",
        "....kbbkkbbk....",
        "....kbbk.kbk....",
        "....kbbk.kbk....",
        "....kbbk.kbbk...",
        "...kkkkk.kkkkk..",
        "................",
    ],
    &[
        "................",
        ".....kkkkkk.....",
        "....kbbbbbbk....",
        "...kbbbybbbbk...",
        "...kkkkkkkkkkk..",
        "....kssssssk....",
        "....kswksswk....",
        "....kssssssSk...",
        "....kmmmmmmk....",
        "....ksskggggk...",
        ".....kkkkkkkkkkk",
        "....kbbbbbbbbbsk",
        "...kbbbbybbbkkk.",
        "...kbkbbbbbk....",
        "...kbkbbybbk....",
        "...kskbbbbbk....",
        "....kbbbbbbk....",
        "....kBBBBBBk....",
        "....kbbkkbbk....",
        "....kbbk.kbk....",
        "....kbbk.kbk....",
        "....kbbk.kbbk...",
        "...kkkkk.kkkkk..",
        "................",
    ],
];

pub fn police() -> Vec<Pixels> {
    POLICE.iter().map(|g| grid(g, POLICE_PAL)).collect()
}

const ORDER_PAL: Palette = &[('k', OUTLINE), ('r', hex(0xe02828)), ('R', hex(0x9a1420)), ('g', hex(0x30b848)), ('G', hex(0x1a7a2c)), ('w', hex(0xffffff))];

/// The order over his head: stop (a red disc, a white hand up) and go (a green disc, a white
/// arrow right; flipped for left). 16x16.
const ORDER: [&[&str]; 2] = [
    &[
        ".....kkkkkk.....",
        "...kkrrrrrrkk...",
        "..krrrrwrwrrrk..",
        ".krrrwrwrwrrrRk.",
        ".krrrwrwrwrwrRk.",
        "krrrrwwwwwwrrrRk",
        "krrrrwwwwwwrrrRk",
        "krrwrwwwwwwrrrRk",
        "krrwwwwwwwwrrrRk",
        "krrrwwwwwwwrrrRk",
        "krrrrwwwwwrrrrRk",
        ".krrrrwwwwrrrRk.",
        ".kRrrrrrrrrrrRk.",
        "..kRRrrrrrrRRk..",
        "...kkRRRRRRkk...",
        ".....kkkkkk.....",
    ],
    &[
        ".....kkkkkk.....",
        "...kkggggggkk...",
        "..kgggggggggGk..",
        ".kggggggwggggGk.",
        ".kggggggwwgggGk.",
        "kgggggggwwwgggGk",
        "kgwwwwwwwwwwggGk",
        "kgwwwwwwwwwwwgGk",
        "kgwwwwwwwwwwwgGk",
        "kgwwwwwwwwwwggGk",
        "kgggggggwwwgggGk",
        ".kggggggwwgggGk.",
        ".kGgggggwggggGk.",
        "..kGGgggggggGk..",
        "...kkGGGGGGkk...",
        ".....kkkkkk.....",
    ],
];

pub fn order_sign() -> Vec<Pixels> {
    ORDER.iter().map(|g| grid(g, ORDER_PAL)).collect()
}

const CONE_PAL: Palette = &[('k', OUTLINE), ('o', hex(0xff7a1a)), ('O', hex(0xc04a08)), ('w', hex(0xffffff)), ('W', hex(0xc8c8d0))];

/// A traffic cone. 16x16.
const CONE: &[&str] = &[
    ".......kk.......",
    "......kook......",
    "......kooOk.....",
    ".....kwwwWk.....",
    ".....kwwwWk.....",
    ".....koooOOk....",
    "....kooooOOk....",
    "....kwwwwwWk....",
    "....kwwwwwWWk...",
    "...kooooooOOk...",
    "...koooooooOOk..",
    "...koooooooOOk..",
    "..kkkkkkkkkkkkk.",
    ".kooooooooooooOk",
    ".kOOOOOOOOOOOOOk",
    ".kkkkkkkkkkkkkkk",
];

pub fn cone() -> Vec<Pixels> {
    vec![grid(CONE, CONE_PAL)]
}

// ---------------------------------------------------------------- the building code

const PAPER_PAL: Palette = &[('k', OUTLINE), ('p', hex(0xfff4d0)), ('P', hex(0xd8c890)), ('i', hex(0x3050c0)), ('r', hex(0xe02828))];

/// The secret paper: rolled at both ends, a big question mark on it. 16x16.
const PAPER: &[&str] = &[
    "................",
    ".kkkkkkkkkkkkkk.",
    "kPPPPPPPPPPPPPPk",
    ".kkkkkkkkkkkkkk.",
    "..kppppiiiipPk..",
    "..kpppipppipPk..",
    "..kppppppipppk..",
    "..kpppppipppPk..",
    "..kppppipppPPk..",
    "..kppppipppPPk..",
    "..kpppppppppPk..",
    "..kppppipppPPk..",
    "..kpppppprrpPk..",
    ".kkkkkkkkkkkkkk.",
    "kPPPPPPPPPPPPPPk",
    ".kkkkkkkkkkkkkk.",
];

pub fn code_paper() -> Vec<Pixels> {
    vec![grid(PAPER, PAPER_PAL)]
}

const DOOR_PAL: Palette = &[
    ('k', OUTLINE),
    ('d', hex(0x5a6a88)), // a steel door
    ('D', hex(0x3a4660)),
    ('f', hex(0x8a8a98)), // frame
    ('F', hex(0x5a5a68)),
    ('p', hex(0x24202c)), // keypad
    ('g', hex(0x60e070)), // its keys' light
    ('r', hex(0xff4040)),
    ('n', hex(0x0c0a10)), // the dark through the open door
    ('y', hex(0xffd860)),
];

/// The door with a keypad: shut (a red light), and open (dark inside, the light green). 16x32.
const DOOR: [&[&str]; 2] = [
    &[
        "................",
        "................",
        "kkkkkkkkkkkkkkkk",
        "kffffffffffffffk",
        "kfkkkkkkkkkkkkFk",
        "kfkddddddddddkFk",
        "kfkdDDDDDDDDdkFk",
        "kfkdDddddddDdkFk",
        "kfkdDddddddDdkFk",
        "kfkdDddddddDdkFk",
        "kfkdDDDDDDDDdkFk",
        "kfkddddddddddkFk",
        "kfkdddddkkkkdkFk",
        "kfkdddddkrrkdkFk",
        "kfkdddddkpppkkFk",
        "kfkdddddkgpgkkFk",
        "kfkdddyddpgpkkFk",
        "kfkdddyddkkkdkFk",
        "kfkddddddddddkFk",
        "kfkdDDDDDDDDdkFk",
        "kfkdDddddddDdkFk",
        "kfkdDddddddDdkFk",
        "kfkdDddddddDdkFk",
        "kfkdDddddddDdkFk",
        "kfkdDDDDDDDDdkFk",
        "kfkddddddddddkFk",
        "kfkddddddddddkFk",
        "kfkddddddddddkFk",
        "kfkddddddddddkFk",
        "kfkkkkkkkkkkkkFk",
        "kFFFFFFFFFFFFFFk",
        "kkkkkkkkkkkkkkkk",
    ],
    &[
        "................",
        "................",
        "kkkkkkkkkkkkkkkk",
        "kffffffffffffffk",
        "kfkkkkkkkkkkkkFk",
        "kfknnnnnnnnnkdkk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdkk",
        "kfknnnnnnnnnkdgk",
        "kfknnnnnnnnnkdgk",
        "kfknnnnnnnnnkdkk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdDk",
        "kfknnnnnnnnnkdkk",
        "kfkkkkkkkkkkkkFk",
        "kFFFFFFFFFFFFFFk",
        "kkkkkkkkkkkkkkkk",
    ],
];

pub fn code_door() -> Vec<Pixels> {
    DOOR.iter().map(|g| grid(g, DOOR_PAL)).collect()
}

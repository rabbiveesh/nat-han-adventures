//! The gadget tiles: breakable bricks (a bar of old soap), crumbling platforms (soggy
//! cardboard), trampolines (a whoopee cushion), crushers (a riveted steel block with an
//! angry face and hazard stripes on its slamming side), conveyor belts, fans (a vent grate),
//! bumpers (a rubber duck) and the red and blue beat blocks.

use super::palette::*;

// ---------------------------------------------------------------- brick: a bar of old soap

const SOAP_PAL: Palette = &[
    ('k', hex(0x3a2a40)), // outline
    ('w', hex(0xfff4fa)), // shine
    ('p', hex(0xf4b8d4)), // soap pink
    ('P', hex(0xd88cb4)), // shade
    ('d', hex(0xa85c88)), // deep shade / letters
    ('c', hex(0x6a3a58)), // crack
];
const BRICK: &[&str] = &[
    "kkkkkkkkkkkkkkkk",
    "kwwwppppppppppPk",
    "kwppppppppppppPk",
    "kpppdpdpdpdpppPk",
    "kpppdpdpdpdpppPk",
    "kpppddpddpdpppPk",
    "kpppppppppppppPk",
    "kPPPPPPPPPPPPPPk",
    "kkkkkkkkkkkkkkkk",
    "kwppppPkkwpppppk",
    "kppppppkkpppppPk",
    "kppppppkkpppppPk",
    "kppppcppkppcppPk",
    "kpppppcpkpppppPk",
    "kPPPPPPPkPPPPPPk",
    "kkkkkkkkkkkkkkkk",
];

pub fn brick() -> Vec<Pixels> {
    vec![grid(BRICK, SOAP_PAL)]
}

// ---------------------------------------------------------------- crumble: soggy cardboard

const CARD_PAL: Palette = &[
    ('k', hex(0x3a2614)),
    ('h', hex(0xe8c890)), // light
    ('b', hex(0xc89a5a)), // cardboard
    ('B', hex(0x9a6e3a)), // shade
    ('d', hex(0x6e4a22)), // damp
    ('c', hex(0x24180c)), // crack
];
const CRUMBLE: [&[&str]; 2] = [
    &[
        "kkkkkkkkkkkkkkkk",
        "hhhhhhhhhhhhhhhh",
        "bbbBbbbbbbBbbbbb",
        "bdbbbbbBbbbbbdbb",
        "BBBBBBBBBBBBBBBB",
        "kkkkkkkkkkkkkkkk",
        ".d...d.....d..d.",
        ".....d........d.",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
    ],
    &[
        "kkkkkkkckkkkkkkk",
        "hhhhhhhchhhhhhhh",
        "bbbBbbbcbbbBbbbb",
        "bdbbbbcBbbbbbdbb",
        "BBBBBBcBBBBBBBBB",
        "kkkkkkckkkkkkkkk",
        ".d...d.....d..d.",
        ".d...d.....d..d.",
        ".....d........d.",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
    ],
];

/// Frame 0: whole; frame 1: cracking.
pub fn crumble() -> Vec<Pixels> {
    CRUMBLE.iter().map(|g| grid(g, CARD_PAL)).collect()
}

// ---------------------------------------------------------------- trampoline: whoopee cushion

const CUSHION_PAL: Palette = &[
    ('k', hex(0x401018)),
    ('w', hex(0xffd0d8)), // shine
    ('r', hex(0xe84860)), // rubber
    ('R', hex(0xb02a40)), // shade
    ('d', hex(0x701828)), // deep
    ('n', hex(0xf0a0b0)), // nozzle
];
const TRAMPOLINE: [&[&str]; 2] = [
    &[
        "................",
        "................",
        "....kkkkkkkk....",
        "..kkwwwrrrrrkk..",
        ".kwwrrrrrrrrrRk.",
        "kwrrrrrrrrrrrrRk",
        "krrrrrrrrrrrrRRk",
        "krrrrrrrrrrrRRRk",
        "kRrrrrrrrrrRRRdk",
        "kRRRRRRRRRRRRddk",
        ".kRRRRRRRRRRddk.",
        "..kkdddddddkkknk",
        "....kkkkkkkk.knk",
        ".............kk.",
        "................",
        "................",
    ],
    &[
        "................",
        "................",
        "................",
        "................",
        "................",
        "......kkkk......",
        "..kkkkwrrrkkkk..",
        ".kwwrrrrrrrrrRk.",
        "kwrrrrrrrrrrrRRk",
        "kRrrrrrrrrrrRRdk",
        "kRRRRRRRRRRRRddk",
        ".kkdddddddddkknk",
        "...kkkkkkkkkkknk",
        ".............kk.",
        "................",
        "................",
    ],
];

/// Frame 0: puffed up; frame 1: squashed (just sprang someone).
pub fn trampoline() -> Vec<Pixels> {
    TRAMPOLINE.iter().map(|g| grid(g, CUSHION_PAL)).collect()
}

// ---------------------------------------------------------------- crusher: angry steel

const STEEL_PAL: Palette = &[
    ('k', hex(0x14161c)),
    ('h', hex(0xd8dce4)), // highlight
    ('s', hex(0x9aa0ac)), // steel
    ('S', hex(0x6a707c)), // shade
    ('r', hex(0x4a4e58)), // rivet shadow
    ('w', hex(0xffffff)), // eye white
    ('y', hex(0xf8d030)), // hazard yellow
    ('e', hex(0xc02020)), // angry red
];
const CRUSHER_BODY: &[&str] = &[
    "kkkkkkkkkkkkkkkk",
    "khhhhhhhhhhhhhsk",
    "khrsssssssssrSSk",
    "khssssssssssssSk",
    "khssssssssssssSk",
    "khssssssssssssSk",
    "khssssssssssssSk",
    "khssssssssssssSk",
    "khssssssssssssSk",
    "khssssssssssssSk",
    "khssssssssssssSk",
    "khssssssssssssSk",
    "khssssssssssssSk",
    "khrsssssssssrSSk",
    "ksSSSSSSSSSSSSSk",
    "kkkkkkkkkkkkkkkk",
];
const CRUSHER_FACE: &[&str] = &[
    "................",
    "................",
    "................",
    "..kk........kk..",
    "...kkk....kkk...",
    "....eekk.kkee...",
    "..kwwwwk.kwwwwk.",
    "..kwwkkk.kkkwwk.",
    "..kwwkkk.kkkwwk.",
    "...kkkk...kkkk..",
    "................",
    "....kkkkkkkk....",
    "...kwkwkwkwkwk..",
    "....kkkkkkkk....",
    "................",
    "................",
];
const CRUSHER_STRIPES: &[&str] = &[
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
    "kkkkkkkkkkkkkkkk",
    "yykkyykkyykkyykk",
    "ykkyykkyykkyykky",
    "kkyykkyykkyykkyy",
    "kkkkkkkkkkkkkkkk",
];

pub fn crusher_body() -> Vec<Pixels> {
    vec![grid(CRUSHER_BODY, STEEL_PAL)]
}

/// The angry face, drawn over the block's middle cell.
pub fn crusher_face() -> Vec<Pixels> {
    vec![grid(CRUSHER_FACE, STEEL_PAL)]
}

/// Hazard stripes along the bottom of the cell (rotated onto the slamming side).
pub fn crusher_stripes() -> Vec<Pixels> {
    vec![grid(CRUSHER_STRIPES, STEEL_PAL)]
}

// ---------------------------------------------------------------- belt: a conveyor

const BELT_PAL: Palette = &[
    ('k', hex(0x1c1a20)),
    ('B', hex(0x5a5660)), // belt, lit edge
    ('b', hex(0x3e3a44)), // belt rubber
    ('y', hex(0xf8d030)), // chevrons
    ('S', hex(0x8a8e98)), // housing
    ('s', hex(0xb8bcc6)),
    ('r', hex(0x6a3a2a)), // roller
    ('h', hex(0xd8a070)), // roller hub
];
const BELT: [&[&str]; 4] = [
    &[
        "kkkkkkkkkkkkkkkk",
        "kBBBBBBByBBBBBBk",
        "kybbbbbbbybbbbbk",
        "kbbbbbbbybbbbbbk",
        "kkkkkkkkkkkkkkkk",
        "kSSSSSSSSSSSSSSk",
        "kSkkkSSSSSSkkkSk",
        "kkrhrkSSSSkrhrkk",
        "kkrhrkSSSSkrhrkk",
        "kkrrrkSSSSkrrrkk",
        "kSkkkSSSSSSkkkSk",
        "kSSSSSSSSSSSSSSk",
        "kSkSkSkSkSkSkSkk",
        "kSSSSSSSSSSSSSSk",
        "kSSSSSSSSSSSSSSk",
        "kkkkkkkkkkkkkkkk",
    ],
    &[
        "kkkkkkkkkkkkkkkk",
        "kByBBBBBBByBBBBk",
        "kbbybbbbbbbybbbk",
        "kbybbbbbbbybbbbk",
        "kkkkkkkkkkkkkkkk",
        "kSSSSSSSSSSSSSSk",
        "kSkkkSSSSSSkkkSk",
        "kkrhrkSSSSkrhrkk",
        "kkrrrkSSSSkrrrkk",
        "kkrrrkSSSSkrrrkk",
        "kSkkkSSSSSSkkkSk",
        "kSSSSSSSSSSSSSSk",
        "kSkSkSkSkSkSkSkk",
        "kSSSSSSSSSSSSSSk",
        "kSSSSSSSSSSSSSSk",
        "kkkkkkkkkkkkkkkk",
    ],
    &[
        "kkkkkkkkkkkkkkkk",
        "kBBByBBBBBBByBBk",
        "kbbbbybbbbbbbybk",
        "kbbbybbbbbbbybbk",
        "kkkkkkkkkkkkkkkk",
        "kSSSSSSSSSSSSSSk",
        "kSkkkSSSSSSkkkSk",
        "kkrhrkSSSSkrhrkk",
        "kkrhrkSSSSkrhrkk",
        "kkrrrkSSSSkrrrkk",
        "kSkkkSSSSSSkkkSk",
        "kSSSSSSSSSSSSSSk",
        "kSkSkSkSkSkSkSkk",
        "kSSSSSSSSSSSSSSk",
        "kSSSSSSSSSSSSSSk",
        "kkkkkkkkkkkkkkkk",
    ],
    &[
        "kkkkkkkkkkkkkkkk",
        "kBBBBByBBBBBBByk",
        "kbbbbbbybbbbbbbk",
        "kbbbbbybbbbbbbyk",
        "kkkkkkkkkkkkkkkk",
        "kSSSSSSSSSSSSSSk",
        "kSkkkSSSSSSkkkSk",
        "kkrhrkSSSSkrhrkk",
        "kkrrrkSSSSkrrrkk",
        "kkrrrkSSSSkrrrkk",
        "kSkkkSSSSSSkkkSk",
        "kSSSSSSSSSSSSSSk",
        "kSkSkSkSkSkSkSkk",
        "kSSSSSSSSSSSSSSk",
        "kSSSSSSSSSSSSSSk",
        "kkkkkkkkkkkkkkkk",
    ],
];

/// A conveyor belt moving right, 4 frames (flip it for one moving left).
pub fn belt() -> Vec<Pixels> {
    BELT.iter().map(|g| grid(g, BELT_PAL)).collect()
}

// ---------------------------------------------------------------- fan: a vent grate

const FAN_PAL: Palette = &[
    ('k', hex(0x16181e)),
    ('S', hex(0x3a98c8)), // frame: a bright vent blue
    ('s', hex(0x9ee4ff)), // frame lit
    ('d', hex(0x1c2c40)), // the dark shaft behind the blades
    ('b', hex(0xd8f6ff)), // blades
    ('h', hex(0xffffff)), // hub shine
    ('g', hex(0x24506a)), // vents
];
const FAN: [&[&str]; 2] = [
    &[
        "kkkkkkkkkkkkkkkk",
        "kSSSSSSSSSSSSSSk",
        "kssssssssssssssk",
        "kskddddbbddddksk",
        "ksddddbbbbddddsk",
        "ksdddddbbdddddsk",
        "ksbbdddhhdddbbsk",
        "ksbbbbhkkhbbbbsk",
        "ksbbdddhhdddbbsk",
        "ksdddddbbdddddsk",
        "ksddddbbbbddddsk",
        "kskddddbbddddksk",
        "kSSSSSSSSSSSSSSk",
        "kgkgkgkgkgkgkggk",
        "kSSSSSSSSSSSSSSk",
        "kkkkkkkkkkkkkkkk",
    ],
    &[
        "kkkkkkkkkkkkkkkk",
        "kSSSSSSSSSSSSSSk",
        "kssssssssssssssk",
        "kskdbdddddddbksk",
        "ksdbbbdddddbbbsk",
        "ksddbbbdddbbbdsk",
        "ksddddbhhbddddsk",
        "ksdddddkkdddddsk",
        "ksddddbhhbddddsk",
        "ksddbbbdddbbbdsk",
        "ksdbbbdddddbbbsk",
        "kskdbdddddddbksk",
        "kSSSSSSSSSSSSSSk",
        "kgkgkgkgkgkgkggk",
        "kSSSSSSSSSSSSSSk",
        "kkkkkkkkkkkkkkkk",
    ],
];

/// A fan in its grate, blades turning: 2 frames.
pub fn fan() -> Vec<Pixels> {
    FAN.iter().map(|g| grid(g, FAN_PAL)).collect()
}

/// A puff of the fan's updraft (white, faded; tint it). 4x6.
pub fn wisp() -> Vec<Pixels> {
    vec![grid(WISP, WISP_PAL)]
}

const WISP_PAL: Palette = &[('w', [255, 255, 255, 255]), ('W', [255, 255, 255, 150])];
const WISP: &[&str] = &[".W..", ".ww.", "..w.", "..wW", ".ww.", ".W.."];

// ---------------------------------------------------------------- the secret's grate

const GRATE_PAL: Palette = &[
    ('k', hex(0x16181e)),
    ('B', hex(0x8a909c)), // bars
    ('L', hex(0xc8ced8)), // bars lit
    ('r', hex(0xb06030)), // rust
];
/// Bars over a hole in the wall, 16x32: the transparent gaps show the dark room behind (and
/// Han in it).
const GRATE: &[&str] = &[
    "kkkkkkkkkkkkkkkk",
    "kBBBBBBBBBBBBBBk",
    "kBLkLkkLkkLkLBBk",
    "kBkkLkkLkkLkkBBk",
    "kB.kL..L..L.kBBk",
    "kB..L..L..L..BBk",
    "kB..L..L..L..BBk",
    "kB..L..L..L..BBk",
    "kB..L..L..L..BBk",
    "kB..L..L..L..BBk",
    "kB..L..L..L..BBk",
    "kB..L..L..L..BBk",
    "kBLLBLLBLLBLLBBk",
    "kBkkBkkBkkBkkBBk",
    "kB..L..L..L..BBk",
    "kB..L..L..L..BBk",
    "kB..L..L..L..BBk",
    "kB..L..L..L..BBk",
    "kB..L..L..L..BBk",
    "kB..L..L..L..BBk",
    "kB..L..L..L..BBk",
    "kB..L..L..Lr.BBk",
    "kBLLBLLBLLBLLBBk",
    "kBkkBkkBkkBkkBBk",
    "kB..L..L..L..BBk",
    "kB..L..L..L..BBk",
    "kB..L..L.rL..BBk",
    "kB..L..L..L..BBk",
    "kB..L..L..L..BBk",
    "kBrrBBBBBBBBBBBk",
    "kBBBBBBBBBBBBBBk",
    "kkkkkkkkkkkkkkkk",
];

/// The secret's grate (`game::secret`): bars over a hole in the wall. 16x32.
pub fn grate() -> Vec<Pixels> {
    vec![grid(GRATE, GRATE_PAL)]
}

// ---------------------------------------------------------------- bumper: a rubber duck

const DUCK_PAL: Palette = &[
    ('k', hex(0x3a2410)),
    ('y', hex(0xf8d838)), // duck
    ('Y', hex(0xd8a818)), // shade
    ('w', hex(0xffffff)), // eye shine
    ('o', hex(0xf08020)), // beak
    ('R', hex(0xe03848)), // the rubber bumper on its face
];
const BUMPER: &[&str] = &[
    "................",
    "......kkkk......",
    ".....kyyyyk.....",
    "....kyyywkyk.kk.",
    "....kyyyyyyk.kRk",
    "....kyyyyyykoRRk",
    ".kk..kyyyykooRRk",
    "kyyk.kyyyyk.kRRk",
    "kyyykyyyyyyykRRk",
    "kyyyyyyyyyyyyRRk",
    ".kyyyyyyyyyykRRk",
    ".kYyyyyyyyyYkRRk",
    "..kYYyyyyyYYkRRk",
    "...kYYYYYYYk.kRk",
    "....kkkkkkkk..kk",
    "kkkkkkkkkkkkkkkk",
];

/// A bumper: a rubber duck with a red rubber face on its right (flip it for a left one).
pub fn bumper() -> Vec<Pixels> {
    vec![grid(BUMPER, DUCK_PAL)]
}

// ---------------------------------------------------------------- beat blocks

const RED_PAL: Palette = &[('k', hex(0x401018)), ('w', hex(0xffb0b8)), ('c', hex(0xe03848)), ('C', hex(0xa01c30))];
const BLUE_PAL: Palette = &[('k', hex(0x101c40)), ('w', hex(0xb0d0ff)), ('c', hex(0x3870e0)), ('C', hex(0x1c3ca0))];
const BEAT: [&[&str]; 2] = [
    &[
        "kkkkkkkkkkkkkkkk",
        "kwwwwwwwwwwwwwCk",
        "kwccccccccccccCk",
        "kwcccccckkccccCk",
        "kwcccccckwkcccCk",
        "kwcccccckcwkccCk",
        "kwcccccckccccCCk",
        "kwcccccckccccCCk",
        "kwccckkkkccccCCk",
        "kwcckkkkkccccCCk",
        "kwcckkkkcccccCCk",
        "kwccckkccccccCCk",
        "kwccccccccccCCCk",
        "kwCCCCCCCCCCCCCk",
        "kCCCCCCCCCCCCCCk",
        "kkkkkkkkkkkkkkkk",
    ],
    &[
        "cc..cc..cc..cc..",
        "c..............c",
        "................",
        "c..............c",
        "c.......c......c",
        "........cc......",
        "........c.c.....",
        "c.......c......c",
        "c....ccccc.....c",
        ".....cccc.......",
        "................",
        "c..............c",
        "c..............c",
        "................",
        "c..............c",
        "..cc..cc..cc..cc",
    ],
];

/// Red beat blocks: there (a note on it), and gone (a dotted outline).
pub fn beat_red() -> Vec<Pixels> {
    BEAT.iter().map(|g| grid(g, RED_PAL)).collect()
}

/// Blue beat blocks: there, and gone.
pub fn beat_blue() -> Vec<Pixels> {
    BEAT.iter().map(|g| grid(g, BLUE_PAL)).collect()
}

#[cfg(test)]
pub fn grids() -> Vec<(&'static str, Vec<&'static str>, Palette)> {
    let mut v: Vec<(&'static str, Vec<&'static str>, Palette)> = vec![
        ("BRICK", BRICK.to_vec(), SOAP_PAL),
        ("CRUSHER_BODY", CRUSHER_BODY.to_vec(), STEEL_PAL),
        ("CRUSHER_FACE", CRUSHER_FACE.to_vec(), STEEL_PAL),
        ("CRUSHER_STRIPES", CRUSHER_STRIPES.to_vec(), STEEL_PAL),
    ];
    for g in CRUMBLE {
        v.push(("CRUMBLE", g.to_vec(), CARD_PAL));
    }
    for g in TRAMPOLINE {
        v.push(("TRAMPOLINE", g.to_vec(), CUSHION_PAL));
    }
    for g in BELT {
        v.push(("BELT", g.to_vec(), BELT_PAL));
    }
    for g in FAN {
        v.push(("FAN", g.to_vec(), FAN_PAL));
    }
    for g in BEAT {
        v.push(("BEAT", g.to_vec(), RED_PAL));
        v.push(("BEAT", g.to_vec(), BLUE_PAL));
    }
    v.push(("BUMPER", BUMPER.to_vec(), DUCK_PAL));
    v.push(("WISP", WISP.to_vec(), WISP_PAL));
    v.push(("GRATE", GRATE.to_vec(), GRATE_PAL));
    v
}

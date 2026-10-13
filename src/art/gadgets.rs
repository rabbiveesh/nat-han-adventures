//! The gadget tiles: breakable bricks (a bar of old soap), crumbling platforms (soggy
//! cardboard), trampolines (a whoopee cushion) and crushers (a riveted steel block with an
//! angry face and hazard stripes on its slamming side).

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
    v
}

//! Things in the sky: the Boom (a plump purple flying pal who runs on beans, puffing out the
//! back), its beans, and the sploosh diapers (a nappy with little wings).

use super::items::OUTLINE;
use super::palette::*;

// ---------------------------------------------------------------- the Boom

const BOOM_PAL: Palette = &[
    ('k', OUTLINE),
    ('p', hex(0xa070e0)), // body
    ('P', hex(0x6c44a8)), // shade
    ('w', hex(0xe8d8ff)), // shine
    ('e', hex(0xffffff)), // eyes
    ('o', hex(0x241c30)), // pupils
    ('r', hex(0xff7090)), // grin
    ('g', hex(0x9ad860)), // bean puff
    ('G', hex(0x5c9a38)),
];

/// Its body sits in the middle 10 rows (its box), the puff out the back (left).
const BOOM: [&[&str]; 2] = [
    &[
        "................",
        "................",
        "................",
        "....kkkkkkkk....",
        "...kwwppppppk...",
        "..kwppeekeepPk..",
        "..kppeoekeoePk..",
        "gkpppeeekeeepPk.",
        "GgkpppppppppPPk.",
        "gkppprrrrrrpPPk.",
        "..kpppkrrrkpPPk.",
        "..kPpppppppPPk..",
        "...kPPPPPPPPk...",
        "....kkkkkkkk....",
        "................",
        "................",
    ],
    &[
        "................",
        "................",
        "................",
        "....kkkkkkkk....",
        "...kwwppppppk...",
        "..kwppeekeepPk..",
        "..kppeoekeoePk..",
        ".kkpppeeekeeepPk",
        "ggGkppppppppPPk.",
        "Ggkppprrrrrrpk..",
        "g.kpppkrrrkpPPk.",
        "..kPpppppppPPk..",
        "...kPPPPPPPPk...",
        "....kkkkkkkk....",
        "................",
        "................",
    ],
];

pub fn boom() -> Vec<Pixels> {
    BOOM.iter().map(|g| grid(g, BOOM_PAL)).collect()
}

const BEAN_PAL: Palette = &[('k', OUTLINE), ('b', hex(0xc0502c)), ('B', hex(0x84301c)), ('w', hex(0xffc8a0))];

const BEAN: &[&str] = &[
    "........",
    "..kkkk..",
    ".kwbbbk.",
    "kwbbbbBk",
    "kbbkkbBk",
    ".kk..kk.",
    "........",
    "........",
];

pub fn bean() -> Vec<Pixels> {
    vec![grid(BEAN, BEAN_PAL)]
}

// ---------------------------------------------------------------- sploosh diapers

const DIAPER_PAL: Palette = &[
    ('k', OUTLINE),
    ('w', hex(0xfafaff)), // diaper
    ('W', hex(0xc4c8dc)), // shade
    ('t', hex(0xff90c0)), // tabs
    ('c', hex(0xd0ecff)), // wings
];

/// Two flaps, then the drop (wings up, falling).
const DIAPER: [&[&str]; 3] = [
    &[
        "................",
        "................",
        "................",
        ".kk..........kk.",
        "kcck........kcck",
        "kccck.kkkk.kccck",
        ".kcckkwwwwkkcck.",
        "..kktwwwwwwtkk..",
        "..ktwwwwwwwwtk..",
        "..kwwwwwwwwwWk..",
        "...kwwwwwwwWk...",
        "....kWwwwwWk....",
        ".....kWWWWk.....",
        "......kkkk......",
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
        "..kkkkwwwwkkkk..",
        ".kccktwwwwwtkcck",
        "kccktwwwwwwwtkcc",
        "kkk.kwwwwwwWk.kk",
        "...kwwwwwwwWk...",
        "....kWwwwwWk....",
        ".....kWWWWk.....",
        "......kkkk......",
        "................",
        "................",
    ],
    &[
        "................",
        "................",
        "...kk......kk...",
        "..kcck....kcck..",
        "..kcck....kcck..",
        "...kck.kk.kck...",
        "....kkwwwwkk....",
        "...ktwwwwwwtk...",
        "..ktwwwwwwwwtk..",
        "..kwwwwwwwwwWk..",
        "...kwwwwwwwWk...",
        "....kWwwwwWk....",
        ".....kWWWWk.....",
        "......kkkk......",
        "................",
        "................",
    ],
];

pub fn diaper() -> Vec<Pixels> {
    DIAPER.iter().map(|g| grid(g, DIAPER_PAL)).collect()
}

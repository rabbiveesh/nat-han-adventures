//! Per-world terrain: ground top / fill, one-way platforms, and deadly liquid.
//!
//! Seamlessness: every GroundTop is its world's cap rows stacked on the *same* rows of the fill
//! tile, so a top tile sits on a fill tile (and next to either) without a seam.

use super::palette::*;

pub const WORLDS: std::ops::RangeInclusive<u8> = 1..=5;

/// Clamp a world number into 1..=5 (anything else uses the nearest world's art).
pub fn world_index(w: u8) -> usize {
    (w.clamp(1, 5) - 1) as usize
}

struct World {
    pal: Palette,
    fill: &'static [&'static str],
    /// Rows drawn over the top of the fill to make the surface tile.
    cap: &'static [&'static str],
    oneway: &'static [&'static str],
}

// ---------------------------------------------------------------- 1: bathroom

const BATH_PAL: Palette = &[
    ('k', hex(0x283a5a)), // dark blue edge
    ('h', hex(0xffffff)), // highlight
    ('w', hex(0xe4ecf8)), // white tile
    ('l', hex(0xb8c6dc)), // white tile shade
    ('c', hex(0x8cbce8)), // blue tile
    ('C', hex(0x5a8cc8)), // blue tile shade
    ('g', hex(0x6a7890)), // grout
];
const BATH_FILL: &[&str] = &[
    "hwwwwwwghcccccCg",
    "wwwwwwlgccccccCg",
    "wwwwwwlgccccccCg",
    "wwwwwwlgccccccCg",
    "wwwwwwlgccccccCg",
    "wwwwwwlgccccccCg",
    "lllllllgCCCCCCCg",
    "gggggggggggggggg",
    "hcccccCghwwwwwwg",
    "ccccccCgwwwwwwlg",
    "ccccccCgwwwwwwlg",
    "ccccccCgwwwwwwlg",
    "ccccccCgwwwwwwlg",
    "ccccccCgwwwwwwlg",
    "CCCCCCCglllllllg",
    "gggggggggggggggg",
];
const BATH_CAP: &[&str] = &[
    "kkkkkkkkkkkkkkkk",
    "hhhhhhhhhhhhhhhh",
    "wwwwwwwwwwwwwwww",
    "llllllllllllllll",
    "kkkkkkkkkkkkkkkk",
];
const BATH_ONEWAY: &[&str] = &[
    "kkkkkkkkkkkkkkkk",
    "hhhhhhhhhhhhhhhh",
    "wwwwwwwwwwwwwwww",
    "llllllllllllllll",
    "kkkkkkkkkkkkkkkk",
    "..kgk......kgk..",
    "...kk......kk...",
];

// ---------------------------------------------------------------- 2: pipes

const PIPE_PAL: Palette = &[
    ('k', hex(0x1c1616)),
    ('O', hex(0xf0a868)), // copper light
    ('o', hex(0xc87a3a)), // copper
    ('d', hex(0x7a4220)), // copper dark
    ('g', hex(0x9ac8b0)), // verdigris light
    ('v', hex(0x6a9a88)), // verdigris
    ('V', hex(0x3e6458)), // verdigris dark
    ('r', hex(0xf0e0b0)), // rivet
];
const PIPE_FILL: &[&str] = &[
    "OOOOOOOOOkOOkOOO",
    "oooooooookOokooo",
    "oooooooookrrkooo",
    "oooooooookookooo",
    "oooooooookookooo",
    "dddddddddkodkddd",
    "dddddddddkddkddd",
    "kkkkkkkkkkkkkkkk",
    "gkggkggggggggggg",
    "vkvvkvvvvvvvvvvv",
    "vkrrkvvvvvvvvvvv",
    "vkvvkvvvvvvvvvvv",
    "VkvVkVVVVVVVVVVV",
    "VkVVkVVVVVVVVVVV",
    "VkVVkVVVVVVVVVVV",
    "kkkkkkkkkkkkkkkk",
];
const PIPE_CAP: &[&str] = &[
    "kkkkkkkkkkkkkkkk",
    "OOOOOOOOOOOOOOOO",
    "oorooooooorooooo",
    "oooooooooooooooo",
    "oooooooookookooo",
];
const PIPE_ONEWAY: &[&str] = &[
    "kkkkkkkkkkkkkkkk",
    "OOOOOOOOOOOOOOOO",
    "oooooooooooooooo",
    "dddddddddddddddd",
    "kkkkkkkkkkkkkkkk",
    "...kok......kok.",
    "...kdk......kdk.",
];

// ---------------------------------------------------------------- 3: sewer

const SEWER_PAL: Palette = &[
    ('k', hex(0x1a1418)), // mortar
    ('h', hex(0x7a5e52)), // brick highlight
    ('b', hex(0x5e4640)), // brick
    ('B', hex(0x44322e)), // brick shade
    ('m', hex(0x3e6a26)), // moss
    ('M', hex(0x6e9e34)), // moss light
    ('w', hex(0x8a6038)), // plank
    ('W', hex(0x5a3a20)), // plank shade
];
const SEWER_FILL: &[&str] = &[
    "hhhhhhhkhhhhhhhk",
    "bbbbbbbkbbbbbbBk",
    "bbbbbbBkbbbbbbBk",
    "kkkkkkkkkkkkkkkk",
    "hhhkhhhhhhhkhhhh",
    "bbbkbbbbbbBkbbbb",
    "BbBkbbbbbbBkbbbB",
    "kkkkkkkkkkkkkkkk",
    "hhhhhhhkhhhhhhhk",
    "bbbbbbBkbbbbbbbk",
    "bbbbbBBkBbbbbbBk",
    "kkkkkkkkkkkkkkkk",
    "hhhkhhhhhhhkhhhh",
    "bbbkbbbbbbbkbbbb",
    "bBBkbbbbbbBkBbbb",
    "kkkkkkkkkkkkkkkk",
];
const SEWER_CAP: &[&str] = &[
    "MMMMMMMMMMMMMMMM",
    "MmMMMmMMMMmMMMmM",
    "mmmmmmmmmmmmmmmm",
    "kmmkmmmkkmmmkmmk",
    "hmhkhhhhhmhkhhmh",
];
const SEWER_ONEWAY: &[&str] = &[
    "MMMmMMMMMMMmMMMM",
    "mmmmmmmmmmmmmmmm",
    "wwwwwwwkwwwwwwwk",
    "WWWWWWWkWWWWWWWk",
    "kkkkkkkkkkkkkkkk",
    "..m..........m..",
];

// ---------------------------------------------------------------- 4: septic & festival

const SEPTIC_PAL: Palette = &[
    ('k', hex(0x2a2a28)),
    ('c', hex(0x8a8a84)), // concrete
    ('C', hex(0x66665e)), // concrete dark
    ('d', hex(0xaaaaa2)), // speckle
    ('g', hex(0x3aa04a)), // green plastic
    ('G', hex(0x1f6a2a)), // dark green
    ('L', hex(0x7ad86a)), // light green
];
const SEPTIC_FILL: &[&str] = &[
    "ccccdccccccccccC",
    "cccccccccCcccccC",
    "ccCcccccccccdccC",
    "cccccccdcccccccC",
    "cdcccccccccCcccC",
    "ccccCccccccccccC",
    "ccccccccdccccdcC",
    "ccdccccccCcccccC",
    "cccccCcccccccccC",
    "cccccccccccdcccC",
    "cCccdccccccccccC",
    "cccccccCcccccdcC",
    "cccdcccccccCcccC",
    "cccccccdcccccccC",
    "cdccCccccccccccC",
    "CCCCCCCCCCCCCCCC",
];
const SEPTIC_CAP: &[&str] = &[
    "GGGGGGGGGGGGGGGG",
    "LLLLLLLLLLLLLLLL",
    "gggGgggGgggGgggG",
    "gggGgggGgggGgggG",
    "GGGGGGGGGGGGGGGG",
    "kkkkkkkkkkkkkkkk",
];
const SEPTIC_ONEWAY: &[&str] = &[
    "GGGGGGGGGGGGGGGG",
    "LLLLLLLLLLLLLLLL",
    "gggGgggGgggGgggG",
    "GGGGGGGGGGGGGGGG",
    ".kGk........kGk.",
    ".kGk........kGk.",
];

// ---------------------------------------------------------------- 5: treatment plant / throne

const PLANT_PAL: Palette = &[
    ('k', hex(0x1a1e28)),
    ('h', hex(0xc0ccd8)), // steel highlight
    ('s', hex(0x8a98a8)), // steel
    ('S', hex(0x5a6878)), // steel dark
    ('y', hex(0xffd24a)), // gold
    ('Y', hex(0xb88a1a)), // gold dark
];
const PLANT_FILL: &[&str] = &[
    "hhhhhhhhhhhhhhhS",
    "hYssssssssssssYS",
    "hssssssssssssssS",
    "hsshSsssshSssssS",
    "hssssssssssssssS",
    "hssssshSssssshSS",
    "hssssssssssssssS",
    "hsshSsssshSssssS",
    "hssssssssssssssS",
    "hssssshSssssshSS",
    "hssssssssssssssS",
    "hsshSsssshSssssS",
    "hssssssssssssssS",
    "hssssshSssssshSS",
    "hYssssssssssssYS",
    "SSSSSSSSSSSSSSSS",
];
const PLANT_CAP: &[&str] = &[
    "yyyyyyyyyyyyyyyy",
    "YYYYYYYYYYYYYYYY",
    "kkkkkkkkkkkkkkkk",
    "hskkhskkhskkhskk",
    "hskkhskkhskkhskk",
    "SSSSSSSSSSSSSSSS",
    "kkkkkkkkkkkkkkkk",
];
const PLANT_ONEWAY: &[&str] = &[
    "yyyyyyyyyyyyyyyy",
    "YYYYYYYYYYYYYYYY",
    "hskkhskkhskkhskk",
    "hskkhskkhskkhskk",
    "SSSSSSSSSSSSSSSS",
    ".kSk........kSk.",
];

const WORLD_DATA: [World; 5] = [
    World { pal: BATH_PAL, fill: BATH_FILL, cap: BATH_CAP, oneway: BATH_ONEWAY },
    World { pal: PIPE_PAL, fill: PIPE_FILL, cap: PIPE_CAP, oneway: PIPE_ONEWAY },
    World { pal: SEWER_PAL, fill: SEWER_FILL, cap: SEWER_CAP, oneway: SEWER_ONEWAY },
    World { pal: SEPTIC_PAL, fill: SEPTIC_FILL, cap: SEPTIC_CAP, oneway: SEPTIC_ONEWAY },
    World { pal: PLANT_PAL, fill: PLANT_FILL, cap: PLANT_CAP, oneway: PLANT_ONEWAY },
];

pub fn ground_fill(w: u8) -> Vec<Pixels> {
    let d = &WORLD_DATA[world_index(w)];
    vec![grid(d.fill, d.pal)]
}

pub fn ground_top(w: u8) -> Vec<Pixels> {
    let d = &WORLD_DATA[world_index(w)];
    let mut rows: Vec<&str> = d.cap.to_vec();
    rows.extend_from_slice(&d.fill[d.cap.len()..]);
    vec![grid(&rows, d.pal)]
}

pub fn one_way(w: u8) -> Vec<Pixels> {
    let d = &WORLD_DATA[world_index(w)];
    let mut out = Pixels::new(16, 16);
    out.blit(&grid(d.oneway, d.pal), 0, 0);
    vec![out]
}

// ---------------------------------------------------------------- liquids

/// Liquid colours per world: foam, light, mid, dark.
const LIQUID_COLOURS: [[Rgba; 4]; 5] = [
    // 1: bluish toilet water
    [hex(0xe8f8ff), hex(0x7ac8f0), hex(0x3a8ad0), hex(0x2a64a8)],
    // 2: murky green-brown (pipes)
    [hex(0xc8c890), hex(0x8a8a48), hex(0x5e6030), hex(0x44461e)],
    // 3: darker sewer sludge
    [hex(0xb8b080), hex(0x7a7040), hex(0x524a28), hex(0x3a341a)],
    // 4: septic green-brown
    [hex(0xd0d090), hex(0x90983a), hex(0x626a22), hex(0x464c16)],
    // 5: toxic bright green
    [hex(0xf0ffb0), hex(0xa8ff40), hex(0x50d020), hex(0x2a9a18)],
];

/// Surface wave, 8 wide (tiles twice per tile), rows 0..=5 of LiquidTop.
const WAVE: &[&str] = &[
    "........",
    "...ww...",
    ".wwLLww.",
    "wLLmmLLw",
    "LmmmmmmL",
    "mmmmmmmm",
];
/// Liquid body, 16x16, tiles in every direction (bubbles `L`, dither `d`).
const BODY: &[&str] = &[
    "mmmmmmmmmmmmmmmm",
    "mmmmdmmmmmmmmmmm",
    "mmmmmmmmmmmLmmmm",
    "mmmmmmmmmmmmmmmm",
    "mmdmmmmmmdmmmmmm",
    "mmmmmmmmmmmmmmdm",
    "mmmmmLmmmmmmmmmm",
    "mmmmmmmmmmmmmmmm",
    "mmmmmmmmmmdmmmmm",
    "mdmmmmmmmmmmmmmm",
    "mmmmmmmmmmmmLmmm",
    "mmmmmmdmmmmmmmmm",
    "mmLmmmmmmmmmmdmm",
    "mmmmmmmmmmmmmmmm",
    "mmmmmmmmdmmmmmmm",
    "mmmmdmmmmmmmmmmm",
];

fn liquid_pal(w: u8) -> [(char, Rgba); 4] {
    let c = LIQUID_COLOURS[world_index(w)];
    [('w', c[0]), ('L', c[1]), ('m', c[2]), ('d', c[3])]
}

pub fn liquid_fill(w: u8) -> Vec<Pixels> {
    vec![grid(BODY, &liquid_pal(w))]
}

pub fn liquid_top(w: u8) -> Vec<Pixels> {
    let pal = liquid_pal(w);
    let body = grid(BODY, &pal);
    let wave = grid(WAVE, &pal);
    [0i32, 3, 5]
        .iter()
        .map(|&shift| {
            let mut p = Pixels::new(16, 16);
            for y in WAVE.len() as i32..16 {
                for x in 0..16 {
                    p.set(x, y, body.get(x, y));
                }
            }
            for x in 0..16 {
                for y in 0..WAVE.len() as i32 {
                    let c = wave.get((x + shift).rem_euclid(8), y);
                    if c[3] > 0 {
                        p.set(x, y, c);
                    }
                }
            }
            p
        })
        .collect()
}

#[cfg(test)]
/// A palette containing every liquid char (colours irrelevant), for grid validation.
const LIQUID_CHARS: Palette = &[('w', CLEAR), ('L', CLEAR), ('m', CLEAR), ('d', CLEAR)];

#[cfg(test)]
pub fn grids() -> Vec<(&'static str, Vec<&'static str>, Palette)> {
    let mut v: Vec<(&'static str, Vec<&'static str>, Palette)> = vec![
        ("WAVE", WAVE.to_vec(), LIQUID_CHARS),
        ("BODY", BODY.to_vec(), LIQUID_CHARS),
    ];
    for d in &WORLD_DATA {
        v.push(("fill", d.fill.to_vec(), d.pal));
        v.push(("cap", d.cap.to_vec(), d.pal));
        v.push(("oneway", d.oneway.to_vec(), d.pal));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_and_fills_are_tile_sized() {
        for d in &WORLD_DATA {
            assert_eq!(d.fill.len(), 16);
            assert!(d.cap.len() < 16);
            assert!(d.oneway.len() <= 8);
        }
    }

    /// The bottom row of GroundTop continues into the top row of GroundFill exactly like fill-on-fill.
    #[test]
    fn top_matches_fill_below() {
        for w in WORLDS {
            let top = &ground_top(w)[0];
            let fill = &ground_fill(w)[0];
            for x in 0..16 {
                assert_eq!(top.get(x, 15), fill.get(x, 15), "world {w}");
            }
        }
    }
}

//! The robot pit's wrong game (`game::robot`): Sparky's sunny meadow, in Robot Buddy
//! Adventure's colours (its grass, sandy path, red roofs, Sparky's grey body, green eyes and
//! red antenna ball), at our 16px.

use super::palette::*;

const GRASS: Rgba = hex(0x4caf50);
const GRASS_DARK: Rgba = hex(0x43a047);
const GRASS_LIGHT: Rgba = hex(0x66bb6a);
const SAND: Rgba = hex(0xdeb887);
const SAND_DARK: Rgba = hex(0xc8a070);
const WATER: Rgba = hex(0x42a5f5);
const WATER_LIGHT: Rgba = hex(0x90caf9);
const WATER_DARK: Rgba = hex(0x1e88e5);

// ---------------------------------------------------------------- ground

fn noisy(base: Rgba, specks: &[(Rgba, u32)], seed: u32) -> Pixels {
    let mut p = Pixels::new(16, 16);
    for y in 0..16 {
        for x in 0..16 {
            let h = hash2(x, y, seed) % 100;
            let mut c = base;
            let mut acc = 0;
            for &(s, pct) in specks {
                acc += pct;
                if h < acc {
                    c = s;
                    break;
                }
            }
            p.set(x, y, c);
        }
    }
    p
}

pub fn grass() -> Vec<Pixels> {
    let mut p = noisy(GRASS, &[], 0);
    // Little blades.
    for (x, y) in [(2, 3), (10, 1), (13, 9), (5, 11), (8, 6), (1, 14), (12, 14)] {
        p.set(x, y, GRASS_DARK);
        p.set(x, y + 1, GRASS_DARK);
        p.set(x + 1, y, GRASS_LIGHT);
    }
    vec![p]
}

pub fn path() -> Vec<Pixels> {
    vec![noisy(SAND, &[(SAND_DARK, 8)], 7)]
}

pub fn water() -> Vec<Pixels> {
    (0..2)
        .map(|f| {
            let mut p = noisy(WATER, &[(WATER_DARK, 4)], 3);
            for (x, y) in [(2, 3), (9, 6), (4, 11), (11, 13)] {
                let x = x + 2 * f;
                p.fill_rect(x, y, 3, 1, WATER_LIGHT);
            }
            p
        })
        .collect()
}

/// The whirlpool home: a spiral turning in the water, 4 frames.
pub fn whirlpool() -> Vec<Pixels> {
    (0..4)
        .map(|f| {
            let mut p = Pixels::new(16, 16);
            p.fill_rect(0, 0, 16, 16, WATER);
            for i in 0..36 {
                let a = i as f32 * 0.45 + f as f32 * std::f32::consts::FRAC_PI_2;
                let r = 7.0 - i as f32 * 0.18;
                let (x, y) = (7.5 + r * a.cos(), 7.5 + r * a.sin());
                p.set(x.round() as i32, y.round() as i32, if i % 3 == 0 { WATER_DARK } else { WATER_LIGHT });
            }
            p.fill_rect(7, 7, 2, 2, WATER_DARK);
            p
        })
        .collect()
}

// ---------------------------------------------------------------- things on grass

const TREE_PAL: Palette = &[
    ('k', hex(0x1b5e20)),
    ('g', hex(0x2e7d32)),
    ('l', hex(0x43a047)),
    ('t', hex(0x6d4c41)),
];
const TREE: &[&str] = &[
    "................",
    ".....kkkkkk.....",
    "...kkggllggkk...",
    "..kgggllllgggk..",
    ".kggglllggggggk.",
    ".kggllggggggggk.",
    "kgggggggggllgggk",
    "kggggggggllllggk",
    "kgggllgggglggggk",
    ".kggllllggggggk.",
    ".kgggllggggggk..",
    "..kkggggggggk...",
    "...kkkggggkk....",
    "......kttk......",
    "......kttk......",
    ".......kk.......",
];

const FLOWER_PAL: Palette = &[('r', hex(0xff5252)), ('y', hex(0xffeb3b)), ('w', hex(0xffffff)), ('p', hex(0xce93d8))];
const FLOWERS: &[&str] = &[
    "................",
    "..r.............",
    ".ryr.......w....",
    "..r.......wyw...",
    "...........w....",
    "................",
    "......p.........",
    ".....pyp........",
    "......p.....r...",
    "...........ryr..",
    "............r...",
    "..w.............",
    ".wyw.....p......",
    "..w.....pyp.....",
    ".........p......",
    "................",
];

const FENCE_PAL: Palette = &[('k', hex(0x5d4037)), ('b', hex(0xa1887f))];
const FENCE: &[&str] = &[
    "................",
    "................",
    "..kk.......kk...",
    ".kbbk.....kbbk..",
    "kkbbkkkkkkkbbkkk",
    "bbbbbbbbbbbbbbbb",
    "kkbbkkkkkkkbbkkk",
    ".kbbk.....kbbk..",
    "kkbbkkkkkkkbbkkk",
    "bbbbbbbbbbbbbbbb",
    "kkbbkkkkkkkbbkkk",
    ".kbbk.....kbbk..",
    ".kbbk.....kbbk..",
    ".kkkk.....kkkk..",
    "................",
    "................",
];

const CHEST_PAL: Palette = &[
    ('k', hex(0x3e2723)),
    ('b', hex(0x8d6e63)),
    ('B', hex(0x6d4c41)),
    ('y', hex(0xffd54f)),
    ('r', hex(0xff5252)),
    ('w', hex(0xffffff)),
    ('d', hex(0x212121)),
];
const CHEST: [&[&str]; 2] = [
    &[
        "................",
        "................",
        "................",
        "..kkkkkkkkkkkk..",
        ".kbbbbbbbbbbbbk.",
        ".kbBBBBBBBBBBbk.",
        ".kbbbbbbbbbbbbk.",
        ".kkkkkkyykkkkkk.",
        ".kbbbbbyybbbbbk.",
        ".kbbbbbbbbbbbbk.",
        ".kbBBBBBBBBBBbk.",
        ".kbbbbbbbbbbbbk.",
        ".kbbbbbbbbbbbbk.",
        ".kkkkkkkkkkkkkk.",
        "................",
        "................",
    ],
    // Open: a Dum Dum sticking out.
    &[
        "......rrr.......",
        ".....rrwrr......",
        ".....rrrrr......",
        "..kkkkrrrkkkkk..",
        ".kddddddddddddk.",
        ".kddddddwddddddk",
        ".kdddddwddddddk.",
        ".kkkkkkyykkkkkk.",
        ".kbbbbbyybbbbbk.",
        ".kbbbbbbbbbbbbk.",
        ".kbBBBBBBBBBBbk.",
        ".kbbbbbbbbbbbbk.",
        ".kbbbbbbbbbbbbk.",
        ".kkkkkkkkkkkkkk.",
        "................",
        "................",
    ],
];

fn on_grass(top: &Pixels) -> Pixels {
    let mut p = grass().remove(0);
    p.blit(top, 0, 0);
    p
}

pub fn tree() -> Vec<Pixels> {
    vec![on_grass(&grid(TREE, TREE_PAL))]
}

pub fn flowers() -> Vec<Pixels> {
    vec![on_grass(&grid(FLOWERS, FLOWER_PAL))]
}

pub fn fence() -> Vec<Pixels> {
    vec![on_grass(&grid(FENCE, FENCE_PAL))]
}

pub fn chest() -> Vec<Pixels> {
    CHEST.iter().map(|g| on_grass(&grid(g, CHEST_PAL))).collect()
}

// ---------------------------------------------------------------- the house

const ROOF: Rgba = hex(0xc62828);
const ROOF_DARK: Rgba = hex(0x8e0000);
const WALL: Rgba = hex(0xfff3e0);
const WALL_LINE: Rgba = hex(0xd7ccc8);
const WOOD: Rgba = hex(0x8d6e63);
const WOOD_DARK: Rgba = hex(0x5d4037);
const GLASS: Rgba = hex(0x90caf9);

pub fn roof() -> Vec<Pixels> {
    let mut p = Pixels::new(16, 16);
    p.fill_rect(0, 0, 16, 16, ROOF);
    for y in [3, 7, 11, 15] {
        p.fill_rect(0, y, 16, 1, ROOF_DARK);
        let off = if y % 8 == 3 { 0 } else { 4 };
        for x in (off..16).step_by(8) {
            p.fill_rect(x, y - 3, 1, 3, ROOF_DARK);
        }
    }
    vec![p]
}

fn wall_base() -> Pixels {
    let mut p = Pixels::new(16, 16);
    p.fill_rect(0, 0, 16, 16, WALL);
    for y in [5, 11] {
        p.fill_rect(0, y, 16, 1, WALL_LINE);
    }
    p
}

pub fn wall() -> Vec<Pixels> {
    vec![wall_base()]
}

pub fn window() -> Vec<Pixels> {
    let mut p = wall_base();
    p.fill_rect(3, 3, 10, 9, WOOD_DARK);
    p.fill_rect(4, 4, 8, 7, GLASS);
    p.fill_rect(7, 4, 2, 7, WOOD_DARK);
    p.fill_rect(4, 7, 8, 1, WOOD_DARK);
    p.set(5, 5, WALL);
    vec![p]
}

pub fn door() -> Vec<Pixels> {
    let mut p = wall_base();
    p.fill_rect(3, 2, 10, 14, WOOD_DARK);
    p.fill_rect(4, 3, 8, 13, WOOD);
    p.set(10, 9, hex(0xffd54f));
    vec![p]
}

// ---------------------------------------------------------------- answer pads

/// A stone answer pad: fresh, and greyed out once picked wrong. The number is text on top.
pub fn answer_pad() -> Vec<Pixels> {
    [(hex(0xeceff1), hex(0x78909c)), (hex(0x9e9e9e), hex(0x616161))]
        .into_iter()
        .map(|(face, rim)| {
            let mut p = grass().remove(0);
            p.fill_rect(1, 1, 14, 14, rim);
            p.fill_rect(2, 2, 12, 12, face);
            p
        })
        .collect()
}

// ---------------------------------------------------------------- Sparky

const SPARKY_PAL: Palette = &[
    ('o', hex(0x78909c)),
    ('h', hex(0xcfd8dc)),
    ('b', hex(0xb0bec5)),
    ('a', hex(0x90a4ae)),
    ('g', hex(0x00e676)),
    ('p', hex(0x1b5e20)),
    ('r', hex(0xff5252)),
];
const SPARKY: [&[&str]; 2] = [
    &[
        "................",
        ".......rr.......",
        "......rrrr......",
        ".......rr.......",
        ".......oo.......",
        ".......oo.......",
        "...oooooooooo...",
        "...ohhhhhhhho...",
        "...ohgghhggho...",
        "...ohgphhgpho...",
        "...ohhhhhhhho...",
        "...ohhgggghho...",
        "...oooooooooo...",
        "..aaobbbbbboaa..",
        "..aaobbbbbboaa..",
        "..aaobbggbboaa..",
        "..aaobbggbboaa..",
        "..aaobbbbbboaa..",
        "....obbbbbbo....",
        "....oooooooo....",
        ".....oo..oo.....",
        ".....oo..oo.....",
        ".....oo..oo.....",
        "....ooo..ooo....",
    ],
    // Blink, antenna bobbing.
    &[
        "................",
        "................",
        ".......rr.......",
        "......rrrr......",
        ".......rr.......",
        ".......oo.......",
        "...oooooooooo...",
        "...ohhhhhhhho...",
        "...ohhhhhhhho...",
        "...ohgghhggho...",
        "...ohhhhhhhho...",
        "...ohhgggghho...",
        "...oooooooooo...",
        "..aaobbbbbboaa..",
        "..aaobbbbbboaa..",
        "..aaobbggbboaa..",
        "..aaobbggbboaa..",
        "..aaobbbbbboaa..",
        "....obbbbbbo....",
        "....oooooooo....",
        ".....oo..oo.....",
        ".....oo..oo.....",
        ".....oo..oo.....",
        "....ooo..ooo....",
    ],
];

pub fn sparky() -> Vec<Pixels> {
    SPARKY.iter().map(|g| grid(g, SPARKY_PAL)).collect()
}

/// Sparky's antenna poking up at the bottom of a robot pit, its ball lit and dim.
const ANTENNA: [&[&str]; 2] = [
    &[
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        ".......rr.......",
        "......rrrr......",
        "......rrrr......",
        ".......rr.......",
        ".......oo.......",
        ".......oo.......",
        ".......oo.......",
        ".......oo.......",
        ".......oo.......",
    ],
    &[
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        ".......pp.......",
        "......pppp......",
        "......pppp......",
        ".......pp.......",
        ".......oo.......",
        ".......oo.......",
        ".......oo.......",
        ".......oo.......",
        ".......oo.......",
    ],
];
const ANTENNA_PAL: Palette = &[('o', hex(0x78909c)), ('r', hex(0xff5252)), ('p', hex(0x8e2a2a))];

pub fn pit_antenna() -> Vec<Pixels> {
    ANTENNA.iter().map(|g| grid(g, ANTENNA_PAL)).collect()
}

#[cfg(test)]
pub fn grids() -> Vec<(&'static str, Vec<&'static str>, Palette)> {
    let mut v: Vec<(&'static str, Vec<&'static str>, Palette)> = vec![
        ("TREE", TREE.to_vec(), TREE_PAL),
        ("FLOWERS", FLOWERS.to_vec(), FLOWER_PAL),
        ("FENCE", FENCE.to_vec(), FENCE_PAL),
    ];
    for g in CHEST {
        v.push(("CHEST", g.to_vec(), CHEST_PAL));
    }
    for g in SPARKY {
        v.push(("SPARKY", g.to_vec(), SPARKY_PAL));
    }
    for g in ANTENNA {
        v.push(("ANTENNA", g.to_vec(), ANTENNA_PAL));
    }
    v
}

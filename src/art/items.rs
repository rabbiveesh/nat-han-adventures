//! Collectibles, checkpoint, goal, throne, hazards, moving platforms, particles and UI icons.

use super::palette::*;

const OUTLINE: Rgba = hex(0x1b1420);

// ---------------------------------------------------------------- nugget

pub const GOLD_PAL: Palette = &[
    ('k', hex(0x4a2a08)),
    ('y', hex(0xffd94a)),
    ('Y', hex(0xe8a420)),
    ('o', hex(0xa8661a)),
    ('w', hex(0xffffff)),
];

const NUGGET: [&[&str]; 3] = [
    &[
        "................",
        "................",
        "................",
        "................",
        "......kkk.......",
        "....kkywyk......",
        "...kyywyyyk.....",
        "...kyyyyyYYk....",
        "..kYyyyyYYYok...",
        "..kYYyyYYYook...",
        "...kYYYYoook....",
        "....kkkkkkk.....",
        "................",
        "................",
        "................",
        "................",
    ],
    &[
        "................",
        "................",
        "................",
        "................",
        ".......kk.......",
        "......kwyk......",
        ".....kywyyk.....",
        ".....kyyyYk.....",
        "....kYyyYok.....",
        "....kYyYYok.....",
        ".....kYYook.....",
        "......kkkk......",
        "................",
        "................",
        "................",
        "................",
    ],
    &[
        "................",
        "................",
        "................",
        "................",
        ".......kk.......",
        ".......kwk......",
        ".......kyk......",
        ".......kyk......",
        "......kyYk......",
        "......kYok......",
        ".......kok......",
        ".......kk.......",
        "................",
        "................",
        "................",
        "................",
    ],
];

const SPARKLE: &[&str] = &[".w.", "www", ".w."];

pub fn nugget() -> Vec<Pixels> {
    let full = grid(NUGGET[0], GOLD_PAL);
    let half = grid(NUGGET[1], GOLD_PAL);
    let edge = grid(NUGGET[2], GOLD_PAL);
    let mut glint = flip_x(&half);
    glint.blit(&grid(SPARKLE, GOLD_PAL), 10, 3);
    vec![full, half, edge, glint]
}

pub fn flip_x(p: &Pixels) -> Pixels {
    let mut out = Pixels::new(p.w, p.h);
    for y in 0..p.h as i32 {
        for x in 0..p.w as i32 {
            out.set(p.w as i32 - 1 - x, y, p.get(x, y));
        }
    }
    out
}

pub fn flip_y(p: &Pixels) -> Pixels {
    let mut out = Pixels::new(p.w, p.h);
    for y in 0..p.h as i32 {
        for x in 0..p.w as i32 {
            out.set(x, p.h as i32 - 1 - y, p.get(x, y));
        }
    }
    out
}

const ICON_NUGGET: &[&str] = &[
    "..kkk...",
    ".kywyk..",
    "kyyyyYk.",
    "kyyyYYok",
    "kYyYYook",
    ".kYYook.",
    "..kkkk..",
    "........",
];

pub fn icon_nugget() -> Vec<Pixels> {
    vec![grid(ICON_NUGGET, GOLD_PAL)]
}

const LOCK_PAL: Palette = &[('k', hex(0x1b1420)), ('g', hex(0xc0c4d0)), ('G', hex(0x707488)), ('w', hex(0xffffff))];
const ICON_LOCK: &[&str] = &[
    "..kkkk..",
    ".kG..Gk.",
    ".kG..Gk.",
    "kkkkkkkk",
    "kwgggggk",
    "kgggkggk",
    "kGGGkGGk",
    "kkkkkkkk",
];

pub fn icon_lock() -> Vec<Pixels> {
    vec![grid(ICON_LOCK, LOCK_PAL)]
}

// ---------------------------------------------------------------- checkpoint

const TP_PAL: Palette = &[
    ('k', OUTLINE),
    ('g', hex(0xc8ccd8)), // chrome
    ('G', hex(0x7a8094)), // chrome shade
    ('w', hex(0xffffff)), // paper
    ('W', hex(0xc0c8dc)), // paper shade
    ('c', hex(0xb08050)), // cardboard tube
    ('y', hex(0xffe060)), // sparkle
];

/// The stand and a big roll seen end-on, rows 0..=11 (shared by both states).
const TP_TOP: &[&str] = &[
    "................",
    ".kk.............",
    "kgGk...kkkk.....",
    "kgGk.kkwwwwkk...",
    "kgGkkwwwwwwwWk..",
    "kgGkkwwwccwwWk..",
    "kgGGkwwcGGcwWk..",
    "kgGGkwwcGGcwWk..",
    "kgGkkwwwccwWWw..",
    "kgGkkWwwwwwWWw..",
    "kgGk.kkWWWWkkw..",
    "kgGk...kkkk.kw..",
];
/// Untouched: a short tail of paper tucked against the roll.
const TP_OFF: &[&str] = &["kgGk.........kk.", "kgGk............", "kggGGk..........", "kkkkkk.........."];
/// Touched: the paper has unrolled all the way to the floor and flutters.
const TP_ON_A: &[&str] = &["kgGk........kw..", "kgGk.......kw...", "kggGGk.....kw...", "kkkkkk...kwwwWk."];
const TP_ON_B: &[&str] = &["kgGk.........kw.", "kgGk.........kw.", "kggGGk......kw..", "kkkkkk...kwwwWk."];

fn tp(bottom: &[&'static str], sparkle: Option<(i32, i32)>) -> Pixels {
    let mut rows: Vec<&str> = TP_TOP.to_vec();
    rows.extend_from_slice(bottom);
    let mut p = grid(&rows, TP_PAL);
    // The tail is 1px wide; give it a right-hand outline wherever it hangs.
    let k = OUTLINE;
    let w = hex(0xffffff);
    for y in 8..16 {
        for x in (1..15).rev() {
            if p.get(x, y) == w && p.get(x + 1, y)[3] == 0 && x >= 12 {
                p.set(x + 1, y, k);
            }
        }
    }
    if let Some((x, y)) = sparkle {
        p.blit(&grid(SPARKLE_Y, TP_PAL), x, y);
    }
    p
}

const SPARKLE_Y: &[&str] = &[".y.", "yyy", ".y."];

pub fn checkpoint_off() -> Vec<Pixels> {
    vec![tp(TP_OFF, None)]
}

pub fn checkpoint_on() -> Vec<Pixels> {
    vec![tp(TP_ON_A, Some((13, 0))), tp(TP_ON_B, Some((12, 1)))]
}

// ---------------------------------------------------------------- goal

const GOAL_PAL: Palette = &[
    ('k', OUTLINE),
    ('t', hex(0xe0b070)), // handle
    ('T', hex(0x9a6430)), // handle shade
    ('r', hex(0xe04048)), // cup
    ('l', hex(0xff9090)), // cup shine
    ('R', hex(0x8a1a2a)), // cup shade
    ('g', hex(0x30b040)), // flag green
    ('w', hex(0xffffff)), // flag white
];

const GOAL_CAP: &[&str] = &["...kk...........", "..kttk.........."];
const GOAL_HANDLE: &str = "..ktTk..........";
const GOAL_CUP: &[&str] = &[
    ".kkrRkk.........",
    ".krrrRRk........",
    "krlrrRRRk.......",
    "krlrrRRRk.......",
    "kRrrRRRRk.......",
    "kkkkkkkkk.......",
];
const FLAG: &[&str] = &[
    "kkkkkkkkkk",
    "kwwggwwggk",
    "kwwggwwggk",
    "kggwwggwwk",
    "kggwwggwwk",
    "kwwggwwggk",
    "kwwggwwggk",
    "kkkkkkkkkk",
];

pub fn goal_flag() -> Vec<Pixels> {
    let mut rows: Vec<&str> = GOAL_CAP.to_vec();
    rows.extend(std::iter::repeat_n(GOAL_HANDLE, 32 - GOAL_CAP.len() - GOAL_CUP.len()));
    rows.extend_from_slice(GOAL_CUP);
    let base = grid(&rows, GOAL_PAL);
    let flag = grid(FLAG, GOAL_PAL);
    (0..3)
        .map(|f| {
            let mut out = base.clone();
            for c in 0..flag.w as i32 {
                // Wave grows away from the handle; three phases.
                let amp = c as f32 / 9.0 * 1.6;
                let dy = ((c as f32 * 0.8 - f as f32 * 2.094).sin() * amp).round() as i32;
                for r in 0..flag.h as i32 {
                    let px = flag.get(c, r);
                    if px[3] > 0 {
                        out.set(6 + c, 3 + r + dy, px);
                    }
                }
            }
            out
        })
        .collect()
}

// ---------------------------------------------------------------- throne

const THRONE_PAL: Palette = &[
    ('k', hex(0x2a2c48)),
    ('w', hex(0xffffff)),
    ('l', hex(0xdce6f5)),
    ('s', hex(0xa0b0d0)),
    ('y', hex(0xffd94a)),
    ('Y', hex(0xc8901e)),
];

const THRONE: &[&str] = &[
    "................................",
    "..kkkkkkkkkkkkk.........y.......",
    ".kwwwwwwwwwwwwlk.......yyy......",
    ".kllllllllllllsk........y.......",
    "..kkkkkkkkkkkkk.................",
    "...kwwwwwwwwlskkkkk.............",
    "...kwwwwwwwwlskyyyk.............",
    "...kwwwwwwwwlskkkkk.............",
    "...kwwwwwwwwlsk.................",
    "...kwywywywwlsk.................",
    "...kwyyyyywwlsk...........y.....",
    "...kwYYYYYwwlsk..........yyy....",
    "...kwwwwwwwwlsk...........y.....",
    "...kwwwwwwwwlskkkkkkkkkkkkkkkkk.",
    "...kwwwwwwwwlskyyyyyyyyyyyyyyyk.",
    "...kwwwwwwwwlskYYYYYYYYYYYYYYYk.",
    "...kwwwwwwwwwwwwwwwwwwwwwwwwwlk.",
    "...kwwwwwwwwwwwwwwwwwwwwwwwwlsk.",
    "....kwwwwwwwwwwwwwwwwwwwwwwwlsk.",
    ".....klwwwwwwwwwwwwwwwwwwwwlsk..",
    "......klwwwwwwwwwwwwwwwwwwlsk...",
    "........klwwwwwwwwwwwwwwlsk.....",
    "..........klwwwwwwwwwwssk.......",
    "...........klwwwwwwwwsk.........",
    "............klwwwwwwsk..........",
    "............klwwwwwwsk..........",
    "............klwwwwwwsk..........",
    "............klwwwwwwsk..........",
    "...........klwwwwwwwwsk.........",
    "..........klwwwwwwwwwwsk........",
    "..........kssssssssssssk........",
    "..........kkkkkkkkkkkkkk........",
];

pub fn throne() -> Vec<Pixels> {
    vec![grid(THRONE, THRONE_PAL)]
}

// ---------------------------------------------------------------- hazards

const BRUSH_PAL: Palette = &[
    ('k', OUTLINE),
    ('w', hex(0xf4f8ff)), // bristles
    ('W', hex(0x9aa8c8)), // bristle shade
    ('b', hex(0x40a0e0)), // holder
    ('B', hex(0x205a98)), // holder shade
];

/// One upturned brush, 8 wide; two per tile.
const BRUSH: &[&str] = &[
    "w..w..w.",
    "Wk.W.kW.",
    ".WkWkW..",
    ".kWwWk..",
    "..kwk...",
    ".kbbbk..",
    ".kbBBk..",
    "kkkkkkk.",
];

pub fn spikes_up() -> Vec<Pixels> {
    let b = grid(BRUSH, BRUSH_PAL);
    let mut out = Pixels::new(16, 16);
    out.blit(&b, 0, 8);
    out.blit(&b, 8, 8);
    vec![out]
}

pub fn spikes_down() -> Vec<Pixels> {
    spikes_up().iter().map(flip_y).collect()
}

const FLY_PAL: Palette = &[
    ('k', OUTLINE),
    ('c', hex(0xd0ecff)), // wings
    ('g', hex(0x3a4a3a)), // body
    ('G', hex(0x7a9a6a)), // body shine
    ('r', hex(0xff4040)), // eyes
];

const FLY: [&[&str]; 2] = [
    &[
        ".kk.kk..",
        "kcckcck.",
        ".kckck..",
        ".kkkkkkk",
        "kgGggkrr",
        "kGgggkrr",
        ".kkkkkk.",
        "..k.k...",
    ],
    &[
        "........",
        "........",
        "kkkkkk..",
        "kccccckk",
        "kgGggkrr",
        "kGgggkrr",
        ".kkkkkk.",
        "..k.k...",
    ],
];

pub fn fly() -> Vec<Pixels> {
    FLY.iter().map(|g| grid(g, FLY_PAL)).collect()
}

const CAN_PAL: Palette = &[
    ('k', OUTLINE),
    ('m', hex(0xd060a8)), // can
    ('M', hex(0x84306c)), // can shade
    ('w', hex(0xffc8ec)), // shine
    ('g', hex(0xc8ccd8)), // metal
    ('G', hex(0x7a8094)),
    ('y', hex(0xffe060)), // flower
];

const SPRAY_CAN: &[&str] = &[
    "................",
    "................",
    ".......kk.......",
    "......kggk......",
    ".....kgggGk.....",
    "....kkkkkkkk....",
    "....kwmmmmMk....",
    "....kwmmmmMk....",
    "....kmmyymMk....",
    "....kmyGymMk....",
    "....kmmyymMk....",
    "....kwmmmmMk....",
    "....kwmmmmMk....",
    "....kmmmmmMk....",
    "....kgggggGk....",
    "....kkkkkkkk....",
];

pub fn spray_can() -> Vec<Pixels> {
    vec![grid(SPRAY_CAN, CAN_PAL)]
}

const MIST_PAL: Palette = &[('w', hex(0xffffff)), ('p', hex(0xf8c0e8)), ('l', hex(0xc090e0))];
const SPRAY_JET: &[&str] = &[
    "......pwwp......",
    ".....pwwwwp.....",
    ".....lpwwpl.....",
    "......pwwp...l..",
    "..l...lwwl......",
    ".....pwwwwp.....",
    "....pwwpwwwp....",
    "....lpwwwwpl....",
    ".....lpwwpl.....",
    "......pwwp......",
    ".l....pwwp......",
    ".....pwwwwp.....",
    ".....pwpwwp..p..",
    "......lwwl......",
    "......pwwp......",
    "......pwwp......",
];

pub fn spray_jet() -> Vec<Pixels> {
    let a = grid(SPRAY_JET, MIST_PAL);
    // Second frame: scrolled half a tile (wrapping vertically) and mirrored, so the mist churns.
    let mut b = Pixels::new(16, 16);
    for y in 0..16 {
        for x in 0..16 {
            b.set(15 - x, (y + 8) % 16, a.get(x, y));
        }
    }
    vec![a, b]
}

// ---------------------------------------------------------------- moving platforms

const TPROLL_PAL: Palette = &[('k', hex(0x3a3a50)), ('w', hex(0xffffff)), ('W', hex(0xdde2ee)), ('s', hex(0xa8b0c8))];
const PLATFORM_TP: &[&str] = &[
    "kkkkkkkkkkkkkkkk",
    "wwwwwwwWwwwwwwwW",
    "wwwwwwwwwwwwwwww",
    "WWWWWWWsWWWWWWWs",
    "WWWWWWWWWWWWWWWW",
    "sssssssWsssssssW",
    "ssssssssssssssss",
    "kkkkkkkkkkkkkkkk",
];

const DUCK_PAL: Palette = &[('k', hex(0x4a2a08)), ('y', hex(0xffd83a)), ('Y', hex(0xe0a020)), ('o', hex(0xff8020)), ('w', hex(0xffffff))];
const PLATFORM_DUCK: &[&str] = &[
    "..........kkkk..",
    ".........kyyyyk.",
    ".........kywkyk.",
    "k........kyyyook",
    "kk.......kyyyyk.",
    "kykkkkkkkyyyyyk.",
    "kyyyyYYYYyyyyYYk",
    ".kYYYYYYYYYYYYk.",
];

const PLUNGER_PAL: Palette = &[('k', hex(0x2a1018)), ('r', hex(0xd8303a)), ('l', hex(0xff8a8a)), ('R', hex(0x8a1a2a)), ('t', hex(0xd8a868))];
const PLATFORM_PLUNGER: &[&str] = &[
    "kkkkkkkkkkkkkkkk",
    "rrllrrrrrrrrrrRr",
    "RRRRRRRRRRRRRRRR",
    "kRrrlrrrrrrrrRRk",
    ".kRrrrrrrrrrRRk.",
    "..kkRRRRRRRRkk..",
    "....kkkttkkk....",
    "......kttk......",
];

fn platform(rows: &[&str], pal: Palette) -> Vec<Pixels> {
    let mut out = Pixels::new(16, 16);
    out.blit(&grid(rows, pal), 0, 0);
    vec![out]
}

pub fn platform_tp() -> Vec<Pixels> {
    platform(PLATFORM_TP, TPROLL_PAL)
}
// ---------------------------------------------------------------- splat stains

/// Nat's browns: a splat stain is a bit of Nat (cartoon mud, really).
const STAIN_PAL: Palette = &[
    ('k', hex(0x2b1608)), // outline
    ('B', hex(0x6b3e1c)), // shade
    ('b', hex(0x9a5b2a)), // brown
    ('h', hex(0xc98a4b)), // shine
];

/// A splat over floor spikes: a flat-topped blob you can stand on, dripping between the
/// bristles (the spike tile underneath stays drawn).
const STAIN_UP: &[&str] = &[
    "kkkkkkkkkkkkkkkk",
    "khhbbbhhbbbbhhbk",
    "kbbbbbbbbbbbbbbk",
    "kbBbbbbbBbbbbbbk",
    "kbbbbBbbbbbbBbbk",
    "kBbbbbbbbbbbbbBk",
    ".kbbBbbbbbBbbbk.",
    ".kBbbkbbbbkbbBk.",
    "..kbk.kbbk.kbk..",
    "..kbk..kk..kBk..",
    "...k........k...",
];

/// A stain raft: a brown blob bobbing on the surface (art in the top 8 rows, like platforms).
const STAIN_RAFT: &[&str] = &[
    "..kkkkkkkkkkkk..",
    ".khhhbbbbbhhbbk.",
    "kbbbbbbBbbbbbbbk",
    "kbBbbbbbbbbBbbbk",
    "kBbbBbbbbbbbbBbk",
    ".kBBBBBBBBBBBBk.",
    "..kkkkkkkkkkkk..",
];

pub fn stain_up() -> Vec<Pixels> {
    let mut out = Pixels::new(16, 16);
    out.blit(&grid(STAIN_UP, STAIN_PAL), 0, 0);
    vec![out]
}

pub fn stain_down() -> Vec<Pixels> {
    stain_up().iter().map(flip_y).collect()
}

/// Two frames: bobbing one pixel.
pub fn stain_raft() -> Vec<Pixels> {
    let g = grid(STAIN_RAFT, STAIN_PAL);
    (0..2)
        .map(|k| {
            let mut out = Pixels::new(16, 16);
            out.blit(&g, 0, k);
            out
        })
        .collect()
}

pub fn platform_duck() -> Vec<Pixels> {
    platform(PLATFORM_DUCK, DUCK_PAL)
}
pub fn platform_plunger() -> Vec<Pixels> {
    platform(PLATFORM_PLUNGER, PLUNGER_PAL)
}

// ---------------------------------------------------------------- particles

pub fn particle() -> Vec<Pixels> {
    let mut p = Pixels::new(2, 2);
    p.fill_rect(0, 0, 2, 2, hex(0xffffff));
    vec![p]
}

const NOTE_PAL: Palette = &[('w', hex(0xffffff))];
const NOTE: &[&str] = &[
    "..ww..",
    "..w.w.",
    "..w..w",
    "..w...",
    ".ww...",
    "www...",
    ".w....",
];

pub fn note() -> Vec<Pixels> {
    vec![grid(NOTE, NOTE_PAL)]
}

/// Pale green cloud puffs: grow, then thin out (dithered) and fade.
pub fn toot_puff() -> Vec<Pixels> {
    let light = hex(0xeaffd0);
    let mid = hex(0xbfeea0);
    let dark = hex(0x86c46a);
    // (radius scale, alpha, dither keep-threshold out of 16)
    let frames = [(0.5f32, 255u8, 16u32), (0.8, 235, 16), (1.0, 180, 16), (1.12, 110, 13)];
    // Overlapping lobes making a round cloud.
    let lobes = [(8.0f32, 8.0f32, 4.0f32), (4.5, 9.0, 3.0), (11.5, 9.0, 3.0), (6.5, 5.0, 3.0), (10.0, 5.5, 2.5), (8.0, 11.0, 3.0)];
    frames
        .iter()
        .map(|&(s, a, keep)| {
            let mut p = Pixels::new(16, 16);
            for y in 0..16 {
                for x in 0..16 {
                    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                    // Signed distance-ish: how deep inside the union of lobes.
                    let mut best = f32::MIN;
                    let mut hi = false;
                    for &(cx, cy, r) in &lobes {
                        let (cx, cy) = (8.0 + (cx - 8.0) * s, 8.0 + (cy - 8.0) * s);
                        let r = r * s + 0.5;
                        let d = r - ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
                        if d > best {
                            best = d;
                            hi = fy < cy - r * 0.2 && fx < cx;
                        }
                    }
                    if best < 0.0 || hash2(x / 2, y / 2, 7) % 16 >= keep {
                        continue;
                    }
                    let c = if best < 1.0 { dark } else if hi { light } else { mid };
                    p.set(x, y, c);
                }
            }
            p.fade(a)
        })
        .collect()
}

#[cfg(test)]
pub fn grids() -> Vec<(&'static str, Vec<&'static str>, Palette)> {
    let mut v: Vec<(&'static str, Vec<&'static str>, Palette)> = vec![
        ("SPARKLE", SPARKLE.to_vec(), GOLD_PAL),
        ("ICON_NUGGET", ICON_NUGGET.to_vec(), GOLD_PAL),
        ("ICON_LOCK", ICON_LOCK.to_vec(), LOCK_PAL),
        ("NOTE", NOTE.to_vec(), NOTE_PAL),
        ("TP_TOP", TP_TOP.to_vec(), TP_PAL),
        ("SPARKLE_Y", SPARKLE_Y.to_vec(), TP_PAL),
        ("TP_OFF", TP_OFF.to_vec(), TP_PAL),
        ("TP_ON_A", TP_ON_A.to_vec(), TP_PAL),
        ("TP_ON_B", TP_ON_B.to_vec(), TP_PAL),
        ("GOAL_CAP", GOAL_CAP.to_vec(), GOAL_PAL),
        ("GOAL_HANDLE", vec![GOAL_HANDLE], GOAL_PAL),
        ("GOAL_CUP", GOAL_CUP.to_vec(), GOAL_PAL),
        ("FLAG", FLAG.to_vec(), GOAL_PAL),
        ("THRONE", THRONE.to_vec(), THRONE_PAL),
        ("BRUSH", BRUSH.to_vec(), BRUSH_PAL),
        ("SPRAY_CAN", SPRAY_CAN.to_vec(), CAN_PAL),
        ("SPRAY_JET", SPRAY_JET.to_vec(), MIST_PAL),
        ("PLATFORM_TP", PLATFORM_TP.to_vec(), TPROLL_PAL),
        ("PLATFORM_DUCK", PLATFORM_DUCK.to_vec(), DUCK_PAL),
        ("PLATFORM_PLUNGER", PLATFORM_PLUNGER.to_vec(), PLUNGER_PAL),
        ("STAIN_UP", STAIN_UP.to_vec(), STAIN_PAL),
        ("STAIN_RAFT", STAIN_RAFT.to_vec(), STAIN_PAL),
    ];
    for g in NUGGET {
        v.push(("NUGGET", g.to_vec(), GOLD_PAL));
    }
    for g in FLY {
        v.push(("FLY", g.to_vec(), FLY_PAL));
    }
    v
}

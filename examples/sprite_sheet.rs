//! Dev tool: render every sprite to PNG contact sheets so the art can be eyeballed.
//!
//! `cargo run --example sprite_sheet [OUT_DIR]` (default `target/sprite_sheet/`) writes:
//! - `sheet.png`: every non-backdrop sprite, all frames, 4x, one row per sprite in
//!   `SpriteId::all()` order (the order is printed to stdout).
//! - `backdrops.png`: the five backdrops, each drawn twice side by side (to check tiling), 1x.
//! - `scenes.png`: a mock gameplay scene per world at 2x, to judge readability in context.

use nat_han_adventures::art::{Pixels, SpriteId, render};
use std::path::PathBuf;

fn main() {
    let out = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("target/sprite_sheet"));
    std::fs::create_dir_all(&out).expect("create output dir");

    // ---- sheet
    let scale = 4;
    let ids: Vec<SpriteId> = SpriteId::all().into_iter().filter(|id| !matches!(id, SpriteId::Backdrop(_))).collect();
    let rows: Vec<(SpriteId, Vec<Pixels>)> = ids.iter().map(|&id| (id, render(id))).collect();
    // Lay sprites out in columns of rows, packed into 3 columns to keep the sheet square-ish.
    let cols = 3;
    let per_col = rows.len().div_ceil(cols);
    let pad = 4;
    let cell_w = rows.iter().map(|(_, f)| f.iter().map(|p| p.w as i32 * scale + pad).sum::<i32>()).max().unwrap();
    let mut col_heights = vec![pad; cols];
    let mut placements = Vec::new();
    for (i, (id, frames)) in rows.iter().enumerate() {
        let c = i / per_col;
        let h = frames.iter().map(|p| p.h as i32).max().unwrap() * scale;
        placements.push((c, col_heights[c], id, frames));
        println!("col {c} y {:4}: {id:?} ({} frames)", col_heights[c], frames.len());
        col_heights[c] += h + pad;
    }
    let sheet_w = cols as i32 * (cell_w + pad) + pad;
    let sheet_h = *col_heights.iter().max().unwrap();
    let mut sheet = checker(sheet_w, sheet_h);
    for (c, y, _, frames) in placements {
        let mut x = pad + c as i32 * (cell_w + pad);
        for f in frames {
            blit_scaled(&mut sheet, f, x, y, scale);
            x += f.w as i32 * scale + pad;
        }
    }
    save(&sheet, &out.join("sheet.png"));

    // ---- backdrops, each twice side by side to show the horizontal wrap.
    let mut bd = Pixels::new(512 + 8, 5 * (144 + 4));
    for w in 1..=5u8 {
        let b = &render(SpriteId::Backdrop(w))[0];
        let y = (w as i32 - 1) * 148;
        bd.blit(b, 0, y);
        bd.blit(b, 256, y);
    }
    save(&bd, &out.join("backdrops.png"));

    // ---- scenes
    let mut scenes = Pixels::new(2 * 256, 2 * 5 * 144);
    for w in 1..=5u8 {
        let s = scene(w);
        blit_scaled(&mut scenes, &s, 0, (w as i32 - 1) * 288, 2);
    }
    save(&scenes, &out.join("scenes.png"));
    println!("wrote {}", out.display());
}

fn first(id: SpriteId) -> Pixels {
    render(id).remove(0)
}

fn nth(id: SpriteId, i: usize) -> Pixels {
    let f = render(id);
    f[i % f.len()].clone()
}

/// A 256x144 mock level: backdrop, ground, liquid, hazards, characters.
fn scene(w: u8) -> Pixels {
    use SpriteId::*;
    let mut s = Pixels::new(256, 144);
    s.blit(&first(Backdrop(w)), 0, 0);
    let ground_y = 112;
    for tx in 0..16 {
        let x = tx * 16;
        if (9..11).contains(&tx) {
            s.blit(&nth(LiquidTop(w), tx as usize), x, ground_y);
            s.blit(&first(LiquidFill(w)), x, ground_y + 16);
            continue;
        }
        s.blit(&first(GroundTop(w)), x, ground_y);
        s.blit(&first(GroundFill(w)), x, ground_y + 16);
    }
    // Raised block (fill next to top) and one-way platforms.
    s.blit(&first(GroundTop(w)), 192, ground_y - 16);
    s.blit(&first(GroundTop(w)), 208, ground_y - 16);
    for tx in 4..7 {
        s.blit(&first(OneWay(w)), tx * 16, 64);
    }
    s.blit(&first(SpikesDown), 96, 0);
    s.blit(&first(SpikesUp), 128, ground_y - 16);
    s.blit(&first(Nugget), 64, 48);
    s.blit(&nth(Nugget, 1), 80, 48);
    s.blit(&nth(Nugget, 3), 96, 48);
    s.blit(&nth(HanRun, 0), 8, ground_y - 16);
    s.blit(&nth(PooRun, 1), 28, ground_y - 16);
    s.blit(&nth(PooJump, 0), 150, 70);
    s.blit(&nth(TootPuff, 1), 150, 86);
    s.blit(&first(Fly), 120, 40);
    s.blit(&first(SprayCan), 176, ground_y - 16);
    for i in 0..2 {
        s.blit(&nth(SprayJet, i), 176, ground_y - 32 - i as i32 * 16);
    }
    s.blit(&first(CheckpointOn), 48, ground_y - 16);
    s.blit(&first(GoalFlag), 224, ground_y - 48);
    s.blit(&first(Throne), 224 - 34, ground_y - 48);
    let plat = [PlatformTp, PlatformDuck, PlatformPlunger][w as usize % 3];
    for i in 0..3 {
        s.blit(&first(plat), 144 + i * 16, 100 - 60);
    }
    s
}

fn checker(w: i32, h: i32) -> Pixels {
    let mut p = Pixels::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let c = if ((x / 8) + (y / 8)) % 2 == 0 { [58, 58, 68, 255] } else { [70, 70, 82, 255] };
            p.set(x, y, c);
        }
    }
    p
}

fn blit_scaled(dst: &mut Pixels, src: &Pixels, x: i32, y: i32, s: i32) {
    for sy in 0..src.h as i32 {
        for sx in 0..src.w as i32 {
            let c = src.get(sx, sy);
            if c[3] == 0 {
                continue;
            }
            for dy in 0..s {
                for dx in 0..s {
                    let d = dst.get(x + sx * s + dx, y + sy * s + dy);
                    let a = c[3] as u32;
                    let mix = |i: usize| ((c[i] as u32 * a + d[i] as u32 * (255 - a)) / 255) as u8;
                    dst.set(x + sx * s + dx, y + sy * s + dy, [mix(0), mix(1), mix(2), 255]);
                }
            }
        }
    }
}

fn save(p: &Pixels, path: &std::path::Path) {
    let file = std::fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), p.w, p.h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&p.data).unwrap();
}

//! Procedural 256x144 backdrops, one per world. They tile horizontally (every feature is drawn
//! with wrap-around x), and are kept darker / lower-contrast than the foreground so gameplay reads.

use super::palette::*;

pub const W: i32 = 256;
pub const H: i32 = 144;

/// Pick `a` or `b` by ordered dither: `t` = 0 is all `a`, 1 is all `b`.
fn dith(x: i32, y: i32, a: Rgba, b: Rgba, t: f32) -> Rgba {
    if (bayer4(x, y) as f32 + 0.5) / 16.0 < t { b } else { a }
}

/// Vertical gradient through `stops` (evenly spaced), dithered between neighbours.
fn vgradient(p: &mut Pixels, y0: i32, y1: i32, stops: &[Rgba]) {
    let n = (stops.len() - 1) as f32;
    for y in y0..y1 {
        let t = (y - y0) as f32 / ((y1 - y0 - 1).max(1)) as f32 * n;
        let i = (t.floor() as usize).min(stops.len() - 2);
        let f = t - i as f32;
        for x in 0..W {
            p.set(x, y, dith(x, y, stops[i], stops[i + 1], f));
        }
    }
}

fn disc_wrap(p: &mut Pixels, cx: i32, cy: i32, r: i32, c: Rgba) {
    for y in -r..=r {
        for x in -r..=r {
            if x * x + y * y <= r * r + r / 2 {
                p.set_wrap(cx + x, cy + y, c);
            }
        }
    }
}

fn ring_wrap(p: &mut Pixels, cx: i32, cy: i32, r0: i32, r1: i32, c: Rgba) {
    for y in -r1..=r1 {
        for x in -r1..=r1 {
            let d = x * x + y * y;
            if d <= r1 * r1 + r1 / 2 && d > r0 * r0 + r0 / 2 {
                p.set_wrap(cx + x, cy + y, c);
            }
        }
    }
}

/// Horizontal cylinder (pipe) across the whole width: shaded light-top / dark-bottom.
fn hpipe(p: &mut Pixels, y: i32, r: i32, cols: [Rgba; 4]) {
    let [outline, dark, mid, light] = cols;
    for x in 0..W {
        for dy in -r..=r {
            let c = if dy.abs() == r {
                outline
            } else {
                let t = (dy + r) as f32 / (2 * r) as f32;
                if t < 0.3 { light } else if t < 0.4 { dith(x, y + dy, light, mid, 0.5) } else if t < 0.7 { mid } else if t < 0.8 { dith(x, y + dy, mid, dark, 0.5) } else { dark }
            };
            p.set(x, y + dy, c);
        }
    }
}

/// Vertical cylinder from y0 to y1 at x (wrapping).
fn vpipe(p: &mut Pixels, x: i32, y0: i32, y1: i32, r: i32, cols: [Rgba; 4]) {
    let [outline, dark, mid, light] = cols;
    for y in y0..y1 {
        for dx in -r..=r {
            let c = if dx.abs() == r {
                outline
            } else {
                let t = (dx + r) as f32 / (2 * r) as f32;
                if t < 0.3 { light } else if t < 0.4 { dith(x + dx, y, light, mid, 0.5) } else if t < 0.7 { mid } else if t < 0.8 { dith(x + dx, y, mid, dark, 0.5) } else { dark }
            };
            p.set_wrap(x + dx, y, c);
        }
    }
}

// ---------------------------------------------------------------- 1: bathroom

fn bathroom() -> Pixels {
    let mut p = Pixels::new(W as u32, H as u32);
    let tile = hex(0x7a8aa4);
    let tile_hi = hex(0x8c9cb6);
    let grout = hex(0x5c6a84);
    let wains = hex(0x4c6088);
    let wains_hi = hex(0x5a6e98);
    let trim = hex(0x3a4a6c);
    let dark = hex(0x2c3854);

    // Wall tiles: 16x16 with 1px grout, a lighter top-left bevel.
    for y in 0..96 {
        for x in 0..W {
            let (tx, ty) = (x % 16, y % 16);
            let c = if tx == 15 || ty == 15 { grout } else if tx == 0 || ty == 0 { tile_hi } else { tile };
            p.set(x, y, c);
        }
    }
    // Trim band with a little diamond pattern.
    for y in 96..102 {
        for x in 0..W {
            let c = if y == 96 || y == 101 { dark } else if (x + y) % 6 == 0 || (x - y).rem_euclid(6) == 0 { wains_hi } else { trim };
            p.set(x, y, c);
        }
    }
    // Wainscot: 16x8 bricks, offset every row.
    for y in 102..H {
        for x in 0..W {
            let row = (y - 102) / 8;
            let ox = if row % 2 == 0 { 0 } else { 8 };
            let (tx, ty) = ((x + ox) % 16, (y - 102) % 8);
            let c = if tx == 15 || ty == 7 { dark } else if ty == 0 { wains_hi } else { wains };
            p.set(x, y, c);
        }
    }

    // Mirror with a silver frame and soft diagonal shine.
    let frame = hex(0xa4acbc);
    let frame_dk = hex(0x5e667a);
    let glass = hex(0x9ab4cc);
    let shine = hex(0xb4cade);
    let (mx, my, mw, mh) = (36, 14, 52, 60);
    p.fill_rect_wrap(mx - 1, my - 1, mw + 2, mh + 2, frame_dk);
    p.fill_rect_wrap(mx, my, mw, mh, frame);
    p.fill_rect_wrap(mx + 3, my + 3, mw - 6, mh - 6, frame_dk);
    for y in my + 4..my + mh - 4 {
        for x in mx + 4..mx + mw - 4 {
            let d = (x - mx) + (y - my);
            let c = if (d % 36) < 4 || (d % 36) == 7 { shine } else { glass };
            p.set_wrap(x, y, c);
        }
    }
    // Shelf under the mirror with a cup and toothbrush.
    p.fill_rect_wrap(mx - 2, my + mh + 3, mw + 4, 2, frame);
    p.fill_rect_wrap(mx - 2, my + mh + 5, mw + 4, 1, frame_dk);
    p.fill_rect_wrap(mx + 8, my + mh - 3, 6, 6, hex(0x8a6a8a));
    p.fill_rect_wrap(mx + 10, my + mh - 8, 1, 5, hex(0x6aa0a0));
    p.fill_rect_wrap(mx + 12, my + mh - 7, 1, 4, hex(0xa07070));

    // Towel rack with a striped towel.
    let bar = hex(0xa4acbc);
    let (rx, ry) = (150, 44);
    p.fill_rect_wrap(rx - 3, ry - 2, 3, 5, frame_dk);
    p.fill_rect_wrap(rx + 52, ry - 2, 3, 5, frame_dk);
    p.fill_rect_wrap(rx, ry, 52, 2, bar);
    let towel = hex(0xa87888);
    let towel_dk = hex(0x84586a);
    let stripe = hex(0xc8a0ac);
    for y in ry + 2..ry + 44 {
        let sway = if y > ry + 30 { 1 } else { 0 };
        for x in rx + 10..rx + 40 {
            let ty = y - ry;
            let c = if x == rx + 10 || x == rx + 39 {
                towel_dk
            } else if (30..34).contains(&ty) || ty == 36 {
                stripe
            } else if ty < 5 {
                towel_dk
            } else {
                towel
            };
            p.set_wrap(x + sway, y, c);
        }
    }
    // A light switch and a little framed duck picture.
    p.fill_rect_wrap(118, 50, 6, 9, hex(0x9aa4b8));
    p.fill_rect_wrap(120, 52, 2, 3, frame_dk);
    p.fill_rect_wrap(212, 20, 22, 18, frame_dk);
    p.fill_rect_wrap(214, 22, 18, 14, hex(0x8aa0b8));
    disc_wrap(&mut p, 222, 30, 3, hex(0xb8a050));
    disc_wrap(&mut p, 226, 27, 2, hex(0xb8a050));
    p
}

// ---------------------------------------------------------------- 2: pipes

fn pipes() -> Pixels {
    let mut p = Pixels::new(W as u32, H as u32);
    vgradient(&mut p, 0, H, &[hex(0x10161a), hex(0x182226), hex(0x1e2a2c)]);
    let copper = [hex(0x0c0e10), hex(0x3c2a20), hex(0x5c3e2a), hex(0x7c5a3a)];
    let green = [hex(0x0c0e10), hex(0x243230), hex(0x34484a), hex(0x4a6460)];
    let grey = [hex(0x0c0e10), hex(0x262a30), hex(0x363c44), hex(0x4c545e)];

    // Far layer: thin vertical pipes.
    for (x, r) in [(12, 2), (58, 3), (104, 2), (140, 3), (186, 2), (238, 3)] {
        vpipe(&mut p, x, 0, H, r, grey);
    }
    // Big horizontal mains.
    hpipe(&mut p, 22, 6, green);
    hpipe(&mut p, 118, 8, copper);
    // Mid verticals with elbows into the mains.
    for (x, r, y0, y1, cols) in [(34, 5, 22, 118, copper), (120, 4, 0, 118, green), (204, 5, 22, 144, copper)] {
        vpipe(&mut p, x, y0, y1, r, cols);
    }
    hpipe_segment(&mut p, 70, 120, 200, 4, grey);
    // Flanges on the mains every 64px.
    for i in 0..4 {
        let x = 16 + i * 64;
        for (y, r, c) in [(22, 7, green), (118, 9, copper)] {
            p.fill_rect_wrap(x - 2, y - r, 5, 2 * r + 1, c[0]);
            p.fill_rect_wrap(x - 1, y - r + 1, 3, 2 * r - 1, c[3]);
            p.set_wrap(x, y - r + 2, hex(0xa09070));
            p.set_wrap(x, y + r - 2, hex(0xa09070));
        }
    }
    // Valve wheels.
    for (x, y) in [(120, 52), (34, 88)] {
        valve(&mut p, x, y);
    }
    // A pressure gauge on the right vertical.
    disc_wrap(&mut p, 204, 64, 6, hex(0x0c0e10));
    disc_wrap(&mut p, 204, 64, 5, hex(0x8a8a7a));
    disc_wrap(&mut p, 204, 64, 4, hex(0xb0ac98));
    for i in 0..4 {
        p.set_wrap(204 + i, 64 - i, hex(0x7a2a2a));
    }
    // Drips under the mains.
    for (x, y) in [(80, 31), (82, 36), (160, 128), (161, 134), (232, 31)] {
        p.set_wrap(x, y, hex(0x4a6a6a));
    }
    p
}

fn hpipe_segment(p: &mut Pixels, y: i32, x0: i32, x1: i32, r: i32, cols: [Rgba; 4]) {
    let mut tmp = Pixels::new(W as u32, H as u32);
    hpipe(&mut tmp, y, r, cols);
    for x in x0..x1 {
        for dy in -r..=r {
            p.set_wrap(x, y + dy, tmp.get(x.rem_euclid(W), y + dy));
        }
    }
    // End caps.
    for x in [x0, x1 - 1] {
        for dy in -r - 1..=r + 1 {
            p.set_wrap(x, y + dy, cols[0]);
        }
    }
}

fn valve(p: &mut Pixels, x: i32, y: i32) {
    let red = hex(0x7a3434);
    let red_dk = hex(0x4a2020);
    ring_wrap(p, x, y, 5, 7, red_dk);
    ring_wrap(p, x, y, 5, 6, red);
    for d in -5..=5 {
        p.set_wrap(x + d, y, red);
        p.set_wrap(x, y + d, red);
    }
    disc_wrap(p, x, y, 1, hex(0xa09070));
}

// ---------------------------------------------------------------- 3: sewer

fn sewer() -> Pixels {
    let mut p = Pixels::new(W as u32, H as u32);
    let mortar = hex(0x141012);
    let brick = hex(0x342828);
    let brick_hi = hex(0x403230);
    let brick_dk = hex(0x2a2020);
    let moss = hex(0x26381e);
    // Bricks 12x6, alternate rows offset by 6 (12 * 2 = 24... 256 isn't a multiple of 12, so use 16x6).
    for y in 0..H {
        for x in 0..W {
            let row = y / 6;
            let ox = if row % 2 == 0 { 0 } else { 8 };
            let (bx, by) = ((x + ox) % 16, y % 6);
            let id = hash2(((x + ox) / 16) % (W / 16), row, 3);
            let base = match id % 5 {
                0 => brick_dk,
                1 => brick_hi,
                _ => brick,
            };
            let c = if bx == 15 || by == 5 { mortar } else { base };
            p.set(x, y, c);
        }
    }
    // Moss creeping down from the top.
    for x in 0..W {
        let wave = (x as f32 * std::f32::consts::TAU * 3.0 / W as f32).sin() * 3.0;
        let len = 5 + (hash2(x / 3, 0, 9) % 8) as i32 + wave as i32;
        for y in 0..len {
            if (bayer4(x, y) as i32) < 16 - y * 16 / len {
                p.set(x, y, moss);
            }
        }
    }
    // Two arches (period 128), dark tunnels with sludge channels and rat eyes.
    let void = hex(0x0a080c);
    let stone = hex(0x4e403a);
    let stone_dk = hex(0x2e2624);
    let sludge = hex(0x26301c);
    let sludge_hi = hex(0x3c4a28);
    for ax in [64, 192] {
        let (r, top) = (38, 62);
        // Voussoir ring.
        for y in top - r - 6..H {
            for x in ax - r - 6..=ax + r + 6 {
                let (dx, dy) = (x - ax, y - top);
                let d2 = dx * dx + dy.min(0) * dy.min(0);
                let inside_outer = if dy < 0 { d2 <= (r + 6) * (r + 6) } else { dx.abs() <= r + 6 };
                let inside = if dy < 0 { d2 <= r * r } else { dx.abs() <= r };
                if inside {
                    p.set_wrap(x, y, void);
                } else if inside_outer {
                    // Stones: split the ring by angle.
                    let ang = (dy.min(0) as f32).atan2(dx as f32);
                    let seg = ((ang * 7.0).rem_euclid(1.0) < 0.12) || (dy >= 0 && y % 8 == 0);
                    p.set_wrap(x, y, if seg { stone_dk } else { stone });
                }
            }
        }
        // Sludge channel at the bottom of the tunnel.
        for y in 128..H {
            for x in ax - r..=ax + r {
                let c = if y == 128 { sludge_hi } else if y == 129 { dith(x, y, sludge_hi, sludge, 0.5) } else { sludge };
                p.set_wrap(x, y, c);
            }
        }
        // Rat eyes glowing in the dark.
        for (ex, ey, c) in [(-18, 100, hex(0xd84030)), (10, 112, hex(0xe0b030)), (22, 92, hex(0xd84030))] {
            p.set_wrap(ax + ex, ey, c);
            p.set_wrap(ax + ex + 3, ey, c);
        }
    }
    // Slime drips running down the bricks between arches.
    let drip = hex(0x3e5a2e);
    let drip_hi = hex(0x5a7a3a);
    for (x, len) in [(4, 40), (122, 58), (132, 30), (250, 64), (16, 22)] {
        for y in 0..len {
            p.set_wrap(x, y, drip);
        }
        p.set_wrap(x, len, drip_hi);
        p.set_wrap(x, len + 4, drip_hi);
    }
    p
}

// ---------------------------------------------------------------- 4: septic & festival

fn festival() -> Pixels {
    let mut p = Pixels::new(W as u32, H as u32);
    vgradient(&mut p, 0, 100, &[hex(0x080a20), hex(0x141838), hex(0x2a2250)]);
    // Stars.
    for i in 0..60 {
        let x = (hash2(i, 1, 41) % W as u32) as i32;
        let y = (hash2(i, 2, 41) % 80) as i32;
        let c = if i % 7 == 0 { hex(0xe8e0b0) } else { hex(0x8a8cb8) };
        p.set(x, y, c);
        if i % 13 == 0 {
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                p.set_wrap(x + dx, y + dy, hex(0x5a5c8a));
            }
        }
    }
    // Moon.
    disc_wrap(&mut p, 200, 22, 10, hex(0xc8c4a0));
    disc_wrap(&mut p, 204, 19, 9, hex(0x1a1c40));
    // Distant crowd / hills.
    let hill = hex(0x1a1a34);
    for x in 0..W {
        let h = 92 + ((x as f32 * std::f32::consts::TAU / 128.0).sin() * 4.0) as i32 + (hash2(x / 2, 7, 5) % 3) as i32;
        for y in h..H {
            p.set(x, y, hill);
        }
    }
    // Grass strip.
    for y in 132..H {
        for x in 0..W {
            p.set(x, y, dith(x, y, hex(0x16281a), hex(0x1e3a22), if y == 132 { 0.6 } else { 0.2 }));
        }
    }
    // Porta-potties: 8 in a row, 32px apart.
    for i in 0..8 {
        porta_potty(&mut p, i * 32 + 5, 84 + if i % 3 == 1 { 2 } else { 0 });
    }
    // Bunting: a sagging string between poles every 64px, with alternating flags.
    let colours = [hex(0x8a3a4a), hex(0xa8984a), hex(0x3a7a4a), hex(0x4a5a9a)];
    let string = hex(0x5a5470);
    for seg in 0..4 {
        let x0 = seg * 64;
        // Pole.
        for y in 14..H {
            p.set_wrap(x0, y, hex(0x2a2440));
        }
        for x in 0..64 {
            let t = x as f32 / 64.0;
            let y = 16 + (t * (1.0 - t) * 4.0 * 14.0) as i32;
            p.set_wrap(x0 + x, y, string);
            if x % 8 == 4 {
                let c = colours[((x / 8) as usize + seg as usize) % 4];
                for fy in 0..5 {
                    for fx in -(2 - fy / 2)..=(2 - fy / 2) {
                        p.set_wrap(x0 + x + fx, y + 1 + fy, c);
                    }
                }
            }
        }
    }
    p
}

fn porta_potty(p: &mut Pixels, x: i32, y: i32) {
    let blue = hex(0x2e4e86);
    let blue_dk = hex(0x203a66);
    let blue_hi = hex(0x3e64a0);
    let roof = hex(0x4a6eaa);
    let out = hex(0x0e1428);
    let (w, h) = (22, 50);
    p.fill_rect_wrap(x - 1, y - 1, w + 2, h + 1, out);
    p.fill_rect_wrap(x, y + 4, w, h - 4, blue);
    p.fill_rect_wrap(x + w - 4, y + 4, 4, h - 4, blue_dk);
    p.fill_rect_wrap(x, y + 4, 2, h - 4, blue_hi);
    // Rounded roof.
    p.fill_rect_wrap(x + 1, y, w - 2, 4, roof);
    p.fill_rect_wrap(x - 1, y + 3, w + 2, 2, out);
    // Door panel, crescent moon window (glowing), handle.
    p.fill_rect_wrap(x + 3, y + 8, w - 8, h - 10, blue_dk);
    p.fill_rect_wrap(x + 4, y + 9, w - 10, h - 12, blue);
    disc_wrap(p, x + 9, y + 15, 3, hex(0xc8b860));
    disc_wrap(p, x + 10, y + 14, 2, blue);
    p.fill_rect_wrap(x + w - 7, y + 28, 2, 4, hex(0x8a8a9a));
    // Vents on the side.
    for v in 0..3 {
        p.fill_rect_wrap(x + w - 3, y + 8 + v * 3, 2, 1, out);
    }
}

// ---------------------------------------------------------------- 5: treatment plant / golden hall

fn plant() -> Pixels {
    let mut p = Pixels::new(W as u32, H as u32);
    vgradient(&mut p, 0, H, &[hex(0x14101e), hex(0x221a30), hex(0x2a2236)]);
    // Back wall panels.
    let seam = hex(0x100c18);
    for y in 0..H {
        for x in 0..W {
            if x % 32 == 0 || y % 36 == 0 {
                p.set(x, y, seam);
            }
        }
    }
    // Banners between pillars.
    for bx in [44, 172] {
        banner(&mut p, bx, 6);
    }
    // Tanks between the pillars.
    for cx in [64, 192] {
        tank(&mut p, cx, 50);
    }
    // Golden pillars every 128px.
    for px in [0, 128] {
        pillar(&mut p, px);
    }
    // A catwalk railing in front of the tanks.
    let rail = hex(0x5a4a2a);
    for x in 0..W {
        p.set(x, 112, rail);
        p.set(x, 113, hex(0x3a2e1a));
        if x % 12 == 0 {
            for y in 112..H {
                p.set(x, y, rail);
            }
        }
    }
    p
}

fn pillar(p: &mut Pixels, x: i32) {
    let gold = hex(0x9a7a2e);
    let gold_hi = hex(0xc49a3a);
    let gold_dk = hex(0x5e4a1c);
    let out = hex(0x0c0a10);
    for y in 0..H {
        for dx in -9i32..=9 {
            let c = if dx.abs() == 9 {
                out
            } else if dx % 4 == 0 {
                gold_dk
            } else if dx < -3 {
                gold_hi
            } else if dx > 4 {
                gold_dk
            } else {
                gold
            };
            p.set_wrap(x + dx, y, c);
        }
    }
    // Capital and base bands.
    for (y, h) in [(8, 6), (H - 10, 6)] {
        p.fill_rect_wrap(x - 12, y, 25, h, gold);
        p.fill_rect_wrap(x - 12, y, 25, 1, gold_hi);
        p.fill_rect_wrap(x - 12, y + h - 1, 25, 1, gold_dk);
        p.fill_rect_wrap(x - 13, y, 1, h, out);
        p.fill_rect_wrap(x + 13, y, 1, h, out);
    }
}

fn tank(p: &mut Pixels, cx: i32, top: i32) {
    let out = hex(0x0c0e14);
    let cols = [hex(0x2e3644), hex(0x3c4656), hex(0x4e5a6c), hex(0x5e6c80)];
    let band = hex(0x8a7030);
    let band_hi = hex(0xb08e3a);
    let r: i32 = 40;
    for y in top - 12..H {
        for dx in -r..=r {
            // Domed top: an ellipse cap.
            let dy = y - top;
            if dy < 0 {
                let e = (dx * dx) as f32 / (r * r) as f32 + (dy * dy) as f32 / 144.0;
                if e > 1.0 {
                    continue;
                }
            }
            let t = (dx + r) as f32 / (2 * r) as f32;
            let c = if dx.abs() == r {
                out
            } else {
                // Cylinder shading: dark edges, light just left of centre.
                let v = 1.0 - ((t - 0.4) * 2.2).abs();
                let i = (v.clamp(0.0, 0.999) * 4.0) as usize;
                let f = v * 4.0 - i as f32;
                let next = (i + 1).min(3);
                dith(cx + dx, y, cols[i], cols[next], f)
            };
            p.set_wrap(cx + dx, y, c);
        }
    }
    // Dome outline.
    for dx in -r..=r {
        let dy = -((1.0 - (dx * dx) as f32 / (r * r) as f32).max(0.0).sqrt() * 12.0) as i32;
        p.set_wrap(cx + dx, top + dy, out);
    }
    // Gold hoops.
    for y in [top + 14, top + 52] {
        for dx in -r + 1..r {
            p.set_wrap(cx + dx, y, band_hi);
            p.set_wrap(cx + dx, y + 1, band);
            p.set_wrap(cx + dx, y + 2, out);
        }
    }
    // Porthole with murky green glow.
    disc_wrap(p, cx - 6, top + 32, 7, out);
    disc_wrap(p, cx - 6, top + 32, 6, band);
    disc_wrap(p, cx - 6, top + 32, 4, hex(0x2a6a20));
    p.set_wrap(cx - 8, top + 30, hex(0x5aa040));
}

fn banner(p: &mut Pixels, x: i32, y: i32) {
    let red = hex(0x5a1a2e);
    let red_dk = hex(0x3e1020);
    let gold = hex(0xa8883a);
    let (w, h) = (40i32, 30i32);
    for yy in 0..h + 6 {
        for xx in 0..w {
            // Swallow-tail bottom.
            let tail = (xx - w / 2).abs();
            if yy >= h && yy - h > 6 - tail * 6 / (w / 2) {
                continue;
            }
            let c = if xx == 0 || xx == w - 1 || yy == 0 { gold } else if xx < 3 { red_dk } else { red };
            p.set_wrap(x + xx, y + yy, c);
        }
    }
    // Little crown emblem.
    let (cx, cy) = (x + w / 2, y + 12);
    p.fill_rect_wrap(cx - 5, cy + 2, 11, 3, gold);
    for d in [-5, 0, 5] {
        p.fill_rect_wrap(cx + d, cy - 2, 1, 4, gold);
    }
}

pub fn backdrop(w: u8) -> Vec<Pixels> {
    let (p, dim) = match w.clamp(1, 5) {
        1 => (bathroom(), 0.74),
        2 => (pipes(), 1.0),
        3 => (sewer(), 1.0),
        4 => (festival(), 1.0),
        _ => (plant(), 0.92),
    };
    vec![darken(p, dim)]
}

/// Scale every colour toward black, keeping the palette (each colour maps to exactly one).
fn darken(mut p: Pixels, f: f32) -> Pixels {
    for px in p.data.chunks_exact_mut(4) {
        for c in &mut px[..3] {
            *c = (*c as f32 * f).round() as u8;
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Backdrops are drawn behind everything: no holes allowed.
    #[test]
    fn backdrops_are_opaque() {
        for w in 1..=5 {
            let b = &backdrop(w)[0];
            assert!(b.data.chunks_exact(4).all(|px| px[3] == 255), "world {w} has holes");
        }
    }

    #[test]
    fn backdrops_use_a_limited_palette() {
        for w in 1..=5 {
            let n = backdrop(w)[0].colour_count();
            assert!(n <= 40, "world {w} backdrop uses {n} colours");
        }
    }
}

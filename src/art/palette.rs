//! Pixel buffers, palettes, and the ASCII-grid decoder every sprite is built from.

pub type Rgba = [u8; 4];

/// Transparent pixel.
pub const CLEAR: Rgba = [0, 0, 0, 0];

/// A palette maps one char to one colour. `.` is always transparent and never needs listing.
pub type Palette = &'static [(char, Rgba)];

/// `#rrggbb` as an opaque colour, usable in consts.
pub const fn hex(v: u32) -> Rgba {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8, 255]
}

/// A plain RGBA8 image (sRGB), row 0 at the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pixels {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

impl Pixels {
    pub fn new(w: u32, h: u32) -> Self {
        Self { w, h, data: vec![0; (w * h * 4) as usize] }
    }

    pub fn get(&self, x: i32, y: i32) -> Rgba {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return CLEAR;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        [self.data[i], self.data[i + 1], self.data[i + 2], self.data[i + 3]]
    }

    /// Set a pixel; out-of-bounds writes are ignored.
    pub fn set(&mut self, x: i32, y: i32, c: Rgba) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        self.data[i..i + 4].copy_from_slice(&c);
    }

    /// Set a pixel, wrapping x around the width (for horizontally tiling images).
    pub fn set_wrap(&mut self, x: i32, y: i32, c: Rgba) {
        self.set(x.rem_euclid(self.w as i32), y, c);
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }

    pub fn fill_rect_wrap(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set_wrap(xx, yy, c);
            }
        }
    }

    /// Draw `src` on top (alpha-tested: transparent source pixels are skipped).
    pub fn blit(&mut self, src: &Pixels, x: i32, y: i32) {
        for sy in 0..src.h as i32 {
            for sx in 0..src.w as i32 {
                let c = src.get(sx, sy);
                if c[3] > 0 {
                    self.set(x + sx, y + sy, c);
                }
            }
        }
    }

    pub fn blit_wrap(&mut self, src: &Pixels, x: i32, y: i32) {
        for sy in 0..src.h as i32 {
            for sx in 0..src.w as i32 {
                let c = src.get(sx, sy);
                if c[3] > 0 {
                    self.set_wrap(x + sx, y + sy, c);
                }
            }
        }
    }

    /// Multiply every pixel's alpha by `a` (0..=255).
    pub fn fade(mut self, a: u8) -> Self {
        for px in self.data.chunks_exact_mut(4) {
            px[3] = (px[3] as u16 * a as u16 / 255) as u8;
        }
        self
    }

    /// Replace colours exactly matching `from` with `to`.
    pub fn recolor(mut self, map: &[(Rgba, Rgba)]) -> Self {
        for px in self.data.chunks_exact_mut(4) {
            for (from, to) in map {
                if px == from {
                    px.copy_from_slice(to);
                    break;
                }
            }
        }
        self
    }

    /// Number of distinct opaque colours (for palette-discipline tests).
    pub fn colour_count(&self) -> usize {
        let mut seen: Vec<Rgba> = Vec::new();
        for px in self.data.chunks_exact(4) {
            let c = [px[0], px[1], px[2], px[3]];
            if c[3] > 0 && !seen.contains(&c) {
                seen.push(c);
            }
        }
        seen.len()
    }
}

/// Why a grid failed to decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridError {
    Empty,
    Ragged { row: usize, len: usize, expected: usize },
    UnknownChar { row: usize, col: usize, ch: char },
}

/// Decode an ASCII grid (one char per pixel) through a palette.
pub fn try_grid(rows: &[&str], pal: &[(char, Rgba)]) -> Result<Pixels, GridError> {
    let h = rows.len();
    let w = rows.first().map(|r| r.chars().count()).unwrap_or(0);
    if h == 0 || w == 0 {
        return Err(GridError::Empty);
    }
    let mut px = Pixels::new(w as u32, h as u32);
    for (y, row) in rows.iter().enumerate() {
        let len = row.chars().count();
        if len != w {
            return Err(GridError::Ragged { row: y, len, expected: w });
        }
        for (x, ch) in row.chars().enumerate() {
            if ch == '.' {
                continue;
            }
            let Some(&(_, c)) = pal.iter().find(|(k, _)| *k == ch) else {
                return Err(GridError::UnknownChar { row: y, col: x, ch });
            };
            px.set(x as i32, y as i32, c);
        }
    }
    Ok(px)
}

/// [`try_grid`], panicking with a helpful message (grids are compile-time constants; tests cover them).
pub fn grid(rows: &[&str], pal: &[(char, Rgba)]) -> Pixels {
    try_grid(rows, pal).unwrap_or_else(|e| panic!("bad sprite grid: {e:?}\n{}", rows.join("\n")))
}

/// Tiny deterministic hash for procedural noise (stable across platforms).
pub fn hash2(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77) ^ seed.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    h
}

/// 4x4 Bayer ordered-dither threshold, 0..16.
pub fn bayer4(x: i32, y: i32) -> u32 {
    const M: [[u32; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    M[y.rem_euclid(4) as usize][x.rem_euclid(4) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: Palette = &[('a', hex(0xff0000))];

    #[test]
    fn grid_decodes() {
        let p = grid(&["a.", ".a"], P);
        assert_eq!((p.w, p.h), (2, 2));
        assert_eq!(p.get(0, 0), hex(0xff0000));
        assert_eq!(p.get(1, 0), CLEAR);
    }

    #[test]
    fn grid_errors() {
        assert_eq!(try_grid(&["a.", "a"], P), Err(GridError::Ragged { row: 1, len: 1, expected: 2 }));
        assert_eq!(try_grid(&["ab"], P), Err(GridError::UnknownChar { row: 0, col: 1, ch: 'b' }));
        assert_eq!(try_grid(&[], P), Err(GridError::Empty));
    }
}

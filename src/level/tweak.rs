//! Per-tile physics tweaks: a layer of letters over the tile grid, each letter a `tweak` header
//! line that bends the physics in its cells (see "Tweaks" in the `level` module docs).
//!
//! Two kinds of keys:
//! - **surface** keys (`friction`, `bounce`, `belt`, `jump`) are the feel of a floor: they apply
//!   while a body stands on a tile painted with the tweak (paint them on ground, `=` or grease);
//! - **volume** keys (`gravity`, `run`, `wind`) are the air: they apply while a body's center is
//!   in a cell painted with the tweak (paint them on empty cells).
//!
//! - **Han's zones** (`han=avoid|weak|boost|wait`) change what Han does in the painted cells
//!   ([`HanZone`]).
//!
//! Nat and Han obey them (`game::physics`, `level::buddy::drive`). The validator and Han's nav
//! graph don't model them: a level with tweaks is checked as if they weren't there, so playtest
//! it.

use bevy::prelude::*;

use super::{Level, TILE};

/// One `tweak <letter>: key=value ...` line. A key left out is the normal physics.
#[derive(Debug, Clone, Copy, PartialEq, Default, Reflect)]
pub struct Tweak {
    /// Surface: × ground acceleration and braking. 0 is ice (you keep sliding, but can still
    /// jump, unlike grease), 2 is sticky.
    pub friction: Option<f32>,
    /// Surface: landing faster than a hop springs you back up at this × your fall speed (1 is
    /// a trampoline that gives back what you fell; a jump off the spring keeps the higher speed).
    pub bounce: Option<f32>,
    /// Surface: a conveyor belt moving you this many tiles/s (negative: left). Stepping or
    /// jumping off it keeps its speed, like a moving platform's.
    pub belt: Option<f32>,
    /// Surface: × the jump off it (a springy floor, a sticky one).
    pub jump: Option<f32>,
    /// Volume: × gravity (negative floats you up).
    pub gravity: Option<f32>,
    /// Volume: × top run speed and acceleration (molasses, a tailwind).
    pub run: Option<f32>,
    /// Volume: a wind pushing you this many tiles/s (negative: left).
    pub wind: Option<f32>,
    /// What Han does in these cells.
    pub han: Option<HanZone>,
}

/// A Han zone: cells painted with a tweak that has `han=<word>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum HanZone {
    /// He never stands here (his route goes round, and on foot he stops at its edge).
    Avoid,
    /// His head's boost is the weak one here, as in a band zone.
    Weak,
    /// His head's boost is full strength here, even in a band zone (the validator checks the
    /// gates still hold).
    Boost,
    /// He stays put here (following Nat) until Nat comes back to him; if Nat goes far he
    /// parachutes in later as usual.
    Wait,
}

impl HanZone {
    pub const ALL: [HanZone; 4] = [HanZone::Avoid, HanZone::Weak, HanZone::Boost, HanZone::Wait];

    pub fn word(self) -> &'static str {
        match self {
            HanZone::Avoid => "avoid",
            HanZone::Weak => "weak",
            HanZone::Boost => "boost",
            HanZone::Wait => "wait",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            HanZone::Avoid => "Han never stands here",
            HanZone::Weak => "Han's boost is the weak one here",
            HanZone::Boost => "Han's boost is full strength here, band zone or not",
            HanZone::Wait => "Han waits here until Nat comes back",
        }
    }

    pub fn from_word(w: &str) -> Option<HanZone> {
        HanZone::ALL.into_iter().find(|z| z.word() == w)
    }
}

/// A tweak key: its word, whether it's a surface key, its range, and what it does.
pub struct Key {
    pub word: &'static str,
    pub surface: bool,
    pub range: std::ops::RangeInclusive<f32>,
    pub about: &'static str,
}

/// Every key, in the order lines are written.
pub const KEYS: [Key; 7] = [
    Key { word: "friction", surface: true, range: 0.0..=4.0, about: "× grip on the floor (0 ice, 2 sticky)" },
    Key { word: "bounce", surface: true, range: 0.0..=1.5, about: "landing springs you up (1 trampoline)" },
    Key { word: "belt", surface: true, range: -12.0..=12.0, about: "conveyor, tiles/s (negative: left)" },
    Key { word: "jump", surface: true, range: 0.0..=3.0, about: "× jumps off the floor" },
    Key { word: "gravity", surface: false, range: -2.0..=3.0, about: "× gravity in the air (negative floats)" },
    Key { word: "run", surface: false, range: 0.0..=3.0, about: "× run speed in the air cells" },
    Key { word: "wind", surface: false, range: -12.0..=12.0, about: "wind, tiles/s (negative: left)" },
];

/// Fastest a bounce tweak springs a body up (px/s): a little over Han's plunger boost.
pub const BOUNCE_MAX: f32 = 600.0;

impl Tweak {
    pub fn get(&self, word: &str) -> Option<f32> {
        match word {
            "friction" => self.friction,
            "bounce" => self.bounce,
            "belt" => self.belt,
            "jump" => self.jump,
            "gravity" => self.gravity,
            "run" => self.run,
            "wind" => self.wind,
            _ => None,
        }
    }

    pub fn slot(&mut self, word: &str) -> Option<&mut Option<f32>> {
        Some(match word {
            "friction" => &mut self.friction,
            "bounce" => &mut self.bounce,
            "belt" => &mut self.belt,
            "jump" => &mut self.jump,
            "gravity" => &mut self.gravity,
            "run" => &mut self.run,
            "wind" => &mut self.wind,
            _ => return None,
        })
    }

    /// The value of a `tweak` line (`friction=0 bounce=0.8`).
    pub fn parse(value: &str) -> Result<Tweak, String> {
        let mut t = Tweak::default();
        for kv in value.split_whitespace() {
            let (k, v) = kv.split_once('=').ok_or_else(|| format!("expected key=value, got `{kv}`"))?;
            if k == "han" {
                t.han = Some(HanZone::from_word(v).ok_or_else(|| {
                    format!("han: `{v}` (zones: {})", HanZone::ALL.map(|z| z.word()).join(", "))
                })?);
                continue;
            }
            let key = KEYS.iter().find(|key| key.word == k).ok_or_else(|| {
                format!("unknown key `{k}` (keys: {})", KEYS.iter().map(|k| k.word).collect::<Vec<_>>().join(", "))
            })?;
            let n: f32 = v.parse().map_err(|_| format!("{k}: bad number `{v}`"))?;
            if !key.range.contains(&n) {
                return Err(format!("{k}={n} is out of range {:?}", key.range));
            }
            *t.slot(k).unwrap() = Some(n);
        }
        Ok(t)
    }

    /// Back to a line's value (`parse` reads it back the same).
    pub fn to_value(&self) -> String {
        KEYS.iter()
            .filter_map(|k| self.get(k.word).map(|v| format!("{}={v}", k.word)))
            .chain(self.han.map(|z| format!("han={}", z.word())))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn has_surface(&self) -> bool {
        KEYS.iter().any(|k| k.surface && self.get(k.word).is_some())
    }

    pub fn has_volume(&self) -> bool {
        KEYS.iter().any(|k| !k.surface && self.get(k.word).is_some())
    }
}

/// The physics a body feels where it is this step (all tweaks applied; [`Here::NORMAL`]
/// without any).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Here {
    pub gravity: f32,
    pub run: f32,
    /// px/s.
    pub wind: f32,
    pub friction: f32,
    pub jump: f32,
    /// 0: no spring.
    pub bounce: f32,
    /// px/s.
    pub belt: f32,
}

impl Here {
    pub const NORMAL: Here = Here { gravity: 1.0, run: 1.0, wind: 0.0, friction: 1.0, jump: 1.0, bounce: 0.0, belt: 0.0 };
}

impl Default for Here {
    fn default() -> Self {
        Here::NORMAL
    }
}

impl Level {
    /// The tweak painted on cell (col, row), if any.
    pub fn tweak_at(&self, col: i32, row: i32) -> Option<&Tweak> {
        if self.tweaks.is_empty() || col < 0 || row < 0 || col >= self.width as i32 || row >= self.height as i32 {
            return None;
        }
        let k = *self.tweak_cells.get(row as usize * self.width + col as usize)?;
        (k > 0).then(|| &self.tweaks[k as usize - 1].1)
    }

    /// The tweak on the tile under the feet of a box centered at `pos` (half extents `half`):
    /// the tile under its center, else under either edge.
    pub fn tweak_underfoot(&self, pos: Vec2, half: Vec2) -> Option<&Tweak> {
        let y = pos.y - half.y - 0.5;
        [pos.x, pos.x - half.x + 0.5, pos.x + half.x - 0.5].into_iter().find_map(|x| {
            let (c, r) = self.cell_at(Vec2::new(x, y));
            let t = self.tile(c, r);
            (t.is_solid() || t.is_one_way()).then(|| self.tweak_at(c, r)).flatten()
        })
    }

    /// The physics a box centered at `pos` feels: the volume keys of the cell its center is in,
    /// and, `grounded` (standing on the tiles, not a carrier), the surface keys of the tile
    /// under its feet.
    pub fn here(&self, pos: Vec2, half: Vec2, grounded: bool) -> Here {
        if self.tweaks.is_empty() {
            return Here::NORMAL;
        }
        let mut h = Here::NORMAL;
        let (c, r) = self.cell_at(pos);
        if let Some(t) = self.tweak_at(c, r) {
            h.gravity = t.gravity.unwrap_or(1.0);
            h.run = t.run.unwrap_or(1.0);
            h.wind = t.wind.unwrap_or(0.0) * TILE;
        }
        if grounded && let Some(t) = self.tweak_underfoot(pos, half) {
            h.friction = t.friction.unwrap_or(1.0);
            h.jump = t.jump.unwrap_or(1.0);
            h.bounce = t.bounce.unwrap_or(0.0);
            h.belt = t.belt.unwrap_or(0.0) * TILE;
        }
        h
    }

    /// The Han zone painted on cell (col, row), if any.
    pub fn han_zone(&self, (col, row): (i32, i32)) -> Option<HanZone> {
        self.tweak_at(col, row).and_then(|t| t.han)
    }

    /// The bounce of the floor a box centered at `pos` just landed on (0: none).
    pub fn bounce_underfoot(&self, pos: Vec2, half: Vec2) -> f32 {
        if self.tweaks.is_empty() {
            return 0.0;
        }
        self.tweak_underfoot(pos, half).and_then(|t| t.bounce).unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_writes_back() {
        let t = Tweak::parse("bounce=0.8 friction=0 wind=-1.5").unwrap();
        assert_eq!((t.friction, t.bounce, t.wind), (Some(0.0), Some(0.8), Some(-1.5)));
        assert_eq!(t.to_value(), "friction=0 bounce=0.8 wind=-1.5");
        assert_eq!(Tweak::parse(&t.to_value()).unwrap(), t);
        assert!(t.has_surface() && t.has_volume());
        assert!(Tweak::parse("bogus=1").is_err());
        assert!(Tweak::parse("bounce=9").is_err());
        assert!(Tweak::parse("bounce").is_err());
        let z = Tweak::parse("han=wait gravity=0.5").unwrap();
        assert_eq!(z.han, Some(HanZone::Wait));
        assert_eq!(z.to_value(), "gravity=0.5 han=wait");
        assert!(Tweak::parse("han=dance").is_err());
    }

    #[test]
    fn here_reads_the_layer() {
        let src = "name: T\ntweak a: bounce=1 belt=2\ntweak b: gravity=0.5\n---\n......\n.P..G.\n######\n---\n......\n..bb..\n.aa...\n";
        let l = Level::parse(src).unwrap();
        assert_eq!(l.tweak_at(1, 2).and_then(|t| t.bounce), Some(1.0));
        assert_eq!(l.tweak_at(0, 2), None);
        let half = Vec2::splat(6.0);
        // Standing on (1, 2): its top is y = 16.
        let feet = Vec2::new(1.5 * TILE, TILE + half.y);
        let h = l.here(feet, half, true);
        assert_eq!((h.bounce, h.belt, h.gravity), (1.0, 2.0 * TILE, 1.0));
        assert_eq!(l.here(feet, half, false).bounce, 0.0, "in the air no floor");
        // Center in (2, 1): low gravity.
        let air = l.tile_center(2, 1);
        assert_eq!(l.here(air, half, false).gravity, 0.5);
        assert!(Level::parse("name: T\n---\nPG\n##\n---\nz.\n").is_err(), "unknown letter");
        assert!(Level::parse("name: T\ntweak a: run=2\n---\nPG\n##\n---\n..a\n").is_err(), "wider than the grid");
    }
}

//! The ASCII level format. Levels live in `levels/NN.txt` and are compiled in with
//! `include_str!`, so the web build loads nothing at runtime.
//!
//! A file is a header, a `---` line, then the tile grid (one char per 16x16 tile, row 0 at the top):
//!
//! ```text
//! name: Bathroom Floor
//! world: 1
//! intro: Gus's line when the level starts.
//! say: Gus's line at the first checkpoint.
//! say: ...at the second checkpoint (checkpoints are numbered in reading order: left to right, then top to bottom).
//! 1: dx=6 dy=0 period=4 kind=tp
//! ---
//! ....................
//! .P......o.o....11...
//! ####===######.....G#
//! ```
//!
//! Grid legend:
//! | char | meaning |
//! |---|---|
//! | `.` or space | empty |
//! | `#` | solid ground |
//! | `=` | one-way platform (jump up through it, stand on top; hold Down+Jump does nothing — no drop-through) |
//! | `P` | player start (exactly one). Gus spawns with you |
//! | `o` | golden nugget (the coin) |
//! | `C` | checkpoint (toilet-paper holder) |
//! | `G` | goal flag (exactly one) |
//! | `^` | spikes on the floor (toilet brushes), deadly; occupies the bottom half of the tile |
//! | `v` | spikes hanging from the ceiling, deadly; top half of the tile |
//! | `~` | deadly liquid (sewage); the whole tile, surface drawn at the top |
//! | `F` | fly swarm: hovers in a circle (radius ~1 tile) around its tile, deadly |
//! | `S` | air-freshener spray: a jet firing straight up 3 tiles, on/off cycle, deadly while on. Sits on the floor |
//! | `1`-`9` | moving platform: a horizontal run of the same digit is one platform, configured by its `N:` header line |
//!
//! Moving platform header: `N: dx=<tiles> dy=<tiles> period=<secs> [kind=tp|duck|plunger] [phase=<0..1>]`.
//! The platform ping-pongs between its grid position and that position + (dx, dy) tiles
//! (dy positive = up), one full round trip per `period`. Platforms are solid on top only
//! (like `=`) and carry whoever stands on them.
//!
//! Rows may be ragged; short rows are padded with empty. Outside the grid: left/right is a solid
//! wall, above is open sky, below is a bottomless pit (death).

use bevy::prelude::*;

pub const TILE: f32 = 16.0;
pub const LEVEL_COUNT: usize = 10;

/// Source of every level, in play order.
pub static LEVEL_SOURCES: [&str; LEVEL_COUNT] = [
    include_str!("../levels/01.txt"),
    include_str!("../levels/02.txt"),
    include_str!("../levels/03.txt"),
    include_str!("../levels/04.txt"),
    include_str!("../levels/05.txt"),
    include_str!("../levels/06.txt"),
    include_str!("../levels/07.txt"),
    include_str!("../levels/08.txt"),
    include_str!("../levels/09.txt"),
    include_str!("../levels/10.txt"),
];

/// All levels, parsed once at startup.
#[derive(Resource, Debug, Clone)]
pub struct Levels(pub Vec<Level>);

impl Default for Levels {
    fn default() -> Self {
        Self(
            LEVEL_SOURCES
                .iter()
                .enumerate()
                .map(|(i, src)| {
                    Level::parse(src).unwrap_or_else(|e| panic!("levels/{:02}.txt: {e}", i + 1))
                })
                .collect(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum Tile {
    Empty,
    Solid,
    OneWay,
    SpikesUp,
    SpikesDown,
    Liquid,
}

impl Tile {
    pub fn is_deadly(self) -> bool {
        matches!(self, Tile::SpikesUp | Tile::SpikesDown | Tile::Liquid)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub enum PlatformKind {
    /// Floating toilet-paper roll.
    #[default]
    Tp,
    /// Rubber duck bobbing on the current.
    Duck,
    /// Plunger head going up and down.
    Plunger,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct MovingPlatformDef {
    /// Grid position of the leftmost tile.
    pub col: usize,
    pub row: usize,
    /// Width in tiles.
    pub width: usize,
    /// Travel in tiles; dy positive is up.
    pub dx: f32,
    pub dy: f32,
    /// Seconds per full round trip.
    pub period: f32,
    /// Starting point in the cycle, 0..1.
    pub phase: f32,
    pub kind: PlatformKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum ThingKind {
    Nugget,
    Checkpoint,
    Fly,
    Spray,
}

/// A non-tile object at a grid cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct Thing {
    pub kind: ThingKind,
    pub col: usize,
    pub row: usize,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct Level {
    pub name: String,
    /// 1..=5: picks tileset, backdrop and music.
    pub world: u8,
    pub intro: String,
    /// Gus's line per checkpoint, in checkpoint order. Missing lines get a generic quip.
    pub says: Vec<String>,
    pub width: usize,
    pub height: usize,
    /// Row-major, row 0 at the top.
    pub tiles: Vec<Tile>,
    pub start: (usize, usize),
    pub goal: (usize, usize),
    /// In reading order (top-to-bottom, left-to-right). Checkpoint `index` is the order among
    /// `ThingKind::Checkpoint` things.
    pub things: Vec<Thing>,
    pub platforms: Vec<MovingPlatformDef>,
}

impl Level {
    pub fn tile(&self, col: i32, row: i32) -> Tile {
        if col < 0 || col >= self.width as i32 {
            Tile::Solid
        } else if row < 0 || row >= self.height as i32 {
            Tile::Empty
        } else {
            self.tiles[row as usize * self.width + col as usize]
        }
    }

    /// World-space (pixels, y up) center of a grid cell. The bottom-left of the grid is (0, 0).
    pub fn tile_center(&self, col: usize, row: usize) -> Vec2 {
        Vec2::new(
            col as f32 * TILE + TILE / 2.0,
            (self.height - 1 - row) as f32 * TILE + TILE / 2.0,
        )
    }

    /// Grid cell containing a world-space point (may be out of bounds).
    pub fn cell_at(&self, p: Vec2) -> (i32, i32) {
        let col = (p.x / TILE).floor() as i32;
        let row = self.height as i32 - 1 - (p.y / TILE).floor() as i32;
        (col, row)
    }

    pub fn size_px(&self) -> Vec2 {
        Vec2::new(self.width as f32 * TILE, self.height as f32 * TILE)
    }

    pub fn nugget_count(&self) -> usize {
        self.things.iter().filter(|t| t.kind == ThingKind::Nugget).count()
    }

    pub fn checkpoints(&self) -> impl Iterator<Item = &Thing> {
        self.things.iter().filter(|t| t.kind == ThingKind::Checkpoint)
    }

    pub fn parse(src: &str) -> Result<Level, String> {
        let (header, grid) = src
            .split_once("\n---\n")
            .ok_or("missing `---` line between header and grid")?;

        let mut name = None;
        let mut world = 1u8;
        let mut intro = String::new();
        let mut says = Vec::new();
        let mut platform_cfg: [Option<(f32, f32, f32, f32, PlatformKind)>; 10] = [None; 10];

        for (n, line) in header.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }
            let (key, value) = line
                .split_once(':')
                .ok_or_else(|| format!("header line {}: expected `key: value`", n + 1))?;
            let value = value.trim();
            match key.trim() {
                "name" => name = Some(value.to_string()),
                "world" => {
                    world = value.parse().map_err(|_| format!("bad world `{value}`"))?;
                    if !(1..=5).contains(&world) {
                        return Err(format!("world must be 1..=5, got {world}"));
                    }
                }
                "intro" => intro = value.to_string(),
                "say" => says.push(value.to_string()),
                d if d.len() == 1 && d.as_bytes()[0].is_ascii_digit() && d != "0" => {
                    let digit = (d.as_bytes()[0] - b'0') as usize;
                    let (mut dx, mut dy, mut period, mut phase, mut kind) =
                        (0.0, 0.0, 4.0, 0.0, PlatformKind::Tp);
                    for kv in value.split_whitespace() {
                        let (k, v) = kv
                            .split_once('=')
                            .ok_or_else(|| format!("platform {digit}: expected k=v, got `{kv}`"))?;
                        let num = || {
                            v.parse::<f32>()
                                .map_err(|_| format!("platform {digit}: bad number `{v}`"))
                        };
                        match k {
                            "dx" => dx = num()?,
                            "dy" => dy = num()?,
                            "period" => period = num()?,
                            "phase" => phase = num()?,
                            "kind" => {
                                kind = match v {
                                    "tp" => PlatformKind::Tp,
                                    "duck" => PlatformKind::Duck,
                                    "plunger" => PlatformKind::Plunger,
                                    _ => return Err(format!("platform {digit}: unknown kind `{v}`")),
                                }
                            }
                            _ => return Err(format!("platform {digit}: unknown key `{k}`")),
                        }
                    }
                    if period <= 0.0 {
                        return Err(format!("platform {digit}: period must be > 0"));
                    }
                    platform_cfg[digit] = Some((dx, dy, period, phase, kind));
                }
                other => return Err(format!("unknown header key `{other}`")),
            }
        }

        let rows: Vec<&str> = grid.lines().map(|l| l.trim_end_matches('\r')).collect();
        // Drop trailing blank lines.
        let rows: Vec<&str> = {
            let end = rows.iter().rposition(|r| !r.trim().is_empty()).map_or(0, |i| i + 1);
            rows[..end].to_vec()
        };
        let height = rows.len();
        let width = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
        if width == 0 || height == 0 {
            return Err("empty grid".into());
        }

        let mut tiles = vec![Tile::Empty; width * height];
        let mut start = None;
        let mut goal = None;
        let mut things = Vec::new();
        let mut platforms = Vec::new();

        for (row, line) in rows.iter().enumerate() {
            let chars: Vec<char> = line.chars().collect();
            let mut col = 0;
            while col < chars.len() {
                let c = chars[col];
                let tile = &mut tiles[row * width + col];
                match c {
                    '.' | ' ' => {}
                    '#' => *tile = Tile::Solid,
                    '=' => *tile = Tile::OneWay,
                    '^' => *tile = Tile::SpikesUp,
                    'v' => *tile = Tile::SpikesDown,
                    '~' => *tile = Tile::Liquid,
                    'P' => {
                        if start.replace((col, row)).is_some() {
                            return Err("more than one `P`".into());
                        }
                    }
                    'G' => {
                        if goal.replace((col, row)).is_some() {
                            return Err("more than one `G`".into());
                        }
                    }
                    'o' => things.push(Thing { kind: ThingKind::Nugget, col, row }),
                    'C' => things.push(Thing { kind: ThingKind::Checkpoint, col, row }),
                    'F' => things.push(Thing { kind: ThingKind::Fly, col, row }),
                    'S' => things.push(Thing { kind: ThingKind::Spray, col, row }),
                    '1'..='9' => {
                        let digit = c.to_digit(10).unwrap() as usize;
                        let start_col = col;
                        while col + 1 < chars.len() && chars[col + 1] == c {
                            col += 1;
                        }
                        let (dx, dy, period, phase, kind) = platform_cfg[digit].ok_or_else(|| {
                            format!("platform `{c}` at row {row} has no `{c}:` header line")
                        })?;
                        platforms.push(MovingPlatformDef {
                            col: start_col,
                            row,
                            width: col - start_col + 1,
                            dx,
                            dy,
                            period,
                            phase,
                            kind,
                        });
                    }
                    other => {
                        return Err(format!("unknown tile `{other}` at row {row}, col {col}"));
                    }
                }
                col += 1;
            }
        }

        Ok(Level {
            name: name.ok_or("missing `name:`")?,
            world,
            intro,
            says,
            width,
            height,
            tiles,
            start: start.ok_or("missing `P` (player start)")?,
            goal: goal.ok_or("missing `G` (goal flag)")?,
            things,
            platforms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "name: Test\nworld: 2\nintro: hi\nsay: one\n1: dx=3 dy=1 period=2 kind=duck\n---\n......\n.P.o11G\n##=~^#\n";

    #[test]
    fn parses_sample() {
        let l = Level::parse(SAMPLE).unwrap();
        assert_eq!((l.width, l.height, l.world), (7, 3, 2));
        assert_eq!(l.start, (1, 1));
        assert_eq!(l.tile(2, 2), Tile::OneWay);
        assert_eq!(l.tile(3, 2), Tile::Liquid);
        assert_eq!(l.tile(-1, 0), Tile::Solid);
        assert_eq!(l.platforms.len(), 1);
        assert_eq!(l.platforms[0].width, 2);
        assert_eq!(l.platforms[0].kind, PlatformKind::Duck);
        assert_eq!(l.nugget_count(), 1);
        assert_eq!(l.tile_center(0, 2), Vec2::new(8.0, 8.0));
        assert_eq!(l.cell_at(Vec2::new(8.0, 8.0)), (0, 2));
    }

    #[test]
    fn all_levels_parse() {
        let levels = Levels::default();
        assert_eq!(levels.0.len(), LEVEL_COUNT);
    }
}

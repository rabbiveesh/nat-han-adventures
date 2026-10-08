//! The ASCII level format. Levels live in `levels/NN.txt` and are compiled in with
//! `include_str!`, so the web build loads nothing at runtime.
//!
//! A file is a header, a `---` line, then the tile grid (one char per 16x16 tile, row 0 at the top):
//!
//! ```text
//! name: Bathroom Floor
//! world: 1
//! intro: Han's line when the level starts.
//! deaths: 0
//! say@13,1: Han's line at the checkpoint in column 13, row 1.
//! hint@4,1 toot: Jump, then jump AGAIN in midair. That's a toot!
//! 1: dx=6 dy=0 period=4 kind=tp
//! ---
//! ....................
//! .P......o.o..C.11...
//! ####===######.....G#
//! ```
//!
//! Header keys (every line `key: value`; blank lines and `// comments` are skipped):
//! - `name`, `world` (1..=5: tileset, backdrop, music), `intro` (Han's line at the start).
//! - `say@<col>,<row>: text`: Han's line when Nat reaches the checkpoint in that cell.
//!   (Old form, still accepted: plain `say: text` lines go, in order, to the checkpoints that
//!   have no `say@`; checkpoints are numbered in reading order, top row first.)
//! - `hint@<col>,<row>[ topic...]: text`: a *hint spot*. Han says the line the first time Nat
//!   comes within [`HINT_RADIUS`] of that cell; once per level visit (dying doesn't repeat it),
//!   and again after a restart. Optional [`Topic`] words name the mechanics it teaches; the
//!   validator ([`validate`]) checks that every mechanic is taught by a hint before the
//!   player first meets it.
//! - `deaths: N`: the deaths the level's design *expects* (splat-stain stepping stones, the
//!   three splats that make the band nervous for a grease chute). [`validate`] checks it; the
//!   adaptive engine can read it.
//! - `N: ...` (a digit): a moving platform, see below.
//! - `gate: <topic> <c0>,<r0> <c1>,<r1>`: a *gate mark*, the cells (inclusive rectangle) of a
//!   gate the level is built around, named by its [`Topic`] word: `giant` (giant wall), `gap`
//!   (long gap), `waltz` (waltz row), `grip` (grease chute), `stain` (stain pit), `boost`
//!   (buddy ledge), `shield` (shield row), `chain` (chain-jump chasm), `buddyraft` (buddy raft
//!   pool). See "Gate marks" below.
//!
//! Lines (intro, say, hint) are at most [`MAX_LINE`] characters.
//!
//! Grid legend:
//! | char | meaning |
//! |---|---|
//! | `.` or space | empty |
//! | `#` | solid ground |
//! | `=` | one-way platform (jump up through it, stand on top; hold Down+Jump does nothing — no drop-through) |
//! | `_` | grease: solid ground with a slick top. Nat can't brake or jump on it and steers weakly, unless the band is nervous (sweaty grip). See `game::physics` |
//! | `P` | player start (exactly one). Han spawns with you |
//! | `o` | golden nugget (the coin) |
//! | `C` | checkpoint (toilet-paper holder) |
//! | `G` | goal flag (exactly one) |
//! | `^` | spikes on the floor (toilet brushes), deadly; occupies the bottom half of the tile |
//! | `v` | spikes hanging from the ceiling, deadly; top half of the tile |
//! | `~` | deadly liquid (sewage); the whole tile, surface drawn at the top |
//! | `F` | fly swarm: hovers in a circle (radius ~1 tile) around its tile, deadly |
//! | `S` | air-freshener spray: a can on the floor firing a jet straight up 3 tiles; the can and its jet are deadly while on (harmless while off). All cans share one on/off clock (while the band waltzes: the music's). 4+ adjacent cans on the floor you walk on, or under a `=` grating you walk on, with a ceiling at most 2 tiles above the walk make a *waltz row* (see `tests/levels.rs`) |
//! | `1`-`9` | moving platform: a horizontal run of the same digit is one platform, configured by its `N:` header line |
//!
//! Moving platform header: `N: dx=<tiles> dy=<tiles> period=<secs> [kind=tp|duck|plunger] [phase=<0..1>]`.
//! The platform ping-pongs between its grid position and that position + (dx, dy) tiles
//! (dy positive = up), one full round trip per `period`. Platforms are solid on top only
//! (like `=`) and carry whoever stands on them.
//!
//! # Gate marks
//! Every gate is marked in the header, and [`validate`] checks the marks against what it finds
//! (each crossing lies in a mark of its kind, each mark holds a crossing), so the marks can't
//! lie. The game reads them instead of re-running the validator at load:
//! - **Han's boost is feeble near the band's gates** (and the death gates): within the *band
//!   zone*, [`HAN_BERTH`] columns (and [`HAN_BERTH_ROWS`] rows) of a `giant`, `gap`, `waltz`,
//!   `grip` or `stain` mark ([`Level::in_band_zone`]), jumping off his head is the weak boost
//!   (`game::WEAK_BOOST_SPEED`, about a normal jump) and he grumbles; there his head only
//!   holds Nat while he's standing on the ground. He follows normally, but keeps out of a
//!   waltz row's mark and off the grease in a grease chute's zone ([`Level::han_keeps_out`]).
//!   So no boost, and no chain of boosts, ever opens them: they always need their mode (the
//!   validator proves it: full boosts from everywhere outside the zones Han may be, weak ones
//!   from everywhere inside them Nat may get to).
//! - **Chain-jump chasms** (`chain`) have no overuse limit: Han boosts you as often as it takes
//!   while you're in the mark's columns ([`Level::in_chasm`]).
//! - **Markers** are drawn from them: giant walls get gold music-staff trim and a note
//!   emblem, buddy ledges red plunger-handle notches and yellow plumber's tape, shield rows a
//!   "PLUMBERS ONLY" sign at their start.
//!
//! Rows may be ragged; short rows are padded with empty. Outside the grid: left/right is a solid
//! wall, above is open sky, below is a bottomless pit (death).
//!
//! # Splat stains
//! Death leaves a mark. Splatting on spikes (`^`/`v`) turns that spike tile into a *stain*
//! ([`Tile::StainUp`]/[`Tile::StainDown`]): safe, and solid on top like `=`. Sinking in `~`
//! leaves a *stain raft* floating at the surface for a while (at least `game::RAFT_LIFE_FLOOR` s), then it
//! sinks. Stains last for the rest of the level visit (checkpoints keep them: they're
//! progress) and are gone after a restart or a reload: they live in the loaded copy of the
//! level (`game::ActiveLevel`) and as level entities. A *stain pit* is a spike pit too wide to
//! jump in any mode: you cross it on the stains of your own splats. Falling out of the bottom
//! of the level leaves nothing (so long gaps are bottomless: no raft can bridge them).

use bevy::prelude::*;


pub mod buddy;
pub mod nav;
pub mod validate;

pub const TILE: f32 = 16.0;
/// Han says a hint when Nat comes this close (px) to its cell's center.
pub const HINT_RADIUS: f32 = 2.5 * TILE;
/// Longest line Han says (intro, checkpoint lines, hints).
pub const MAX_LINE: usize = 60;
pub const LEVEL_COUNT: usize = 10;
/// The band zone: this many columns around a band (or death) gate's mark, Han's boost is the
/// weak one (see "Gate marks"). (The name is from when he kept this berth.)
pub const HAN_BERTH: i32 = 14;
/// ...and this many rows above/below it.
pub const HAN_BERTH_ROWS: i32 = 10;

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
    /// Solid ground with a greasy top (`_`).
    Grease,
    /// Floor spikes splatted over: safe, solid on top (one-way). Only made at runtime.
    StainUp,
    /// Ceiling spikes splatted over: safe, solid on top (one-way). Only made at runtime.
    StainDown,
}

impl Tile {
    pub fn is_deadly(self) -> bool {
        matches!(self, Tile::SpikesUp | Tile::SpikesDown | Tile::Liquid)
    }

    /// Blocks from every side (ground, grease).
    pub fn is_solid(self) -> bool {
        matches!(self, Tile::Solid | Tile::Grease)
    }

    /// Solid on top only: you jump up through it and stand on it (`=`, stains).
    pub fn is_one_way(self) -> bool {
        matches!(self, Tile::OneWay | Tile::StainUp | Tile::StainDown)
    }

    /// The stain a splat on this tile leaves, if it's spikes.
    pub fn stained(self) -> Option<Tile> {
        match self {
            Tile::SpikesUp => Some(Tile::StainUp),
            Tile::SpikesDown => Some(Tile::StainDown),
            _ => None,
        }
    }
}

/// What a hint spot teaches (the words after `hint@col,row`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub enum Topic {
    /// The mid-air double jump (`toot`).
    Toot,
    /// One-way platforms `=` (`oneway`).
    OneWay,
    /// Moving platforms (`platform`).
    Platform,
    /// Fly swarms (`fly`).
    Fly,
    /// Spray cans (`spray`).
    Spray,
    /// Giant walls and Giant Steps (`giant`).
    Giant,
    /// Long gaps and the fired-up band (`gap`).
    Gap,
    /// Waltz rows and the waltzing band (`waltz`).
    Waltz,
    /// Stain pits: splat to build stepping stones (`stain`).
    Stain,
    /// Grease (`grease`).
    Grease,
    /// Sweaty grip: the nervous band lets Nat brake on grease (`grip`).
    Grip,
    /// Buddy ledges: land on Han's head and jump, the plunger boost (`boost`).
    Boost,
    /// Shield rows: Han goes ahead into the jets, you walk behind him (`shield`).
    Shield,
    /// Chain-jump chasms: boost off Han in mid-air, toot, again (`chain`).
    Chain,
    /// Buddy raft pools: Han wades in and leaves a big raft (`buddyraft`).
    BuddyRaft,
}

impl Topic {
    pub const ALL: [Topic; 15] = [
        Topic::Toot,
        Topic::OneWay,
        Topic::Platform,
        Topic::Fly,
        Topic::Spray,
        Topic::Giant,
        Topic::Gap,
        Topic::Waltz,
        Topic::Stain,
        Topic::Grease,
        Topic::Grip,
        Topic::Boost,
        Topic::Shield,
        Topic::Chain,
        Topic::BuddyRaft,
    ];

    pub fn word(self) -> &'static str {
        match self {
            Topic::Toot => "toot",
            Topic::OneWay => "oneway",
            Topic::Platform => "platform",
            Topic::Fly => "fly",
            Topic::Spray => "spray",
            Topic::Giant => "giant",
            Topic::Gap => "gap",
            Topic::Waltz => "waltz",
            Topic::Stain => "stain",
            Topic::Grease => "grease",
            Topic::Grip => "grip",
            Topic::Boost => "boost",
            Topic::Shield => "shield",
            Topic::Chain => "chain",
            Topic::BuddyRaft => "buddyraft",
        }
    }

    pub fn from_word(w: &str) -> Option<Topic> {
        Topic::ALL.into_iter().find(|t| t.word() == w)
    }

    /// Topics that name a gate (the words a `gate:` mark takes).
    pub fn is_gate(self) -> bool {
        matches!(
            self,
            Topic::Giant
                | Topic::Gap
                | Topic::Waltz
                | Topic::Grip
                | Topic::Stain
                | Topic::Boost
                | Topic::Shield
                | Topic::Chain
                | Topic::BuddyRaft
        )
    }

    /// The band's gates and the death gates: Han's boost is weak in their zone.
    pub fn han_keeps_clear(self) -> bool {
        matches!(self, Topic::Giant | Topic::Gap | Topic::Waltz | Topic::Grip | Topic::Stain)
    }
}

/// A `gate:` header line: the cells `c0..=c1` x `r0..=r1` hold a gate of kind `topic`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct GateMark {
    pub topic: Topic,
    pub c0: i32,
    pub r0: i32,
    pub c1: i32,
    pub r1: i32,
}

impl GateMark {
    pub fn contains(&self, (c, r): (i32, i32)) -> bool {
        (self.c0..=self.c1).contains(&c) && (self.r0..=self.r1).contains(&r)
    }

    /// The band zone (Han's boost is weak there): the mark grown by [`HAN_BERTH`] x
    /// [`HAN_BERTH_ROWS`].
    pub fn berth(&self) -> GateMark {
        GateMark {
            c0: self.c0 - HAN_BERTH,
            c1: self.c1 + HAN_BERTH,
            r0: self.r0 - HAN_BERTH_ROWS,
            r1: self.r1 + HAN_BERTH_ROWS,
            ..*self
        }
    }
}

/// A line of Han's tied to a grid cell (`say@` / `hint@`).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct Spot {
    pub col: usize,
    pub row: usize,
    pub text: String,
    /// What it teaches (hints only).
    pub topics: Vec<Topic>,
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
    /// A stain raft left by a splat in liquid (spawned at runtime, never in level files).
    Raft,
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
    /// Plain `say:` lines (old form), handed out in order to checkpoints without a `say@`.
    /// Use [`Level::checkpoint_line`].
    pub says: Vec<String>,
    /// `say@col,row:` lines: Han's line at the checkpoint in that cell.
    pub say_at: Vec<Spot>,
    /// `hint@col,row:` hint spots.
    pub hints: Vec<Spot>,
    /// `deaths:` the deaths the design expects (`None`: not declared).
    pub deaths: Option<u32>,
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
    /// `gate:` marks.
    pub gates: Vec<GateMark>,
}

impl Level {
    /// The band (or death) gate mark whose zone holds cell (col, row), if any: there Han's
    /// boost is the weak one.
    pub fn band_zone(&self, cell: (i32, i32)) -> Option<&GateMark> {
        self.gates.iter().find(|g| g.topic.han_keeps_clear() && g.berth().contains(cell))
    }

    /// Is cell (col, row) in a band zone (Han's boost is weak there)?
    pub fn in_band_zone(&self, cell: (i32, i32)) -> bool {
        self.band_zone(cell).is_some()
    }

    /// Is Han's boost full strength in cell (col, row)? Outside every band zone. (The name is
    /// from when he kept out of the zones; he goes anywhere now but [`Level::han_keeps_out`].)
    pub fn han_allowed(&self, cell: (i32, i32)) -> bool {
        !self.in_band_zone(cell)
    }

    /// Cells Han won't stand in: a waltz row's mark (his body would plug the jets for Nat), and
    /// grease in a grease chute's zone (Nat on his head would be jumping off the grease).
    pub fn han_keeps_out(&self, (c, r): (i32, i32)) -> bool {
        let greasy = self.tile(c, r + 1) == Tile::Grease;
        self.gates.iter().any(|g| match g.topic {
            Topic::Waltz => g.contains((c, r)),
            Topic::Grip => greasy && g.berth().contains((c, r)),
            _ => false,
        })
    }

    /// Is column `col` inside a chain-jump chasm mark (no overuse limit there)?
    pub fn in_chasm(&self, col: i32) -> bool {
        self.gates.iter().any(|g| g.topic == Topic::Chain && (g.c0..=g.c1).contains(&col))
    }

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

    /// Han's line for each checkpoint (in checkpoint order): its `say@` line, else the next
    /// unused plain `say:` line, else `None` (a generic quip).
    pub fn checkpoint_lines(&self) -> Vec<Option<&str>> {
        let mut plain = self.says.iter();
        self.checkpoints()
            .map(|c| match self.say_at.iter().find(|s| (s.col, s.row) == (c.col, c.row)) {
                Some(s) => Some(s.text.as_str()),
                None => plain.next().map(String::as_str),
            })
            .collect()
    }

    /// Han's line at checkpoint number `index`, if the level has one.
    pub fn checkpoint_line(&self, index: usize) -> Option<&str> {
        self.checkpoint_lines().get(index).copied().flatten()
    }

    pub fn has_grease(&self) -> bool {
        self.tiles.contains(&Tile::Grease)
    }

    pub fn parse(src: &str) -> Result<Level, String> {
        let (header, grid) = src
            .split_once("\n---\n")
            .ok_or("missing `---` line between header and grid")?;

        let mut name = None;
        let mut world = 1u8;
        let mut intro = String::new();
        let mut says = Vec::new();
        let mut say_at = Vec::new();
        let mut hints = Vec::new();
        let mut deaths = None;
        let mut gates = Vec::new();
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
                "deaths" => {
                    deaths = Some(value.parse().map_err(|_| format!("bad deaths `{value}`"))?);
                }
                "gate" => {
                    let bad = || format!("expected `gate: <topic> <c0>,<r0> <c1>,<r1>`, got `{value}`");
                    let w: Vec<&str> = value.split_whitespace().collect();
                    let [word, a, b] = w[..] else { return Err(bad()) };
                    let topic = Topic::from_word(word)
                        .filter(|t| t.is_gate())
                        .ok_or_else(|| format!("gate: `{word}` is not a gate kind"))?;
                    let cell = |s: &str| -> Option<(i32, i32)> {
                        let (c, r) = s.split_once(',')?;
                        Some((c.trim().parse().ok()?, r.trim().parse().ok()?))
                    };
                    let ((c0, r0), (c1, r1)) = (cell(a).ok_or_else(bad)?, cell(b).ok_or_else(bad)?);
                    gates.push(GateMark { topic, c0: c0.min(c1), r0: r0.min(r1), c1: c0.max(c1), r1: r0.max(r1) });
                }
                k if k.starts_with("say@") || k.starts_with("hint@") => {
                    let (what, rest) = k.split_once('@').unwrap();
                    let mut words = rest.split_whitespace();
                    let at = words.next().unwrap_or("");
                    let (c, r) = at
                        .split_once(',')
                        .and_then(|(c, r)| Some((c.trim().parse().ok()?, r.trim().parse().ok()?)))
                        .ok_or_else(|| format!("expected `{what}@<col>,<row>`, got `{k}`"))?;
                    let topics = words
                        .map(|w| Topic::from_word(w).ok_or_else(|| format!("{what}@{at}: unknown topic `{w}`")))
                        .collect::<Result<Vec<_>, _>>()?;
                    if what == "say" && !topics.is_empty() {
                        return Err(format!("say@{at}: checkpoint lines don't take topics"));
                    }
                    let spot = Spot { col: c, row: r, text: value.to_string(), topics };
                    if what == "say" {
                        say_at.push(spot);
                    } else {
                        hints.push(spot);
                    }
                }
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
                    '_' => *tile = Tile::Grease,
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

        for s in say_at.iter().chain(&hints) {
            if s.col >= width || s.row >= height {
                return Err(format!("line at col {} row {} is outside the grid", s.col, s.row));
            }
        }
        Ok(Level {
            name: name.ok_or("missing `name:`")?,
            world,
            intro,
            says,
            say_at,
            hints,
            deaths,
            width,
            height,
            tiles,
            start: start.ok_or("missing `P` (player start)")?,
            goal: goal.ok_or("missing `G` (goal flag)")?,
            things,
            platforms,
            gates,
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
    fn parses_spots_deaths_and_grease() {
        let src = "name: T\ndeaths: 2\nsay: plain\nsay@4,0: at four\nhint@1,0 toot grip: hi there\n---\n.P..C.C.G\n##__#####\n";
        let l = Level::parse(src).unwrap();
        assert_eq!(l.deaths, Some(2));
        assert_eq!(l.tile(2, 1), Tile::Grease);
        assert!(Tile::Grease.is_solid());
        assert_eq!(l.hints[0].topics, vec![Topic::Toot, Topic::Grip]);
        assert_eq!((l.hints[0].col, l.hints[0].row), (1, 0));
        // say@ goes to its checkpoint; the plain line to the other one.
        assert_eq!(l.checkpoint_lines(), vec![Some("at four"), Some("plain")]);
        assert!(Level::parse("name: T\nhint@1,0 bogus: x\n---\nPG\n##\n").is_err());
        assert!(Level::parse("name: T\nhint@9,9: x\n---\nPG\n##\n").is_err());
    }

    #[test]
    fn parses_gate_marks() {
        let src = "name: T\ngate: giant 20,3 5,9\ngate: chain 30,1 40,9\n---\nPG\n##\n";
        let l = Level::parse(src).unwrap();
        assert_eq!(l.gates[0], GateMark { topic: Topic::Giant, c0: 5, r0: 3, c1: 20, r1: 9 });
        assert!(l.in_band_zone((5 - HAN_BERTH, 3)) && !l.han_allowed((5 - HAN_BERTH, 3)));
        assert!(!l.in_band_zone((5 - HAN_BERTH - 1, 3)));
        assert_eq!(l.band_zone((10, 4)).map(|g| g.topic), Some(Topic::Giant));
        assert!(l.han_allowed((35, 5)), "full boosts in chasms");
        assert!(!l.han_keeps_out((10, 4)), "Han goes near giant walls");
        assert!(l.in_chasm(35) && !l.in_chasm(41));
        assert!(Level::parse("name: T\ngate: toot 1,1 2,2\n---\nPG\n##\n").is_err());
        assert!(Level::parse("name: T\ngate: giant 1,1\n---\nPG\n##\n").is_err());
    }

    #[test]
    fn all_levels_parse() {
        let levels = Levels::default();
        assert_eq!(levels.0.len(), LEVEL_COUNT);
    }
}

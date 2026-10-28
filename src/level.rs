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
//! say@13,1: Two-ply!
//! hint@4,1 toot: Toot!
//! 1: dx=6 dy=0 period=4 kind=tp
//! ---
//! ....................
//! .P......o.o..C.11...
//! ####===######.....G#
//! ```
//!
//! Header keys (every line `key: value`; blank lines and `// comments` are skipped):
//! - `name`, `world` (1..=5: tileset, backdrop, music), `intro` (Han's line at the start).
//! - `say@<col>,<row>: text`: Han's bark when Nat reaches the checkpoint in that cell.
//!   (Old form, still accepted: plain `say: text` lines go, in order, to the checkpoints that
//!   have no `say@`; checkpoints are numbered in reading order, top row first.)
//! - `hint@<col>,<row> topic...: text`: a *hint spot*. The first time Nat comes within
//!   [`HINT_RADIUS`] of that cell (once per level visit; dying doesn't repeat it, a restart
//!   does), Han barks the line and the lesson card of each [`Topic`] word comes up, once per
//!   save (`ui::lesson`). The validator ([`validate`]) checks that every mechanic is taught by a
//!   hint before the player first meets it, and that every hint names a topic.
//! - `solo: nat`: Nat plays it alone (no Han: no barks, boosts or tricks).
//! - `secret@<col>,<row>: text` (NAT ALONE): a *secret*, Han glimpsed through a grate. The cell
//!   and the one above it must be solid (the grate is a wall): Han is drawn standing behind
//!   it, feet on the cell's floor. The first time in a level visit Nat comes within
//!   [`SECRET_RADIUS`] of it, Han barks the line and the save remembers it was found
//!   (`game::secret`). [`validate`] checks Nat can get that close.
//! - `shout: <col>`: from that column on, the band plays shout choruses (the closing stretch);
//!   `shout: row <row>` (a tower): once above that row.
//! - `deaths: N`: the deaths the level's design *expects* (splat-stain stepping stones, the
//!   three splats that make the band nervous for a grease chute). [`validate`] checks it; the
//!   adaptive engine can read it.
//! - `N: ...` (a digit): a moving platform, see below.
//! - `crusher@<col>,<row>: phase=<0..1> beat=<bars>` (either or both): shift the cycle of the
//!   crusher covering that cell (so neighbours don't slam in lockstep); with `beat=N` it slams
//!   on the music's ONE every N bars instead of on its own clock. See "Crushers" below.
//! - `gate: <topic> <c0>,<r0> <c1>,<r1>`: a *gate mark*, the cells (inclusive rectangle) of a
//!   gate the level is built around, named by its [`Topic`] word: `giant` (giant wall), `gap`
//!   (long gap), `waltz` (glide launch), `grip` (grease chute), `stain` (stain pit), `mega`
//!   (mega wall), `shaft` (cling shaft), `trapeze` (trapeze), `chain` (fired-up chasm), `buddyraft` (buddy raft
//!   pool). See "Gate marks" below.
//!
//! Lines (intro, say, hint) are at most [`MAX_LINE`] characters; all but the intro are barks of
//! at most [`MAX_BARK_WORDS`] words (Han is nearly silent: cards teach).
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
//! | `S` | air-freshener spray: a can on the floor firing a jet straight up 3 tiles; the can and its jet are deadly while on (harmless while off). All cans share one on/off clock (while the band waltzes: the music's). 4+ adjacent cans are too many to time: the validator counts them deadly |
//! | `1`-`9` | moving platform: a horizontal run of the same digit is one platform, configured by its `N:` header line |
//! | `%` | breakable brick: solid, but rising into it from below (a jump, a toot, a boost) smashes it and carries on up. Gone for the level visit (back on a restart) |
//! | `-` | crumbling platform: one-way like `=`; [`CRUMBLE_DELAY`] s after someone stands on it, it falls, and comes back [`CRUMBLE_BACK`] s later |
//! | `T` | trampoline: a solid block; landing on its top springs you up ([`SPRING_SPEED`], ~6.5 tiles; the toot comes back) |
//! | `{` `}` | conveyor belt: solid; standing on it carries you left (`{`) or right (`}`) at [`BELT_SPEED`], and jumping or stepping off keeps its speed |
//! | `*` | fan: a solid grate; its updraft fills the empty tiles above it ([`FAN_REACH`] at most) and floats anyone in it up ([`FAN_RISE`]) to bob at the top |
//! | `(` `)` | bumper (a rubber duck): solid; running or flying into its springy face (`(`: the left face, `)`: the right) bats you back the other way and up ([`BUMPER_SPEED`], [`BUMPER_LIFT`]) |
//! | `[` `]` | beat block: red (`[`) is solid on even bars of the music and gone on odd ones, blue (`]`) the other way round. One that comes back around someone waits until they're clear |
//! | `!` `e` `w` | fans laid on a ceiling (`!`, blowing down: a downdraft you can't rise through) or a wall (`e` blowing right, `w` left: a gust that pushes you along at [`WIND_SPEED`]); like `*`, [`FAN_REACH`] empty tiles |
//! | `b` `d` `m` | cushions laid on a wall (`b`: its spring on its right face, `d`: its left; run or fly into it and it catches you, holds you [`CUSHION_HOLD`] (in the waltz, until ONE: [`WALTZ_SPRING_BOOST`] harder) and slings you away; hold into it as it fires for a steep shot, away for a long flat one, neither for the plain one ([`cushion_shot`]); a jump fires it early) or a ceiling (`m`: bump your head on it and it springs you back down); the toot comes back |
//! | `\|` | brushes sticking out of a wall sideways, pointing away from it (there must be a wall on exactly one side); deadly in that half of the tile |
//! | `B` | the Boom: a bean-fuelled flying pal, parked bobbing in its cell. Touch it and it catches Nat and flies wherever you steer (all four arrows; Jump hops off). Anything deadly it touches splats Nat. Its tank holds [`BOOM_FUEL`] s; when it runs dry it sputters and drops him. See `game::boom` |
//! | `:` | beans: the Boom's fuel. Flying through them (on the Boom) tops the tank up [`BEAN_FUEL`]; Nat on foot leaves them be. Eaten beans are back the next time the Boom parks |
//! | `D` | sploosh diaper: flaps back and forth [`DIAPER_RANGE`] tiles either side of its cell, and drops (with a whistle) when it would land on Nat's head if he kept on: stop or turn and it misses; deadly while it flies and falls, then splooshes and flaps back [`DIAPER_BACK`] s later. See `game::diapers` |
//! | `H` | the "helper": a shifty guy on a ledge, his rope dangling off it. Grab the rope and he hauls Nat up, swinging it about, and he lets go when he likes (a snicker first): his tricks go drop, fling, honest, then round again. Jump lets go of it, keeping the swing. See `game::helper` |
//! | `M` | the blast button, with its speakers: land on it and the band BLASTS (a shout chorus a key up, flat out, louder, [`BLAST_SECS`]) and the blast throws Nat up ([`BLAST_SPEED`]) and every diaper near out of the sky. See `game::toys` |
//! | `@` | a stop sign: touch it and the level stops right there (cleared, like the goal). See `game::toys` |
//! | `A` `K` `a` | a house (its door in this cell, the house around it): Up at the door goes in. `A`: nobody's in, out the chimney Nat flies, up high; `K`: a chef's home (smoke, a lit window) and Nat comes out a schnitzel (a death); `a`: the chef's in every other [`HOUSE_SHIFT`] s. See `game::houses` |
//! | `X` `<` `>` | crusher: a rectangle of the same char is one block that slams down (`X`), left (`<`) or right (`>`) until it meets a tile, holds, and creeps back ([`Crusher`]). Its slamming face squashes; it's solid, and rideable on top |
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
//!   `grip`, `stain`, `mega`, `shaft` or `trapeze` mark ([`Level::in_band_zone`]), jumping off his head is the weak boost
//!   (`game::WEAK_BOOST_SPEED`, about a normal jump) and he grumbles; there his head only
//!   holds Nat while he's standing on the ground. He follows normally, but keeps off the
//!   grease in a grease chute's zone ([`Level::han_keeps_out`]).
//!   So no boost, and no chain of boosts, ever opens them: they always need their mode (the
//!   validator proves it: full boosts from everywhere outside the zones Han may be, weak ones
//!   from everywhere inside them Nat may get to).
//! - **Mega walls** (`mega`): too tall for Giant Steps alone and for Han's plain boost. In a
//!   mega wall's zone, while Giant Steps plays, his boost is full strength: the floaty
//!   *mega-boost* (Giant Steps' low gravity under the plunger boost) and a toot clear it
//!   ([`Level::weak_boost`]).
//! - **Cling shafts** (`shaft`): a shaft too tall for any mode and any boost from its floor.
//!   With the nervous band's grip Han climbs a wall of it and clings there
//!   ([`CLING_ROWS`] up), Nat hops onto his head and kicks off it: a full boost, away from the
//!   wall. In a cling shaft's zone his boost is weak without the grip.
//! - **Trapezes** (`trapeze`): a gap out of every mode's reach (its far side higher, spikes on
//!   its first tiles) under a ceiling that ends over it. In the waltz Han sticks his plunger in
//!   the ceiling's edge and swings (`buddy::trapeze_swing`), through the bottom on ONE; Nat
//!   rides his cap, and a jump off it is a full boost plus the swing's speed. In a trapeze's
//!   zone his boost is weak out of the waltz.
//! - **Fired-up chasms** (`chain`) have no overuse limit: Han throws you as often as it takes
//!   while you're in the mark's columns ([`Level::in_chasm`]), and only there does his head
//!   catch you in mid-air (fired up) to throw you again.
//! - **Markers** are drawn from them: giant walls get gold music-staff trim and a note
//!   emblem, glide launches a 3/4 emblem in the ground at each end, mega walls red
//!   plunger-handle notches and yellow plumber's tape.
//!
//! # Crushers
//! A crusher ([`CrusherDef`]) rests at home [`CRUSH_WAIT`] s, slams [`CRUSH_SLAM_SPEED`] px/s
//! towards its stop (the first tile in its way, at most [`CRUSH_MAX_TRAVEL`] tiles), holds
//! there [`CRUSH_HOLD`] s and creeps home at [`CRUSH_RISE_SPEED`] (`phase` shifts its cycle).
//! Everyone in its way when it slams gets squashed: Nat dies, Han goes flat (on Side B that's
//! his end). Its block is solid at every point of its cycle, and carries whoever stands on it.
//! The validator treats its home as solid and its sweep like a spray can's jet: a timed
//! hazard you can run through, unless it's too big to time ([`Crusher::timeable`]).
//! A *beat crusher* (`beat=N`) slams on the ONE every N bars of the music instead; when the
//! band plays fast it holds less and creeps back faster, so it's home at least
//! [`CRUSH_BEAT_REST`] of its cycle (the validator assumes [`NOMINAL_BAR`]s).
//!
//! Rows may be ragged; short rows are padded with empty. Outside the grid: left/right is a solid
//! wall, above is a solid ceiling (the top of the screen: the camera never shows above row 0, so
//! nobody may go there, or a wall reaching row 0 could be walked over on its roof, unseen),
//! below is a bottomless pit (death).
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
/// Nat finds a `secret@` when he comes this close (px) to the grate's center.
pub const SECRET_RADIUS: f32 = 1.5 * TILE;
/// Longest line Han says (intro, checkpoint lines, hints).
pub const MAX_LINE: usize = 60;
/// Han nearly never talks: everything he says but a level's intro is a bark of at most this
/// many words (lesson cards do the teaching).
pub const MAX_BARK_WORDS: usize = 3;

/// A line short enough for Han: at most [`MAX_BARK_WORDS`] words.
pub fn is_bark(line: &str) -> bool {
    line.split_whitespace().count() <= MAX_BARK_WORDS
}
pub const LEVEL_COUNT: usize = 10;
/// A crumbling platform (`-`) falls this long (s) after someone stands on it...
pub const CRUMBLE_DELAY: f32 = 0.5;
/// ...and comes back this long (s) after it fell (once nobody's in its way).
pub const CRUMBLE_BACK: f32 = 3.0;
/// Upward speed (px/s) a trampoline (`T`) springs you up at: 540²/2800 ≈ 104 px ≈ 6.5 tiles
/// in normal gravity, and the toot comes back on top.
pub const SPRING_SPEED: f32 = 540.0;
/// A crusher rests at home this long (s)...
pub const CRUSH_WAIT: f32 = 1.6;
/// ...slams at this speed (px/s)...
pub const CRUSH_SLAM_SPEED: f32 = 480.0;
/// ...holds at its stop this long (s)...
pub const CRUSH_HOLD: f32 = 0.6;
/// ...and creeps home at this speed (px/s).
pub const CRUSH_RISE_SPEED: f32 = 48.0;
/// A beat crusher (`crusher@...: beat=N`) is home with at least this share of its cycle left
/// to rest: it creeps back faster when the band plays fast.
pub const CRUSH_BEAT_REST: f32 = 0.4;
/// The bar the validator and free play assume for a beat crusher (s): 4 beats at 120 bpm.
pub const NOMINAL_BAR: f32 = 2.0;
/// A conveyor belt (`{` `}`) moves whoever stands on it this fast (px/s): 2.5 tiles/s. Jumping or
/// stepping off keeps its speed, like a moving platform's.
pub const BELT_SPEED: f32 = 40.0;
/// A fan (`*`) blows straight up through this many empty tiles above it...
pub const FAN_REACH: i32 = 6;
/// ...twice that while the band plays Giant Steps (every fan, laid any way: `game::TileMood`).
pub const GIANT_FAN_REACH: i32 = 2 * FAN_REACH;
/// While the band plays Giant Steps a ceiling cushion (`m`) bats you back down this much
/// slower: a soft paddle, so you can toot back up into it and rally along under it.
pub const GIANT_CUSHION_BAT: f32 = 0.1;
/// While the band's fired up, conveyor belts and wall fans' gusts run this much faster.
pub const FIRED_UP_TILES: f32 = 1.5;
/// In the waltz a trampoline catches whoever lands on it and springs them on the next ONE this
/// much faster (≈ ×2 the height); a cushion or a bumper hit on ONE flings this much faster too.
pub const WALTZ_SPRING_BOOST: f32 = 1.4;
/// ...lifting whoever's in its column (px/s², on top of gravity)...
pub const FAN_LIFT: f32 = 2600.0;
/// ...up to this rising speed (px/s): you float up the column and bob at its top.
pub const FAN_RISE: f32 = 200.0;
/// A bumper (`(` `)`, a sideways spring) bats whoever runs or flies into its face back this
/// fast (px/s, kept until they land or hit a wall, like a platform's launch)...
pub const BUMPER_SPEED: f32 = 240.0;
/// ...and this fast up (px/s, at least).
pub const BUMPER_LIFT: f32 = 300.0;

/// A wall fan (`e` `w`) pushes whoever's in its gust along this fast (px/s, on top of their
/// own speed, like a belt's): 1.5 tiles a second short of a run against it.
pub const WIND_SPEED: f32 = 110.0;
/// A wall cushion (`b` `d`) catches whoever runs or flies into its face: they sink into it and
/// it holds them this long (s; in the waltz, until the next ONE), then slings them away (a
/// jump fires it early)...
pub const CUSHION_HOLD: f32 = 0.2;
/// ...this fast (px/s, kept until they land or hit a wall, like a bumper's), with nothing held
/// as it fires...
pub const CUSHION_FLING: f32 = 420.0;
/// ...and this fast up (px/s).
pub const CUSHION_LIFT: f32 = 260.0;
/// Held into the cushion as it fires (pulled back, a slingshot): steep, (away, up) px/s.
pub const CUSHION_STEEP: (f32, f32) = (150.0, 520.0);
/// Held away from it: long and flat, (away, up) px/s.
pub const CUSHION_LONG: (f32, f32) = (600.0, 220.0);

/// A wall cushion's shot (away from its face, up; px/s) for the aim held as it fires: `aim`
/// > 0 away from the cushion ([`CUSHION_LONG`]), < 0 into it ([`CUSHION_STEEP`]), 0 neither
/// ([`CUSHION_FLING`], [`CUSHION_LIFT`]).
pub fn cushion_shot(aim: f32) -> (f32, f32) {
    if aim > 0.0 {
        CUSHION_LONG
    } else if aim < 0.0 {
        CUSHION_STEEP
    } else {
        (CUSHION_FLING, CUSHION_LIFT)
    }
}

/// The Boom's full tank (s of flying).
pub const BOOM_FUEL: f32 = 8.0;
/// A bean tops the Boom's tank up this much (s, never past full).
pub const BEAN_FUEL: f32 = 2.5;
/// The Boom flies this fast (px/s, its top speed in any direction)...
pub const BOOM_SPEED: f32 = 120.0;
/// ...and this much slower once the tank's nearly dry ([`BOOM_LOW`] s left): it sputters.
pub const BOOM_SPUTTER: f32 = 0.6;
pub const BOOM_LOW: f32 = 2.0;
/// A diaper flaps this many tiles either side of its cell...
pub const DIAPER_RANGE: f32 = 3.0;
/// ...at this speed (px/s, at its fastest)...
pub const DIAPER_SPEED: f32 = 50.0;
/// ...and is back flapping this long (s) after it splooshed.
pub const DIAPER_BACK: f32 = 2.5;

/// The helper hauls his rope in this fast (px/s)...
pub const HELPER_HAUL: f32 = 40.0;
/// ...from at most this long a rope (tiles; it hangs to just above the floor below him).
pub const HELPER_ROPE: f32 = 8.0;
/// The blast button throws Nat up this fast (px/s, ~10 tiles)...
pub const BLAST_SPEED: f32 = 700.0;
/// ...and the band blasts this long (s).
pub const BLAST_SECS: f32 = 12.0;
/// A sometimes house (`a`): the chef's in, then out, this long each (s).
pub const HOUSE_SHIFT: f32 = 3.0;
/// A house nobody's in throws Nat out of its chimney this fast (px/s, ~8 tiles up).
pub const CHIMNEY_SPEED: f32 = 620.0;

/// The farthest (tiles) a crusher slams, with nothing in its way.
pub const CRUSH_MAX_TRAVEL: i32 = 10;
/// The validator times its way past a crusher whose face is at most this many tiles across...
pub const CRUSH_MAX_FACE: i32 = 3;
/// ...and (one slamming sideways, whose lane you run along) that slams at most this far.
pub const CRUSH_MAX_LANE: i32 = 6;

/// How many rows above the floor Han's feet cling in a cling shaft.
pub const CLING_ROWS: i32 = 3;

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
    /// Breakable brick (`%`): solid, smashed by rising into it from below.
    Brick,
    /// Crumbling platform (`-`): one-way; falls a moment after someone stands on it.
    Crumble,
    /// Trampoline (`T`): a solid block that springs whoever lands on top up high.
    Trampoline,
    /// Conveyor belt (`{`: moving left, `}`: right): solid, carries whoever stands on it at
    /// [`BELT_SPEED`].
    BeltLeft,
    BeltRight,
    /// Fan (`*`): a solid grate blowing up its column ([`FAN_REACH`] empty tiles above it).
    Fan,
    /// Bumper: a solid block with a spring on one face (`(`: its left face, `)`: its right).
    /// Running or flying into that face bats you back ([`BUMPER_SPEED`], [`BUMPER_LIFT`]).
    BumperLeft,
    BumperRight,
    /// Beat blocks: red (`[`) are solid on even bars of the music and gone on odd ones, blue
    /// (`]`) the other way round. The `Off` variants are the gone ones (only made at runtime,
    /// but `]` starts as one: bar 0 is even).
    BeatRed,
    BeatRedOff,
    BeatBlue,
    BeatBlueOff,
    /// Fans laid sideways or upside down: a ceiling fan (`!`) blowing down its column, wall fans
    /// blowing right (`e`, east) and left (`w`, west) along their row ([`FAN_REACH`] empty
    /// tiles, like `*`). See [`Level::wind`].
    FanDown,
    FanRight,
    FanLeft,
    /// Cushions laid sideways or upside down: one on a wall (`b`: its spring on its right
    /// face, `d`: its left) flings whoever runs or flies into it away ([`CUSHION_FLING`]); one
    /// on a ceiling (`m`) springs a head bumped into it back down ([`SPRING_SPEED`]). Both give
    /// the toot back, like `T`.
    CushionRight,
    CushionLeft,
    CushionDown,
    /// Brushes sticking out of a wall sideways (`|`), pointing away from the wall beside them
    /// (right: the wall's on their left), deadly in that half of the tile.
    SpikesRight,
    SpikesLeft,
}

impl Tile {
    pub fn is_deadly(self) -> bool {
        matches!(self, Tile::SpikesUp | Tile::SpikesDown | Tile::Liquid | Tile::SpikesRight | Tile::SpikesLeft)
    }

    /// Blocks from every side (ground, grease, bricks, trampolines). (A brick gives way to
    /// anyone rising into it: `game::step_body`.)
    pub fn is_solid(self) -> bool {
        matches!(
            self,
            Tile::Solid
                | Tile::Grease
                | Tile::Brick
                | Tile::Trampoline
                | Tile::BeltLeft
                | Tile::BeltRight
                | Tile::Fan
                | Tile::BumperLeft
                | Tile::BumperRight
                | Tile::BeatRed
                | Tile::BeatBlue
                | Tile::FanDown
                | Tile::FanRight
                | Tile::FanLeft
                | Tile::CushionRight
                | Tile::CushionLeft
                | Tile::CushionDown
        )
    }

    /// A conveyor belt's speed (px/s, negative: left), 0 for anything else.
    pub fn belt(self) -> f32 {
        match self {
            Tile::BeltLeft => -BELT_SPEED,
            Tile::BeltRight => BELT_SPEED,
            _ => 0.0,
        }
    }

    /// A bumper's springy face: -1 its left face, 1 its right, 0 not a bumper.
    pub fn bumper(self) -> f32 {
        match self {
            Tile::BumperLeft => -1.0,
            Tile::BumperRight => 1.0,
            _ => 0.0,
        }
    }

    /// A wall cushion's springy face: -1 its left face, 1 its right, 0 not one.
    pub fn fling(self) -> f32 {
        match self {
            Tile::CushionLeft => -1.0,
            Tile::CushionRight => 1.0,
            _ => 0.0,
        }
    }

    /// Any fan, the way it blows: (x, y), y up.
    pub fn fan(self) -> Option<(i32, i32)> {
        match self {
            Tile::Fan => Some((0, 1)),
            Tile::FanDown => Some((0, -1)),
            Tile::FanRight => Some((1, 0)),
            Tile::FanLeft => Some((-1, 0)),
            _ => None,
        }
    }

    /// A beat block, on or off.
    pub fn is_beat(self) -> bool {
        matches!(self, Tile::BeatRed | Tile::BeatRedOff | Tile::BeatBlue | Tile::BeatBlueOff)
    }

    /// A beat block as it is on a bar of this parity (`odd`); anything else as it is.
    pub fn on_bar(self, odd: bool) -> Tile {
        match self {
            Tile::BeatRed | Tile::BeatRedOff => {
                if odd {
                    Tile::BeatRedOff
                } else {
                    Tile::BeatRed
                }
            }
            Tile::BeatBlue | Tile::BeatBlueOff => {
                if odd {
                    Tile::BeatBlue
                } else {
                    Tile::BeatBlueOff
                }
            }
            t => t,
        }
    }

    /// Solid on top only: you jump up through it and stand on it (`=`, stains, crumbling
    /// platforms).
    pub fn is_one_way(self) -> bool {
        matches!(self, Tile::OneWay | Tile::StainUp | Tile::StainDown | Tile::Crumble)
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
    /// Glide launches and the waltzing band (`waltz`).
    Waltz,
    /// Stain pits: splat to build stepping stones (`stain`).
    Stain,
    /// Grease (`grease`).
    Grease,
    /// Sweaty grip: the nervous band lets Nat brake on grease (`grip`).
    Grip,
    /// Mega walls: Giant Steps and Han's mega-boost off his head, a toot (`mega`).
    Mega,
    /// Cling shafts: the nervous band's grip, Han clinging to the wall up the shaft, a kick
    /// off his head (`shaft`).
    Shaft,
    /// Trapezes: the waltz, Han swinging from his plunger stuck in the ceiling, a launch off
    /// his head on ONE (`trapeze`).
    Trapeze,
    /// Fired-up chasms: the band fired up, Han's running throw off his head at the edge, a toot
    /// (`chain`).
    Chain,
    /// Buddy raft pools: Han wades in and leaves a big raft (`buddyraft`).
    BuddyRaft,
    /// Breakable bricks `%` (`brick`).
    Brick,
    /// Crumbling platforms `-` (`crumble`).
    Crumble,
    /// Trampolines `T` (`spring`).
    Spring,
    /// Crushers `X` `<` `>` (`crusher`).
    Crusher,
    /// Conveyor belts `{` `}` (`belt`).
    Belt,
    /// Fans `*` (`fan`).
    Fan,
    /// Bumpers `(` `)` (`bumper`).
    Bumper,
    /// Beat blocks `[` `]` (`beat`).
    Beat,
    /// Fans laid sideways or upside down `!` `e` `w` (`wind`).
    Wind,
    /// Cushions laid sideways or upside down `b` `d` `m` (`fling`).
    Fling,
    /// The Boom `B` and its beans `:` (`boom`).
    Boom,
    /// Sploosh diapers `D` (`diaper`).
    Diaper,
    /// The helper `H` and his rope (`helper`).
    Helper,
    /// The blast button `M` (`blast`).
    Blast,
    /// Stop signs `@` (`stop`).
    Stop,
    /// Houses `A` `K` `a` (`house`).
    House,
}

impl Topic {
    pub const ALL: [Topic; 32] = [
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
        Topic::Mega,
        Topic::Shaft,
        Topic::Trapeze,
        Topic::Chain,
        Topic::BuddyRaft,
        Topic::Brick,
        Topic::Crumble,
        Topic::Spring,
        Topic::Crusher,
        Topic::Belt,
        Topic::Fan,
        Topic::Bumper,
        Topic::Beat,
        Topic::Wind,
        Topic::Fling,
        Topic::Boom,
        Topic::Diaper,
        Topic::Helper,
        Topic::Blast,
        Topic::Stop,
        Topic::House,
    ];

    /// The gadget tiles: not in the campaign yet (only `levels/playground/gadgets.txt`), so the
    /// campaign doesn't owe them a lesson until a level uses one.
    pub const GADGETS: [Topic; 10] = [
        Topic::Brick,
        Topic::Crumble,
        Topic::Spring,
        Topic::Crusher,
        Topic::Belt,
        Topic::Fan,
        Topic::Bumper,
        Topic::Beat,
        Topic::Wind,
        Topic::Fling,
    ];

    /// The toys (the Boom and its beans, sploosh diapers, the helper, the blast button, stop
    /// signs, houses): only in the secret playground (`game::toybox`, `levels/toybox/`), since
    /// the validator can't fly the Boom, swing on a rope or go indoors.
    pub const TOYS: [Topic; 6] = [Topic::Boom, Topic::Diaper, Topic::Helper, Topic::Blast, Topic::Stop, Topic::House];

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
            Topic::Mega => "mega",
            Topic::Shaft => "shaft",
            Topic::Trapeze => "trapeze",
            Topic::Chain => "chain",
            Topic::BuddyRaft => "buddyraft",
            Topic::Brick => "brick",
            Topic::Crumble => "crumble",
            Topic::Spring => "spring",
            Topic::Crusher => "crusher",
            Topic::Belt => "belt",
            Topic::Fan => "fan",
            Topic::Bumper => "bumper",
            Topic::Beat => "beat",
            Topic::Wind => "wind",
            Topic::Fling => "fling",
            Topic::Boom => "boom",
            Topic::Diaper => "diaper",
            Topic::Helper => "helper",
            Topic::Blast => "blast",
            Topic::Stop => "stop",
            Topic::House => "house",
        }
    }

    /// Its place in [`Topic::ALL`].
    pub fn index(self) -> usize {
        Topic::ALL.iter().position(|&t| t == self).expect("every topic is in ALL")
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
                | Topic::Mega
                | Topic::Shaft
                | Topic::Trapeze
                | Topic::Chain
                | Topic::BuddyRaft
        )
    }

    /// The band's gates, the death gates, mega walls, cling shafts and trapezes: Han's boost is
    /// weak in their zone (except where the band [`Granted`] his trick there:
    /// [`Level::weak_boost`]).
    pub fn han_keeps_clear(self) -> bool {
        matches!(
            self,
            Topic::Giant
                | Topic::Gap
                | Topic::Waltz
                | Topic::Grip
                | Topic::Stain
                | Topic::Mega
                | Topic::Shaft
                | Topic::Trapeze
        )
    }
}

/// What the band is playing that grants Han a trick in its gate's zone: Giant Steps (a mega
/// wall's mega-boost), the nervous band's grip (a cling shaft's cling), the waltz (a trapeze).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Granted {
    pub giant_steps: bool,
    pub grip: bool,
    pub waltz: bool,
}

impl Granted {
    pub const NONE: Granted = Granted { giant_steps: false, grip: false, waltz: false };

    /// Is Han's trick at a `topic` gate granted?
    pub fn grants(self, topic: Topic) -> bool {
        match topic {
            Topic::Mega => self.giant_steps,
            Topic::Shaft => self.grip,
            Topic::Trapeze => self.waltz,
            _ => false,
        }
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

    /// Does this mark make Han's boost weak in `cell` (see [`Level::weak_boost`])?
    pub fn weakens(&self, cell: (i32, i32), granted: Granted) -> bool {
        self.topic.han_keeps_clear() && !granted.grants(self.topic) && self.berth().contains(cell)
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

/// Which way a crusher slams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum CrushDir {
    /// `X`
    Down,
    /// `<`
    Left,
    /// `>`
    Right,
}

impl CrushDir {
    /// Its grid char.
    pub fn char(self) -> char {
        match self {
            CrushDir::Down => 'X',
            CrushDir::Left => '<',
            CrushDir::Right => '>',
        }
    }

    /// One tile of travel, in grid steps (col, row; row down).
    pub fn step(self) -> (i32, i32) {
        match self {
            CrushDir::Down => (0, 1),
            CrushDir::Left => (-1, 0),
            CrushDir::Right => (1, 0),
        }
    }
}

/// A crusher (`X`, `<`, `>`): its block at home is the cells `col..col+w` × `row..row+h`; it
/// slams `travel` tiles `dir`. Its timing is the `CRUSH_*` constants (see "Crushers" above).
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct CrusherDef {
    pub col: usize,
    pub row: usize,
    pub w: usize,
    pub h: usize,
    pub dir: CrushDir,
    /// Tiles from home to its stop.
    pub travel: i32,
    /// Starting point in its cycle, 0..1 (0: the start of its rest at home; a beat crusher's:
    /// the share of its bars it's late by).
    pub phase: f32,
    /// A beat crusher (`beat=N`): it slams on the ONE every `N` bars of the music. 0: it runs
    /// on the sim clock.
    pub beat: u8,
}

/// When it is for a crusher: the sim clock (s) and where the music is.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CrushTime {
    pub sim: f32,
    /// Bars since the music's bar 0 (fractional).
    pub bars: f32,
    /// Seconds per bar.
    pub bar_secs: f32,
}

impl CrushTime {
    /// Sim time `t` with the band playing [`NOMINAL_BAR`]s from the start (the validator's and
    /// headless tests' music).
    pub fn sim(t: f32) -> CrushTime {
        CrushTime { sim: t, bars: t / NOMINAL_BAR, bar_secs: NOMINAL_BAR }
    }
}

/// A crusher's cycle (pure, shared by the game, Han and the validator).
pub struct Crusher;

impl Crusher {
    /// Seconds per cycle for a (sim clock) crusher slamming `travel` tiles.
    pub fn period(travel: i32) -> f32 {
        let d = travel as f32 * TILE;
        CRUSH_WAIT + d / CRUSH_SLAM_SPEED + CRUSH_HOLD + d / CRUSH_RISE_SPEED
    }

    /// Where `def` is in its cycle at `at`: seconds since its slam started, and its slam, hold
    /// and rise times and the whole cycle (s). (A sim-clock crusher's cycle starts with its
    /// rest; that's counted at the end here, like a beat crusher's.)
    fn cycle(def: &CrusherDef, at: CrushTime) -> (f32, [f32; 3], f32) {
        let d = def.travel as f32 * TILE;
        let slam = d / CRUSH_SLAM_SPEED;
        if def.beat == 0 {
            let period = Crusher::period(def.travel);
            let u = (at.sim + def.phase * period - CRUSH_WAIT).rem_euclid(period);
            return (u, [slam, CRUSH_HOLD, d / CRUSH_RISE_SPEED], period);
        }
        let n = def.beat as f32;
        let bar = at.bar_secs.max(0.1);
        let cycle = n * bar;
        let u = (at.bars - def.phase * n).rem_euclid(n) * bar;
        // Fast music: hold less and creep back faster, so it's home with time to get past.
        let busy = cycle * (1.0 - CRUSH_BEAT_REST);
        let hold = CRUSH_HOLD.min((busy - slam) * 0.4).max(0.0);
        let rise = (d / CRUSH_RISE_SPEED).min(busy - slam - hold).max(0.05);
        (u, [slam, hold, rise], cycle)
    }

    /// How far (px) from home `def` is at `at`, and whether it's slamming then.
    pub fn offset(def: &CrusherDef, at: CrushTime) -> (f32, bool) {
        let d = def.travel as f32 * TILE;
        let (mut u, [slam, hold, rise], _) = Crusher::cycle(def, at);
        if u < slam {
            return (d * u / slam.max(1e-3), true);
        }
        u -= slam;
        if u < hold {
            return (d, false);
        }
        u -= hold;
        if u < rise {
            return (d * (1.0 - u / rise), false);
        }
        (0.0, false)
    }

    /// Seconds left of its rest at home at `at` (`None`: it's not resting).
    pub fn rest_left(def: &CrusherDef, at: CrushTime) -> Option<f32> {
        let (u, [slam, hold, rise], cycle) = Crusher::cycle(def, at);
        (u >= slam + hold + rise).then(|| cycle - u)
    }

    /// Can a player time their way past it (the validator's call, like a lone spray can's)?
    /// Its face at most [`CRUSH_MAX_FACE`] tiles across; slamming sideways, at most
    /// [`CRUSH_MAX_LANE`] tiles far.
    pub fn timeable(def: &CrusherDef) -> bool {
        match def.dir {
            CrushDir::Down => def.w as i32 <= CRUSH_MAX_FACE,
            CrushDir::Left | CrushDir::Right => def.h as i32 <= CRUSH_MAX_FACE && def.travel <= CRUSH_MAX_LANE,
        }
    }

    /// Every cell its block covers somewhere in its cycle, home included.
    pub fn sweep(def: &CrusherDef) -> impl Iterator<Item = (i32, i32)> + '_ {
        let (dc, dr) = def.dir.step();
        (0..=def.travel).flat_map(move |k| {
            (0..def.h as i32).flat_map(move |r| {
                (0..def.w as i32).map(move |c| (def.col as i32 + c + dc * k, def.row as i32 + r + dr * k))
            })
        })
    }

    /// The cells of its block at home.
    pub fn home(def: &CrusherDef) -> impl Iterator<Item = (i32, i32)> + '_ {
        (0..def.h as i32).flat_map(move |r| (0..def.w as i32).map(move |c| (def.col as i32 + c, def.row as i32 + r)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum ThingKind {
    Nugget,
    Checkpoint,
    Fly,
    Spray,
    /// The Boom (`B`), parked.
    Boom,
    /// Beans (`:`), the Boom's fuel.
    Bean,
    /// A sploosh diaper (`D`).
    Diaper,
    /// The helper (`H`), on his ledge.
    Helper,
    /// The blast button (`M`).
    Blaster,
    /// A stop sign (`@`).
    StopSign,
    /// A house's door (`A` `K` `a`).
    House(Cook),
}

/// Who's in a house (`A` `K` `a`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum Cook {
    /// `A`: nobody: out the chimney.
    Never,
    /// `K`: a chef, always.
    Always,
    /// `a`: a chef, every other [`HOUSE_SHIFT`] s (out first).
    Sometimes,
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
    /// Crushers (`X`, `<`, `>`).
    pub crushers: Vec<CrusherDef>,
    /// `gate:` marks.
    pub gates: Vec<GateMark>,
    /// `solo: nat`: Nat alone, no Han in the level.
    pub solo: bool,
    /// `secret@col,row:` Han behind a grate (its bark in `text`).
    pub secret: Option<Spot>,
    /// `shout:` where the shout choruses start.
    pub shout: Option<Shout>,
}

/// Where a level's closing stretch starts (`shout:`): past a column, or (a tower) above a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum Shout {
    Col(usize),
    Row(usize),
}

impl Shout {
    /// Is a body at `pos` (world px) in the closing stretch of a level `height` rows tall?
    pub fn reached(self, pos: Vec2, height: usize) -> bool {
        match self {
            Shout::Col(c) => pos.x >= c as f32 * TILE,
            Shout::Row(r) => pos.y >= (height - r) as f32 * TILE,
        }
    }
}

impl Level {
    /// Is play here timed to the written beat: Nat alone (the gadget world's routes), beat
    /// crushers, beat blocks? The music then holds its tempo (`audio::plugin::strict_tempo`).
    pub fn on_the_beat(&self) -> bool {
        self.solo || self.crushers.iter().any(|c| c.beat > 0) || self.tiles.iter().any(|t| t.is_beat())
    }

    /// The band (or death) gate mark whose zone holds cell (col, row), if any: there Han's
    /// boost is the weak one.
    pub fn band_zone(&self, cell: (i32, i32)) -> Option<&GateMark> {
        self.gates.iter().find(|g| g.topic.han_keeps_clear() && g.berth().contains(cell))
    }

    /// Is cell (col, row) in a band zone (Han's boost is weak there)?
    pub fn in_band_zone(&self, cell: (i32, i32)) -> bool {
        self.band_zone(cell).is_some()
    }

    /// Is Han's boost weak in cell (col, row), with what the band plays `granted`? In a band
    /// zone, but in a mega wall's only while Giant Steps doesn't play (there it's the
    /// mega-boost), in a cling shaft's only without the grip, in a trapeze's only out of the
    /// waltz.
    pub fn weak_boost(&self, cell: (i32, i32), granted: Granted) -> bool {
        self.gates.iter().any(|g| g.weakens(cell, granted))
    }

    /// Is `cell` inside a cling shaft's mark (Han clings to its walls there, nervous)?
    pub fn in_shaft(&self, cell: (i32, i32)) -> bool {
        self.in_mark(Topic::Shaft, cell)
    }

    /// Is `cell` inside a `topic` gate mark?
    pub fn in_mark(&self, topic: Topic, cell: (i32, i32)) -> bool {
        self.gates.iter().any(|g| g.topic == topic && g.contains(cell))
    }

    /// Is Han's boost full strength in cell (col, row)? Outside every band zone. (The name is
    /// from when he kept out of the zones; he goes anywhere now but [`Level::han_keeps_out`].)
    pub fn han_allowed(&self, cell: (i32, i32)) -> bool {
        !self.in_band_zone(cell)
    }

    /// Cells Han won't stand in: grease in a grease chute's zone (Nat on his head would be
    /// jumping off the grease).
    pub fn han_keeps_out(&self, (c, r): (i32, i32)) -> bool {
        let greasy = self.tile(c, r + 1) == Tile::Grease;
        self.gates.iter().any(|g| g.topic == Topic::Grip && greasy && g.berth().contains((c, r)))
    }

    /// Is column `col` inside a fired-up chasm mark (no overuse limit there, and Han catches
    /// Nat in mid-air)?
    pub fn in_chasm(&self, col: i32) -> bool {
        self.gates.iter().any(|g| g.topic == Topic::Chain && (g.c0..=g.c1).contains(&col))
    }

    /// The tile at (col, row); outside the grid: solid walls left, right and above (the
    /// ceiling at the top of the screen), empty below (the pit).
    pub fn tile(&self, col: i32, row: i32) -> Tile {
        if col < 0 || col >= self.width as i32 || row < 0 {
            Tile::Solid
        } else if row >= self.height as i32 {
            Tile::Empty
        } else {
            self.tiles[row as usize * self.width + col as usize]
        }
    }

    /// Is grid cell (col, row) in a fan's updraft: a fan (`*`) at most [`FAN_REACH`] tiles
    /// below it, with nothing solid between?
    pub fn updraft(&self, col: i32, row: i32) -> bool {
        self.wind(col, row).1 > 0
    }

    /// The gusts grid cell (col, row) is in, (x, y) with y up: each fan at most [`FAN_REACH`]
    /// tiles away blowing its way at it, with nothing solid between (an updraft: y 1, a ceiling
    /// fan's downdraft: y -1; a wall fan's: x ±1). Opposite gusts cancel.
    pub fn wind(&self, col: i32, row: i32) -> (i32, i32) {
        self.wind_within(col, row, FAN_REACH)
    }

    /// [`Level::wind`] with fans blowing through `reach` empty tiles (Giant Steps:
    /// [`GIANT_FAN_REACH`]).
    pub fn wind_within(&self, col: i32, row: i32, reach: i32) -> (i32, i32) {
        if col < 0 || col >= self.width as i32 || self.tile(col, row).is_solid() {
            return (0, 0);
        }
        let mut wind = (0, 0);
        // Look back up each gust: (dc, dr) is grid steps towards the fan.
        for (dc, dr) in [(0, 1), (0, -1), (-1, 0), (1, 0)] {
            for k in 1..=reach {
                let t = self.tile(col + dc * k, row + dr * k);
                if let Some((x, y)) = t.fan() {
                    // Blowing back along the look (grid rows run down: y flips).
                    if (x, -y) == (-dc, -dr) {
                        wind.0 += x;
                        wind.1 += y;
                    }
                }
                if t.is_solid() {
                    break;
                }
            }
        }
        wind
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
        let mut solo = false;
        let mut secret = None;
        let mut shout = None;
        let mut gates = Vec::new();
        let mut platform_cfg: [Option<(f32, f32, f32, f32, PlatformKind)>; 10] = [None; 10];
        let mut crusher_phase: Vec<((i32, i32), Option<f32>, Option<u8>)> = Vec::new();

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
                "solo" => {
                    if value != "nat" {
                        return Err(format!("expected `solo: nat`, got `{value}`"));
                    }
                    solo = true;
                }
                "say" => says.push(value.to_string()),
                "shout" => {
                    let bad = || format!("bad shout `{value}` (want `<col>` or `row <row>`)");
                    shout = Some(match value.strip_prefix("row ") {
                        Some(r) => Shout::Row(r.trim().parse().map_err(|_| bad())?),
                        None => Shout::Col(value.parse().map_err(|_| bad())?),
                    });
                }
                k if k.starts_with("secret@") => {
                    let at = &k["secret@".len()..];
                    let (col, row) = at
                        .split_once(',')
                        .and_then(|(c, r)| Some((c.trim().parse().ok()?, r.trim().parse().ok()?)))
                        .ok_or_else(|| format!("expected `secret@<col>,<row>`, got `{k}`"))?;
                    secret = Some(Spot { col, row, text: value.to_string(), topics: Vec::new() });
                }
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
                k if k.starts_with("crusher@") => {
                    let at = &k["crusher@".len()..];
                    let cell = at
                        .split_once(',')
                        .and_then(|(c, r)| Some((c.trim().parse().ok()?, r.trim().parse().ok()?)))
                        .ok_or_else(|| format!("expected `crusher@<col>,<row>`, got `{k}`"))?;
                    let (mut phase, mut beat) = (None, None);
                    for kv in value.split_whitespace() {
                        let bad = || format!("{k}: bad `{kv}` (expected `phase=<0..1>` and/or `beat=<1..8 bars>`)");
                        match kv.split_once('=') {
                            Some(("phase", v)) => phase = Some(v.parse::<f32>().ok().filter(|p| (0.0..=1.0).contains(p)).ok_or_else(bad)?),
                            Some(("beat", v)) => beat = Some(v.parse::<u8>().ok().filter(|b| (1..=8).contains(b)).ok_or_else(bad)?),
                            _ => return Err(bad()),
                        }
                    }
                    if phase.is_none() && beat.is_none() {
                        return Err(format!("{k}: expected `phase=<0..1>` and/or `beat=<1..8>`"));
                    }
                    crusher_phase.push((cell, phase, beat));
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
        let mut crusher_cells: Vec<(CrushDir, usize, usize)> = Vec::new();

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
                    '%' => *tile = Tile::Brick,
                    '-' => *tile = Tile::Crumble,
                    'T' => *tile = Tile::Trampoline,
                    '{' => *tile = Tile::BeltLeft,
                    '}' => *tile = Tile::BeltRight,
                    '*' => *tile = Tile::Fan,
                    '(' => *tile = Tile::BumperLeft,
                    ')' => *tile = Tile::BumperRight,
                    '[' => *tile = Tile::BeatRed,
                    ']' => *tile = Tile::BeatBlueOff,
                    '!' => *tile = Tile::FanDown,
                    'e' => *tile = Tile::FanRight,
                    'w' => *tile = Tile::FanLeft,
                    'b' => *tile = Tile::CushionRight,
                    'd' => *tile = Tile::CushionLeft,
                    'm' => *tile = Tile::CushionDown,
                    // Pointing away from the wall beside it (below).
                    '|' => *tile = Tile::SpikesRight,
                    'X' => crusher_cells.push((CrushDir::Down, col, row)),
                    '<' => crusher_cells.push((CrushDir::Left, col, row)),
                    '>' => crusher_cells.push((CrushDir::Right, col, row)),
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
                    'B' => things.push(Thing { kind: ThingKind::Boom, col, row }),
                    ':' => things.push(Thing { kind: ThingKind::Bean, col, row }),
                    'D' => things.push(Thing { kind: ThingKind::Diaper, col, row }),
                    'H' => things.push(Thing { kind: ThingKind::Helper, col, row }),
                    'M' => things.push(Thing { kind: ThingKind::Blaster, col, row }),
                    '@' => things.push(Thing { kind: ThingKind::StopSign, col, row }),
                    'A' => things.push(Thing { kind: ThingKind::House(Cook::Never), col, row }),
                    'K' => things.push(Thing { kind: ThingKind::House(Cook::Always), col, row }),
                    'a' => things.push(Thing { kind: ThingKind::House(Cook::Sometimes), col, row }),
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

        // Wall brushes point away from the wall they stick out of.
        for row in 0..height {
            for col in 0..width {
                if tiles[row * width + col] != Tile::SpikesRight {
                    continue;
                }
                let at = |c: i32| (c >= 0 && (c as usize) < width).then(|| tiles[row * width + c as usize]);
                let wall = |t: Option<Tile>| t.is_some_and(|t| t.is_solid());
                match (wall(at(col as i32 - 1)), wall(at(col as i32 + 1))) {
                    (true, false) => {}
                    (false, true) => tiles[row * width + col] = Tile::SpikesLeft,
                    _ => return Err(format!("wall brushes `|` at row {row}, col {col} need a wall on exactly one side")),
                }
            }
        }

        let mut crushers = group_crushers(&crusher_cells, &tiles, width, height)?;
        for ((c, r), phase, beat) in crusher_phase {
            let k = crushers
                .iter()
                .position(|d| Crusher::home(d).any(|h| h == (c, r)))
                .ok_or_else(|| format!("crusher@{c},{r}: no crusher there"))?;
            crushers[k].phase = phase.unwrap_or(0.0);
            crushers[k].beat = beat.unwrap_or(0);
        }

        for s in say_at.iter().chain(&hints).chain(&secret) {
            if s.col >= width || s.row >= height {
                return Err(format!("line at col {} row {} is outside the grid", s.col, s.row));
            }
        }
        if let Some(s) = &secret {
            let tile = |c: usize, r: usize| tiles[r * width + c];
            if s.row == 0 || !tile(s.col, s.row).is_solid() || !tile(s.col, s.row - 1).is_solid() {
                return Err(format!("secret@{},{}: the grate (that cell and the one above) must be solid", s.col, s.row));
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
            crushers,
            gates,
            solo,
            secret,
            shout,
        })
    }
}

/// Group crusher cells into blocks: each connected group of the same char must be a filled
/// rectangle. Its travel runs from home until a tile (anything not empty) is in its way, the
/// level's edge (walls left and right; below, the pit: [`CRUSH_MAX_TRAVEL`]), or another
/// crusher's home.
fn group_crushers(cells: &[(CrushDir, usize, usize)], tiles: &[Tile], width: usize, height: usize) -> Result<Vec<CrusherDef>, String> {
    use std::collections::HashSet;
    let all: HashSet<(usize, usize)> = cells.iter().map(|&(_, c, r)| (c, r)).collect();
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let mut out = Vec::new();
    for &(dir, c, r) in cells {
        if seen.contains(&(c, r)) {
            continue;
        }
        // Flood the same char.
        let same: HashSet<(usize, usize)> = cells.iter().filter(|x| x.0 == dir).map(|&(_, c, r)| (c, r)).collect();
        let mut group = vec![(c, r)];
        seen.insert((c, r));
        let mut i = 0;
        while i < group.len() {
            let (gc, gr) = group[i];
            i += 1;
            let ns = [(gc.wrapping_sub(1), gr), (gc + 1, gr), (gc, gr.wrapping_sub(1)), (gc, gr + 1)];
            for n in ns {
                if same.contains(&n) && seen.insert(n) {
                    group.push(n);
                }
            }
        }
        let (c0, c1) = (group.iter().map(|g| g.0).min().unwrap(), group.iter().map(|g| g.0).max().unwrap());
        let (r0, r1) = (group.iter().map(|g| g.1).min().unwrap(), group.iter().map(|g| g.1).max().unwrap());
        let (w, h) = (c1 - c0 + 1, r1 - r0 + 1);
        if group.len() != w * h {
            return Err(format!("crusher `{}` at row {r0}, col {c0} isn't a filled rectangle", dir.char()));
        }
        let mut def = CrusherDef { col: c0, row: r0, w, h, dir, travel: 0, phase: 0.0, beat: 0 };
        let blocked = |cc: i32, rr: i32| {
            if cc < 0 || cc >= width as i32 || rr < 0 {
                return true;
            }
            if rr >= height as i32 {
                return false;
            }
            let (cu, ru) = (cc as usize, rr as usize);
            tiles[ru * width + cu] != Tile::Empty || (all.contains(&(cu, ru)) && !group.contains(&(cu, ru)))
        };
        let (dc, dr) = dir.step();
        while def.travel < CRUSH_MAX_TRAVEL {
            let k = def.travel + 1;
            let hit = Crusher::home(&def).any(|(hc, hr)| {
                let (nc, nr) = (hc + dc * k, hr + dr * k);
                blocked(nc, nr) && !Crusher::home(&def).any(|x| x == (nc, nr))
            });
            if hit {
                break;
            }
            def.travel = k;
        }
        if def.travel == 0 {
            return Err(format!("crusher `{}` at row {r0}, col {c0} has no room to slam", dir.char()));
        }
        out.push(def);
    }
    Ok(out)
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
    fn parses_the_secret_and_the_shout() {
        let src = "name: T\nsolo: nat\nsecret@3,1: Nat?!\nshout: 2\n---\n...#G\nP..##\n#####\n";
        let l = Level::parse(src).unwrap();
        let s = l.secret.as_ref().unwrap();
        assert_eq!((s.col, s.row, s.text.as_str()), (3, 1, "Nat?!"));
        assert_eq!(l.shout, Some(Shout::Col(2)));
        let tower = Level::parse("name: T\nshout: row 1\n---\n....G\nP..##\n#####\n").unwrap();
        assert_eq!(tower.shout, Some(Shout::Row(1)));
        // Row 1's top edge is 2 tiles up from the bottom of a 3-row level.
        assert!(Shout::Row(1).reached(Vec2::new(0.0, 2.0 * TILE), 3));
        assert!(!Shout::Row(1).reached(Vec2::new(0.0, 1.5 * TILE), 3));
        // The grate is a wall: its cell and the one above must be solid.
        assert!(Level::parse("name: T\nsecret@2,1: x\n---\n...#G\nP..##\n#####\n").is_err());
        assert!(Level::parse("name: T\nsecret@9,9: x\n---\nPG\n##\n").is_err());
    }

    #[test]
    fn all_levels_parse() {
        let levels = Levels::default();
        assert_eq!(levels.0.len(), LEVEL_COUNT);
    }
}

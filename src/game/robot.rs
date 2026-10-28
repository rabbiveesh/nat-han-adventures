//! The robot pit (`robotpit:` in a level file): a joke crossover with the dev's other game,
//! Robot Buddy Adventure (a top-down math RPG for kids, Sparky the robot its buddy).
//!
//! One gap in a level isn't bottomless: Sparky's antenna blinks at its bottom. Fall in and Nat
//! lands in the wrong game. The platformer freezes ([`GameSet`] doesn't run while the
//! [`RobotRoom`] is up) and Nat walks a sunny top-down meadow in four directions, gravity gone.
//! Sparky greets him ("BEEP BOOP! Welcome back... wait. You're a POO.") and asks a robot-game
//! question about nuggets; Nat answers by standing on a numbered pad. A wrong pad just drains
//! Sparky's battery a notch (the robot game never punishes) and greys out; the right one wins
//! a Dum Dum and opens the whirlpool (the robot game's dive spot), which is, of course, a
//! flush: Nat swirls down it and lands back on the ledge before the pit. Han: "Where were you?!"
//!
//! The room sits far below the level, [`ROOM_DROP`] under the pit (he did fall a long way).
//! Nothing in it can hurt Nat; falling in isn't a death (no `PlayerDied`: the adaptive engine
//! never hears of it).

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::physics::{Body, PlayerControl};
use super::{ActiveLevel, GameSet, LevelEntity, Player, Pos, PrevPos, RestartLevel, stand_pos};
use crate::audio::Sfx;
use crate::events::{HanSays, PlaySfx};
use crate::input::Action;
use crate::level::{Level, TILE};
use crate::state::{AppState, PlayState};

/// Sparky's meadow, one char per 16px tile, row 0 at the top. `T` tree, `.` grass, `,` path,
/// `f` flowers, `R` roof, `H` house wall, `W` window, `D` door, `F` fence, `C` chest, `~` water,
/// `@` the whirlpool (water till it opens), `N` where Nat lands, `S` Sparky, `1` `2` `3` the
/// answer pads. Its trees run wide so a wide window shows no edge.
pub const ROOM: &[&str] = &[
    "TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT",
    "TTTTTTT..f......RRRR.....f.TTTTTTT",
    "TTTTTT.....f....HWDH.........TTTTT",
    "TTTTT......,,,,,,,,,,,,..f....TTTT",
    "TTTTT.f....,..........,.......TTTT",
    "TTTTT......,..N...S...,..~~~..TTTT",
    "TTTTT..FFF.,..........,..~@...TTTT",
    "TTTTT......,..1..2..3.,..~~~..TTTT",
    "TTTTT...f..,..........,.......TTTT",
    "TTTTT......,,,,,,,,,,,,..C....TTTT",
    "TTTTTT.f..........f.......f.TTTTTT",
    "TTTTTTT.....................TTTTTT",
    "TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT",
    "TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT",
];
pub const ROOM_COLS: usize = 34;
pub const ROOM_ROWS: usize = 14;
/// The room's top sits this far under the level's bottom (px).
pub const ROOM_DROP: f32 = 40.0 * TILE;
/// Nat keeps falling (off screen) this long before the cut (s).
pub const FALL_SECS: f32 = 0.45;
/// "WRONG GAME!" holds this long before Nat can walk (s).
pub const ARRIVE_SECS: f32 = 1.8;
/// Each of Sparky's chatty lines before the question (s).
pub const LINE_SECS: f32 = 2.4;
/// A wrong answer's line (s), then the question again.
pub const WRONG_SECS: f32 = 2.2;
/// The prize line, then where the way home is.
pub const PRIZE_SECS: f32 = 3.0;
/// Swirling down the whirlpool (s).
pub const FLUSH_SECS: f32 = 1.4;
/// Nat's top-down walking speed (px/s): the robot game's stroll, not a platformer's dash (a
/// tile in 1/6 s).
pub const WALK_SPEED: f32 = 96.0;
/// Sparky's battery: full, and what a wrong answer costs.
pub const BATTERY: u8 = 2;
/// Han's line when Nat's back.
pub const BACK_LINE: &str = "Where were you?!";

/// A robot-game question: Sparky's line, the answer, and the three pads (left to right).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Question {
    pub text: &'static str,
    pub answer: u32,
    pub pads: [u32; 3],
}

/// The questions, a new one each visit (in turn).
pub const QUESTIONS: [Question; 4] = [
    Question { text: "NAT has 3 nuggets. HAN finds 2 more. How many nuggets now?", answer: 5, pads: [4, 5, 6] },
    Question { text: "HAN has 4 plungers. NAT hides 1. How many plungers are left?", answer: 3, pads: [5, 2, 3] },
    Question { text: "2 toots + 2 toots = how many toots?", answer: 4, pads: [22, 4, 3] },
    Question { text: "The band plays 4 bars, then 4 more. How many bars?", answer: 8, pads: [8, 44, 6] },
];

const FIRST_VISIT: [&str; 3] = [
    "BEEP BOOP! Welcome back, NAT! I missed you!",
    "...wait. You're not a kid. You're a POO!",
    "That's OK! Math time! Stand on the answer!",
];
const LATER_VISIT: [&str; 2] = ["BEEP BOOP! NAT?! Again?!", "Math time! Stand on the answer!"];
const WRONG_LINE: &str = "Hmm, that's not right. BZZT... battery low...";
const PRIZE_LINE: &str = "CORRECT! You win a DUM DUM! ...oh. Poos can't hold lollipops.";
const HOME_LINE: &str = "The whirlpool takes you home! BEEP BOOP!";
const FLAT_LINE: &str = "Battery... empty... The whirlpool... goes home... boop.";
const BYE_LINE: &str = "BYE NAT! Come back for fractions!";

/// Nat's in the wrong game.
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct RobotRoom {
    pub phase: RoomPhase,
    /// Seconds in this phase.
    pub t: f32,
    /// The room's bottom-left corner (world px).
    pub origin: Vec2,
    /// Where Nat comes back (his middle): the ledge before the pit.
    pub back: Vec2,
    /// Visits to the wrong game this play session, this one included (1 = the first).
    pub visit: u32,
    /// Sparky's battery, out of [`BATTERY`].
    pub battery: u8,
    /// [`QUESTIONS`] index.
    pub question: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum RoomPhase {
    /// Still falling, off screen.
    Falling,
    /// "WRONG GAME!": Nat's just landed, frozen.
    Arrive,
    /// Sparky's chatter, Nat free to walk.
    Talk,
    /// The question is up: stand on the answer.
    Ask,
    /// Not that one (back to [`RoomPhase::Ask`]).
    Wrong,
    /// Right (or Sparky's flat): the whirlpool's open.
    Open,
    /// Down the whirlpool.
    Flush,
}

/// Times Nat's fallen into the wrong game this session (the questions take turns).
#[derive(Resource, Debug, Default, Clone, Copy, Reflect)]
#[reflect(Resource)]
pub struct RobotVisits(pub u32);

/// Nat fell into a robot pit (from `hazards`, instead of splatting).
#[derive(Message, Debug, Clone, Copy)]
pub struct FellInRobotPit {
    pub pit: (usize, usize),
}

/// Everything of the room (despawned on the way out; also a [`LevelEntity`]).
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct RoomEntity;

/// A tile of the room: its [`ROOM`] char.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct RoomTile(pub char);

/// Sparky. Pos is his middle.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct Sparky;

/// An answer pad. `used`: a wrong answer already picked (greyed out).
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct AnswerPad {
    pub value: u32,
    pub used: bool,
}

/// The whirlpool home (water till it opens).
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct Whirlpool {
    pub open: bool,
}

/// The chest: pops open with the prize.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct RoomChest {
    pub open: bool,
}

/// Nat walks top-down (the visuals run his legs whichever way he goes).
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct TopDown;

/// Sparky's antenna, blinking at the bottom of a robot pit (the tell). Transform only.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct PitAntenna;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<RobotRoom>()
        .register_type::<RobotVisits>()
        .register_type::<AnswerPad>()
        .register_type::<Whirlpool>()
        .add_message::<FellInRobotPit>()
        .init_resource::<RobotVisits>()
        .add_systems(Update, (spawn_antennas, leave_on_restart))
        .add_systems(FixedUpdate, fall_in.after(GameSet::Interact).before(GameSet::Follow))
        .add_systems(
            FixedUpdate,
            run_room.run_if(in_state(PlayState::Running).and_then(resource_exists::<RobotRoom>)),
        )
        .add_systems(OnExit(AppState::Playing), |mut commands: Commands| commands.remove_resource::<RobotRoom>());
}

impl RobotRoom {
    pub fn question(&self) -> Question {
        QUESTIONS[self.question % QUESTIONS.len()]
    }

    fn intro(&self) -> &'static [&'static str] {
        if self.visit <= 1 { &FIRST_VISIT } else { &LATER_VISIT }
    }

    /// What Sparky's saying (the dialogue box), if anything.
    pub fn line(&self) -> Option<&'static str> {
        match self.phase {
            RoomPhase::Falling | RoomPhase::Arrive => None,
            RoomPhase::Talk => self.intro().get((self.t / LINE_SECS) as usize).copied(),
            RoomPhase::Ask => Some(self.question().text),
            RoomPhase::Wrong => Some(WRONG_LINE),
            RoomPhase::Open if self.battery == 0 => Some(FLAT_LINE),
            RoomPhase::Open => Some(if self.t < PRIZE_SECS { PRIZE_LINE } else { HOME_LINE }),
            RoomPhase::Flush => Some(BYE_LINE),
        }
    }

    /// Can Nat walk?
    pub fn walking(&self) -> bool {
        matches!(self.phase, RoomPhase::Talk | RoomPhase::Ask | RoomPhase::Wrong | RoomPhase::Open)
    }

    /// World center of room cell (col, row).
    pub fn cell_center(&self, col: usize, row: usize) -> Vec2 {
        self.origin + Vec2::new((col as f32 + 0.5) * TILE, (ROOM_ROWS - 1 - row) as f32 * TILE + TILE / 2.0)
    }

    /// The room cell holding world point `p`.
    pub fn cell_at(&self, p: Vec2) -> (i32, i32) {
        let d = p - self.origin;
        ((d.x / TILE).floor() as i32, ROOM_ROWS as i32 - 1 - (d.y / TILE).floor() as i32)
    }

    /// The room's middle (world px) and size.
    pub fn center(&self) -> Vec2 {
        self.origin + Self::size() / 2.0
    }

    pub fn size() -> Vec2 {
        Vec2::new(ROOM_COLS as f32, ROOM_ROWS as f32) * TILE
    }

    /// Where the room's `ch` is (the first).
    pub fn find(ch: char) -> (usize, usize) {
        ROOM.iter()
            .enumerate()
            .find_map(|(r, row)| row.chars().position(|c| c == ch).map(|c| (c, r)))
            .unwrap_or_else(|| panic!("no `{ch}` in the robot room"))
    }

    /// Does `ch` block walking (the whirlpool till it's open)?
    fn solid(&self, col: i32, row: i32, open: bool) -> bool {
        if col < 0 || row < 0 || col >= ROOM_COLS as i32 || row >= ROOM_ROWS as i32 {
            return true;
        }
        match ROOM[row as usize].as_bytes()[col as usize] as char {
            'T' | 'R' | 'H' | 'W' | 'D' | 'F' | 'C' | '~' | 'S' => true,
            '@' => !open,
            _ => false,
        }
    }

    fn set(&mut self, phase: RoomPhase) {
        self.phase = phase;
        self.t = 0.0;
    }
}

/// The ledge before the pit (columns `c0..`): Nat's middle standing on it, two columns back if
/// there's floor there, else right at the edge.
pub fn ledge_before(level: &Level, (c0, _): (usize, usize)) -> Vec2 {
    let floor = |col: usize| {
        (1..level.height).find(|&r| level.tile(col as i32, r as i32).is_solid() && !level.tile(col as i32, r as i32 - 1).is_solid())
    };
    let col = [c0.saturating_sub(2), c0 - 1].into_iter().find(|&c| floor(c).is_some()).unwrap_or(c0 - 1);
    let row = floor(col).unwrap_or(level.height - 1);
    stand_pos(level, col, row - 1)
}

/// Which robot pit (if any) a fall at x (world px) went into.
pub fn pit_at(level: &Level, x: f32) -> Option<(usize, usize)> {
    let col = (x / TILE).floor();
    level.robot_pits.iter().copied().find(|&(c0, c1)| col >= c0 as f32 && col <= c1 as f32)
}

fn spawn_antennas(mut commands: Commands, active: Option<Res<ActiveLevel>>, antennas: Query<(), With<PitAntenna>>) {
    let Some(active) = active else { return };
    if active.level.robot_pits.is_empty() || !antennas.is_empty() {
        return;
    }
    for &(c0, c1) in &active.level.robot_pits {
        let at = Vec2::new((c0 + c1 + 1) as f32 * TILE / 2.0, TILE / 2.0);
        commands.spawn((Name::new("PitAntenna"), LevelEntity, PitAntenna, Transform::from_translation(at.extend(2.0))));
    }
}

/// A fall into a robot pit: the wrong game begins.
fn fall_in(
    mut commands: Commands,
    mut fell: MessageReader<FellInRobotPit>,
    active: Option<Res<ActiveLevel>>,
    room: Option<Res<RobotRoom>>,
    mut visits: ResMut<RobotVisits>,
) {
    let Some(f) = fell.read().last() else { return };
    let (Some(active), None) = (active, room) else { return };
    let level = &active.level;
    let mid = (f.pit.0 + f.pit.1 + 1) as f32 * TILE / 2.0;
    let origin = Vec2::new(mid - RobotRoom::size().x / 2.0, -ROOM_DROP - RobotRoom::size().y);
    visits.0 += 1;
    commands.insert_resource(RobotRoom {
        phase: RoomPhase::Falling,
        t: 0.0,
        origin,
        back: ledge_before(level, f.pit),
        visit: visits.0,
        battery: BATTERY,
        question: (visits.0 as usize - 1) % QUESTIONS.len(),
    });
}

fn spawn_room(commands: &mut Commands, room: &RobotRoom) {
    for (r, row) in ROOM.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            let at = room.cell_center(c, r);
            let tile = match ch {
                'N' | 'S' | '1' | '2' | '3' => '.',
                '@' => '~',
                ch => ch,
            };
            commands.spawn((Name::new("RoomTile"), LevelEntity, RoomEntity, RoomTile(tile), Transform::from_translation(at.extend(0.0))));
            match ch {
                'S' => {
                    commands.spawn((Name::new("Sparky"), LevelEntity, RoomEntity, Sparky, Pos(at), PrevPos(at), Transform::from_translation(at.extend(4.0))));
                }
                '1' | '2' | '3' => {
                    let value = room.question().pads[(ch as u8 - b'1') as usize];
                    commands.spawn((Name::new("AnswerPad"), LevelEntity, RoomEntity, AnswerPad { value, used: false }, Transform::from_translation(at.extend(0.5))));
                }
                '@' => {
                    commands.spawn((Name::new("Whirlpool"), LevelEntity, RoomEntity, Whirlpool::default(), Transform::from_translation(at.extend(0.5))));
                }
                'C' => {
                    commands.spawn((Name::new("RoomChest"), LevelEntity, RoomEntity, RoomChest::default(), Transform::from_translation(at.extend(0.5))));
                }
                _ => {}
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn run_room(
    mut commands: Commands,
    time: Res<Time>,
    mut room: ResMut<RobotRoom>,
    input: Single<&ActionState<Action>>,
    mut nat: Query<(Entity, &mut Pos, &mut PrevPos, &mut Body, &mut PlayerControl), With<Player>>,
    mut pads: Query<(&mut AnswerPad, &Transform)>,
    mut pool: Query<(&mut Whirlpool, &Transform)>,
    mut chest: Query<&mut RoomChest>,
    old: Query<Entity, With<RoomEntity>>,
    mut sfx: MessageWriter<PlaySfx>,
    mut says: MessageWriter<HanSays>,
) {
    let dt = time.delta_secs();
    let Ok((entity, mut pos, mut prev, mut body, mut ctl)) = nat.single_mut() else { return };
    prev.0 = pos.0;
    room.t += dt;
    let line_before = room.line();
    match room.phase {
        RoomPhase::Falling => {
            pos.0.y -= super::tuning::MAX_FALL * dt;
            if room.t >= FALL_SECS {
                spawn_room(&mut commands, &room);
                let (c, r) = RobotRoom::find('N');
                pos.0 = room.cell_center(c, r);
                prev.0 = pos.0;
                *body = Body::player();
                body.on_ground = true;
                commands.entity(entity).insert(TopDown);
                sfx.write(PlaySfx(Sfx::Land));
                room.set(RoomPhase::Arrive);
            }
        }
        RoomPhase::Arrive if room.t >= ARRIVE_SECS => room.set(RoomPhase::Talk),
        RoomPhase::Talk if room.t >= LINE_SECS * room.intro().len() as f32 => room.set(RoomPhase::Ask),
        RoomPhase::Wrong if room.t >= WRONG_SECS => room.set(RoomPhase::Ask),
        RoomPhase::Flush if room.t >= FLUSH_SECS => {
            for e in &old {
                commands.entity(e).despawn();
            }
            commands.entity(entity).remove::<TopDown>();
            commands.remove_resource::<RobotRoom>();
            pos.0 = room.back;
            prev.0 = pos.0;
            *body = Body::player();
            ctl.facing = 1.0;
            says.write(HanSays { text: BACK_LINE.to_string() });
            return;
        }
        _ => {}
    }

    // Walking, top-down: one axis at a time against the room's solid tiles.
    if room.walking() {
        let dir = Vec2::new(
            input.pressed(&Action::Right) as i32 as f32 - input.pressed(&Action::Left) as i32 as f32,
            input.pressed(&Action::Up) as i32 as f32 - input.pressed(&Action::Down) as i32 as f32,
        );
        let vel = dir.normalize_or_zero() * WALK_SPEED;
        if dir.x != 0.0 {
            ctl.facing = dir.x.signum();
        }
        let open = room.phase == RoomPhase::Open;
        let half = body.half;
        let blocked = |p: Vec2| {
            let (a, b) = (room.cell_at(p - half + 0.01), room.cell_at(p + half - 0.01));
            (a.0..=b.0).any(|c| (b.1..=a.1).any(|r| room.solid(c, r, open)))
        };
        for axis in [Vec2::X, Vec2::Y] {
            let step = pos.0 + vel * axis * dt;
            if !blocked(step) {
                pos.0 = step;
            }
        }
        body.vel = vel;
        body.on_ground = true;
    } else {
        body.vel = Vec2::ZERO;
    }

    // The pads (only while the question's up).
    if room.phase == RoomPhase::Ask {
        let answer = room.question().answer;
        if let Some(mut pad) = pads.iter_mut().find_map(|(p, tf)| on_cell(pos.0, tf).then_some(p)).filter(|p| !p.used) {
            if pad.value == answer {
                open_home(&mut room, &mut pool, &mut chest, &mut sfx);
            } else {
                pad.used = true;
                room.battery = room.battery.saturating_sub(1);
                if room.battery == 0 {
                    open_home(&mut room, &mut pool, &mut chest, &mut sfx);
                } else {
                    sfx.write(PlaySfx(Sfx::MenuMove));
                    room.set(RoomPhase::Wrong);
                }
            }
        }
    }

    // Home.
    if room.phase == RoomPhase::Open && pool.iter().any(|(w, tf)| w.open && on_cell(pos.0, tf)) {
        let (_, tf) = pool.single().expect("one whirlpool");
        pos.0 = tf.translation.truncate();
        body.vel = Vec2::ZERO;
        sfx.write(PlaySfx(Sfx::Flush));
        room.set(RoomPhase::Flush);
    }

    if room.line() != line_before && room.line().is_some() {
        sfx.write(PlaySfx(Sfx::HanBlip));
    }
}

/// Is `p` within the middle of the cell drawn at `tf`?
fn on_cell(p: Vec2, tf: &Transform) -> bool {
    (p - tf.translation.truncate()).abs().max_element() < TILE * 0.4
}

fn open_home(
    room: &mut RobotRoom,
    pool: &mut Query<(&mut Whirlpool, &Transform)>,
    chest: &mut Query<&mut RoomChest>,
    sfx: &mut MessageWriter<PlaySfx>,
) {
    for (mut w, _) in pool.iter_mut() {
        w.open = true;
    }
    if room.battery > 0 {
        for mut c in chest.iter_mut() {
            c.open = true;
        }
    }
    sfx.write(PlaySfx(Sfx::Secret));
    room.set(RoomPhase::Open);
}

/// A restart (pause menu) leaves the wrong game: the level's despawned and reloaded anyway.
fn leave_on_restart(mut commands: Commands, mut restart: MessageReader<RestartLevel>) {
    if restart.read().count() > 0 {
        commands.remove_resource::<RobotRoom>();
    }
}

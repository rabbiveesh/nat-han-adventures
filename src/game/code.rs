//! The building code: a secret paper (`?`), a door with a keypad (`&`), the secret room (`$`).
//!
//! The level's code (`code:`, a few arrows) is written on the paper, hidden somewhere. Touch
//! it and the code comes up to read ([`CodeRead`], `ui::code`) for a moment: remember it! Up
//! at the door and the keypad's on ([`AtKeypad`]): the arrows type, one press a key. The right
//! code and the door opens, Nat goes through into the secret room, and the band plays the
//! level's `mode:` (one of its feels) from then on ([`ModeMusic`]). A wrong one buzzes and
//! the keypad's off again.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::physics::{Body, Dead, Finished, PlayerControl};
use super::{ActiveLevel, GameSet, LevelEntity, Player, Pos, PrevPos, cell_floor};
use crate::events::{LevelStarted, PlayerRespawned, ToyShout};
use crate::input::Action;
use crate::level::{Arrow, Level, RoomMode, TILE, ThingKind};
use crate::state::AppState;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<CodePaper>()
        .register_type::<CodeDoor>()
        .register_type::<AtKeypad>()
        .init_resource::<RoomPlaying>()
        .add_message::<CodeRead>()
        .add_message::<KeypadEvent>()
        .add_message::<ModeMusic>()
        .add_systems(Update, mode_off)
        .add_systems(FixedUpdate, (keypad, go_to_keypad, through_door).chain().in_set(GameSet::Player).before(super::physics::player_step))
        .add_systems(FixedUpdate, (read_paper, leave_keypad).in_set(GameSet::Interact));
}

/// Up at a door within this much (px) of its middle.
const DOOR_HALF: f32 = 8.0;
/// The paper's box (half extents, px).
const PAPER_HALF: Vec2 = Vec2::new(6.0, 7.0);
/// The keypad gives up on a Nat who types nothing for this long (s)...
const KEYPAD_IDLE: f32 = 8.0;
/// ...and lets him go this long (s) after a wrong code.
const BUZZ_SECS: f32 = 0.4;
/// The door takes this long (s) to let Nat through.
const DOOR_SECS: f32 = 0.6;

/// The secret paper. Its transform is its cell's middle.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct CodePaper;

/// The door with a keypad. Its transform is its cell's floor.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct CodeDoor {
    pub open: bool,
    /// Since the last wrong code (s; for its red light).
    pub since_wrong: f32,
}

/// On Nat at a keypad: what he's typed, and since his last key (s); `buzzed`: a wrong code,
/// this long ago (he stands there a moment, so the key that buzzed isn't a jump too).
#[derive(Component, Debug, Clone, Default, Reflect)]
#[reflect(Component)]
pub struct AtKeypad {
    pub door: Option<Entity>,
    pub typed: Vec<Arrow>,
    pub idle: f32,
    pub buzzed: Option<f32>,
}

/// On Nat going through an open door (`t` s so far): out of sight, into the secret room.
#[derive(Component, Debug, Clone, Copy)]
pub struct ThroughDoor {
    pub t: f32,
}

/// The paper's read: here's the code (`ui::code` shows it).
#[derive(Message, Debug, Clone)]
pub struct CodeRead {
    pub code: Vec<Arrow>,
    pub pos: Vec2,
}

/// The keypad (for sounds).
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub enum KeypadEvent {
    On(Vec2),
    Key(Arrow),
    Open(Vec2),
    Wrong(Vec2),
}

/// The band plays this mode (`Some`) or what it likes again (`None`): the audio plugin plays it.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeMusic(pub Option<RoomMode>);

/// The mode the band's playing for a building code (`None`: none).
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RoomPlaying(pub Option<RoomMode>);

/// Touch the paper: read the code.
fn read_paper(
    mut commands: Commands,
    active: Res<ActiveLevel>,
    nat: Query<(&Pos, &Body), (With<Player>, Without<Dead>)>,
    papers: Query<(Entity, &Transform), With<CodePaper>>,
    mut read: MessageWriter<CodeRead>,
) {
    let Ok((pos, body)) = nat.single() else { return };
    for (e, tf) in &papers {
        let at = tf.translation.truncate();
        if (pos.0.x - at.x).abs() < body.half.x + PAPER_HALF.x && (pos.0.y - at.y).abs() < body.half.y + PAPER_HALF.y {
            commands.entity(e).despawn();
            read.write(CodeRead { code: active.level.code.clone(), pos: at });
        }
    }
}

/// Up at a door: the keypad's on.
#[allow(clippy::type_complexity)]
fn go_to_keypad(
    mut commands: Commands,
    input: Single<&ActionState<Action>>,
    mut nat: Query<(Entity, &Pos, &mut Body), (With<Player>, Without<Dead>, Without<Finished>, Without<AtKeypad>, Without<ThroughDoor>)>,
    doors: Query<(Entity, &CodeDoor, &Transform)>,
    mut events: MessageWriter<KeypadEvent>,
    mut shout: MessageWriter<ToyShout>,
) {
    if !input.just_pressed(&Action::Up) {
        return;
    }
    let Ok((e, pos, mut body)) = nat.single_mut() else { return };
    if !body.on_ground {
        return;
    }
    let feet = pos.0.y - body.half.y;
    for (de, door, tf) in &doors {
        let at = tf.translation.truncate();
        if door.open || (pos.0.x - at.x).abs() > DOOR_HALF || (feet - at.y).abs() > 3.0 {
            continue;
        }
        body.vel = Vec2::ZERO;
        body.launch = 0.0;
        commands.entity(e).insert(AtKeypad { door: Some(de), ..default() });
        events.write(KeypadEvent::On(at));
        shout.write(ToyShout { pos: at + Vec2::Y * 2.6 * TILE, text: "TYPE THE CODE" });
        return;
    }
}

/// At the keypad: each arrow types. The whole code typed: open, or a buzz.
#[allow(clippy::type_complexity)]
fn keypad(
    mut commands: Commands,
    time: Res<Time>,
    active: Res<ActiveLevel>,
    input: Single<&ActionState<Action>>,
    mut nat: Query<(Entity, &mut Body, &mut AtKeypad), With<Player>>,
    mut doors: Query<(&mut CodeDoor, &Transform)>,
    mut playing: ResMut<RoomPlaying>,
    mut events: MessageWriter<KeypadEvent>,
    mut music: MessageWriter<ModeMusic>,
    mut shout: MessageWriter<ToyShout>,
) {
    let Ok((e, mut body, mut pad)) = nat.single_mut() else { return };
    body.vel = Vec2::ZERO;
    pad.idle += time.delta_secs();
    let Some((mut door, tf)) = pad.door.and_then(|d| doors.get_mut(d).ok()) else {
        commands.entity(e).remove::<AtKeypad>();
        return;
    };
    let at = tf.translation.truncate();
    if let Some(t) = pad.buzzed.as_mut() {
        *t += time.delta_secs();
        if *t > BUZZ_SECS {
            commands.entity(e).remove::<AtKeypad>();
        }
        return;
    }
    for (action, arrow) in [(Action::Up, Arrow::Up), (Action::Down, Arrow::Down), (Action::Left, Arrow::Left), (Action::Right, Arrow::Right)] {
        if input.just_pressed(&action) {
            pad.typed.push(arrow);
            pad.idle = 0.0;
            events.write(KeypadEvent::Key(arrow));
        }
    }
    let code = &active.level.code;
    if pad.typed.len() >= code.len() {
        if pad.typed[..code.len()] == code[..] {
            commands.entity(e).remove::<AtKeypad>();
            door.open = true;
            commands.entity(e).insert(ThroughDoor { t: 0.0 });
            events.write(KeypadEvent::Open(at));
            let mode = active.level.mode;
            playing.0 = Some(mode);
            music.write(ModeMusic(Some(mode)));
            shout.write(ToyShout { pos: at + Vec2::Y * 2.6 * TILE, text: "OPEN!" });
        } else {
            door.since_wrong = 0.0;
            pad.buzzed = Some(0.0);
            events.write(KeypadEvent::Wrong(at));
            shout.write(ToyShout { pos: at + Vec2::Y * 2.6 * TILE, text: "WRONG CODE!" });
        }
    } else if pad.idle > KEYPAD_IDLE {
        commands.entity(e).remove::<AtKeypad>();
    }
}

/// Through the open door: a moment later, out in the secret room.
#[allow(clippy::type_complexity)]
fn through_door(
    mut commands: Commands,
    time: Res<Time>,
    active: Res<ActiveLevel>,
    mut nat: Query<(Entity, &mut Pos, &mut PrevPos, &mut Body, &mut PlayerControl, &mut ThroughDoor), With<Player>>,
    mut doors: Query<&mut CodeDoor>,
    mut shout: MessageWriter<ToyShout>,
) {
    for mut d in &mut doors {
        d.since_wrong += time.delta_secs();
    }
    let Ok((e, mut pos, mut prev, mut body, mut ctl, mut through)) = nat.single_mut() else { return };
    body.vel = Vec2::ZERO;
    through.t += time.delta_secs();
    if through.t < DOOR_SECS {
        return;
    }
    commands.entity(e).remove::<ThroughDoor>();
    let Some(room) = active.level.things.iter().find(|t| t.kind == ThingKind::SecretRoom) else { return };
    let at = cell_floor(&active.level, room.col, room.row) + Vec2::Y * (body.half.y + 0.5);
    pos.0 = at;
    prev.0 = at;
    body.on_ground = false;
    body.riding = None;
    *ctl = PlayerControl { facing: ctl.facing, has_toot: true, ..PlayerControl::default() };
    let words = match active.level.mode {
        RoomMode::Funk => "FUNK MODE!",
        RoomMode::Bossa => "BOSSA MODE!",
        RoomMode::Samba => "SAMBA MODE!",
        RoomMode::Rock => "ROCK MODE!",
    };
    shout.write(ToyShout { pos: at + Vec2::Y * 2.0 * TILE, text: words });
}

/// A death at the keypad or in the door: off it.
fn leave_keypad(mut commands: Commands, nat: Query<Entity, (With<Player>, With<Dead>, Or<(With<AtKeypad>, With<ThroughDoor>)>)>) {
    for e in &nat {
        commands.entity(e).remove::<(AtKeypad, ThroughDoor)>();
    }
}

/// The mode's over at a level start, a respawn before the door, or out of play.
fn mode_off(
    state: Res<State<AppState>>,
    mut started: MessageReader<LevelStarted>,
    mut respawned: MessageReader<PlayerRespawned>,
    doors: Query<&CodeDoor>,
    mut playing: ResMut<RoomPlaying>,
    mut music: MessageWriter<ModeMusic>,
) {
    let restart = started.read().count() > 0;
    let back = respawned.read().count() > 0 && !doors.iter().any(|d| d.open);
    if playing.0.is_some() && (restart || back || *state.get() != AppState::Playing) {
        playing.0 = None;
        music.write(ModeMusic(None));
    }
}

pub(super) fn spawn_paper(commands: &mut Commands, level: &Level, col: usize, row: usize) {
    commands.spawn((Name::new("CodePaper"), LevelEntity, CodePaper, Transform::from_translation(level.tile_center(col, row).extend(2.0))));
}

pub(super) fn spawn_door(commands: &mut Commands, level: &Level, col: usize, row: usize) {
    commands.spawn((Name::new("CodeDoor"), LevelEntity, CodeDoor::default(), Transform::from_translation(cell_floor(level, col, row).extend(1.8))));
}

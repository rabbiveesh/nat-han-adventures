//! Houses (`A` `K` `a`): Up at the door and Nat goes in. Who's home decides how he comes out:
//! - nobody ([`Cook::Never`], and a sometimes house with the chef out): a moment later he
//!   shoots out of the chimney ([`CHIMNEY_SPEED`]), up to wherever that reaches;
//! - the chef ([`Cook::Always`], and a sometimes house every other [`HOUSE_SHIFT`] s):
//!   "SIZZLE!" and out of the door he flies, breaded and fried: a schnitzel (a death,
//!   [`DeathCause::Schnitzel`]).
//!
//! The chef home shows: smoke from the chimney, a lit window, and "COME IN!" when Nat's near.
//! Nat indoors is out of the physics' hands and out of sight ([`Indoors`]).

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::hazards::Doomed;
use super::physics::{Body, Dead, Finished, PlayerControl};
use super::{GameSet, LevelEntity, Player, Pos, PrevPos, SimClock, cell_floor};
use crate::events::{DeathCause, ToyShout};
use crate::input::Action;
use crate::level::{CHIMNEY_SPEED, Cook, HOUSE_SHIFT, Level, TILE};

pub(super) fn plugin(app: &mut App) {
    app.register_type::<House>()
        .add_message::<HouseEvent>()
        .add_systems(FixedUpdate, go_in.in_set(GameSet::Player).before(super::physics::player_step))
        .add_systems(FixedUpdate, (indoors, chef_calls).chain().in_set(GameSet::Interact).before(super::hazards::check_hazards));
}

/// Nat's this long (s) in a house.
pub const INDOORS_SECS: f32 = 1.0;
/// The chimney's top: this far right of the door and this high over the floor (px).
pub const CHIMNEY: Vec2 = Vec2::new(5.0, 40.0);
/// Nat goes in at a door within this much (px) of its middle.
const DOOR_HALF: f32 = 6.0;
/// The chef calls a Nat this near (px)...
const CALL_REACH: f32 = 5.0 * TILE;
/// ...this often (s).
const CALL_EVERY: f32 = 2.5;

/// A house. Its transform is the door's floor (its cell's floor).
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct House {
    pub cook: Cook,
    pub since_call: f32,
}

impl House {
    /// Is the chef in at play time `t`?
    pub fn chef_home(&self, t: f32) -> bool {
        chef_home(self.cook, t)
    }
}

/// Is a `cook` house's chef in at play time `t`? A sometimes house's is out first.
pub fn chef_home(cook: Cook, t: f32) -> bool {
    match cook {
        Cook::Never => false,
        Cook::Always => true,
        Cook::Sometimes => (t / HOUSE_SHIFT).floor() as i64 % 2 == 1,
    }
}

/// On Nat while he's in `house`, `t` s so far; `chef` if the chef was in.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Indoors {
    pub house: Entity,
    pub t: f32,
    pub chef: bool,
}

/// What happened at a house (for sounds and smoke).
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub enum HouseEvent {
    /// In at the door at `pos`.
    In(Vec2),
    /// The chef's at him (`pos`: the door).
    Sizzle(Vec2),
    /// Out of the chimney at `pos`.
    Chimney(Vec2),
}

/// Up at a door: in he goes.
#[allow(clippy::type_complexity)]
fn go_in(
    mut commands: Commands,
    clock: Res<SimClock>,
    input: Single<&ActionState<Action>>,
    mut nat: Query<(Entity, &Pos, &mut Body), (With<Player>, Without<Dead>, Without<Finished>, Without<Indoors>, Without<super::boom::Aboard>, Without<super::helper::OnRope>)>,
    houses: Query<(Entity, &House, &Transform)>,
    mut events: MessageWriter<HouseEvent>,
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
    for (he, house, tf) in &houses {
        let door = tf.translation.truncate();
        if (pos.0.x - door.x).abs() > DOOR_HALF || (feet - door.y).abs() > 3.0 {
            continue;
        }
        let chef = house.chef_home(clock.time);
        body.vel = Vec2::ZERO;
        commands.entity(e).insert(Indoors { house: he, t: 0.0, chef });
        events.write(HouseEvent::In(door));
        shout.write(ToyShout { pos: door + Vec2::Y * 3.6 * TILE, text: if chef { "DINNER TIME!" } else { "HELLO?" } });
        return;
    }
}

/// Inside: the chef fries him, or nobody's home and out the chimney he goes.
#[allow(clippy::type_complexity)]
fn indoors(
    mut commands: Commands,
    time: Res<Time>,
    mut nat: Query<(Entity, &mut Pos, &mut PrevPos, &mut Body, &mut PlayerControl, &mut Indoors), With<Player>>,
    houses: Query<&Transform, With<House>>,
    mut events: MessageWriter<HouseEvent>,
    mut shout: MessageWriter<ToyShout>,
) {
    let Ok((e, mut pos, mut prev, mut body, mut ctl, mut inside)) = nat.single_mut() else { return };
    let Ok(tf) = houses.get(inside.house) else {
        commands.entity(e).remove::<Indoors>();
        return;
    };
    let door = tf.translation.truncate();
    // Out of the way of everything while he's in.
    pos.0 = door + Vec2::Y * body.half.y;
    prev.0 = pos.0;
    body.vel = Vec2::ZERO;
    let was = inside.t;
    inside.t += time.delta_secs();
    if inside.chef && was < INDOORS_SECS * 0.4 && inside.t >= INDOORS_SECS * 0.4 {
        events.write(HouseEvent::Sizzle(door));
        shout.write(ToyShout { pos: door + Vec2::Y * 3.6 * TILE, text: "SIZZLE!" });
    }
    if inside.t < INDOORS_SECS {
        return;
    }
    commands.entity(e).remove::<Indoors>();
    if inside.chef {
        commands.entity(e).insert(Doomed(DeathCause::Schnitzel));
        return;
    }
    let top = door + CHIMNEY + Vec2::Y * (body.half.y + 1.0);
    pos.0 = top;
    prev.0 = top;
    body.vel = Vec2::new(0.0, CHIMNEY_SPEED);
    body.on_ground = false;
    body.riding = None;
    *ctl = PlayerControl { facing: ctl.facing, has_toot: true, ..PlayerControl::default() };
    events.write(HouseEvent::Chimney(door + CHIMNEY));
    shout.write(ToyShout { pos: top + Vec2::Y * 12.0, text: "WHEEE!" });
}

/// "COME IN!": the chef's home and Nat's near.
fn chef_calls(
    time: Res<Time>,
    clock: Res<SimClock>,
    nat: Query<&Pos, (With<Player>, Without<Dead>, Without<Indoors>)>,
    mut houses: Query<(&mut House, &Transform)>,
    mut shout: MessageWriter<ToyShout>,
) {
    let nat = nat.single().ok();
    for (mut h, tf) in &mut houses {
        h.since_call += time.delta_secs();
        let door = tf.translation.truncate();
        if h.chef_home(clock.time) && h.since_call >= CALL_EVERY && nat.is_some_and(|p| p.0.distance(door) < CALL_REACH) {
            h.since_call = 0.0;
            shout.write(ToyShout { pos: door + Vec2::new(-10.0, 2.2 * TILE), text: "COME IN!" });
        }
    }
}

pub(super) fn spawn_house(commands: &mut Commands, level: &Level, col: usize, row: usize, cook: Cook) {
    commands.spawn((
        Name::new("House"),
        LevelEntity,
        House { cook, since_call: CALL_EVERY },
        Transform::from_translation(cell_floor(level, col, row).extend(-0.5)),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sometimes_chef_is_out_first() {
        assert!(!chef_home(Cook::Sometimes, 0.0));
        assert!(chef_home(Cook::Sometimes, HOUSE_SHIFT + 0.1));
        assert!(!chef_home(Cook::Sometimes, 2.0 * HOUSE_SHIFT + 0.1));
        assert!(chef_home(Cook::Always, 0.0) && !chef_home(Cook::Never, 99.0));
    }
}

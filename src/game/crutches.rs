//! The crutch pit (`crutchpit:` in a level file): the other trick gap. A WET FLOOR sign stands at
//! its bottom, and it has one: Nat lands on it with a CRACK instead of splatting. OUCH: both legs
//! broken. The platformer freezes ([`GameSet`] doesn't run while the [`CrutchRescue`] is up)
//! while Han's plunger drops down on its rope, sticks to Nat's head (SCHLORP) and hauls him out
//! to the ledge before the pit; then St. Plunger's Hospital (a card, `ui::crutches`) hands him
//! crutches. On crutches ([`OnCrutches`]) Nat can't fly (no toot), walks slow and jumps small,
//! until his next death heals him: which, on crutches, comes soon enough. Not a death itself:
//! no `PlayerDied`, the adaptive engine never hears of it.

use bevy::prelude::*;

use super::physics::Body;
use super::robot::ledge_before;
use super::{ActiveLevel, GameSet, LevelEntity, Player, Pos, PrevPos, RestartLevel};
use crate::audio::Sfx;
use crate::events::{HanSays, PlaySfx, PlayerDied};
use crate::level::{Level, TILE};
use crate::state::{AppState, PlayState};

/// Lying at the bottom, legs broken (s).
pub const OUCH_SECS: f32 = 1.2;
/// The plunger coming down on its rope (s).
pub const LOWER_SECS: f32 = 0.9;
/// Stuck on: a beat before the haul (s).
pub const STUCK_SECS: f32 = 0.35;
/// Hauled up and over to the ledge (s).
pub const HAUL_SECS: f32 = 1.3;
/// The hospital card (s).
pub const HOSPITAL_SECS: f32 = 3.6;
/// The plunger's rope comes from this far over Nat (px): off the top of the screen.
pub const ROPE_FROM: f32 = 10.0 * TILE;
/// How high over the ledge the haul's arc peaks (px).
const HAUL_PEAK: f32 = 3.0 * TILE;
/// On crutches: run speed and ground-jump speed, as fractions of Nat's own.
pub const CRUTCH_RUN: f32 = 0.5;
pub const CRUTCH_JUMP: f32 = 0.8;
/// Han's lines: down in the pit, and once Nat's back on crutches (barks: at most 3 words).
pub const RESCUE_LINE: &str = "Hang on, Nat!";
pub const CAREFUL_LINE: &str = "Careful now!";

/// Nat's in the crutch pit: the rescue's under way.
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct CrutchRescue {
    pub phase: RescuePhase,
    /// Seconds in this phase.
    pub t: f32,
    /// Where Nat lies (his middle), and where he's hauled to (the ledge before the pit).
    pub from: Vec2,
    pub to: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum RescuePhase {
    /// OUCH.
    Ouch,
    /// The plunger comes down.
    Lower,
    /// SCHLORP: stuck on his head.
    Stuck,
    /// Up and over to the ledge.
    Haul,
    /// St. Plunger's Hospital.
    Hospital,
}

/// Nat's on crutches: no toot, slow, small jumps (`physics::player_step`). His next death heals him.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct OnCrutches;

/// Nat landed in a crutch pit (from `hazards`, instead of splatting).
#[derive(Message, Debug, Clone, Copy)]
pub struct FellInCrutchPit {
    pub pit: (usize, usize),
}

/// Han's plunger on its rope, coming for Nat. Pos is the cup's bottom middle.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct RescuePlunger;

/// The WET FLOOR sign at the bottom of a crutch pit (the tell). Transform only.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct WetFloorSign;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<CrutchRescue>()
        .register_type::<OnCrutches>()
        .add_message::<FellInCrutchPit>()
        .add_systems(Update, (spawn_signs, leave_on_restart, heal_on_death))
        .add_systems(FixedUpdate, fall_in.after(GameSet::Interact).before(GameSet::Follow))
        .add_systems(
            FixedUpdate,
            run_rescue.run_if(in_state(PlayState::Running).and_then(resource_exists::<CrutchRescue>)),
        )
        .add_systems(OnExit(AppState::Playing), |mut commands: Commands| commands.remove_resource::<CrutchRescue>());
}

/// Which crutch pit (if any) holds x (world px).
pub fn pit_at(level: &Level, x: f32) -> Option<(usize, usize)> {
    let col = (x / TILE).floor();
    level.crutch_pits.iter().copied().find(|&(c0, c1)| col >= c0 as f32 && col <= c1 as f32)
}

impl CrutchRescue {
    /// Where the plunger's cup is (its bottom middle), if it's out.
    pub fn plunger(&self) -> Option<Vec2> {
        let head = |p: Vec2| p + Vec2::Y * Body::player().half.y;
        match self.phase {
            RescuePhase::Ouch | RescuePhase::Hospital => None,
            RescuePhase::Lower => {
                let k = ease(self.t / LOWER_SECS);
                Some(head(self.from) + Vec2::Y * ROPE_FROM * (1.0 - k))
            }
            RescuePhase::Stuck => Some(head(self.from)),
            RescuePhase::Haul => Some(head(self.nat())),
        }
    }

    /// Where Nat is (his middle).
    pub fn nat(&self) -> Vec2 {
        match self.phase {
            RescuePhase::Haul => {
                let k = ease(self.t / HAUL_SECS);
                let peak = self.from.y.max(self.to.y) + HAUL_PEAK;
                let x = self.from.x + (self.to.x - self.from.x) * k;
                // Up out of the pit, over, and down onto the ledge: a parabola through the peak.
                let y = if k < 0.5 {
                    self.from.y + (peak - self.from.y) * (1.0 - (1.0 - 2.0 * k).powi(2))
                } else {
                    self.to.y + (peak - self.to.y) * (1.0 - (2.0 * k - 1.0).powi(2))
                };
                Vec2::new(x, y)
            }
            RescuePhase::Hospital => self.to,
            _ => self.from,
        }
    }

    fn set(&mut self, phase: RescuePhase) {
        self.phase = phase;
        self.t = 0.0;
    }
}

fn ease(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

fn spawn_signs(mut commands: Commands, active: Option<Res<ActiveLevel>>, signs: Query<(), With<WetFloorSign>>) {
    let Some(active) = active else { return };
    if active.level.crutch_pits.is_empty() || !signs.is_empty() {
        return;
    }
    for &(c0, c1) in &active.level.crutch_pits {
        let at = Vec2::new((c0 + c1 + 1) as f32 * TILE / 2.0, TILE / 2.0);
        commands.spawn((Name::new("WetFloorSign"), LevelEntity, WetFloorSign, Transform::from_translation(at.extend(2.0))));
    }
}

/// A landing in a crutch pit: OUCH, and the rescue begins.
fn fall_in(
    mut commands: Commands,
    mut fell: MessageReader<FellInCrutchPit>,
    active: Option<Res<ActiveLevel>>,
    rescue: Option<Res<CrutchRescue>>,
    mut nat: Query<(&mut Pos, &mut PrevPos, &mut Body), With<Player>>,
    mut sfx: MessageWriter<PlaySfx>,
    mut says: MessageWriter<HanSays>,
) {
    let Some(f) = fell.read().last() else { return };
    let (Some(active), None, Ok((mut pos, mut prev, mut body))) = (active, rescue, nat.single_mut()) else { return };
    // On the pit's floor (its bottom edge), legs broken.
    pos.0.y = body.half.y;
    prev.0 = pos.0;
    *body = Body::player();
    body.on_ground = true;
    sfx.write(PlaySfx(Sfx::Splat));
    says.write(HanSays { text: RESCUE_LINE.to_string() });
    commands.insert_resource(CrutchRescue {
        phase: RescuePhase::Ouch,
        t: 0.0,
        from: pos.0,
        to: ledge_before(&active.level, f.pit),
    });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn run_rescue(
    mut commands: Commands,
    time: Res<Time>,
    mut rescue: ResMut<CrutchRescue>,
    mut nat: Query<(Entity, &mut Pos, &mut PrevPos, &mut Body), (With<Player>, Without<RescuePlunger>)>,
    mut plunger: Query<(Entity, &mut Pos, &mut PrevPos), With<RescuePlunger>>,
    mut sfx: MessageWriter<PlaySfx>,
    mut says: MessageWriter<HanSays>,
) {
    let Ok((entity, mut pos, mut prev, mut body)) = nat.single_mut() else { return };
    rescue.t += time.delta_secs();
    match rescue.phase {
        RescuePhase::Ouch if rescue.t >= OUCH_SECS => {
            rescue.set(RescuePhase::Lower);
            let at = rescue.plunger().unwrap_or(rescue.from);
            commands.spawn((Name::new("RescuePlunger"), LevelEntity, RescuePlunger, Pos(at), PrevPos(at), Transform::from_translation(at.extend(4.5))));
        }
        RescuePhase::Lower if rescue.t >= LOWER_SECS => {
            sfx.write(PlaySfx(Sfx::Boing));
            rescue.set(RescuePhase::Stuck);
        }
        RescuePhase::Stuck if rescue.t >= STUCK_SECS => rescue.set(RescuePhase::Haul),
        RescuePhase::Haul if rescue.t >= HAUL_SECS => {
            for (e, _, _) in &plunger {
                commands.entity(e).despawn();
            }
            sfx.write(PlaySfx(Sfx::Land));
            rescue.set(RescuePhase::Hospital);
        }
        RescuePhase::Hospital if rescue.t >= HOSPITAL_SECS => {
            commands.entity(entity).insert(OnCrutches);
            commands.remove_resource::<CrutchRescue>();
            *body = Body::player();
            pos.0 = rescue.to;
            prev.0 = pos.0;
            says.write(HanSays { text: CAREFUL_LINE.to_string() });
            return;
        }
        _ => {}
    }
    prev.0 = pos.0;
    pos.0 = rescue.nat();
    body.vel = Vec2::ZERO;
    body.on_ground = rescue.phase != RescuePhase::Haul;
    if let (Some(at), Ok((_, mut p, mut pp))) = (rescue.plunger(), plunger.single_mut()) {
        pp.0 = p.0;
        p.0 = at;
    }
}

/// Dying heals: the crutches go.
fn heal_on_death(mut commands: Commands, mut died: MessageReader<PlayerDied>, nat: Query<Entity, (With<Player>, With<OnCrutches>)>) {
    if died.read().count() == 0 {
        return;
    }
    for e in &nat {
        commands.entity(e).remove::<OnCrutches>();
    }
}

/// A restart (pause menu) ends the rescue: the level's reloaded, Nat whole again.
fn leave_on_restart(mut commands: Commands, mut restart: MessageReader<RestartLevel>) {
    if restart.read().count() > 0 {
        commands.remove_resource::<CrutchRescue>();
    }
}

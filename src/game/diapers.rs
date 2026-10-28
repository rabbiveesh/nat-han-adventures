//! Sploosh diapers (`D`): little winged diapers flapping back and forth across the sky,
//! [`DIAPER_RANGE`] tiles either side of their cell. A diaper drops when it would land on
//! Nat's head: it leads him, aiming where he'll be if he keeps on, and lets go with a
//! whistle; stop or turn back (or, on the Boom, swerve) and it lands in front of him. Deadly
//! while it flies and while it falls; it splooshes on whatever it lands on and flaps back home
//! [`DIAPER_BACK`] s later. Under Giant Steps it falls as floaty as Nat does.

use bevy::prelude::*;

use super::physics::{Body, Dead, Finished};
use super::{ActiveLevel, GameSet, Groove, LevelEntity, Player, Pos, PrevPos};
use crate::level::{DIAPER_BACK, DIAPER_RANGE, DIAPER_SPEED, Level, TILE};

pub(super) fn plugin(app: &mut App) {
    app.add_message::<Splooshed>().add_message::<DiaperDropped>().add_systems(FixedUpdate, flap_and_drop.in_set(GameSet::World));
}

/// A diaper's hitbox (half extents, px).
pub const DIAPER_HALF: Vec2 = Vec2::new(5.0, 4.0);
/// It falls under this gravity (px/s², × the band's)...
pub const DIAPER_GRAVITY: f32 = 700.0;
/// ...to this speed (px/s).
pub const DIAPER_MAX_FALL: f32 = 260.0;
/// It keeps this share of its flapping speed as it falls.
pub const DIAPER_CARRY: f32 = 0.5;
/// It drops when its landing spot is this close (px) to where Nat's head will be.
pub const DIAPER_AIM: f32 = 6.0;
/// It only drops on Nat this many tiles below it at most.
pub const DIAPER_SIGHT: f32 = 10.0;
/// Back home, it flaps this long (s) before it can drop again.
pub const DIAPER_RELOAD: f32 = 0.8;

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Diaper {
    /// Its cell's center: it flaps either side of it.
    pub home: Vec2,
    /// Seconds flapping since it last came home (sets where it is in its swing).
    pub t: f32,
    pub phase: f32,
    pub state: DiaperState,
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub enum DiaperState {
    Flapping,
    Falling { vel: Vec2 },
    /// Splooshed `t` s ago (harmless; gone till it's back).
    Splooshed { t: f32 },
}

impl Diaper {
    /// Is it deadly now?
    pub fn deadly(&self) -> bool {
        !matches!(self.state, DiaperState::Splooshed { .. })
    }
}

/// A diaper landed (for the SPLOOSH).
#[derive(Message, Debug, Clone, Copy)]
pub struct Splooshed {
    pub pos: Vec2,
}

/// A diaper let go (for its whistle).
#[derive(Message, Debug, Clone, Copy)]
pub struct DiaperDropped {
    pub pos: Vec2,
}

/// Where a flapping diaper is, and its velocity, `t` s into its swing.
pub fn swing(home: Vec2, t: f32, phase: f32) -> (Vec2, Vec2) {
    let r = DIAPER_RANGE * TILE;
    let w = DIAPER_SPEED / r;
    let a = w * t + phase * std::f32::consts::TAU;
    let bob = (t * 5.0).sin() * 2.0;
    (home + Vec2::new(r * a.sin(), bob), Vec2::new(r * w * a.cos(), 0.0))
}

/// Seconds to fall `h` px from rest under `g`, at most [`DIAPER_MAX_FALL`].
pub fn fall_time(h: f32, g: f32) -> f32 {
    let t_max = DIAPER_MAX_FALL / g;
    let d_max = 0.5 * g * t_max * t_max;
    if h <= d_max { (2.0 * h / g).sqrt() } else { t_max + (h - d_max) / DIAPER_MAX_FALL }
}

/// Is the column at `x` clear of anything solid or one-way between heights `lo` and `hi`?
fn clear_drop(level: &Level, x: f32, lo: f32, hi: f32) -> bool {
    let col = (x / TILE).floor() as i32;
    let mut y = hi;
    while y > lo {
        let (_, row) = level.cell_at(Vec2::new(x, y));
        let t = level.tile(col, row);
        if t.is_solid() || t.is_one_way() {
            return false;
        }
        y -= TILE / 2.0;
    }
    true
}

/// Does a diaper's box at `p` sit in (or on) anything it lands on?
fn landed(level: &Level, p: Vec2) -> bool {
    if p.y < -TILE {
        return true;
    }
    let bottom = p.y - DIAPER_HALF.y;
    [p.x - DIAPER_HALF.x + 0.5, p.x, p.x + DIAPER_HALF.x - 0.5].into_iter().any(|x| {
        let (c, r) = level.cell_at(Vec2::new(x, bottom));
        let t = level.tile(c, r);
        t.is_solid() || t.is_one_way() || t == crate::level::Tile::Liquid
    })
}

#[allow(clippy::type_complexity)]
fn flap_and_drop(
    time: Res<Time>,
    active: Res<ActiveLevel>,
    groove: Res<Groove>,
    nat: Query<(&Pos, &Body, Option<&super::boom::Aboard>), (With<Player>, Without<Dead>, Without<Finished>, Without<Diaper>)>,
    mut diapers: Query<(&mut Diaper, &mut Pos), Without<Player>>,
    mut splooshed: MessageWriter<Splooshed>,
    mut dropped: MessageWriter<DiaperDropped>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let level = &active.level;
    let g = DIAPER_GRAVITY * groove.gravity_now();
    let target = nat.single().ok().map(|(p, b, aboard)| (p.0, aboard.map_or(b.vel + Vec2::new(b.launch, 0.0), |a| a.vel), b.half));
    for (mut d, mut pos) in &mut diapers {
        match d.state {
            DiaperState::Flapping => {
                d.t += dt;
                let (p, v) = swing(d.home, d.t, d.phase);
                pos.0 = p;
                let Some((nat, nat_vel, half)) = target else { continue };
                let head = nat.y + half.y;
                let h = p.y - DIAPER_HALF.y - head;
                if d.t < DIAPER_RELOAD || h <= 0.0 || h > DIAPER_SIGHT * TILE {
                    continue;
                }
                let fall = fall_time(h, g);
                let lands = p.x + v.x * DIAPER_CARRY * fall;
                let there = nat.x + nat_vel.x * fall;
                if (lands - there).abs() < DIAPER_AIM && clear_drop(level, p.x, head, p.y - DIAPER_HALF.y) {
                    d.state = DiaperState::Falling { vel: Vec2::new(v.x * DIAPER_CARRY, 0.0) };
                    dropped.write(DiaperDropped { pos: p });
                }
            }
            DiaperState::Falling { mut vel } => {
                vel.y = (vel.y - g * dt).max(-DIAPER_MAX_FALL);
                pos.0 += vel * dt;
                d.state = DiaperState::Falling { vel };
                if landed(level, pos.0) {
                    d.state = DiaperState::Splooshed { t: 0.0 };
                    splooshed.write(Splooshed { pos: pos.0 - Vec2::new(0.0, DIAPER_HALF.y) });
                }
            }
            DiaperState::Splooshed { t } => {
                let t = t + dt;
                d.state = DiaperState::Splooshed { t };
                if t >= DIAPER_BACK {
                    d.state = DiaperState::Flapping;
                    d.t = 0.0;
                    pos.0 = swing(d.home, 0.0, d.phase).0;
                }
            }
        }
    }
}

/// A diaper flapping about cell (col, row)'s center.
pub(super) fn spawn_diaper(commands: &mut Commands, center: Vec2, phase: f32) {
    let start = swing(center, 0.0, phase).0;
    commands.spawn((
        Name::new("Diaper"),
        LevelEntity,
        Diaper { home: center, t: 0.0, phase, state: DiaperState::Flapping },
        Pos(start),
        PrevPos(start),
        Transform::from_translation(start.extend(2.0)),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_take_longer_from_higher() {
        let g = DIAPER_GRAVITY;
        assert!(fall_time(16.0, g) < fall_time(64.0, g));
        assert!(fall_time(400.0, g) > fall_time(200.0, g));
        let (p, _) = swing(Vec2::ZERO, 0.0, 0.0);
        assert_eq!(p.x, 0.0, "starts at home");
    }
}

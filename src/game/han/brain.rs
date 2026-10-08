//! Han's brain: what he wants each step ([`HanInput`]), and his body step ([`drive`]).
//!
//! Modes ([`HanMode`]): **Follow** (routes on the nav graph to his slot behind Nat; braces when
//! Nat comes down near him; goes for an intercept when Nat's in the air over a drop), **Intercept**
//! (the goalkeeper reflex), **Ahead** ("Lemme check that": a determined march into a hazard),
//! **Parachute** (lost, or back from the sewage), **Sinking** / **Gone** (splatted in sewage).

use bevy::prelude::*;

use super::{
    BACK_WARN_LINE, FALL_BEHIND_SECS, FOLLOW_GAP, GRUMBLE_EVERY, HAN_CATCH_UP, HanAnim, HanPose, LEMME_LINE,
    LOOK_AHEAD, NERVOUS_GAP, PARACHUTE_FALL, PARACHUTE_HEIGHT, PRO_LINE, REPLAN_SECS, STUCK_SECS, UNION_LINE,
    WHEEZE_LINE, brace_range, breather, go_ahead_delay, grumble_line, intercept_range, overuse_limit,
};
use crate::audio::Harmony;
use crate::events::HanSays;
use crate::game::{
    ActiveLevel, Assists, Body, Carrier, Dead, Fall, Groove, Han, HanBoosted, HanHead, MovingPlatform, Player,
    PlayerControl, Pos, PrevPos, SimClock, platform_pos,
};
use crate::level::buddy::{HALF, HanCtl, HanInput, HanPhys, Kin, drive, head_holds, intercept, waltz_step};
use crate::level::nav::{Edge, HanMove, Nav, Route, Step, take_off};
use crate::level::validate::{Map, Mode};
use crate::level::{Level, PlatformKind, TILE, ThingKind, Tile};

/// Han's routes for the level visit (one graph per physics he's had: normal, Giant Steps).
#[derive(Resource)]
pub struct HanNav {
    pub normal: Nav,
    pub giant: Option<Nav>,
}

impl Default for HanNav {
    fn default() -> Self {
        HanNav { normal: Nav::new(Mode::Normal), giant: None }
    }
}

impl HanNav {
    fn for_groove(&mut self, groove: &Groove) -> &mut Nav {
        if groove.harmony == Harmony::Coltrane {
            self.giant.get_or_insert_with(|| Nav::new(Mode::GiantSteps))
        } else {
            &mut self.normal
        }
    }

    fn invalidate(&mut self) {
        self.normal.invalidate();
        if let Some(g) = &mut self.giant {
            g.invalidate();
        }
    }
}

/// Cells (not yet simulated) a route search may simulate per step: a few ms at most.
const ROUTE_BUDGET: usize = 24;

#[derive(Debug, Clone, Copy, Default, PartialEq, Reflect)]
pub enum HanMode {
    #[default]
    Follow,
    Intercept,
    /// Marching ahead into a hazard, heading `dir`.
    Ahead {
        dir: f32,
        t: f32,
    },
    /// Floating down; `pro`: back from the sewage ("I'm fine! I'm a professional!").
    Parachute {
        pro: bool,
    },
    Sinking {
        t: f32,
    },
    Gone {
        t: f32,
    },
}

/// A jump edge being carried out: walk to the take-off (`x0`), then replay the move.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct Exec {
    pub mv: u16,
    /// The cell it takes off from, and the one it lands in.
    pub from: (i32, i32),
    pub to: (i32, i32),
    pub x0: f32,
    pub t: f32,
    pub flying: bool,
    pub wait: f32,
}

/// Lines Han says once per level visit.
mod once {
    pub const WHEEZE: u32 = 2;
}

#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct HanBrain {
    pub mode: HanMode,
    pub ctl: HanCtl,
    #[reflect(ignore)]
    pub path: Vec<Edge>,
    pub exec: Option<Exec>,
    /// Where he's routing to (his slot's cell).
    pub goal: Option<(i32, i32)>,
    pub replan: f32,
    /// No progress toward the goal for this long (s); closest he's been (px).
    pub stuck: f32,
    pub best: f32,
    /// No route / stuck: he'll parachute in once out of sight.
    pub lost: bool,
    /// Which side of Nat his slot is (-1: left, behind a Nat heading right).
    pub side: f32,
    /// Nat's standing cell (last time Nat was on the ground).
    pub nat_cell: Option<(i32, i32)>,
    /// Nat standing still facing a hazard for this long (s).
    pub still: f32,
    /// Boosts in a row (outside chain chasms), time since the last, Nat on the ground since.
    pub streak: u32,
    pub since_boost: f32,
    pub nat_grounded: f32,
    pub warned: bool,
    /// Breather left (s): no boosts.
    pub winded: f32,
    /// Boosts given this level visit (stats, tests).
    pub boosts: u32,
    /// Parachute drops this level visit (stats, tests).
    pub drops: u32,
    pub said: u32,
    /// Braced for Nat this step.
    pub braced: bool,
    /// Where his feet were (world y) when he last stood on something: in mid-air his head holds
    /// Nat only [`HAN_CATCH_RISE`](crate::level::buddy::HAN_CATCH_RISE) above it.
    pub floor_y: f32,
    /// Weak boosts given this level visit (stats, tests), grumbles said, and seconds since the
    /// last grumble.
    pub weak_boosts: u32,
    pub grumbles: u32,
    pub since_grumble: f32,
    /// The slot moved: a new route is wanted (the old one is kept until it's in).
    pub reroute: bool,
    /// Seconds he's been far behind Nat and out of sight (see [`FALL_BEHIND_SECS`]).
    pub behind: f32,
    /// How far (px, sideways) Nat was last step.
    pub gap: f32,
}

impl Default for HanBrain {
    fn default() -> Self {
        HanBrain {
            mode: HanMode::Follow,
            ctl: HanCtl::default(),
            path: Vec::new(),
            exec: None,
            goal: None,
            replan: 0.0,
            stuck: 0.0,
            best: f32::INFINITY,
            lost: false,
            side: -1.0,
            nat_cell: None,
            still: 0.0,
            streak: 0,
            since_boost: f32::INFINITY,
            nat_grounded: 0.0,
            warned: false,
            winded: 0.0,
            boosts: 0,
            drops: 0,
            said: 0,
            braced: false,
            floor_y: 0.0,
            weak_boosts: 0,
            grumbles: 0,
            since_grumble: f32::INFINITY,
            reroute: false,
            behind: 0.0,
            gap: 0.0,
        }
    }
}

/// The cell a box at `pos` stands in (its feet's cell).
pub fn feet_cell(level: &Level, pos: Vec2) -> (i32, i32) {
    level.cell_at(pos - Vec2::new(0.0, HALF.y - 1.0))
}

/// The nav cell Han stands in: his feet's cell, or, when he's standing on the very edge of a
/// ledge with his middle out over the drop, the cell of the ledge under his feet.
pub fn nav_cell(level: &Level, pos: Vec2) -> (i32, i32) {
    let feet = feet_cell(level, pos);
    if floor_at(level, feet.0, feet.1 + 1) {
        return feet;
    }
    [-1.0, 1.0]
        .into_iter()
        .map(|s| feet_cell(level, pos + Vec2::new(s * (HALF.x - 1.0), 0.0)))
        .find(|c| floor_at(level, c.0, c.1 + 1))
        .unwrap_or(feet)
}

/// Box center standing in cell `c` (world).
pub fn stand(level: &Level, c: (i32, i32)) -> Vec2 {
    crate::level::buddy::stand(level, c)
}

fn floor_at(level: &Level, c: i32, r: i32) -> bool {
    let t = level.tile(c, r);
    t.is_solid() || t.is_one_way()
}

/// Below `pos` (and a column on, the way Nat's drifting) there's nothing to land on but the
/// pit, sewage or spikes: Nat's heading for a splat.
fn doomed(level: &Level, pos: Vec2, vx: f32) -> bool {
    let (c, r) = level.cell_at(pos);
    let cols = [c, c + if vx > 20.0 { 1 } else if vx < -20.0 { -1 } else { 0 }];
    cols.iter().all(|&cc| {
        let first = (r..level.height as i32).find(|&rr| {
            let t = level.tile(cc, rr + 1);
            floor_at(level, cc, rr + 1) || t.is_deadly()
        });
        first.is_none_or(|rr| level.tile(cc, rr + 1).is_deadly())
    })
}

/// What's ahead of someone standing in `cell` facing `dir`, within [`LOOK_AHEAD`] tiles:
/// sewage, a spray can, a fly swarm.
pub fn hazard_ahead(level: &Level, cell: (i32, i32), dir: f32) -> bool {
    let d = if dir < 0.0 { -1 } else { 1 };
    for k in 1..=LOOK_AHEAD {
        let c = cell.0 + k * d;
        if level.tile(c, cell.1).is_solid() {
            return false;
        }
        if level.tile(c, cell.1 + 1) == Tile::Liquid {
            return true;
        }
        let hit = level.things.iter().any(|t| {
            let (tc, tr) = (t.col as i32, t.row as i32);
            match t.kind {
                ThingKind::Spray => tc == c && (cell.1..=cell.1 + 2).contains(&tr),
                ThingKind::Fly => (tc - c).abs() <= 1 && (cell.1 - 3..=cell.1 + 1).contains(&tr),
                _ => false,
            }
        });
        if hit {
            return true;
        }
    }
    false
}

/// The cell of Han's slot: nearest standable (for Han) cell to `x` around Nat's cell.
fn slot_cell(map: &Map, level: &Level, nat: (i32, i32), x: f32) -> Option<(i32, i32)> {
    let col = (x / TILE).floor() as i32;
    let side = (col - nat.0).signum();
    let mut cands = Vec::new();
    for dc in [0, -side, side, -2 * side, 2 * side] {
        for dr in [0, 1, 2, -1, 3] {
            if (col + dc, nat.1 + dr) != nat {
                cands.push((col + dc, nat.1 + dr));
            }
        }
    }
    cands.push(nat);
    cands.into_iter().find(|&c| Nav::node(map, level, c))
}

/// Where Han's parachute opens: above Nat's area (behind him, as high as the ceiling allows,
/// up to [`PARACHUTE_HEIGHT`]), not over a waltz row or a chute's grease.
pub fn parachute_spot(level: &Level, nat: Vec2, side: f32) -> Vec2 {
    let mut best: Option<(f32, Vec2)> = None;
    // Behind Nat (the way he came) first: the first spot there with room to float down wins;
    // else wherever has the most room.
    for k in [1.5, 3.0, 4.5, 6.0, 0.0, -1.5, -3.0, -4.5, -6.0] {
        let x = nat.x + side * k * TILE;
        if x < TILE || x > level.size_px().x - TILE {
            continue;
        }
        let free = |y: f32| {
            let (c, r) = level.cell_at(Vec2::new(x, y));
            !level.tile(c, r).is_solid() && !level.tile(c, r).is_deadly()
        };
        if !free(nat.y) {
            continue;
        }
        let mut y = nat.y;
        while y < nat.y + PARACHUTE_HEIGHT && free(y + TILE) && free(y + HALF.y + TILE) {
            y += TILE;
        }
        if level.han_keeps_out(level.cell_at(Vec2::new(x, y))) {
            continue;
        }
        let room = y - nat.y;
        if k > 0.0 && room >= 4.0 * TILE {
            return Vec2::new(x, y);
        }
        // Low ceilings everywhere: still behind him if at all possible (ahead may be across
        // something he can't walk back over).
        let score = room + if k > 0.0 { 100.0 * TILE } else { 0.0 };
        if best.is_none_or(|(s, _)| score > s) {
            best = Some((score, Vec2::new(x, y)));
        }
    }
    best.map_or(nat + Vec2::new(0.0, 2.0 * TILE), |(_, p)| p)
}

/// Can Han land on cell `to` in `secs`: static floor, or a moving platform that will be there.
fn landing_ready(
    level: &Level,
    platforms: &[(Vec2, MovingPlatform)],
    clock: &SimClock,
    groove: &Groove,
    to: (i32, i32),
    secs: f32,
) -> bool {
    if floor_at(level, to.0, to.1 + 1) {
        return true;
    }
    let floor_y = (level.height as i32 - 1 - to.1) as f32 * TILE;
    let x = to.0 as f32 * TILE + TILE / 2.0;
    platforms.iter().any(|(now, p)| {
        let at = if p.kind == PlatformKind::Raft {
            *now
        } else {
            platform_pos(p, clock.platform_time + secs * groove.platform_rate())
        };
        let half_w = p.width as f32 * TILE / 2.0;
        // The cell's middle well on it when he gets there.
        (at.y + TILE / 2.0 - floor_y).abs() < 6.0 && (at.x - x).abs() < half_w - 4.0
    })
}

/// Where to steer to land in cell `to`: its middle, or (no floor there, a platform's path) the
/// platform passing over it right now.
fn landing_x(level: &Level, platforms: &[(Vec2, MovingPlatform)], to: (i32, i32), me: f32) -> f32 {
    let x = to.0 as f32 * TILE + TILE / 2.0;
    if floor_at(level, to.0, to.1 + 1) {
        return x;
    }
    let floor_y = (level.height as i32 - 1 - to.1) as f32 * TILE;
    platforms
        .iter()
        .filter(|(at, p)| (at.y + TILE / 2.0 - floor_y).abs() < 10.0 && (at.x - x).abs() < p.width as f32 * TILE)
        .map(|(at, p)| {
            let half = p.width as f32 * TILE / 2.0 - 6.0;
            me.clamp(at.x - half, at.x + half)
        })
        .min_by(|a, b| (a - me).abs().total_cmp(&(b - me).abs()))
        .unwrap_or(x)
}

/// Out of Nat's sight (the camera shows ~±13 tiles across, ±6.75 up and down).
pub fn out_of_sight(han: Vec2, nat: Vec2) -> bool {
    (han.x - nat.x).abs() > 14.0 * TILE || (han.y - nat.y).abs() > 8.0 * TILE
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn think(
    time: Res<Time>,
    active: Res<ActiveLevel>,
    groove: Res<Groove>,
    assists: Res<Assists>,
    clock: Res<SimClock>,
    nav: Option<ResMut<HanNav>>,
    mut commands: Commands,
    player: Query<(Entity, &Pos, &Body, &PlayerControl, Has<Dead>), (With<Player>, Without<Han>)>,
    mut han: Query<
        (Entity, &mut Pos, &mut PrevPos, &mut Body, &mut HanBrain, &mut HanHead, &mut HanAnim),
        (With<Han>, Without<Player>),
    >,
    platforms: Query<(Entity, &Pos, &PrevPos, &MovingPlatform), (Without<Han>, Without<Player>)>,
    mut boosted: MessageReader<HanBoosted>,
    mut says: MessageWriter<HanSays>,
) {
    let dt = time.delta_secs();
    let Some(mut nav) = nav else {
        commands.insert_resource(HanNav::default());
        return;
    };
    if dt <= 0.0 {
        return;
    }
    let level = &active.level;
    if active.is_changed() {
        nav.invalidate(); // a new stain: the ground changed under him
    }
    let Ok((han_e, mut pos, mut prev, mut body, mut brain, mut head, mut anim)) = han.single_mut() else { return };
    let Ok((_, npos, nbody, nctl, nat_dead)) = player.single() else { return };
    let brain = &mut *brain;
    let e = assists.han_eagerness;
    let phys = HanPhys::of(&groove);
    let gs = groove.harmony == Harmony::Coltrane;
    let nat = Kin { pos: npos.0, vel: nbody.vel, grounded: nbody.on_ground };
    let nat_on_han = nbody.riding == Some(han_e);
    let say = |says: &mut MessageWriter<HanSays>, text: &str| {
        says.write(HanSays { text: text.to_string() });
    };

    // --- Overuse: boosts in a row.
    brain.since_grumble += dt;
    for b in boosted.read() {
        if b.weak {
            // The band zone: a feeble hop and a grumble (not his back's business).
            brain.weak_boosts += 1;
            if brain.since_grumble >= GRUMBLE_EVERY {
                let topic = level.band_zone(feet_cell(level, pos.0)).or(level.band_zone(level.cell_at(b.pos))).map(|g| g.topic);
                say(&mut says, grumble_line(topic, brain.grumbles));
                brain.grumbles += 1;
                brain.since_grumble = 0.0;
            }
            continue;
        }
        brain.boosts += 1;
        brain.since_boost = 0.0;
        if !level.in_chasm(level.cell_at(b.pos).0) {
            brain.streak += 1;
        }
    }
    brain.since_boost += dt;
    brain.nat_grounded = if nbody.on_ground && !nat_on_han { brain.nat_grounded + dt } else { 0.0 };
    if brain.nat_grounded > 1.2 || brain.since_boost > 4.0 {
        brain.streak = 0;
        brain.warned = false;
    }
    let limit = overuse_limit(e);
    if brain.streak + 1 == limit && !brain.warned {
        say(&mut says, BACK_WARN_LINE);
        brain.warned = true;
    }
    if brain.streak >= limit {
        say(&mut says, UNION_LINE);
        brain.winded = breather(e);
        brain.streak = 0;
        brain.warned = false;
    }
    brain.winded = (brain.winded - dt).max(0.0);

    let plats: Vec<(Vec2, MovingPlatform)> = platforms.iter().map(|(_, p, _, m)| (p.0, m.clone())).collect();
    let carriers: Vec<Carrier> =
        platforms.iter().map(|(ent, p, pr, m)| Carrier::platform(ent, p.0, pr.0, m.width)).collect();

    // --- Out of the picture: sinking, gone.
    match brain.mode {
        HanMode::Sinking { t } => {
            pos.0.y -= 10.0 * dt;
            body.vel = Vec2::ZERO;
            brain.mode = if t - dt <= 0.0 {
                HanMode::Gone { t: super::HAN_SEWAGE_RESPAWN }
            } else {
                HanMode::Sinking { t: t - dt }
            };
            *head = HanHead::default();
            let fl = anim.facing_left;
            set_anim(&mut anim, HanPose::Splat, fl, &groove, false);
            return;
        }
        HanMode::Gone { t } => {
            *head = HanHead::default();
            body.vel = Vec2::ZERO;
            if t - dt <= 0.0 && !nat_dead {
                start_parachute(level, &mut pos, &mut prev, &mut body, brain, nat.pos, true);
            } else {
                brain.mode = HanMode::Gone { t: t - dt };
                let fl = anim.facing_left;
                set_anim(&mut anim, HanPose::Splat, fl, &groove, true);
                return;
            }
        }
        _ => {}
    }
    if pos.0.y + HALF.y < -2.0 * TILE {
        // Fell out of the level: he'll float back down.
        brain.mode = HanMode::Gone { t: 1.0 };
        return;
    }

    let me = Kin { pos: pos.0, vel: body.vel, grounded: body.on_ground };
    let gap = if groove.grip() { NERVOUS_GAP } else { FOLLOW_GAP };
    if nbody.on_ground && !nat_on_han {
        let cell = feet_cell(level, nat.pos);
        // Nat landed somewhere new while Han has nothing left to do: re-plan right away.
        if brain.nat_cell != Some(cell) && brain.path.is_empty() && brain.exec.is_none() {
            brain.replan = 0.0;
        }
        brain.nat_cell = Some(cell);
        if nbody.vel.x.abs() > 30.0 {
            brain.side = -nbody.vel.x.signum();
        }
    }
    let slot_x = (nat.pos.x + brain.side * gap).clamp(HALF.x + 1.0, level.size_px().x - HALF.x - 1.0);
    let mut input = HanInput::default();
    brain.braced = false;

    // --- Mode changes.
    let dx_nat = nat.pos.x - pos.0.x;
    let nat_above = nat.pos.y - HALF.y > pos.0.y + HALF.y;
    let in_chasm = level.in_chasm(level.cell_at(nat.pos).0) || level.in_chasm(level.cell_at(pos.0).0);
    // The band zone: his boost is weak, and he's no catch in mid-air there.
    let zoned = |p: Vec2| level.in_band_zone(level.cell_at(p));
    let in_zone = zoned(pos.0) || zoned(nat.pos);
    match brain.mode {
        HanMode::Follow if !nat_dead && !nat.grounded && nat_above && brain.winded == 0.0 => {
            let coming = nat.vel.y < 0.0 || in_chasm;
            let near = dx_nat.abs() < intercept_range(e);
            if coming && near && !in_zone && (in_chasm || doomed(level, nat.pos, nat.vel.x)) {
                brain.mode = HanMode::Intercept;
                brain.exec = None;
                brain.path.clear();
            } else if coming && dx_nat.abs() < brace_range(e) && body.on_ground {
                brain.braced = true;
            }
        }
        HanMode::Intercept => {
            let done = nat_dead
                || (nat.grounded && !nat_on_han)
                || (body.on_ground && dx_nat.abs() > intercept_range(e) * 1.5)
                || (body.on_ground && !nat_above && nat.vel.y <= 0.0 && !nat_on_han && !in_chasm);
            if done {
                brain.mode = HanMode::Follow;
                brain.replan = 0.0;
            }
        }
        _ => {}
    }
    if nat_on_han {
        brain.braced = true;
    }

    // --- "Lemme check that": Nat standing still, facing a hazard.
    if brain.mode == HanMode::Follow && !nat_dead && nat.grounded && !nat_on_han && nbody.vel.x.abs() < 5.0 {
        let cell = feet_cell(level, nat.pos);
        let behind = (pos.0.x - nat.pos.x) * nctl.facing <= 8.0;
        if hazard_ahead(level, cell, nctl.facing) && behind && body.on_ground && (pos.0 - nat.pos).length() < 6.0 * TILE && !in_zone {
            brain.still += dt;
            if brain.still >= go_ahead_delay(e) {
                brain.mode = HanMode::Ahead { dir: nctl.facing.signum(), t: 0.0 };
                brain.still = 0.0;
                brain.exec = None;
                brain.path.clear();
                say(&mut says, LEMME_LINE);
            }
        } else {
            brain.still = 0.0;
        }
    } else {
        brain.still = 0.0;
    }

    // --- What he wants.
    let ahead_floor = |x: f32, dir: f32| {
        let p = Vec2::new(x + dir * (HALF.x + 4.0), pos.0.y - HALF.y - 2.0);
        let (c, r) = level.cell_at(p);
        floor_at(level, c, r) || level.tile(c, r) == Tile::Liquid
    };
    let mut parachute = false;
    match brain.mode {
        HanMode::Parachute { .. } => {
            parachute = true;
            let dx = slot_x - pos.0.x;
            input = HanInput { dir: if dx.abs() > 3.0 { dx.signum() } else { 0.0 }, speed: 0.5, ..default() };
        }
        HanMode::Ahead { dir, t } => {
            let t = t + dt;
            let me_cell = feet_cell(level, pos.0);
            // An escort: he waits for Nat to keep up, and he's done when they're both through.
            let lead = (pos.0.x - nat.pos.x) * dir;
            let clear = !hazard_ahead(level, me_cell, dir);
            let nat_clear = !hazard_ahead(level, feet_cell(level, nat.pos), dir);
            // Never into a band zone: he'd be escorting Nat through the band's gate.
            let next = level.cell_at(pos.0 + Vec2::new(dir * TILE, 0.0));
            let next_ok = level.han_allowed(next) && !level.han_keeps_out(next);
            let backed_off = lead > 6.0 * TILE;
            if (lead > TILE && clear && nat_clear) || backed_off || t > 20.0 || !next_ok || (body.on_ground && !ahead_floor(pos.0.x, dir)) {
                brain.mode = HanMode::Follow;
                brain.replan = 0.0;
            } else {
                brain.mode = HanMode::Ahead { dir, t };
                let wait = lead > super::ESCORT_LEAD && !clear;
                input = HanInput { dir: if wait { 0.0 } else { dir }, speed: super::HAN_MARCH_SPEED / phys.speed, ..default() };
            }
        }
        HanMode::Intercept => {
            if !nat_on_han {
                let dir = (nat.pos.x - pos.0.x).signum();
                input = intercept(&me, &nat, brain.ctl.toots_left, Fall::of(&groove).gravity, ahead_floor(pos.0.x, dir));
            }
        }
        HanMode::Follow => {
            if !brain.braced && !nat_dead {
                input = follow(level, &mut nav, brain, &me, &body, &nat, slot_x, &plats, &clock, &groove, dt, &mut says);
            }
        }
        _ => {}
    }
    // On the run-up to a chain chasm he hangs back a few tiles from its edge until Nat jumps
    // (then it's an intercept): mid-air catches are much more forgiving from there than from
    // right on Nat's heels.
    if brain.mode == HanMode::Follow && body.on_ground && input.dir != 0.0 && chasm_ahead(level, pos.0, input.dir) {
        input.dir = 0.0;
        input.jump = false;
    }
    // The waltz: a step on each beat.
    if phys.on_the_beat && brain.mode == HanMode::Follow && !waltz_step(&groove) {
        input.dir = 0.0;
        input.jump = false;
    }
    // Keep out of a waltz row and a chute's grease (on foot).
    if !parachute && input.dir != 0.0 && body.on_ground {
        let here = level.han_keeps_out(feet_cell(level, pos.0));
        let next = level.han_keeps_out(feet_cell(level, pos.0 + Vec2::new(input.dir * (HALF.x + 2.0), 0.0)));
        if !here && next {
            input.dir = 0.0;
        }
    }

    // --- Move.
    let drove = drive(level, &mut pos.0, &mut body, &mut brain.ctl, input, &phys, &carriers, dt);
    if parachute {
        body.vel.y = body.vel.y.max(-PARACHUTE_FALL);
        if drove.contact.ground {
            if let HanMode::Parachute { pro: true } = brain.mode {
                say(&mut says, PRO_LINE);
            }
            brain.mode = HanMode::Follow;
            brain.replan = 0.0;
            brain.lost = false;
        }
    }
    if let HanMode::Ahead { .. } = brain.mode
        && drove.contact.blocked_x
    {
        brain.mode = HanMode::Follow;
    }
    if let Some(ex) = &mut brain.exec
        && ex.flying
    {
        ex.t += dt;
    }

    // Left behind (Nat running on away from him, out of sight, on the side he came from) for a
    // while: as good as lost. (Not while he's closing in on Nat, or waiting with him for a ride.)
    let gap = (nat.pos.x - pos.0.x).abs();
    let left_behind = brain.mode == HanMode::Follow
        && out_of_sight(pos.0, nat.pos)
        && (pos.0.x - nat.pos.x) * brain.side > 0.0
        && gap > brain.gap + 0.5 * TILE * dt;
    brain.gap = gap;
    brain.behind = if nat_dead || !out_of_sight(pos.0, nat.pos) {
        0.0
    } else if left_behind {
        brain.behind + dt
    } else {
        (brain.behind - dt).max(0.0)
    };
    // Lost and out of sight: parachute in.
    let lost = brain.lost || brain.behind > FALL_BEHIND_SECS;
    if lost && brain.mode == HanMode::Follow && out_of_sight(pos.0, nat.pos) && !nat_dead && nat.grounded {
        start_parachute(level, &mut pos, &mut prev, &mut body, brain, nat.pos, false);
    }
    // Fired up: he can't keep up.
    if groove.harmony == Harmony::Quartal && dx_nat.abs() > 8.0 * TILE && brain.said & once::WHEEZE == 0 {
        brain.said |= once::WHEEZE;
        say(&mut says, WHEEZE_LINE);
    }

    // --- His head: a perch for Nat (standing, or in mid-air not too high up and not in a band
    // zone, see `head_holds`), the plunger boost, the weak one in a band zone.
    if body.on_ground {
        brain.floor_y = pos.0.y;
    }
    let up = matches!(brain.mode, HanMode::Follow | HanMode::Intercept | HanMode::Ahead { .. });
    let my_cell = feet_cell(level, pos.0);
    let zone = level.in_band_zone(my_cell) || level.in_band_zone((my_cell.0, my_cell.1 - 1));
    let holds = head_holds(body.on_ground, pos.0.y - brain.floor_y, zone, level.in_chasm(my_cell.0))
        && !level.han_keeps_out(my_cell);
    *head = HanHead {
        solid: up && holds,
        boost: up && holds && brain.winded == 0.0,
        block: matches!(brain.mode, HanMode::Ahead { .. }),
        weak: zone,
    };

    // --- Pose.
    let grounded = body.on_ground;
    let running = body.vel.x.abs() > 20.0;
    let pose = match brain.mode {
        HanMode::Parachute { .. } => HanPose::Parachute,
        HanMode::Ahead { .. } => HanPose::March,
        HanMode::Intercept if !grounded && gs => HanPose::Paddle,
        HanMode::Intercept => HanPose::Intercept,
        HanMode::Sinking { .. } | HanMode::Gone { .. } => HanPose::Splat,
        HanMode::Follow if brain.winded > 0.0 => HanPose::Winded,
        HanMode::Follow if brain.braced => HanPose::Braced,
        HanMode::Follow if !grounded => {
            if gs {
                HanPose::Paddle
            } else {
                HanPose::Jump
            }
        }
        HanMode::Follow if running && groove.bouncy() => HanPose::Roll,
        HanMode::Follow if running => HanPose::Run,
        HanMode::Follow if groove.harmony == Harmony::Quartal && dx_nat.abs() > 4.0 * TILE => HanPose::Winded,
        HanMode::Follow => HanPose::Idle,
    };
    let facing_left = if brain.braced || pose == HanPose::Idle {
        dx_nat < 0.0
    } else if body.vel.x.abs() > 1.0 {
        body.vel.x < 0.0
    } else {
        anim.facing_left
    };
    set_anim(&mut anim, pose, facing_left, &groove, false);
}

fn set_anim(anim: &mut HanAnim, pose: HanPose, facing_left: bool, groove: &Groove, hidden: bool) {
    let new = HanAnim { pose, facing_left, tremble: groove.grip(), wobble: anim.wobble, hidden };
    if *anim != new {
        *anim = new;
    }
}

fn start_parachute(
    level: &Level,
    pos: &mut Pos,
    prev: &mut PrevPos,
    body: &mut Body,
    brain: &mut HanBrain,
    nat: Vec2,
    pro: bool,
) {
    let at = parachute_spot(level, nat, brain.side);
    pos.0 = at;
    prev.0 = at;
    *body = Body::player();
    body.vel.y = -PARACHUTE_FALL;
    brain.mode = HanMode::Parachute { pro };
    brain.ctl = HanCtl::default();
    brain.path.clear();
    brain.exec = None;
    brain.lost = false;
    brain.stuck = 0.0;
    brain.best = f32::INFINITY;
    brain.goal = None;
    brain.reroute = false;
    brain.behind = 0.0;
    brain.drops += 1;
}

/// Following: route to the slot and steer along it.
#[allow(clippy::too_many_arguments)]
fn follow(
    level: &Level,
    nav: &mut HanNav,
    brain: &mut HanBrain,
    me: &Kin,
    body: &Body,
    nat: &Kin,
    slot_x: f32,
    plats: &[(Vec2, MovingPlatform)],
    clock: &SimClock,
    groove: &Groove,
    dt: f32,
    _says: &mut MessageWriter<HanSays>,
) -> HanInput {
    let nav = nav.for_groove(groove);
    let my_cell = if body.riding.is_some() { feet_cell(level, me.pos) } else { nav_cell(level, me.pos) };
    brain.replan -= dt;
    let flying = brain.exec.is_some_and(|e| e.flying);
    if brain.replan <= 0.0 && me.grounded && !flying {
        brain.replan = REPLAN_SECS;
        let map = Map::for_han(level);
        let goal = brain.nat_cell.and_then(|nc| slot_cell(&map, level, nc, slot_x));
        if goal != brain.goal {
            brain.goal = goal;
            // Re-route, but keep going the way he's going until the new route is in: the slot
            // moves on with a running Nat a few times a second, and stopping to think each
            // time is how he falls behind.
            brain.reroute = true;
            // Progress is measured afresh toward the new slot, but a Nat on the move keeps
            // moving it: that alone isn't progress (or he'd never notice he's stuck).
            brain.best = goal.map_or(f32::INFINITY, |g| (stand(level, g) - me.pos).length());
            if goal.is_none_or(|g| g == my_cell) {
                brain.path.clear();
                brain.exec = None;
                brain.reroute = false;
            }
        }
        if let Some(goal) = goal
            && (brain.reroute || (brain.path.is_empty() && brain.exec.is_none()))
            && my_cell != goal
            && Nav::node(&map, level, my_cell)
        {
            match nav.route(&map, my_cell, goal, ROUTE_BUDGET) {
                Route::Found(p) => {
                    // The jump he's lining up for, if it's still the way to go.
                    let same = |e: &Exec| p.first().is_some_and(|f| f.to == e.to && f.step == Step::Move(e.mv));
                    if !brain.exec.as_ref().is_some_and(same) {
                        brain.exec = None;
                    }
                    brain.path = p;
                    brain.lost = false;
                    brain.reroute = false;
                }
                Route::NoRoute => {
                    brain.path.clear();
                    brain.exec = None;
                    brain.lost = true;
                    brain.reroute = false;
                }
                Route::Budget => brain.replan = 0.0, // keep thinking next step
            }
        }
    }
    let Some(goal) = brain.goal else { return HanInput::default() };

    // Progress (stuck detection).
    let d = (stand(level, goal) - me.pos).length();
    if d < brain.best - 6.0 {
        brain.best = d;
        brain.stuck = 0.0;
    } else if my_cell != goal && !brain.exec.is_some_and(|e| e.wait > 0.0) {
        brain.stuck += dt;
        if brain.stuck > STUCK_SECS {
            brain.lost = true;
        }
    }

    // Off the graph (riding a raft, standing somewhere odd) or lost: steer straight for the slot
    // (but never off a ledge into a drop or the sewage).
    if !brain.lost && (brain.path.is_empty() && brain.exec.is_none()) {
        // Nat just jumped on the run: run on with him (his reflexes take it from here: an
        // intercept, or the route to where Nat lands) rather than brake in the old slot.
        let behind_nat = (nat.pos.x - me.pos.x) * me.vel.x > 0.0;
        if !nat.grounded && nat.vel.x * me.vel.x > 0.0 && me.vel.x.abs() > 30.0 && behind_nat && body.riding.is_none() {
            let top = HanPhys::of(groove).speed;
            let on = HanInput { dir: me.vel.x.signum(), speed: (me.vel.x.abs() / top).min(1.0), ..default() };
            return careful(level, me, body, on);
        }
        if my_cell == goal {
            // In the slot's cell: stand right at the slot (not on Nat). Nat running on along
            // the ground: keep to the slot as it moves on, ahead of the next re-plan (minding
            // the drops).
            if nat.grounded && nat.vel.x.abs() > 30.0 && body.riding.is_none() {
                // (Matching Nat's pace, so he runs right in his slot rather than a few px behind
                // it: where the chain proofs put him. Not when the nervous band has him clinging
                // close: no overshooting into Nat when he stops.)
                if !groove.grip() {
                    return careful(level, me, body, pace(nat.vel.x, slot_x - me.pos.x, HanPhys::of(groove).speed));
                }
                return careful(level, me, body, arrive(slot_x - me.pos.x, 2.0));
            }
            let dx =
                if (slot_x - stand(level, goal).x).abs() < TILE { slot_x } else { stand(level, goal).x } - me.pos.x;
            return arrive(dx, 2.0);
        }
        if !Nav::node(&Map::for_han(level), level, my_cell) || body.riding.is_some() {
            return careful(level, me, body, arrive(slot_x - me.pos.x, 4.0));
        }
        // Waiting on a route while Nat runs on: keep after him (minding the drops) rather than
        // stop to think.
        if nat.vel.x.abs() > 30.0 && nat.vel.x * (slot_x - me.pos.x) > 0.0 {
            return careful(level, me, body, arrive(slot_x - me.pos.x, 4.0));
        }
        return HanInput::default();
    }
    if brain.lost {
        // Try anyway: walk toward the slot, hop when blocked.
        let mut i = careful(level, me, body, arrive(slot_x - me.pos.x, 4.0));
        i.jump = me.grounded && body.vel.x.abs() < 5.0 && i.dir != 0.0 && nat.pos.y > me.pos.y + TILE;
        i.hold = true;
        return i;
    }

    // Carry out the next edge.
    if brain.exec.is_none() {
        let Some(edge) = brain.path.first().copied() else { return HanInput::default() };
        match edge.step {
            Step::Walk => {
                let tx = stand(level, edge.to).x;
                if (tx - me.pos.x).abs() < 3.0 || feet_cell(level, me.pos) == edge.to {
                    brain.path.remove(0);
                }
                // Don't walk off a platform you're riding.
                if body.riding.is_some() && !floor_at(level, edge.to.0, edge.to.1 + 1) {
                    return HanInput::default();
                }
                let dx = tx - me.pos.x;
                // More walking after this, or Nat running on ahead: keep running.
                let more = brain.path.first().is_some_and(|e| e.step == Step::Walk)
                    || (nat.vel.x * dx > 0.0 && nat.vel.x.abs() > 30.0);
                if brain.path.is_empty() && nat.vel.x.abs() > 30.0 {
                    brain.replan = 0.0;
                }
                let speed = catch_up(groove, me.pos.x, slot_x, nat.vel.x);
                return if more { HanInput { dir: dx.signum(), speed, ..default() } } else { arrive(dx, 2.0) };
            }
            Step::Move(k) => {
                let map = Map::for_han(level);
                let m = nav.moves()[k as usize];
                let Some(x0) = take_off(&map, &nav.env, my_cell, &m) else {
                    brain.path.clear();
                    return HanInput::default();
                };
                brain.exec = Some(Exec { mv: k, from: my_cell, to: edge.to, x0, t: 0.0, flying: false, wait: 0.0 });
            }
        }
    }
    let ex = brain.exec.as_mut().expect("set above");
    if !ex.flying {
        let m = nav.moves()[ex.mv as usize];
        let dx = ex.x0 - me.pos.x;
        let secs = brain.path.first().map_or(0.5, |e| e.air);
        // Running up to a jump the way it goes: take off on the run as soon as a jump that way,
        // from here at this speed, lands where the route goes (the planned cell, or further
        // along the same walk); else stop at the take-off point and jump from a standstill, the
        // way the route was planned.
        let rolling = (me.grounded
            && body.riding.is_none()
            && m.dir * me.vel.x > 25.0
            && dx.abs() < 2.0 * TILE
            && my_cell == ex.from
            && landing_ready(level, plats, clock, groove, ex.to, secs))
        .then(|| rolling_take_off(level, nav, &brain.path, ex, &m, me, nat.pos.x))
        .flatten();
        if let Some((k, mv, to)) = rolling {
            brain.path.drain(1..=k);
            ex.mv = mv;
            ex.to = to;
        }
        let rolling = rolling.is_some();
        if !rolling && (dx.abs() > 1.5 || me.vel.x.abs() > 25.0 || !me.grounded) {
            return arrive(dx, 1.0);
        }
        if !rolling && !landing_ready(level, plats, clock, groove, ex.to, secs) {
            ex.wait += dt;
            if ex.wait > 8.0 {
                brain.exec = None;
                brain.path.clear();
                brain.lost = true;
            }
            return HanInput::default();
        }
        ex.flying = true;
        ex.t = 0.0;
    }
    let m = nav.moves()[ex.mv as usize];
    let t = ex.t;
    if me.grounded && t > 0.05 {
        // Landed: on to the next edge (or re-plan if this wasn't the cell).
        let to = ex.to;
        brain.exec = None;
        if feet_cell(level, me.pos) == to {
            brain.path.remove(0);
        } else {
            brain.path.clear();
            brain.replan = 0.0;
        }
        return HanInput::default();
    }
    if t > 3.5 {
        brain.exec = None;
        brain.path.clear();
        return HanInput::default();
    }
    let last_toot = m.toots.iter().copied().filter(|x| x.is_finite()).fold(0.0f32, f32::max);
    let correcting = t > last_toot + 0.1 && t > 0.25;
    let mut dir = if t < m.release { m.dir } else { 0.0 };
    if correcting {
        let dx = landing_x(level, plats, ex.to, me.pos.x) - me.pos.x;
        dir = if dx.abs() > 3.0 { dx.signum() } else { 0.0 };
    }
    HanInput {
        dir,
        speed: 1.0,
        jump: m.jump && t == 0.0,
        hold: t < m.hold,
        toot: m.toots.iter().any(|&tt| tt.is_finite() && tt > t - 1e-4 && tt <= t + 1.0 / 60.0 - 1e-4),
    }
}

/// A rolling take-off for the jump `ex` (the route's first edge, `path[0]`): the planned move,
/// or another ground jump the same way, that from where Han is, at the speed he's running,
/// lands on the planned cell or one further along the walk after it (`path[k].to`, the edges
/// between all walks), or, when that walk ends the route, a bit further on toward Nat (at `nat_x`).
/// Returns (k, move, landing cell); k past the route's end: drop the rest of it.
fn rolling_take_off(
    level: &Level,
    nav: &mut Nav,
    path: &[Edge],
    ex: &Exec,
    m: &HanMove,
    me: &Kin,
    nat_x: f32,
) -> Option<(usize, u16, (i32, i32))> {
    let map = Map::for_han(level);
    let walks = path.iter().skip(1).take_while(|e| e.step == Step::Walk).count();
    let last = path.get(walks).map_or(ex.to, |e| e.to);
    let nat_col = (nat_x / TILE).floor() as i32;
    let toward_nat = |to: (i32, i32)| {
        walks + 1 == path.len() && (to.0 - last.0) * m.dir as i32 > 0 && (nat_col - to.0) * m.dir as i32 >= 0
    };
    let ok = |to: (i32, i32)| {
        (0..=walks)
            .rev()
            .find(|&k| path.get(k).is_some_and(|e| e.to == to) || (k == 0 && to == ex.to))
            .or(toward_nat(to).then_some(path.len().saturating_sub(1)))
    };
    let planned = std::iter::once(ex.mv);
    let others = (0..nav.moves().len() as u16).filter(|&k| k != ex.mv);
    for k in planned.chain(others) {
        let mk = nav.moves()[k as usize];
        if mk.dir != m.dir || !mk.jump {
            continue;
        }
        if let Some(to) = nav.lands_rolling(&map, ex.from, &mk, me.pos.x, me.vel.x)
            && let Some(i) = ok(to)
        {
            return Some((i, k, to));
        }
    }
    None
}

/// Don't walk off a ledge (into a drop of more than 3 tiles, or into sewage) when steering
/// without a route.
fn careful(level: &Level, me: &Kin, body: &Body, i: HanInput) -> HanInput {
    if !me.grounded || i.dir == 0.0 || body.riding.is_some() {
        return i;
    }
    let (c, r) = level.cell_at(me.pos + Vec2::new(i.dir * (HALF.x + 3.0), -HALF.y + 1.0));
    let safe = (r + 1..=r + 3).find(|&rr| floor_at(level, c, rr) || level.tile(c, rr) == Tile::Liquid);
    match safe {
        Some(rr) if level.tile(c, rr) != Tile::Liquid => i,
        _ => HanInput::default(),
    }
}

/// Running along a walk after a Nat running away (at `nat_vx`): faster (up to
/// [`HAN_CATCH_UP`] × his top speed) the further he is from his slot (at `slot_x`), so he
/// catches up after a hold-up. Not while the band's fired up: then he just can't keep up.
fn catch_up(groove: &Groove, x: f32, slot_x: f32, nat_vx: f32) -> f32 {
    if groove.harmony == Harmony::Quartal || nat_vx * (slot_x - x) <= 0.0 || nat_vx.abs() < 30.0 {
        return 1.0;
    }
    let far = ((slot_x - x).abs() / TILE - 3.0) / 4.0;
    1.0 + (HAN_CATCH_UP - 1.0) * far.clamp(0.0, 1.0)
}

/// A chain chasm's drop within [`CHASM_HANG_BACK`] tiles ahead (heading `dir`) of Han at `pos`.
fn chasm_ahead(level: &Level, pos: Vec2, dir: f32) -> bool {
    let (c, r) = feet_cell(level, pos);
    let reach = (HALF.x / TILE + CHASM_HANG_BACK).ceil() as i32;
    (1..=reach)
        .map(|k| c + k * dir as i32)
        .any(|cc| level.in_chasm(cc) && !(r + 1..=r + 3).any(|rr| floor_at(level, cc, rr)))
}

/// How far (tiles) Han hangs back from a chain chasm's edge while Nat runs up to it.
const CHASM_HANG_BACK: f32 = 2.5;

/// Keep pace with a slot moving at `vx` (px/s), `dx` px away: its speed, plus a bit to close
/// the gap (`top`: his top speed).
fn pace(vx: f32, dx: f32, top: f32) -> HanInput {
    let v = vx + 4.0 * dx;
    if v.abs() < 5.0 {
        return HanInput::default();
    }
    HanInput { dir: v.signum(), speed: (v.abs() / top).clamp(0.15, 1.0), ..default() }
}

/// The deceleration (px/s²) [`arrive`] plans its stop with.
const ARRIVE_DECEL: f32 = 1500.0;

/// Run toward `dx` (px away), slowing to stop on it.
fn arrive(dx: f32, tol: f32) -> HanInput {
    if dx.abs() <= tol {
        return HanInput::default();
    }
    let speed = ((2.0 * ARRIVE_DECEL * dx.abs()).sqrt() / super::HAN_RUN_SPEED).clamp(0.15, 1.0);
    HanInput { dir: dx.signum(), speed, ..default() }
}

//! Han's body and reflexes, pure (no Bevy app): his per-mode physics ([`HanPhys`]), the
//! controller that turns an intent ([`HanInput`]) into motion with the shared body step
//! ([`drive`]), and the goalkeeper reflex that gets him under a falling Nat ([`intercept`]).
//! The game's Han (`game::han`) runs exactly this code every step, and the level validator
//! runs it to prove chain-jump chasms crossable ([`chain_cross`]), so what the validator
//! proves is what Han does.
//!
//! Coordinates here are the game's: world pixels, y up.

use bevy::prelude::*;

use super::{Level, TILE};
use crate::audio::Harmony;
use crate::game::{
    BOOST_SPEED, Body, Carrier, Contact, Fall, GIANT_STEPS_GRAVITY, GIANT_STEPS_SPEED, Groove, han_carrier, move_towards,
    step_body, tuning::*, HanHead,
};

/// Han's top running speed (px/s): a touch faster than Nat's, so he can catch up.
pub const HAN_RUN_SPEED: f32 = 165.0;
/// His determined march when he goes ahead into a hazard ("Lemme check that").
pub const HAN_MARCH_SPEED: f32 = 110.0;
/// Han's mid-air toots (forgiving navigation). His toots never count for the band.
pub const HAN_TOOTS: u8 = 3;
/// Under Giant Steps Han floats even more than Nat: gravity × this on top of the band's.
pub const HAN_GS_FLOAT: f32 = 0.8;
/// The laughing band: Han rolls along, bouncier than Nat (bounce speed × this).
pub const HAN_ROLL_BOUNCE: f32 = 1.3;

/// Han's physics under the music.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HanPhys {
    pub fall: Fall,
    /// Top running speed (px/s) and accelerations.
    pub speed: f32,
    pub ground_accel: f32,
    pub air_accel: f32,
    pub jump_speed: f32,
    pub toot_speed: f32,
    /// Landings bounce (the laughing band).
    pub bounce: bool,
    /// Moves only on the beat (the waltz).
    pub on_the_beat: bool,
}

impl HanPhys {
    /// Han's physics for the band's mood:
    /// - Giant Steps: floatier than Nat (gravity × [`HAN_GS_FLOAT`] more), paddling;
    /// - fired up (quartal): Han does *not* speed up: he can't keep up, wheezes;
    /// - waltz: he moves only on the beat (see [`waltz_step`]);
    /// - nervous: normal (he clings close and trembles: that's the brain and the art);
    /// - laughing band: he rolls and bounces.
    pub fn of(groove: &Groove) -> HanPhys {
        let gs = groove.harmony == Harmony::Coltrane;
        let gravity = if gs { GRAVITY * GIANT_STEPS_GRAVITY * HAN_GS_FLOAT } else { GRAVITY };
        let speed = if gs { GIANT_STEPS_SPEED } else { 1.0 };
        HanPhys {
            fall: Fall { gravity, max_fall: MAX_FALL * (gravity / GRAVITY).sqrt() },
            speed: HAN_RUN_SPEED * speed,
            ground_accel: GROUND_ACCEL * speed,
            air_accel: AIR_ACCEL * speed,
            jump_speed: JUMP_SPEED,
            toot_speed: DOUBLE_JUMP_SPEED,
            bounce: groove.bounce,
            on_the_beat: groove.waltz(),
        }
    }

    pub fn normal() -> HanPhys {
        HanPhys::of(&Groove::default())
    }
}

/// The waltz: Han takes a little step on each beat, standing still the rest of it.
pub fn waltz_step(groove: &Groove) -> bool {
    groove.clock.phase < 0.4
}

/// What Han wants to do this step.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HanInput {
    /// -1, 0, +1 (times `speed`).
    pub dir: f32,
    /// Fraction of his top speed (march, wheezing...). 0 is read as 1.
    pub speed: f32,
    /// Ground jump (if grounded).
    pub jump: bool,
    /// Keep holding jump (releasing cuts the rise, like Nat's).
    pub hold: bool,
    /// Toot (mid-air, if he has toots left).
    pub toot: bool,
}

/// Han's controller state.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct HanCtl {
    pub toots_left: u8,
    pub cut_armed: bool,
}

impl Default for HanCtl {
    fn default() -> Self {
        HanCtl { toots_left: HAN_TOOTS, cut_armed: false }
    }
}

/// What happened in a [`drive`] step.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Drove {
    pub contact: Contact,
    pub jumped: bool,
    pub tooted: bool,
    pub bounced: bool,
}

/// One step of Han: his input onto his body (accelerate, jump, toot, cut), then the shared body
/// step ([`step_body`]) and his landing (toots back, the laughing band's bounce).
#[allow(clippy::too_many_arguments)]
pub fn drive(
    level: &Level,
    pos: &mut Vec2,
    body: &mut Body,
    ctl: &mut HanCtl,
    input: HanInput,
    phys: &HanPhys,
    carriers: &[Carrier],
    dt: f32,
) -> Drove {
    let mut out = Drove::default();
    let speed = if input.speed > 0.0 { input.speed } else { 1.0 };
    let target = input.dir * phys.speed * speed;
    let accel = if body.on_ground {
        if input.dir == 0.0 || input.dir * body.vel.x < 0.0 { GROUND_DECEL } else { phys.ground_accel }
    } else {
        phys.air_accel
    };
    body.vel.x = move_towards(body.vel.x, target, accel * dt);
    if body.on_ground {
        ctl.toots_left = HAN_TOOTS;
    }
    if input.jump && body.on_ground {
        body.vel.y = phys.jump_speed;
        body.on_ground = false;
        body.riding = None;
        ctl.cut_armed = true;
        out.jumped = true;
    } else if input.toot && !body.on_ground && ctl.toots_left > 0 {
        body.vel.y = phys.toot_speed;
        ctl.toots_left -= 1;
        ctl.cut_armed = false;
        out.tooted = true;
    }
    if ctl.cut_armed && !input.hold && body.vel.y > 0.0 {
        body.vel.y *= JUMP_CUT;
        ctl.cut_armed = false;
    }
    if body.vel.y <= 0.0 {
        ctl.cut_armed = false;
    }
    let contact = step_body(level, pos, body, phys.fall, carriers, &[], dt);
    if contact.ground {
        ctl.toots_left = HAN_TOOTS;
        if phys.bounce && !contact.was_on_ground && contact.fall_speed > BOUNCE_MIN {
            body.vel.y = (contact.fall_speed * crate::game::BOUNCE_RESTITUTION).min(crate::game::BOUNCE_SPEED * HAN_ROLL_BOUNCE);
            body.on_ground = false;
            body.riding = None;
            out.bounced = true;
        }
    }
    out.contact = contact;
    out
}

const BOUNCE_MIN: f32 = crate::game::BOUNCE_MIN_SPEED;

/// A body's motion as Han's reflexes see it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kin {
    pub pos: Vec2,
    pub vel: Vec2,
    pub grounded: bool,
}

/// Half extents of Nat's and Han's boxes.
pub const HALF: Vec2 = Vec2::new(PLAYER_SIZE.0 / 2.0, PLAYER_SIZE.1 / 2.0);

/// When Nat's feet, falling under `gravity`, come down to height `y` (s from now), if ever.
pub fn time_to_fall_to(nat: &Kin, y: f32, gravity: f32) -> Option<f32> {
    let h = nat.pos.y - HALF.y - y;
    let v = nat.vel.y;
    // h + v t - g t²/2 = 0, the later root.
    let d = v * v + 2.0 * gravity * h;
    (d >= 0.0).then(|| (v + d.sqrt()) / gravity).filter(|t| *t >= 0.0)
}

/// The goalkeeper: get under a Nat in the air so he can land on Han's head (and boost off it).
/// Runs to where Nat will come down to head height, jumps off ledges rather than walking off
/// them (`floor_ahead`: is there ground a step ahead in the direction he's running), and toots
/// to rise into Nat's path when he's sinking below it.
pub fn intercept(han: &Kin, nat: &Kin, toots_left: u8, gravity: f32, floor_ahead: bool) -> HanInput {
    let head = han.pos.y + HALF.y;
    let feet = nat.pos.y - HALF.y;
    let above = feet - head;
    let t = time_to_fall_to(nat, head, gravity).unwrap_or(0.0).min(1.5);
    let x_to = nat.pos.x + nat.vel.x * t;
    let dx = x_to - han.pos.x;
    let dir = if dx.abs() > 1.5 { dx.signum() } else { 0.0 };
    let running_off = han.grounded && dir != 0.0 && !floor_ahead;
    let high = above > 2.5 * TILE && dx.abs() < 2.5 * TILE && nat.vel.y < 60.0;
    let jump = han.grounded && (running_off || high);
    // Sinking below Nat's path: toot back up into it.
    let toot = !han.grounded && toots_left > 0 && han.vel.y < -40.0 && above > 2.0 && above < 6.0 * TILE && dx.abs() < 3.0 * TILE;
    HanInput { dir, speed: 1.0, jump, hold: true, toot }
}

/// A chain-jump chasm: walk row `row`, open (bottomless) columns `c0..=c1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chasm {
    pub row: i32,
    pub c0: i32,
    pub c1: i32,
}

impl Chasm {
    pub fn width(&self) -> usize {
        (self.c1 - self.c0 + 1) as usize
    }
}

/// How Nat plays a chain: jump at the edge running, toot after `toot1`; on landing on Han's
/// head jump at once (the boost; a buffered press), toot `toot2` after it, again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChainPlay {
    pub toot1: f32,
    pub toot2: f32,
}

/// When the band switches to Giant Steps mid-chain: never, or `delay` s after Nat's toot number
/// `nth` (1-based) of the chain (the 5 toots that summon it may have started before).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GsSwitch {
    Never,
    After { nth: u32, delay: f32 },
}

/// The ways the band might switch to Giant Steps during a chain (every number of earlier toots,
/// at once or a bar later).
pub fn gs_switches() -> Vec<GsSwitch> {
    let mut v = vec![GsSwitch::Never];
    for nth in 1..=5 {
        for delay in [0.0, 1.0, 2.0] {
            v.push(GsSwitch::After { nth, delay });
        }
    }
    v
}

/// The plays a human might use for a chain.
pub fn chain_plays() -> Vec<ChainPlay> {
    let mut v = Vec::new();
    for toot1 in [0.25, 0.32, 0.4, 0.5] {
        for toot2 in [0.3, 0.4, 0.5, 0.6] {
            v.push(ChainPlay { toot1, toot2 });
        }
    }
    v
}

/// Where Nat's box center goes when standing in cell (col, row).
pub fn stand(level: &Level, (c, r): (i32, i32)) -> Vec2 {
    let center = level.tile_center(c.max(0) as usize, r.max(0) as usize);
    Vec2::new(center.x, center.y - TILE / 2.0 + HALF.y)
}

/// Simulate one chain over `chasm` from its edge (heading `dir`), Nat playing `play`, Han
/// following with the [`intercept`] reflex from [`FOLLOW_GAP`] behind, the band switching to
/// Giant Steps per `gs`. Returns the cell Nat lands in past the chasm, if he does.
pub fn chain_try(level: &Level, chasm: &Chasm, dir: i32, play: ChainPlay, gs: GsSwitch) -> Option<(i32, i32)> {
    const DT: f32 = 1.0 / 60.0;
    let d = dir as f32;
    let edge = if dir > 0 { chasm.c0 - 1 } else { chasm.c1 + 1 };
    let start = stand(level, (edge, chasm.row));
    // Nat runs from two tiles back and jumps 4 px before the edge's end.
    let edge_x = start.x + d * (TILE / 2.0);
    let mut nat = start - Vec2::new(d * 2.0 * TILE, 0.0);
    let mut nb = Body::player();
    nb.on_ground = true;
    nb.vel.x = d * RUN_SPEED;
    let mut han = nat - Vec2::new(d * FOLLOW_GAP, 0.0);
    let mut hb = Body::player();
    hb.on_ground = true;
    hb.vel.x = d * RUN_SPEED;
    let mut hctl = HanCtl::default();
    let han_id = Entity::from_raw_u32(1).unwrap_or(Entity::PLACEHOLDER);
    let head = HanHead { solid: true, boost: true, block: false };
    let mut groove = Groove::default();
    let (mut jumped, mut has_toot, mut toots, mut t, mut last_launch, mut tooted_since) =
        (false, true, 0u32, 0.0f32, 0.0f32, false);
    let mut gs_at = f32::INFINITY;
    let mut boosts = 0;
    let mut toot_due = play.toot1;
    while t < 6.0 {
        t += DT;
        if t >= gs_at && groove.harmony != Harmony::Coltrane {
            groove = Groove::of(Harmony::Coltrane);
        }
        let speed = RUN_SPEED * groove.speed_scale;
        // --- Han first (he moves before Nat in the game), on last step's Nat.
        let han_prev = han;
        let hk = Kin { pos: han, vel: hb.vel, grounded: hb.on_ground };
        let nk = Kin { pos: nat, vel: nb.vel, grounded: nb.on_ground };
        let phys = HanPhys::of(&groove);
        let ahead = han + Vec2::new(d * (HALF.x + 4.0), -HALF.y - 2.0);
        let (ac, ar) = level.cell_at(ahead);
        let floor_ahead = { let t = level.tile(ac, ar); t.is_solid() || t.is_one_way() };
        let mut input = if nb.riding == Some(han_id) {
            HanInput::default() // braced: no taxi rides
        } else {
            intercept(&hk, &nk, hctl.toots_left, Fall::of(&groove).gravity, floor_ahead)
        };
        if !jumped {
            input = HanInput { dir: d, speed: 1.0, ..default() };
        }
        drive(level, &mut han, &mut hb, &mut hctl, input, &phys, &[], DT);
        // --- Nat.
        nb.vel.x = move_towards(nb.vel.x, d * speed, if nb.on_ground { GROUND_ACCEL } else { AIR_ACCEL } * groove.speed_scale * DT);
        let on_han = nb.riding == Some(han_id);
        if !jumped && nat.x * d >= (edge_x - d * 4.0) * d {
            nb.vel.y = JUMP_SPEED;
            nb.on_ground = false;
            jumped = true;
            last_launch = t;
        } else if jumped && on_han && nb.on_ground {
            nb.vel.y = BOOST_SPEED;
            nb.on_ground = false;
            nb.riding = None;
            has_toot = true;
            tooted_since = false;
            boosts += 1;
            last_launch = t;
            toot_due = play.toot2;
        } else if jumped && has_toot && !tooted_since && t - last_launch >= toot_due {
            nb.vel.y = DOUBLE_JUMP_SPEED;
            has_toot = false;
            tooted_since = true;
            toots += 1;
            if let GsSwitch::After { nth, delay } = gs
                && toots == nth
            {
                gs_at = t + delay;
            }
        }
        let carriers: Vec<Carrier> = han_carrier(han_id, han, han_prev, &head).into_iter().collect();
        let contact = step_body(level, &mut nat, &mut nb, Fall::of(&groove), &carriers, &[], DT);
        if nat.y + HALF.y < 0.0 {
            return None; // fell out of the level
        }
        let (c, r) = level.cell_at(nat - Vec2::new(0.0, HALF.y - 1.0));
        if level.tile(c, r).is_deadly() {
            return None;
        }
        if contact.ground && contact.riding != Some(han_id) && jumped {
            let past = if dir > 0 { c > chasm.c1 } else { c < chasm.c0 };
            return past.then_some((c, r));
        }
        if boosts > 6 {
            return None;
        }
    }
    None
}

/// Han starts this far behind Nat (px): his follow slot, ~1.5 tiles.
pub const FOLLOW_GAP: f32 = 1.5 * TILE;

/// Can a human chain across `chasm` heading `dir`, whenever the band switches to Giant Steps?
/// For every [`GsSwitch`], some [`ChainPlay`] must land past it. Returns the landing of the
/// no-switch case.
pub fn chain_cross(level: &Level, chasm: &Chasm, dir: i32) -> Option<(i32, i32)> {
    let plays = chain_plays();
    let mut first = None;
    for gs in gs_switches() {
        let hit = plays.iter().find_map(|&p| chain_try(level, chasm, dir, p, gs))?;
        first.get_or_insert(hit);
    }
    first
}

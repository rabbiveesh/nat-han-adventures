//! **The double game** (World 4, a spike): Nat and Han at once, one storey each, in one window.
//!
//! A duo level is 18 rows tall: Nat's storey on top (rows 0-7), a two-row slab (rows 8-9)
//! hidden behind the bandstand (the HUD's seam, `ui::duo`), Han's storey below (rows 10-17).
//! The storeys are one level: holes in the slab drop you into the other storey, and whatever
//! is in the slab's rows is there, just unseen (the blind jump's ledge).
//!
//! - **Nat** is played as in the story, and his moves charge the band's counters as ever.
//! - **Han** runs by himself, Side B style (`han::brain::charge`): always on, jumping at walls,
//!   edges and brushes, falling down holes that have a floor under them. Down calls the band,
//!   and the band's mode is both brothers' physics: the call Han needs is the one Nat gets.
//! - **One window**: the camera rides Han ([`Duo::view_x`]), so Han is the autoscroll. Nat
//!   can't run past the window's right edge, and left behind off its left edge he's done for.
//! - **Deaths** ([`DeathRule`], Tab switches): *separate* (each pops back in where he last
//!   stood, inside the window) or *both* (either death sends both back to the last checkpoint).
//!
//! Ways in: `NATHAN_DUO=1 cargo run` (`NATHAN_DUO_DEATHS=both`), `?duo=1` on the web.

use bevy::prelude::*;

use super::han::{HanBrain, HanMode, HanNav};
use super::physics::{Body, Dead, Finished};
use super::{ActiveLevel, GameSet, GeneratedLevel, Han, LevelRun, Player, Pos, PrevPos, RestartLevel, SimClock};
use crate::events::{DeathCause, HanSays, PlayerDied, PlayerRespawned};
use crate::level::buddy::HALF;
use crate::level::{Level, TILE, Tile};
use crate::state::AppState;

pub static DUO_SOURCES: [&str; 1] = [include_str!("../../levels/duo/d1.txt")];

/// A duo level's rows: Nat's storey, the slab, Han's storey.
pub const ROWS: usize = 18;
pub const SLAB: (usize, usize) = (8, 9);
/// The camera shows the whole level's height: 18 tiles.
pub const VIEW_HEIGHT: f32 = ROWS as f32 * TILE;
/// Half the window's width the rules assume (16:9; a wider screen shows a little more).
pub const VIEW_HALF_W: f32 = VIEW_HEIGHT * 16.0 / 9.0 / 2.0;
/// How fast the window catches up with Han (px/s): smooth when he pops back in.
const VIEW_SPEED: f32 = 420.0;
/// Han down: this long before he pops back in (separate deaths).
pub const HAN_BACK_SECS: f32 = 1.5;
/// No progress for this long while running: he's bonked on something he can't get over.
const HAN_STALL_SECS: f32 = 2.5;
/// A call holds this long here (the story's hold is 20 s: a whole level of fired up).
pub const HOLD_SECS: f32 = 8.0;
pub const LEFT_BEHIND_LINE: &str = "Nat?!";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum DeathRule {
    /// Each pops back in where he last stood (inside the window); the other carries on.
    Separate,
    /// Either one's death sends both back to the last checkpoint.
    Both,
}

impl DeathRule {
    pub fn label(self) -> &'static str {
        match self {
            DeathRule::Separate => "DEATHS: SEPARATE",
            DeathRule::Both => "DEATHS: BOTH REWIND",
        }
    }
}

/// A duo level is on. Exists while one is being played.
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct Duo {
    /// Which duo level (index into the duo levels).
    pub level: usize,
    pub rule: DeathRule,
    /// Han's start cell (the `han:` header).
    pub han_start: (usize, usize),
    /// The window's centre x (gameplay's: the camera follows it).
    pub view_x: f32,
    /// Han's down (dead) for this long more; `None` while he runs.
    pub han_down: Option<f32>,
    pub han_deaths: u32,
    /// Where each last stood on solid ground (separate deaths put them back there).
    pub nat_safe: Option<Vec2>,
    pub han_safe: Option<Vec2>,
    han_best: f32,
    han_stall: f32,
    /// Han's been put at his start since the level (re)loaded.
    placed: bool,
}

impl Duo {
    pub fn new(level: usize, han_start: (usize, usize), rule: DeathRule) -> Duo {
        Duo {
            level,
            rule,
            han_start,
            view_x: 0.0,
            han_down: None,
            han_deaths: 0,
            nat_safe: None,
            han_safe: None,
            han_best: 0.0,
            han_stall: 0.0,
            placed: false,
        }
    }

    pub fn view_left(&self) -> f32 {
        self.view_x - VIEW_HALF_W
    }

    pub fn view_right(&self) -> f32 {
        self.view_x + VIEW_HALF_W
    }
}

/// A duo level: the level, and Han's start.
#[derive(Debug, Clone)]
pub struct DuoLevel {
    pub level: Level,
    pub han_start: (usize, usize),
}

impl DuoLevel {
    /// The usual format (`level` module docs) with a `han: <col>,<row>` header line, 18 rows.
    pub fn parse(src: &str) -> Result<DuoLevel, String> {
        let mut han = None;
        let mut rest = String::new();
        let mut in_header = true;
        for line in src.lines() {
            if line.trim() == "---" {
                in_header = false;
            }
            match line.split_once(':') {
                Some((k, v)) if in_header && k.trim() == "han" => {
                    let (c, r) = v.trim().split_once(',').ok_or("han: <col>,<row>")?;
                    han = Some((c.trim().parse().map_err(|_| "bad han col")?, r.trim().parse().map_err(|_| "bad han row")?));
                }
                _ => {
                    rest.push_str(line);
                    rest.push('\n');
                }
            }
        }
        let level = Level::parse(&rest)?;
        if level.height != ROWS {
            return Err(format!("a duo level is {ROWS} rows tall, not {}", level.height));
        }
        Ok(DuoLevel { level, han_start: han.ok_or("no `han:` start")? })
    }

    pub fn load(n: usize) -> DuoLevel {
        DuoLevel::parse(DUO_SOURCES[n]).unwrap_or_else(|e| panic!("duo level {}: {e}", n + 1))
    }
}

/// Start duo level `n` (0-based).
pub fn start(commands: &mut Commands, n: usize, rule: DeathRule) {
    let d = DuoLevel::load(n);
    commands.insert_resource(Duo::new(n, d.han_start, rule));
    commands.insert_resource(GeneratedLevel(d.level));
    commands.insert_resource(super::AssistMode::Manual);
    commands.insert_resource(HanNav::side_b());
    commands.queue(|w: &mut World| w.resource_mut::<NextState<AppState>>().set(AppState::Playing));
}

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Duo>().add_systems(Update, (toggle_rule, replace_on_restart).run_if(resource_exists::<Duo>)).add_systems(
        FixedUpdate,
        (
            place_han.in_set(GameSet::Prepare).after(super::lifecycle::tick_clocks),
            (han_fate, nat_window, respawned).chain().in_set(GameSet::Interact).after(super::hazards::respawn),
            follow_han.in_set(GameSet::Follow),
        )
            .run_if(resource_exists::<Duo>),
    );
    for s in [AppState::Title, AppState::LevelSelect, AppState::FreePlaySetup] {
        app.add_systems(OnEnter(s), leave);
    }
    app.add_systems(Startup, dev_start);
}

fn dev_start(mut commands: Commands) {
    if let Some((n, rule)) = asked_for() {
        start(&mut commands, n.saturating_sub(1).min(DUO_SOURCES.len() - 1), rule);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn asked_for() -> Option<(usize, DeathRule)> {
    let n = std::env::var("NATHAN_DUO").ok()?.parse().ok()?;
    let both = std::env::var("NATHAN_DUO_DEATHS").is_ok_and(|v| v == "both");
    Some((n, if both { DeathRule::Both } else { DeathRule::Separate }))
}

#[cfg(target_arch = "wasm32")]
fn asked_for() -> Option<(usize, DeathRule)> {
    let search = web_sys::window()?.location().search().ok()?;
    let kv = |k: &str| search.trim_start_matches('?').split('&').find_map(|kv| kv.strip_prefix(k).map(str::to_string));
    let n = kv("duo=")?.parse().ok()?;
    let both = kv("deaths=").is_some_and(|v| v == "both");
    Some((n, if both { DeathRule::Both } else { DeathRule::Separate }))
}

fn leave(mut commands: Commands, duo: Option<Res<Duo>>, mut mode: ResMut<super::AssistMode>) {
    if duo.is_some() {
        commands.remove_resource::<Duo>();
        commands.remove_resource::<GeneratedLevel>();
        commands.insert_resource(HanNav::default());
        *mode = super::AssistMode::Story;
    }
}

/// Tab: switch the death rule (the spike's toggle; `ui::duo` says which it is).
fn toggle_rule(keys: Option<Res<ButtonInput<KeyCode>>>, mut duo: ResMut<Duo>) {
    if keys.is_some_and(|k| k.just_pressed(KeyCode::Tab)) {
        duo.rule = match duo.rule {
            DeathRule::Separate => DeathRule::Both,
            DeathRule::Both => DeathRule::Separate,
        };
    }
}

fn replace_on_restart(mut restarts: MessageReader<RestartLevel>, mut duo: ResMut<Duo>) {
    if restarts.read().count() > 0 {
        *duo = Duo { han_deaths: duo.han_deaths, ..Duo::new(duo.level, duo.han_start, duo.rule) };
    }
}

/// Which storey a point is in: Nat's (top) or Han's (bottom).
fn top_storey(level: &Level, p: Vec2) -> bool {
    level.cell_at(p).1 < SLAB.0 as i32
}

/// A spot to stand on in the storey `top`, at column `from` or the nearest one on (`dir`) from
/// it: an empty cell over a floor, no brushes or sewage.
pub fn stand_spot(level: &Level, from: i32, dir: i32, top: bool) -> Option<Vec2> {
    let rows = if top { 0..SLAB.0 as i32 - 1 } else { SLAB.1 as i32 + 1..ROWS as i32 - 1 };
    let ok = |c: i32, r: i32| {
        let below = level.tile(c, r + 1);
        level.tile(c, r) == Tile::Empty && (below.is_solid() || below.is_one_way())
    };
    (0..level.width as i32)
        .map(|k| from + k * dir)
        .take_while(|c| (0..level.width as i32).contains(c))
        .find_map(|c| rows.clone().rev().find(|&r| ok(c, r)).map(|r| super::stand_pos(level, c as usize, r as usize)))
}

fn put_han(pos: &mut Pos, prev: &mut PrevPos, body: &mut Body, brain: &mut HanBrain, at: Vec2) {
    pos.0 = at;
    prev.0 = at;
    *body = Body { on_ground: true, ..Body::player() };
    *brain = HanBrain::default();
}

/// Han at his start once the level's (re)loaded, and running (unless he's down).
#[allow(clippy::type_complexity)]
fn place_han(
    active: Res<ActiveLevel>,
    mut duo: ResMut<Duo>,
    mut commands: Commands,
    nav: Option<Res<HanNav>>,
    mut han: Query<(&mut Pos, &mut PrevPos, &mut Body, &mut HanBrain), With<Han>>,
) {
    let Ok((mut pos, mut prev, mut body, mut brain)) = han.single_mut() else {
        return;
    };
    if !nav.is_some_and(|n| n.side_b) {
        commands.insert_resource(HanNav::side_b());
    }
    let level = &active.level;
    if !duo.placed {
        let at = super::stand_pos(level, duo.han_start.0, duo.han_start.1);
        put_han(&mut pos, &mut prev, &mut body, &mut brain, at);
        duo.placed = true;
        duo.view_x = at.x;
        duo.han_best = at.x;
    }
    // At the plunger he stops (and waits for Nat).
    let goal = level.tile_center(level.goal.0, level.goal.1).x;
    let home = pos.0.x >= goal - TILE / 2.0 && !top_storey(level, pos.0);
    brain.charge = (duo.han_down.is_none() && !home).then_some(1.0);
}

/// How Han meets his end (Side B's fates), and how he's back: where he last stood (separate),
/// or, both rewinding, with Nat at the checkpoint (`respawned`).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn han_fate(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<SimClock>,
    active: Res<ActiveLevel>,
    mut duo: ResMut<Duo>,
    mut run: ResMut<LevelRun>,
    mut han: Query<(&mut Pos, &mut PrevPos, &mut Body, &mut HanBrain), (With<Han>, Without<Player>)>,
    nat: Query<(Entity, &Pos, Has<Dead>, Has<Finished>), (With<Player>, Without<Han>)>,
    mut says: MessageWriter<HanSays>,
    mut died: MessageWriter<PlayerDied>,
) {
    let dt = time.delta_secs();
    let level = &active.level;
    let Ok((mut pos, mut prev, mut body, mut brain)) = han.single_mut() else {
        return;
    };
    if let Some(t) = duo.han_down {
        if duo.rule == DeathRule::Both {
            // (He's back when Nat is: `respawned`.)
            return;
        }
        if t - dt > 0.0 {
            duo.han_down = Some(t - dt);
            return;
        }
        let at = duo.han_safe.filter(|p| p.x > duo.view_left() + 2.0 * TILE).or_else(|| {
            let col = ((duo.view_left() + 3.0 * TILE) / TILE) as i32;
            stand_spot(level, col, 1, duo.han_safe.is_some_and(|p| top_storey(level, p)))
        });
        let at = at.unwrap_or_else(|| super::stand_pos(level, duo.han_start.0, duo.han_start.1));
        put_han(&mut pos, &mut prev, &mut body, &mut brain, at);
        duo.han_down = None;
        duo.han_best = at.x;
        duo.han_stall = 0.0;
        return;
    }
    if clock.time < 0.1 {
        return;
    }
    if body.on_ground && body.riding.is_none() && !super::sideb::on_spikes(level, pos.0, body.half) {
        duo.han_safe = Some(pos.0);
    }
    if brain.charge.is_some() && pos.0.x > duo.han_best + TILE / 2.0 {
        duo.han_best = pos.0.x;
        duo.han_stall = 0.0;
    } else if brain.charge.is_some() {
        duo.han_stall += dt;
    } else {
        duo.han_stall = 0.0;
    }
    use super::sideb::Fate;
    let fate = if matches!(brain.mode, HanMode::Sinking { .. } | HanMode::Gone { .. }) {
        Some(if pos.0.y + HALF.y < 0.0 { Fate::Pit } else { Fate::Sewage })
    } else if pos.0.y + body.half.y < 0.0 {
        Some(Fate::Pit)
    } else if super::sideb::on_spikes(level, pos.0, body.half) {
        Some(Fate::Spikes)
    } else if duo.han_stall > HAN_STALL_SECS {
        Some(Fate::Bonk)
    } else {
        None
    };
    let Some(f) = fate else { return };
    if matches!(f, Fate::Spikes | Fate::Bonk) {
        brain.mode = HanMode::Sinking { t: 30.0 };
    }
    brain.charge = None;
    duo.han_down = Some(HAN_BACK_SECS);
    duo.han_deaths += 1;
    says.write(HanSays { text: f.line().to_string() });
    // Both rewinding: Nat goes down with him.
    if duo.rule == DeathRule::Both
        && let Ok((e, npos, false, false)) = nat.single()
    {
        kill_nat(&mut commands, &mut run, &mut died, level, e, npos.0, DeathCause::Pit);
    }
}

fn kill_nat(
    commands: &mut Commands,
    run: &mut LevelRun,
    died: &mut MessageWriter<PlayerDied>,
    level: &Level,
    e: Entity,
    at: Vec2,
    cause: DeathCause,
) {
    commands.entity(e).insert(Dead { remaining: super::tuning::RESPAWN_DELAY });
    run.deaths += 1;
    let skill = super::hazards::death_skill(level, at, cause);
    died.write(PlayerDied { pos: at, cause, skill, slip: false });
}

/// The window's edges: Nat can't run past the right one, and off the left one he's left behind.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn nat_window(
    mut commands: Commands,
    active: Res<ActiveLevel>,
    mut duo: ResMut<Duo>,
    mut run: ResMut<LevelRun>,
    mut nat: Query<(Entity, &mut Pos, &mut Body), (With<Player>, Without<Han>, Without<Dead>, Without<Finished>)>,
    mut says: MessageWriter<HanSays>,
    mut died: MessageWriter<PlayerDied>,
) {
    let Ok((e, mut pos, mut body)) = nat.single_mut() else {
        return;
    };
    let level = &active.level;
    let right = duo.view_right() - body.half.x;
    if pos.0.x > right {
        pos.0.x = right;
        body.vel.x = body.vel.x.min(0.0);
    }
    if body.on_ground && body.riding.is_none() {
        duo.nat_safe = Some(pos.0);
    }
    if pos.0.x + body.half.x < duo.view_left() {
        says.write(HanSays { text: LEFT_BEHIND_LINE.to_string() });
        kill_nat(&mut commands, &mut run, &mut died, level, e, pos.0, DeathCause::Pit);
    }
}

/// Nat's back (the story's respawn put him at the checkpoint): separate, he's back where he
/// last stood instead, in the window; both, Han's back too, under (or over) him.
#[allow(clippy::type_complexity)]
fn respawned(
    active: Res<ActiveLevel>,
    mut duo: ResMut<Duo>,
    mut back: MessageReader<PlayerRespawned>,
    mut nat: Query<(&mut Pos, &mut PrevPos), (With<Player>, Without<Han>)>,
    mut han: Query<(&mut Pos, &mut PrevPos, &mut Body, &mut HanBrain), (With<Han>, Without<Player>)>,
) {
    let Some(at) = back.read().last().map(|r| r.pos) else {
        return;
    };
    let level = &active.level;
    match duo.rule {
        DeathRule::Separate => {
            let Ok((mut pos, mut prev)) = nat.single_mut() else {
                return;
            };
            let spot = duo.nat_safe.filter(|p| p.x > duo.view_left() + 2.0 * TILE).or_else(|| {
                let col = ((duo.view_left() + 3.0 * TILE) / TILE) as i32;
                stand_spot(level, col, 1, duo.nat_safe.is_none_or(|p| top_storey(level, p)))
            });
            if let Some(p) = spot {
                pos.0 = p;
                prev.0 = p;
            }
        }
        DeathRule::Both => {
            let Ok((mut pos, mut prev, mut body, mut brain)) = han.single_mut() else {
                return;
            };
            let col = level.cell_at(at).0;
            let spot = if level.cell_at(at) == (level.start.0 as i32, level.start.1 as i32) {
                Some(super::stand_pos(level, duo.han_start.0, duo.han_start.1))
            } else {
                stand_spot(level, col, 1, !top_storey(level, at))
            };
            let spot = spot.unwrap_or_else(|| super::stand_pos(level, duo.han_start.0, duo.han_start.1));
            put_han(&mut pos, &mut prev, &mut body, &mut brain, spot);
            duo.han_down = None;
            duo.han_best = spot.x;
            duo.han_stall = 0.0;
            duo.view_x = spot.x;
        }
    }
}

/// The window rides Han (clamped to the level), catching up smoothly when he pops back in.
fn follow_han(time: Res<Time>, active: Res<ActiveLevel>, mut duo: ResMut<Duo>, han: Query<&Pos, With<Han>>) {
    let Ok(pos) = han.single() else { return };
    if duo.han_down.is_some() {
        return;
    }
    let w = active.level.size_px().x;
    let want = if w <= 2.0 * VIEW_HALF_W { w / 2.0 } else { pos.0.x.clamp(VIEW_HALF_W, w - VIEW_HALF_W) };
    duo.view_x = super::move_towards(duo.view_x, want, VIEW_SPEED * time.delta_secs());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duo_levels_parse() {
        for n in 0..DUO_SOURCES.len() {
            let d = DuoLevel::load(n);
            assert_eq!(d.level.height, ROWS);
            let l = &d.level;
            assert!(l.tile(d.han_start.0 as i32, d.han_start.1 as i32 + 1).is_solid(), "Han starts on a floor");
            assert!(d.han_start.1 > SLAB.1 && l.start.1 < SLAB.0, "Nat up top, Han down below");
        }
    }

    #[test]
    fn stand_spots_stay_in_their_storey() {
        let d = DuoLevel::load(0);
        let l = &d.level;
        let top = stand_spot(l, 10, 1, true).unwrap();
        let bottom = stand_spot(l, 10, 1, false).unwrap();
        assert!(top_storey(l, top) && !top_storey(l, bottom));
    }
}

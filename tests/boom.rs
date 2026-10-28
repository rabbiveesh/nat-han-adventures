//! The Boom (`B`, beans `:`) and sploosh diapers (`D`), headless: catching a ride, steering,
//! beans in the tank, running dry, hopping off, splatting the Boom on spikes, and a diaper
//! dropped on a Nat who stands still (and missing one who stops at its whistle).

use std::time::Duration;

use bevy::{input::InputPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use leafwing_input_manager::prelude::*;
use nat_han_adventures::{
    events::{DeathCause, PlayerDied},
    game::{
        Player, Pos,
        boom::{Aboard, Boom, BoomState},
        diapers::{Diaper, DiaperState},
    },
    level::{BEAN_FUEL, BOOM_FUEL, Level, Levels, TILE, ThingKind},
    state::AppState,
};

const DT: f64 = 1.0 / 60.0;
const SPACE: KeyCode = KeyCode::Space;
const RIGHT: KeyCode = KeyCode::ArrowRight;
const LEFT: KeyCode = KeyCode::ArrowLeft;
const UP: KeyCode = KeyCode::ArrowUp;
const DOWN: KeyCode = KeyCode::ArrowDown;

#[derive(Resource, Default)]
struct Deaths(Vec<DeathCause>);

fn app(level: &str) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)))
        .add_plugins(nat_han_adventures::gameplay)
        .insert_resource(Levels(vec![Level::parse(level).expect("test level parses")]))
        .init_resource::<Deaths>()
        .add_systems(Last, |mut d: ResMut<Deaths>, mut r: MessageReader<PlayerDied>| d.0.extend(r.read().map(|m| m.cause)));
    app.finish();
    app.cleanup();
    app.update();
    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Playing);
    app.update();
    app.update();
    app
}

fn step(app: &mut App, secs: f32) {
    for _ in 0..(secs as f64 / DT).round() as usize {
        app.update();
    }
}

fn nat(app: &mut App) -> Vec2 {
    app.world_mut().query_filtered::<&Pos, With<Player>>().single(app.world()).unwrap().0
}

fn aboard(app: &mut App) -> bool {
    app.world_mut().query_filtered::<(), (With<Player>, With<Aboard>)>().iter(app.world()).count() == 1
}

fn boom(app: &mut App) -> Boom {
    *app.world_mut().query::<&Boom>().single(app.world()).unwrap()
}

fn deaths(app: &App) -> Vec<DeathCause> {
    app.world().resource::<Deaths>().0.clone()
}

fn press(app: &mut App, k: KeyCode) {
    k.press(app.world_mut());
}

fn release(app: &mut App, k: KeyCode) {
    k.release(app.world_mut());
}

/// Walk right onto the Boom (parked two tiles along) and get caught.
fn catch(app: &mut App) {
    press(app, RIGHT);
    for _ in 0..120 {
        step(app, 1.0 / 60.0);
        if aboard(app) {
            break;
        }
    }
    release(app, RIGHT);
    assert!(aboard(app), "the Boom caught Nat");
    assert_eq!(boom(app).state, BoomState::Flying);
}

const SKY: &str = "name: Sky
solo: nat
---
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
.P..B........................................G
##############################################
";

#[test]
fn the_boom_catches_nat_and_flies_where_you_steer() {
    let mut app = app(SKY);
    step(&mut app, 0.3);
    catch(&mut app);
    let start = nat(&mut app);
    press(&mut app, UP);
    step(&mut app, 1.0);
    release(&mut app, UP);
    let up = nat(&mut app);
    assert!(up.y > start.y + 4.0 * TILE, "Up flies up: {start} -> {up}");
    assert!(aboard(&mut app), "Up (a jump key too) steers, it doesn't hop off");
    // Let go: it hovers.
    step(&mut app, 0.5);
    let hover = nat(&mut app);
    step(&mut app, 0.5);
    assert!((nat(&mut app) - hover).length() < 1.0, "hovers with nothing held");
    press(&mut app, RIGHT);
    press(&mut app, DOWN);
    step(&mut app, 0.5);
    release(&mut app, RIGHT);
    release(&mut app, DOWN);
    let dr = nat(&mut app);
    assert!(dr.x > hover.x + 2.0 * TILE && dr.y < hover.y - 2.0 * TILE, "down and right: {hover} -> {dr}");
    // Down steered: it didn't call the band (nothing to check here but that Nat's still aboard).
    assert!(aboard(&mut app));
    assert!(deaths(&app).is_empty());
}

#[test]
fn a_dry_tank_drops_nat_and_the_boom_goes_home() {
    let mut app = app(SKY);
    step(&mut app, 0.3);
    catch(&mut app);
    press(&mut app, UP);
    step(&mut app, 0.8);
    release(&mut app, UP);
    step(&mut app, BOOM_FUEL - 1.2);
    assert!(aboard(&mut app), "still flying just before the tank's dry");
    step(&mut app, 0.6);
    assert!(!aboard(&mut app), "dry: Nat's dropped");
    assert!(matches!(boom(&mut app).state, BoomState::Home { .. }));
    step(&mut app, 4.0);
    assert_eq!(boom(&mut app).state, BoomState::Parked, "flown home");
    assert!(deaths(&app).is_empty(), "a drop to the floor is no death: {:?}", deaths(&app));
}

#[test]
fn space_hops_off() {
    let mut app = app(SKY);
    step(&mut app, 0.3);
    catch(&mut app);
    press(&mut app, UP);
    step(&mut app, 0.4);
    release(&mut app, UP);
    // (Up is a jump key too: let go of it first, or Space isn't a new jump.)
    step(&mut app, 0.1);
    let before = nat(&mut app);
    press(&mut app, SPACE);
    step(&mut app, 0.2);
    release(&mut app, SPACE);
    assert!(!aboard(&mut app));
    assert!(nat(&mut app).y > before.y, "a jump off it");
}

const BEANS: &str = "name: Beans
solo: nat
---
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
.....::::.....................................
..............................................
.P..B........................................G
##############################################
";

#[test]
fn beans_top_up_the_tank_and_come_back_when_it_parks() {
    let mut app = app(BEANS);
    assert_eq!(Level::parse(BEANS).unwrap().things.iter().filter(|t| t.kind == ThingKind::Bean).count(), 4);
    step(&mut app, 0.3);
    catch(&mut app);
    // Up a tile and a half and along through the beans.
    press(&mut app, UP);
    step(&mut app, 0.25);
    release(&mut app, UP);
    press(&mut app, RIGHT);
    step(&mut app, 0.8);
    release(&mut app, RIGHT);
    let fuel = boom(&mut app).fuel;
    assert!(fuel > BOOM_FUEL - 1.2 - 0.1, "beans topped it up (never past full): {fuel}");
    let eaten = app.world_mut().query::<&nat_han_adventures::game::boom::Bean>().iter(app.world()).filter(|b| b.eaten).count();
    assert!(eaten >= 2, "flew through the beans: {eaten} eaten");
    // Hop off; once it's parked they're back.
    press(&mut app, SPACE);
    step(&mut app, 0.1);
    release(&mut app, SPACE);
    step(&mut app, 4.0);
    assert_eq!(boom(&mut app).state, BoomState::Parked);
    let eaten = app.world_mut().query::<&nat_han_adventures::game::boom::Bean>().iter(app.world()).filter(|b| b.eaten).count();
    assert_eq!(eaten, 0, "the beans are back");
    let _ = BEAN_FUEL;
}

const SPIKES: &str = "name: Spikes
solo: nat
---
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
..............................................
.P..B.........................................
######^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^#####G
";

#[test]
fn spikes_the_boom_touches_splat_nat() {
    let mut app = app(SPIKES);
    step(&mut app, 0.3);
    catch(&mut app);
    press(&mut app, RIGHT);
    step(&mut app, 0.5);
    release(&mut app, RIGHT);
    // Nat's box alone is clear of them; the Boom's under him isn't.
    press(&mut app, DOWN);
    step(&mut app, 0.5);
    release(&mut app, DOWN);
    assert_eq!(deaths(&app).first(), Some(&DeathCause::Spikes), "{:?}", deaths(&app));
    step(&mut app, 1.0);
    assert!(!aboard(&mut app));
    assert_eq!(boom(&mut app).state, BoomState::Parked, "back home after the crash");
}

const DIAPER: &str = "name: Diaper
solo: nat
---
..............................................
..............................................
..............................................
..............................................
..........D...................................
..............................................
..............................................
..............................................
..............................................
..............................................
..........P...................................G
##############################################
";

fn diaper(app: &mut App) -> Diaper {
    *app.world_mut().query::<&Diaper>().single(app.world()).unwrap()
}

#[test]
fn a_diaper_drops_on_nat_standing_under_its_swing() {
    let mut app = app(DIAPER);
    let mut dropped = false;
    for _ in 0..(8 * 60) {
        step(&mut app, 1.0 / 60.0);
        dropped |= matches!(diaper(&mut app).state, DiaperState::Falling { .. });
        if !deaths(&app).is_empty() {
            break;
        }
    }
    assert!(dropped, "it let go");
    assert_eq!(deaths(&app).first(), Some(&DeathCause::Diaper), "{:?}", deaths(&app));
}

#[test]
fn stopping_at_the_whistle_dodges_a_diaper() {
    let mut app = app(DIAPER);
    step(&mut app, 0.3);
    // Run left, then come back right under it; stop the moment it drops.
    press(&mut app, LEFT);
    step(&mut app, 0.8);
    release(&mut app, LEFT);
    press(&mut app, RIGHT);
    let mut dropped = false;
    for _ in 0..(3 * 60) {
        step(&mut app, 1.0 / 60.0);
        if matches!(diaper(&mut app).state, DiaperState::Falling { .. }) {
            dropped = true;
            break;
        }
    }
    release(&mut app, RIGHT);
    assert!(dropped, "it aimed at the runner");
    step(&mut app, 1.5);
    assert!(matches!(diaper(&mut app).state, DiaperState::Splooshed { .. }), "SPLOOSH");
    assert!(deaths(&app).is_empty(), "stopped, it landed in front: {:?}", deaths(&app));
}

#[test]
fn the_playground_parses() {
    let level = Level::parse(include_str!("../levels/playground/boom.txt")).expect("boom.txt parses");
    assert!(level.things.iter().any(|t| t.kind == ThingKind::Boom));
    assert!(level.things.iter().any(|t| t.kind == ThingKind::Diaper));
}

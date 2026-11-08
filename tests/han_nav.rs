//! Han's navigation graph on the campaign: routes exist, and what it costs.
//! `cargo test --release --test han_nav -- --nocapture` prints the timings.

use std::time::Instant;

use nat_han_adventures::level::nav::{Nav, Route};
use nat_han_adventures::level::validate::{Map, Mode};
use nat_han_adventures::level::{LEVEL_SOURCES, Level};

#[test]
fn nav_graph_costs() {
    let mut total_nodes = 0;
    let t_all = Instant::now();
    for (i, src) in LEVEL_SOURCES.iter().enumerate() {
        let level = Level::parse(src).unwrap();
        let t0 = Instant::now();
        let map = Map::for_han(&level);
        let t_map = t0.elapsed();
        let mut nav = Nav::new(Mode::Normal);
        let t1 = Instant::now();
        let nodes = nav.build_all(&map);
        let t_build = t1.elapsed();
        total_nodes += nodes;
        // A long route over the warm cache.
        let start = (level.start.0 as i32, level.start.1 as i32);
        let t2 = Instant::now();
        let mut far = start;
        for r in 0..level.height as i32 {
            for c in 0..level.width as i32 {
                if Nav::node(&map, &level, (c, r)) && (c - start.0).abs() > (far.0 - start.0).abs() && (c - start.0).abs() < 40 {
                    far = (c, r);
                }
            }
        }
        let route = nav.route(&map, start, far, usize::MAX);
        let t_route = t2.elapsed();
        eprintln!(
            "level {:2}: map {:>8.2?}, {nodes:5} nodes built in {:>8.2?} ({:.1} µs/node), route {:?}->{:?} {} in {:>8.2?}",
            i + 1,
            t_map,
            t_build,
            t_build.as_secs_f64() * 1e6 / nodes.max(1) as f64,
            start,
            far,
            match &route {
                Route::Found(p) => format!("{} edges", p.len()),
                r => format!("{r:?}"),
            },
            t_route
        );
    }
    eprintln!("all: {total_nodes} nodes in {:.2?}", t_all.elapsed());
}

/// A cold route (lazy: only the cells A* reaches get simulated) on every level.
#[test]
fn lazy_routes_are_cheap() {
    for src in LEVEL_SOURCES {
        let level = Level::parse(src).unwrap();
        let map = Map::for_han(&level);
        let mut nav = Nav::new(Mode::Normal);
        let start = (level.start.0 as i32, level.start.1 as i32);
        let to = (start.0 + 6, start.1);
        let t = Instant::now();
        let r = nav.route(&map, start, to, 400);
        eprintln!("{}: cold 6-tile route {:?} in {:.2?}, {} cells simulated", level.name, matches!(r, Route::Found(_)), t.elapsed(), nav.expanded);
    }
}

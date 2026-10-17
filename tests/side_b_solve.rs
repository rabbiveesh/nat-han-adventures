//! The Side B prover (`game::sideb_solve`): Han replays a plan in the real gameplay, and the
//! search finds one. The levels' own plans live in `tests/side_b.rs`; two are replayed here
//! through the prover, written as `solution:` lines.

use nat_han_adventures::game::{
    sideb::{Fate, SideBLevel},
    sideb_solve::{self, Act, SolveOpts, Solved, Step},
};
use std::sync::atomic::AtomicBool;

fn level(n: usize) -> SideBLevel {
    SideBLevel::load(n)
}

#[test]
fn plans_read_and_write_as_text() {
    let b = level(0); // giant, gap
    let plan = sideb_solve::parse_plan(&b, "3/giant 22/gap 30/pick-giant").unwrap();
    assert_eq!(plan[1], Step { col: 22.0, act: Act::Call(1) });
    assert_eq!(plan[2].act, Act::Pick(0));
    assert_eq!(sideb_solve::plan_text(&b, &plan), "3/giant 22/gap 30/pick-giant");
    assert!(sideb_solve::parse_plan(&b, "3/melt").is_err(), "not on the set list");
    let (rest, sol) = sideb_solve::split_solution("name: x\nsolution: 3/giant\nsetlist: giant=1\n---\nP.G\n###\n");
    assert_eq!(sol.as_deref(), Some("3/giant"));
    assert!(!rest.contains("solution"));
}

#[test]
fn a_levels_solution_clears_it_and_a_wrong_one_doesnt() {
    let b = level(0);
    // Over the deck (`ROUTES` in tests/side_b.rs).
    let good = sideb_solve::parse_plan(&b, "7/giant 52/giant 75/pick-gap 95/pick-giant 106/gap 136/giant").unwrap();
    let r = sideb_solve::play(&b, &good);
    assert!(r.cleared, "{r:?}");
    let r = sideb_solve::play(&b, &good[..4]);
    assert!(!r.cleared);
    assert!(r.fate.is_some() && r.col > 100.0, "no fired up for the gap: {r:?}");
    let r = sideb_solve::play(&b, &[]);
    assert_eq!(r.fate, Some(Fate::Bonk), "no music: the first wall");
}

#[test]
fn rush_hour_replays_through_the_prover() {
    let b = level(4); // giant, gap, melt; under, under
    let plan = sideb_solve::parse_plan(
        &b,
        "4/pick-melt 6/pick-gap 8/giant 83/giant 98/melt 101/gap 140/pick-melt 147/melt 186/melt 189/pick-giant 230/gap 252/giant",
    )
    .unwrap();
    assert!(sideb_solve::play(&b, &plan).cleared);
}

/// The search on Down the Drain (~40 s here; plays run in parallel):
/// `cargo test --test side_b_solve -- --ignored --nocapture`.
#[test]
#[ignore]
fn the_search_finds_down_the_drain() {
    let b = level(0);
    let t = std::time::Instant::now();
    let found = sideb_solve::solve(&b, SolveOpts { every: 4.0, ..SolveOpts::default() }, &|p| eprintln!("{p:?}"), &AtomicBool::new(false));
    eprintln!("{:.1}s", t.elapsed().as_secs_f32());
    let Solved::Cleared(plan) = found else { panic!("no plan: {found:?}") };
    assert!(sideb_solve::play(&b, &plan).cleared, "{}", sideb_solve::plan_text(&b, &plan));
}

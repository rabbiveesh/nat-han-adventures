//! Tier 1 formal checks (fast, part of every `cargo test`): the band director's state machine
//! explored exhaustively to a bounded depth, and its properties proved from every reachable
//! state. The model, the properties and the trace format are in `tests/formal_model`; tier 2
//! (`tests/deep.rs`, ignored) goes deeper and adds the levels.
//!
//! A failure prints the shortest counterexample, e.g.
//! `t=0 nugget ×4 | t=1 death | t=2.25 wait`. To keep a bug fixed, paste the `state:` part into
//! [`regressions`] as `regression("...")`.

mod formal_model;

use std::time::Duration;

use formal_model::*;
use nat_han_adventures::audio::{Filters, Harmony};
use nat_han_adventures::game::Groove;

/// Tier 1's depth (acts from a level start): as deep as fits the budget with room to spare.
const DEPTH: usize = 5;
/// The whole of tier 1 (exploring, every property, the mutants) must take at most this long.
const BUDGET: Duration = Duration::from_secs(2);

fn run(cfg: &Config) -> Result<Coverage, String> {
    explore(cfg).map(|ex| ex.coverage).map_err(|c| c.to_string())
}

/// Every property from every state reachable in [`DEPTH`] acts, and the mutants caught, all
/// within [`BUDGET`] (of CPU time where the OS says, so a busy machine doesn't fail it).
#[test]
fn tier1_director_properties_within_budget() {
    let ((cov, mutants), cpu, wall) = timed(|| {
        let cov = match run(&Config::tier1(DEPTH)) {
            Ok(c) => c,
            Err(e) => panic!(
                "tier 1 found a counterexample:\n{e}\n(to keep it fixed: add the state to `regressions` in tests/formal.rs)"
            ),
        };
        (cov, mutants_caught())
    });
    eprintln!(
        "tier 1: {} states (depth {}, per layer {:?}), properties {} hold from every one, {} mutants caught; {cpu:.2?} CPU, {wall:.2?} wall",
        cov.states,
        cov.depth,
        cov.layers,
        PROPERTIES.iter().map(|p| p.0).collect::<String>(),
        mutants.len(),
    );
    for (m, ce) in &mutants {
        eprintln!("mutant {m:?} caught:\n{ce}");
    }
    assert!(cpu <= BUDGET, "tier 1 took {cpu:.2?} (budget {BUDGET:?}): lower DEPTH in tests/formal.rs");
}

/// The mutants and the counterexample tier 1 finds for each (panics if one survives).
fn mutants_caught() -> Vec<(Mutation, String)> {
    [Mutation::GripOnlyFromMelodicMinor, Mutation::RestartKeepsBand, Mutation::SummonWaitsForCheck]
        .into_iter()
        .map(|m| match explore(&Config::tier1(DEPTH).with(m)) {
            Ok(_) => panic!("mutant {m:?} survived tier 1: the checks have lost their teeth"),
            Err(c) => (m, c.to_string()),
        })
        .collect()
}

/// The old grease-chute soft-lock (grip only from the nervous band, deaths during a held
/// summon not counting) is caught, with a short trace that is about grip.
#[test]
fn tier1_catches_the_old_chute_soft_lock() {
    let ce = explore(&Config::tier1(DEPTH).with(Mutation::GripOnlyFromMelodicMinor)).err().expect("mutant survived");
    eprintln!("{ce}");
    assert_eq!(ce.violation.prop, "a", "{ce}");
    assert!(ce.state.0.len() + ce.violation.witness.len() <= 40, "not short: {ce}");
}

/// Grip from the nervous layer whatever the band plays, laughing band or not (`Groove::new`
/// plus the layer, as the audio plugin writes it).
#[test]
fn the_grip_layer_beats_every_filter() {
    use Harmony::*;
    for harmony in [Original, Coltrane, Quartal, Waltz, MelodicMinor] {
        for just_intonation in [false, true] {
            let f = Filters { harmony, just_intonation };
            assert!(Groove { nervous: true, ..Groove::new(f) }.grip(), "{f:?}");
            assert!(groove_of(f, 3, Mutation::None).grip(), "{f:?}");
            assert_eq!(groove_of(f, 2, Mutation::None).grip(), harmony == MelodicMinor, "{f:?}");
        }
    }
}

/// States the search treats as one (same key) really do behave the same.
#[test]
fn the_abstraction_is_sound() {
    let pairs = check_abstraction(3, 12, 40).unwrap_or_else(|e| panic!("{e}"));
    assert!(pairs > 1000, "only {pairs} pairs compared");
}

/// The trace format round-trips, and replays what it says.
#[test]
fn traces_print_and_parse() {
    let t = Trace(vec![Act::TOOT, Act::TOOT, Act::Wait(8), Act::DEATH, Act::Wait(2), Act::Frame(4 | 2), Act::Wait(3)]);
    let s = t.to_string();
    assert_eq!(s, "t=0 toot ×2 | t=2 death | t=2.5 nugget+death | t=3.25 wait");
    assert_eq!(Trace::parse(&s).unwrap(), t);
    let sim = replay(&Trace::parse("t=0 toot ×5").unwrap(), Mutation::None);
    assert_eq!(sim.harmony(), Harmony::Coltrane);
    assert!(Trace::parse("t=0.1 toot").is_err());
}

/// Counterexamples found and fixed, replayed with every property checked along the way.
/// (Paste a counterexample's `state:` here.)
#[test]
fn regressions() {
    let regression = |s: &str| check_trace(s).unwrap_or_else(|e| panic!("{e}"));
    // The grease chute: a quick nugget line summons the fired-up band, the chute kills thrice
    // while it's held (ghost nuggets keep it alive): grip must still come.
    regression("t=0 nugget ×4 | t=0 death | t=1.25 nugget | t=1.5 death | t=2.75 nugget | t=3 death | t=4 wait");
    // Giant Steps held, deaths at a checkpoint (laughing band), the hold running out.
    regression("t=0 toot ×5 | t=1 death | t=2 death | t=3 death | t=25 wait");
}

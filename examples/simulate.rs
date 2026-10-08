//! Run synthetic players through the adaptive difficulty engine (`nat_han_adventures::adapt`).
//!
//! ```sh
//! cargo run --example simulate -- --profile learner --seed 7 --rooms 200
//! cargo run --example simulate -- --all              # summary table, every profile
//! cargo run --example simulate -- --all --seeds 20   # averaged over 20 seeds
//! ```
//!
//! Profiles: precise, sloppy, nervous_beginner, speedrunner, uneven, learner
//! (see `adapt::sim::PLAYERS`). An example rather than a bin so plain `cargo run` still starts
//! the game.

use nat_han_adventures::adapt::sim::{self, Metrics, SimRun, TARGET, WARMUP};
use nat_han_adventures::adapt::{Cue, Outcome, Skill};

struct Args {
    profile: String,
    seed: u64,
    rooms: usize,
    all: bool,
    seeds: u64,
}

fn parse() -> Args {
    let mut a = Args { profile: "learner".into(), seed: 42, rooms: 200, all: false, seeds: 1 };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    let val = |i: usize| argv.get(i + 1).cloned().unwrap_or_else(|| usage(&format!("missing value for {}", argv[i])));
    while i < argv.len() {
        match argv[i].as_str() {
            "--profile" => {
                a.profile = val(i);
                i += 1;
            }
            "--seed" => {
                a.seed = val(i).parse().unwrap_or_else(|_| usage("bad --seed"));
                i += 1;
            }
            "--rooms" => {
                a.rooms = val(i).parse().unwrap_or_else(|_| usage("bad --rooms"));
                i += 1;
            }
            "--seeds" => {
                a.seeds = val(i).parse::<u64>().unwrap_or_else(|_| usage("bad --seeds")).max(1);
                i += 1;
            }
            "--all" => a.all = true,
            "-h" | "--help" => usage(""),
            other => usage(&format!("unknown argument {other}")),
        }
        i += 1;
    }
    a
}

fn usage(msg: &str) -> ! {
    if !msg.is_empty() {
        eprintln!("{msg}");
    }
    let names: Vec<&str> = sim::PLAYERS.iter().map(|p| p.name).collect();
    eprintln!("usage: simulate [--profile <{}>] [--seed N] [--rooms N] [--all [--seeds N]]", names.join("|"));
    std::process::exit(if msg.is_empty() { 0 } else { 2 });
}

fn main() {
    let a = parse();
    if a.all {
        summary(a.seed, a.seeds, a.rooms);
    } else {
        let p = sim::player(&a.profile).unwrap_or_else(|| usage(&format!("unknown profile {}", a.profile)));
        detail(&sim::run(p, a.seed, a.rooms));
    }
}

fn header(t: &str) {
    println!("\n\x1b[1;33m{}\n  {t}\n{}\x1b[0m", "═".repeat(72), "═".repeat(72));
}

fn centers(c: &[u8; Skill::COUNT]) -> String {
    c.iter().map(|b| format!("{b:>3}")).collect()
}

fn detail(run: &SimRun) {
    let p = run.player;
    println!("\n\x1b[1;36mSimulating: {} ({})\x1b[0m  rooms: {}", p.name, p.about, run.rooms.len());
    header("ROOMS");
    let names: String = Skill::ALL.iter().map(|s| format!("{:>3}", &s.name()[..3])).collect();
    println!("    #  skill      band  result     assist   centers:{names}");
    for (i, r) in run.rooms.iter().enumerate() {
        let mark = match r.outcome {
            Outcome::Clean => "\x1b[32mclean\x1b[0m    ",
            Outcome::Careless => "\x1b[33mslip\x1b[0m     ",
            Outcome::Struggle => "\x1b[31mstruggle\x1b[0m ",
        };
        let notes: Vec<String> = r
            .cues
            .iter()
            .map(|c| match c {
                Cue::Promoted(s, b) => format!("\x1b[42;1m ⬆ {} {b} \x1b[0m", s.name()),
                Cue::Demoted(s, b) => format!("\x1b[41;1m ⬇ {} {b} \x1b[0m", s.name()),
                Cue::Eased(s, b) => format!("\x1b[45;1m ↓ {} {b} \x1b[0m", s.name()),
                Cue::Encourage(sig) => format!("\x1b[35mHan: encourage ({sig:?})\x1b[0m"),
                Cue::Calibrated(pl) => format!(
                    "\x1b[46;1m placed: band {} spread {:.2} assists {:.2} \x1b[0m",
                    pl.band, pl.spread, pl.assists
                ),
            })
            .collect();
        println!(
            "  {:>3}  {:<10} {:>4}  {mark} {:>4.2}     {}  {}",
            i + 1,
            r.request.skill.name(),
            r.request.band,
            r.assists,
            centers(&r.centers),
            notes.join(" ")
        );
    }
    header("BAND TRAJECTORIES (center every 20 rooms)");
    println!("  room  {names}");
    for (i, r) in run.rooms.iter().enumerate() {
        if (i + 1) % 20 == 0 || i + 1 == run.rooms.len() {
            println!("  {:>4}  {}", i + 1, centers(&r.centers));
        }
    }
    header("SUMMARY");
    let m = run.metrics();
    println!("  clean-clear rate      {:>5.1}%  (after {} warm-up rooms: {:.1}%)", 100.0 * m.clean_rate, WARMUP, 100.0 * m.clean_after_warmup);
    println!("  time near target      {:>5.1}%  (trailing-20 clean in {:.0}–{:.0}%)", 100.0 * m.time_near_target, 100.0 * TARGET.0, 100.0 * TARGET.1);
    println!("  oscillations          {:>5}  (reversals within {} rooms; {} reversals in all)", m.oscillations, sim::OSC_SPAN, m.reversals);
    println!("  promotions/demotions  {:>5} / {}", m.promotions, m.demotions);
    println!("  frustration events    {:>5}", m.frustrations);
    println!("  mean assists          {:>5.2}  (maxed {:.1}% of rooms)", m.mean_assists, 100.0 * m.maxed_assists);
    println!("  final centers         {}", centers(&m.final_centers));
}

fn summary(seed: u64, seeds: u64, rooms: usize) {
    println!(
        "\n{rooms} rooms, seed {seed}{}; warm-up {WARMUP}; target {:.0}–{:.0}% clean\n",
        if seeds > 1 { format!("..{} (averaged over {seeds})", seed + seeds - 1) } else { String::new() },
        100.0 * TARGET.0,
        100.0 * TARGET.1
    );
    let names: Vec<&str> = Skill::ALL.iter().map(|s| &s.name()[..3]).collect();
    println!("final centers (last seed) per skill: {}\n", names.join(" "));
    println!("| profile          | clean | warm clean | near target | osc | rev | prom/dem  | frust | assists | maxed | final centers           |");
    println!("|------------------|-------|------------|-------------|-----|-----|-----------|-------|---------|-------|-------------------------|");
    for p in sim::PLAYERS {
        let ms: Vec<Metrics> = (0..seeds).map(|k| sim::run(p, seed + k, rooms).metrics()).collect();
        let n = ms.len() as f32;
        let avg = |f: &dyn Fn(&Metrics) -> f32| ms.iter().map(f).sum::<f32>() / n;
        let last = ms.last().unwrap();
        println!(
            "| {:<16} | {:>4.0}% | {:>9.0}% | {:>10.0}% | {:>3.1} | {:>3.1} | {:>4.1}/{:<4.1} | {:>5.1} | {:>7.2} | {:>4.0}% | {} |",
            p.name,
            100.0 * avg(&|m| m.clean_rate),
            100.0 * avg(&|m| m.clean_after_warmup),
            100.0 * avg(&|m| m.time_near_target),
            avg(&|m| m.oscillations as f32),
            avg(&|m| m.reversals as f32),
            avg(&|m| m.promotions as f32),
            avg(&|m| m.demotions as f32),
            avg(&|m| m.frustrations as f32),
            avg(&|m| m.mean_assists),
            100.0 * avg(&|m| m.maxed_assists),
            centers(&last.final_centers),
        );
    }
}

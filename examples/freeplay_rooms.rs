//! Free-play room generator report: validity and timing per template and band.
//!
//! ```sh
//! cargo run --release --example freeplay_rooms -- [seeds]
//! cargo run --release --example freeplay_rooms -- --dump <template index> <band> <seed>
//! ```

use std::time::Instant;

use nat_han_adventures::adapt::{AssistLevers, RoomRequest};
use nat_han_adventures::freeplay::generate::{RoomPlan, draw, physics, validate};
use nat_han_adventures::freeplay::templates::TEMPLATES;
use nat_han_adventures::level::validate::{Options, check_with};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let t0 = Instant::now();
    physics();
    println!("physics tables: {:.1?}", t0.elapsed());
    if args.first().map(String::as_str) == Some("--dump") {
        let ti: usize = args[1].parse().unwrap();
        let band: u8 = args[2].parse().unwrap();
        let seed: u32 = args[3].parse().unwrap();
        let plan = plan(ti, band, seed);
        let level = draw(seed, &plan, 0);
        let r = validate(&level);
        let mut l = level.clone();
        l.deaths = Some(r.deaths);
        let rep = check_with(&l, &Options { dump: true }, physics());
        println!("{}\nerrors: {:#?}", rep.dump.unwrap_or_default(), r.errs);
        return;
    }
    let seeds: u32 = args.first().and_then(|s| s.parse().ok()).unwrap_or(20);
    for (ti, t) in TEMPLATES.iter().enumerate() {
        for band in [1u8, 5, 10] {
            let (mut ok, mut worst, mut total) = (0, 0u128, 0u128);
            let mut first_err = None;
            for seed in 0..seeds {
                let plan = plan(ti, band, seed);
                let s = Instant::now();
                let level = draw(seed, &plan, 0);
                let r = validate(&level);
                let us = s.elapsed().as_micros();
                worst = worst.max(us);
                total += us;
                if r.errs.is_empty() {
                    ok += 1;
                } else if first_err.is_none() {
                    first_err = Some((seed, r.errs.clone()));
                }
            }
            println!(
                "{:18} band {band:2}: {ok:3}/{seeds} valid first try, mean {:6.2} ms, worst {:6.2} ms  {}",
                t.name,
                total as f64 / seeds as f64 / 1000.0,
                worst as f64 / 1000.0,
                first_err.map(|(s, e)| format!("seed {s}: {}", e.join(" | "))).unwrap_or_default()
            );
        }
    }
}

fn plan(ti: usize, band: u8, seed: u32) -> RoomPlan {
    let request = RoomRequest { skill: TEMPLATES[ti].skill, band, assists: AssistLevers::NONE };
    RoomPlan { template: ti, hint: true, ..RoomPlan::new(seed, 5, request, 2, false, false, 10) }
}

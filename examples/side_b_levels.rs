//! Builds Side B's levels B1-B7 from the building blocks (`freeplay::blocks`) and writes them,
//! with every route's plan: `cargo run --example side_b_levels [dir]` (default `levels/side-b`)
//! writes `b1.txt`..`b7.txt` and `routes.txt` (`b1 shelf: <plan>` lines, which
//! `tests/side_b.rs` plays). Each level is built once per route (its forks' picks): the grids
//! must come out the same, only the plans differ.

use nat_han_adventures::freeplay::blocks::{Beat, Builder, Call, Deck, Low};
use std::fmt::Write as _;

struct Def {
    file: &'static str,
    name: &'static str,
    setlist: &'static [Call],
    h: usize,
    top: i32,
    routes: &'static [(&'static str, &'static [u32])],
    build: fn(&mut Builder),
}

use Call::*;

const LEVELS: &[Def] = &[
    Def {
        file: "b1",
        name: "Down the Drain",
        setlist: &[Giant, Gap, Melt],
        h: 30,
        top: 12,
        routes: &[("shelf", &[0]), ("under", &[1])],
        build: b1,
    },
    Def {
        file: "b2",
        name: "Spike Alley",
        setlist: &[Giant, Gap, Melt],
        h: 26,
        top: 18,
        routes: &[("roof, springboard, roof", &[0, 0, 0]), ("alley, under, alley", &[1, 1, 1]), ("alley, springboard, roof", &[1, 0, 0])],
        build: b2,
    },
    Def {
        file: "b3",
        name: "Drip Pipe",
        setlist: &[Giant, Gap, Melt],
        h: 32,
        top: 10,
        routes: &[("deck, springboard", &[1, 0]), ("pipe, under", &[0, 1])],
        build: b3,
    },
    Def {
        file: "b5",
        name: "Rush Hour",
        setlist: &[Giant, Gap, Melt],
        h: 30,
        top: 14,
        routes: &[("over", &[1]), ("pipe", &[0])],
        build: b5,
    },
    Def {
        file: "b6",
        name: "Double Trouble",
        setlist: &[Gap, Nervous, Melt],
        h: 34,
        top: 26,
        routes: &[("face, over", &[0, 1]), ("under, pipe", &[1, 0])],
        build: b6,
    },
    Def {
        file: "b7",
        name: "The Royal Flush",
        setlist: &[Waltz, Nervous, Melt],
        h: 40,
        top: 16,
        routes: &[("trapeze, face", &[1, 0]), ("well, under", &[0, 1])],
        build: b7,
    },
];

/// B1, Down the Drain: the calls one at a time down a staircase, then holding the record (a
/// pit straight off a step's top), a springboard, and the shelf fork at the bottom.
fn b1(b: &mut Builder) {
    b.start(12);
    b.pit(9, false);
    b.run(7, &[3]);
    b.step(3, 8);
    b.wall(6, 3);
    b.run(9, &[4]);
    b.step(3, 2);
    b.cp();
    b.run(8, &[5]);
    b.drips(12, 3);
    b.run(6, &[]);
    b.climb_then_pit(5);
    b.run(8, &[4]);
    b.stairs_down(2, 3, 5);
    b.cp();
    b.run(8, &[5]);
    b.spikes(2);
    b.run(6, &[3]);
    b.spring_step(8);
    b.run(4, &[]);
    b.drips(10, 3);
    b.run(6, &[]);
    b.climb_then_pit(4);
    b.run(8, &[4]);
    b.stairs_down(3, 3, 4);
    b.cp();
    b.run(10, &[6]);
    b.shelf_fork(36, &[14], &[4, 10, 20, 28]);
    b.run(6, &[3]);
    b.pit(9, false);
    b.run(8, &[4]);
    b.step(3, 6);
    b.cp();
    b.run(6, &[3]);
    b.wall(6, 3);
    b.run(8, &[4]);
    b.drips(12, 3);
    b.run(6, &[3]);
    b.pit(9, false);
    b.run(14, &[5]);
}

/// B2, Spike Alley: up on the alley's roof or along it under drips, a springboard to the high
/// road over sewage or under it melted, and the alley again.
fn b2(b: &mut Builder) {
    b.start(12);
    b.pit(9, false);
    b.run(8, &[4]);
    b.wall(6, 3);
    b.run(6, &[]);
    b.spikes(2);
    b.run(8, &[]);
    b.spikes(3);
    b.run(10, &[6]);
    b.cp();
    b.run(8, &[5]);
    b.shelf_fork(44, &[20], &[6, 10, 30, 34]);
    b.run(6, &[3]);
    b.pit(9, false);
    b.run(6, &[]);
    b.cp();
    b.run(8, &[5]);
    b.springboard(Beat::Red, 22, Low::Drips);
    b.run(6, &[3]);
    b.spikes(3);
    b.run(8, &[4]);
    b.climb_then_pit(5);
    b.run(10, &[5]);
    b.cp();
    b.run(6, &[]);
    b.drips(14, 3);
    b.run(4, &[]);
    b.pit(9, false);
    b.run(8, &[4]);
    b.springboard(Beat::Blue, 18, Low::Sewage);
    b.run(6, &[3]);
    b.wall(6, 3);
    b.run(8, &[4]);
    b.cp();
    b.run(6, &[3]);
    b.spikes(3);
    b.run(6, &[]);
    b.shelf_fork(34, &[10, 22], &[4, 16, 28]);
    b.run(12, &[3, 8]);
    b.pit(9, false);
    b.run(14, &[4]);
}

/// B3, Drip Pipe: the hit. Doors and bridges, the deck's trapdoor fork, a scratch back to a
/// trapdoor behind a dead end, drums.
fn b3(b: &mut Builder) {
    b.start(14);
    b.door(Beat::Red, 6);
    b.run(10, &[5]);
    b.bridge(Beat::Blue, 9);
    b.run(6, &[]);
    b.door(Beat::Blue, 6);
    b.run(8, &[4]);
    b.bridge(Beat::Red, 9);
    b.run(6, &[]);
    b.bridge(Beat::Blue, 9);
    b.run(10, &[5]);
    b.cp();
    b.run(8, &[5]);
    b.deck_fork(Deck::Walls(2), true);
    b.cp();
    b.run(8, &[5]);
    b.scratch_back(7);
    b.run(10, &[5]);
    b.cp();
    b.run(6, &[]);
    b.drum(Beat::Blue, 3, 9, 6);
    b.run(8, &[4]);
    b.door(Beat::Red, 6);
    b.run(8, &[4]);
    b.cp();
    b.run(6, &[3]);
    b.springboard(Beat::Blue, 20, Low::Drips);
    b.run(6, &[3]);
    b.door(Beat::Blue, 6);
    b.run(6, &[3]);
    b.wall(6, 3);
    b.run(8, &[4]);
    b.bridge(Beat::Red, 9);
    b.run(8, &[4]);
    b.pit(9, false);
    b.run(14, &[4]);
}

/// B5, Rush Hour: bridges to drum across and ferries over the canal, a springboard to the high
/// road over sewage, the deck's pit (fired up) or its trapdoor into the pipe, and a last rush.
fn b5(b: &mut Builder) {
    b.start(12);
    b.drum(Beat::Blue, 3, 9, 6);
    b.run(8, &[4]);
    b.ferry_then_pit(8);
    b.run(8, &[4]);
    b.cp();
    b.run(8, &[5]);
    b.springboard(Beat::Red, 20, Low::Sewage);
    b.run(4, &[]);
    b.ferry(8);
    b.run(6, &[3]);
    b.wall(6, 3);
    b.run(10, &[5]);
    b.cp();
    b.run(6, &[3]);
    b.deck_fork(Deck::Pit, true);
    b.cp();
    b.run(6, &[3]);
    b.door(Beat::Red, 6);
    b.run(6, &[3]);
    b.drum(Beat::Red, 4, 9, 6);
    b.run(8, &[4]);
    b.drips(12, 3);
    b.run(6, &[3]);
    b.pit(9, false);
    b.run(14, &[5]);
}

/// B6, Double Trouble: two towers (nervous up their walls, a door and a bridge on top), the
/// cliff over a drip pipe (up its face or melted under), a pit straight off a cliff's top (held),
/// and the deck's pit or trapdoor.
fn b6(b: &mut Builder) {
    b.start(12);
    b.pit(9, false);
    b.run(8, &[4]);
    b.tower(9);
    b.cp();
    b.run(8, &[4]);
    b.face_over_drips(14, 9);
    b.run(12, &[4, 8]);
    b.shaft_then_pit(9, 4);
    b.run(6, &[]);
    b.cp();
    b.run(6, &[3]);
    b.door(Beat::Blue, 6);
    b.run(6, &[]);
    b.bridge(Beat::Red, 9);
    b.run(8, &[4]);
    b.deck_fork(Deck::Pit, true);
    b.cp();
    b.run(6, &[3]);
    b.tower(9);
    b.spikes(2);
    b.run(6, &[3]);
    b.drips(12, 3);
    b.run(6, &[3]);
    b.pit(9, false);
    b.run(14, &[5]);
}

/// B7, The Royal Flush: trapezes down into the chasm, held before each drip pipe for the
/// melting, the deck's trapeze or its trapdoor down the well, and shafts up out of it.
fn b7(b: &mut Builder) {
    b.start(12);
    b.door(Beat::Red, 6);
    b.run(8, &[4]);
    b.trapeze(2, 17);
    b.run(10, &[5]);
    b.trapeze(2, 17);
    b.run(6, &[3]);
    b.cp();
    b.run(10, &[3]);
    b.drips_held(12);
    b.run(8, &[3]);
    b.deck_fork(Deck::Trapeze, false);
    b.cp();
    b.run(10, &[3]);
    b.trapeze(3, 17);
    b.run(8, &[4]);
    b.trapeze(3, 17);
    b.run(14, &[4, 9]);
    b.drips_held(10);
    b.run(6, &[3]);
    b.cp();
    b.run(6, &[3]);
    b.shaft(9, 8);
    b.run(4, &[]);
    b.door(Beat::Blue, 6);
    b.run(8, &[4]);
    b.shaft(9, 8);
    b.run(4, &[]);
    b.bridge(Beat::Red, 9);
    b.run(10, &[5]);
    b.cp();
    b.run(6, &[3]);
    b.face_over_drips(14, 9);
    b.run(12, &[4, 8]);
    b.trapeze(2, 17);
    b.run(8, &[4]);
    b.drips_held(10);
    b.run(16, &[5, 9]);
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "levels/side-b".into());
    let mut routes = String::new();
    for d in LEVELS {
        let mut text = None;
        for &(rname, picks) in d.routes {
            let mut b = Builder::new(d.h, d.top, 0, picks.to_vec());
            b.auto_space = true;
            (d.build)(&mut b);
            assert!(b.min_top >= 4 && (b.max_top as usize) < d.h - 2, "{}: the floor goes from row {} to {}", d.file, b.min_top, b.max_top);
            for c in &b.calls {
                assert!(d.setlist.contains(c), "{} {rname}: {} isn't on its set list", d.file, c.word());
            }
            if !b.tight.is_empty() {
                eprintln!("{} {rname} spaced: {}", d.file, b.tight.join("; "));
            }
            let plan = b.plan(d.setlist);
            let t = b.level_text(d.name, d.setlist);
            match &text {
                None => text = Some(t),
                Some(t0) => assert!(*t0 == t, "{}: route {rname} drew a different grid", d.file),
            }
            writeln!(routes, "{} {rname}: {plan}", d.file).unwrap();
        }
        std::fs::write(format!("{dir}/{}.txt", d.file), text.unwrap()).unwrap();
    }
    std::fs::write(format!("{dir}/routes.txt"), &routes).unwrap();
    print!("{routes}");
}

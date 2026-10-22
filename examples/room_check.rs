//! One generated free play room as text, with the validator's dump, verdict and density:
//! `cargo run --example room_check -- "rooftop set" <band> <seed> [attempt]`. `ROOM_FILE=<path>`
//! also writes it as a level file to play (`NATHAN_LEVEL_FILE=<path> cargo run`) or to hand
//! `tests/set_bot.rs` (a bot that plays it and writes a timeline to record).
use nat_han_adventures::adapt::{AssistLevers, RoomRequest};
use nat_han_adventures::freeplay::generate::{RoomPlan, draw, draw_text, physics, validate};
use nat_han_adventures::freeplay::templates::TEMPLATES;
use nat_han_adventures::level::validate::{Options, check_with};
use nat_han_adventures::level::{Level, ThingKind, Tile};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ti = TEMPLATES.iter().position(|t| t.name == a[0]).expect("template");
    let band: u8 = a[1].parse().unwrap();
    let seed: u32 = a[2].parse().unwrap();
    let attempt: u32 = a.get(3).map_or(0, |s| s.parse().unwrap());
    let request = RoomRequest { skill: TEMPLATES[ti].skill, band, assists: AssistLevers::NONE };
    let plan = RoomPlan { template: ti, hint: true, ..RoomPlan::new(seed, 5, request, 2, false, false, 10) };
    let level = draw(seed, &plan, attempt);
    println!("{}", text(&level));
    let r = validate(&level);
    let mut l = level.clone();
    l.deaths = Some(r.deaths);
    l.gates = r.marks.clone();
    let rep = check_with(&l, &Options { dump: true }, physics());
    if let Ok(out) = std::env::var("ROOM_FILE") {
        // The room as a level file (`NATHAN_LEVEL_FILE`), with the marks and deaths the
        // validator found.
        let src = draw_text(seed, &plan, attempt);
        let (head, grid) = src.split_once("---\n").unwrap();
        let mut text = format!("{head}deaths: {}\n", r.deaths);
        for m in &r.marks {
            text += &format!("gate: {} {},{} {},{}\n", m.topic.word(), m.c0, m.r0, m.c1, m.r1);
        }
        text += "---\n";
        text += grid;
        std::fs::write(out, text).unwrap();
    }
    let (longest, plain) = nat_han_adventures::freeplay::sets::density(&level);
    println!("{}\nroom errors: {:#?}\ndensity: longest plain {longest} cols, plain share {:.0}%", rep.dump.unwrap_or_default(), r.errs, plain * 100.0);
}

pub fn text(l: &Level) -> String {
    let mut g: Vec<Vec<char>> = (0..l.height)
        .map(|r| {
            (0..l.width)
                .map(|c| match l.tile(c as i32, r as i32) {
                    Tile::Empty => '.',
                    Tile::Solid => '#',
                    Tile::OneWay => '=',
                    Tile::SpikesUp => '^',
                    Tile::SpikesDown => 'v',
                    Tile::Liquid => '~',
                    Tile::Grease => '_',
                    Tile::Brick => '%',
                    Tile::Crumble => '-',
                    Tile::Trampoline => 'T',
                    Tile::BeltLeft => '{',
                    Tile::BeltRight => '}',
                    Tile::Fan => '*',
                    Tile::BumperLeft => '(',
                    Tile::BumperRight => ')',
                    Tile::BeatRed => '[',
                    Tile::BeatBlue | Tile::BeatBlueOff => ']',
                    Tile::FanDown => '!',
                    Tile::FanRight => 'e',
                    Tile::FanLeft => 'w',
                    Tile::CushionRight => 'b',
                    Tile::CushionLeft => 'd',
                    Tile::CushionDown => 'm',
                    Tile::SpikesRight | Tile::SpikesLeft => '|',
                    _ => '?',
                })
                .collect()
        })
        .collect();
    for t in &l.things {
        g[t.row][t.col] = match t.kind {
            ThingKind::Nugget => 'o',
            ThingKind::Checkpoint => 'C',
            ThingKind::Fly => 'F',
            ThingKind::Spray => 'S',
        };
    }
    g[l.start.1][l.start.0] = 'P';
    g[l.goal.1][l.goal.0] = 'G';
    for p in &l.platforms {
        for k in 0..p.width {
            g[p.row][p.col + k] = '1';
        }
    }
    g.into_iter().map(|r| r.into_iter().collect::<String>()).collect::<Vec<_>>().join("\n")
}

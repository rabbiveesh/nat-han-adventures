//! Fit the arranger's critic ([`bandstand::live::taste`]) from A/B listening.
//!
//! `cargo run --release -p bandstand --example fit_taste -- taps.tsv [--write]`
//!
//! `taps.tsv`: one tap a line, tab-separated: `kind song before chorus a b winner`, as the
//! taste booth page stores them (`kind`: `chorus` or `feel`; `chorus`: the pass's chorus for a
//! feel pair, `-` for a chorus pair; `a`, `b`: chorus slugs, `shout+key-up`, or feel slugs;
//! `winner`: `a`, `b` or `tie`; `kind` `solo`: a soloist's table, `before` its name (`arc`,
//! `mode`, `device`, `target`, `idea`), `a` and `b` rows of it, or `-` for the dice as written,
//! each take a chorus played with its row leaned hard, and the preference read as one about
//! the rows in the tune's tempo; `kind` `band`: the same for a big band table, `upper`, `take`,
//! `low`, `pad`, `ending`, `riff`, `answer`, with `chorus` the pass's chorus). A Bradley-Terry model: the chance A wins is
//! `sigmoid(score(A) - score(B))`, the scores linear in the features of the moment
//! (`taste::Context`), with an L2 pull to neutral so a few taps can't swing the band far.
//! Prints how well it predicts held-out taps (5-fold) and the fitted table; `--write` saves it
//! to `music/taste.critic` (keeping the file's comments).
use bandstand::live::{
    Engine,
    chorus::{Call, Chorus},
    feel::Feel,
    library,
    soloist::Moment,
    taste::{Context, Table, Taste, Tune, WEIGHTS},
};
use std::collections::HashMap;

struct Tap {
    x: [f32; WEIGHTS],
    y: f32,
}

fn call(s: &str) -> Option<Call> {
    match s.strip_suffix("+key-up") {
        Some(c) => Some(Call { chorus: Chorus::parse(c)?, key_up: true }),
        None => Some(Call { chorus: Chorus::parse(s)?, key_up: false }),
    }
}

fn fit(taps: &[&Tap], lambda: f32) -> [f32; WEIGHTS] {
    let mut w = [0f32; WEIGHTS];
    let (mut m, mut v) = ([0f32; WEIGHTS], [0f32; WEIGHTS]);
    let n = taps.len().max(1) as f32;
    for step in 1..=3000 {
        let mut g = [0f32; WEIGHTS];
        for t in taps {
            let z: f32 = t.x.iter().zip(&w).map(|(a, b)| a * b).sum();
            let p = 1.0 / (1.0 + (-z).exp());
            for i in 0..WEIGHTS {
                g[i] += (p - t.y) * t.x[i] / n;
            }
        }
        for i in 0..WEIGHTS {
            g[i] += lambda * w[i] / n;
            m[i] = 0.9 * m[i] + 0.1 * g[i];
            v[i] = 0.999 * v[i] + 0.001 * g[i] * g[i];
            let (mh, vh) = (m[i] / (1.0 - 0.9f32.powi(step)), v[i] / (1.0 - 0.999f32.powi(step)));
            w[i] -= 0.05 * mh / (vh.sqrt() + 1e-8);
        }
    }
    w
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.iter().find(|a| !a.starts_with("--")).expect("taps.tsv");
    let write = args.iter().any(|a| a == "--write");
    let lambda: f32 = args.iter().find_map(|a| a.strip_prefix("--lambda=")).map_or(4.0, |x| x.parse().unwrap());
    let mut tunes: HashMap<String, Tune> = HashMap::new();
    let mut taps = Vec::new();
    let (mut skipped, mut ties) = (0, 0);
    for line in std::fs::read_to_string(path).unwrap().lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 7 || f[0] == "kind" {
            continue;
        }
        let tune = *tunes.entry(f[1].to_string()).or_insert_with(|| {
            let file = library::load(f[1]).unwrap_or_else(|e| panic!("{}: {e}", f[1]));
            Engine::new(&file, 16_000).unwrap().tune()
        });
        let y = match f[6] {
            "a" => 1.0,
            "b" => 0.0,
            _ => {
                ties += 1;
                0.5
            }
        };
        let x = match f[0] {
            "chorus" => {
                let ctx = Context::new(&tune, 1, Chorus::parse(f[2]));
                let (Some(a), Some(b)) = (call(f[4]), call(f[5])) else {
                    skipped += 1;
                    continue;
                };
                let (fa, fb) = (Taste::chorus_features(a, &ctx), Taste::chorus_features(b, &ctx));
                std::array::from_fn(|i| fa[i] - fb[i])
            }
            "feel" => {
                let ctx = Context::new(&tune, 1, Chorus::parse(f[3]));
                let (Some(a), Some(b)) = (Feel::parse(f[4]), Feel::parse(f[5])) else {
                    skipped += 1;
                    continue;
                };
                let (fa, fb) = (Taste::feel_features(a, &ctx), Taste::feel_features(b, &ctx));
                std::array::from_fn(|i| fa[i] - fb[i])
            }
            "solo" | "band" => {
                let Some(table) = Table::parse(f[2]) else {
                    skipped += 1;
                    continue;
                };
                let ctx = if table.is_section() {
                    // A big band take: a pass of `chorus` (f[3]) with the row leaned all
                    // through it, read as a middle section's taste.
                    let chorus = Chorus::parse(f[3]).unwrap_or(Chorus::Head);
                    Context::section(tune.bpm, 1, 4, chorus, chorus.lead_solos(0.7), 1)
                } else {
                    Context::solo(&Moment { bpm: tune.bpm, chorus: 1, ..Moment::default() })
                };
                let row = |name: &str| -> Option<[f32; WEIGHTS]> {
                    if name == "-" {
                        return Some([0.0; WEIGHTS]);
                    }
                    let i = table.rows().iter().position(|&r| r == name)?;
                    Some(Taste::solo_features(table, i, &ctx))
                };
                let (Some(fa), Some(fb)) = (row(f[4]), row(f[5])) else {
                    skipped += 1;
                    continue;
                };
                std::array::from_fn(|i| fa[i] - fb[i])
            }
            _ => {
                skipped += 1;
                continue;
            }
        };
        taps.push(Tap { x, y });
    }
    println!("{} taps ({ties} ties, {skipped} unreadable)", taps.len());
    // 5-fold: how often the model picks the tap's winner (ties left out of the score).
    let (mut right, mut judged) = (0, 0);
    for k in 0..5 {
        let train: Vec<&Tap> = taps.iter().enumerate().filter(|(i, _)| i % 5 != k).map(|x| x.1).collect();
        let w = fit(&train, lambda);
        for t in taps.iter().enumerate().filter(|(i, _)| i % 5 == k).map(|x| x.1).filter(|t| t.y != 0.5) {
            let z: f32 = t.x.iter().zip(&w).map(|(a, b)| a * b).sum();
            judged += 1;
            if (z > 0.0) == (t.y > 0.5) {
                right += 1;
            }
        }
    }
    if judged > 0 {
        println!("held out: picks the winner {right} of {judged} ({:.0}%; a coin gets 50%)", 100.0 * right as f32 / judged as f32);
    }
    let all: Vec<&Tap> = taps.iter().collect();
    let taste = Taste::from_weights(&fit(&all, lambda));
    let text = taste.to_text();
    println!("{text}");
    if write {
        let file = concat!(env!("CARGO_MANIFEST_DIR"), "/music/taste.critic");
        let old = std::fs::read_to_string(file).unwrap_or_default();
        let comments: String = old.lines().take_while(|l| l.starts_with('#') || l.trim().is_empty()).map(|l| format!("{l}\n")).collect();
        std::fs::write(file, format!("{comments}{text}")).unwrap();
        println!("wrote {file}");
    }
}

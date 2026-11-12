//! A level file through the validator, with its dump: `cargo run --release --example
//! check_level -- levels/03.txt` (also under the laughing band's
//! melting and alien phrases; the file is read at run time: edit and rerun, no rebuild).
use nat_han_adventures::level::Level;
use nat_han_adventures::level::validate::{Options, Physics, check, check_with};

fn main() {
    let path = std::env::args().nth(1).expect("a level file");
    let src = std::fs::read_to_string(&path).expect("readable");
    let level = Level::parse(&src).unwrap_or_else(|e| panic!("{path}: {e}"));
    let t0 = std::time::Instant::now();
    let r = check(&level, &Options { dump: true });
    if let Some(d) = &r.dump {
        println!("{d}");
    }
    for e in &r.errs {
        println!("ERROR: {e}");
    }
    // The laughing band's phrases that take something away: the level must stay beatable.
    for (name, phys) in Physics::laughing() {
        for e in check_with(&level, &Options::default(), &phys).errs {
            println!("ERROR ({name}): {e}");
        }
    }
    println!("{} errors, {:.2?}", r.errs.len(), t0.elapsed());
}

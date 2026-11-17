//! What the critic knows of each tune in the library: tempo, form, melody notes per beat.
use bandstand::live::{Engine, library};

fn main() {
    for (stem, _) in library::FILES {
        let Ok(f) = library::load(stem) else { continue };
        let Ok(e) = Engine::new(&f, 48_000) else { continue };
        let t = e.tune();
        println!("{stem:<28} {:>5.0} bpm {:>3} bars {:>5.2} notes/beat", t.bpm, t.bars, t.density);
    }
}

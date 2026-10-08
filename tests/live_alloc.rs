//! The live engine doesn't allocate (or free) on the audio thread once it's running.
//! (Its own test binary: the counting allocator is global.)

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use kira::Frame;
use nat_han_adventures::audio::{
    Filters, Harmony,
    live::{Engine, Input, feel::Feel, library},
};

struct Counting;

thread_local! {
    static WATCHING: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if WATCHING.with(Cell::get) {
            COUNT.with(|c| c.set(c.get() + 1));
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if WATCHING.with(Cell::get) {
            COUNT.with(|c| c.set(c.get() + 1));
        }
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

#[test]
fn fill_doesnt_allocate() {
    for (stem, f) in [("sweet_georgia_brown", 0.7), ("muskrat_ramble", 1.0), ("shave_and_a_haircut", 0.7), ("tiger_rag", 0.4)] {
        let file = library::load(stem).unwrap();
        let mut e = Engine::new(&file, 32_000).unwrap();
        // (Every harmony, the waltz and its meter switches included.)
        e.post(Input::SetFreedom { lead: f, comp: f, bass: f, drums: f, dynamics: 0.7 });
        let mut buf = vec![Frame::ZERO; 512];
        let mut state = e.state();
        // Warm up: a pass, every voice used.
        for _ in 0..(e.shape().len as usize / 512 + 8) {
            e.fill(&mut buf);
        }
        e.state_into(&mut state);
        WATCHING.with(|w| w.set(true));
        for k in 0..4000 {
            if k % 100 == 0 {
                e.post(Input::Toot);
                e.post(Input::Death);
                e.post(Input::Checkpoint);
                e.post(Input::SetFilters(Filters { harmony: Harmony::ALL[k / 100 % 5], just_intonation: k % 200 == 0 }));
                // The feels too (the band's own choices at these freedoms, and forced ones).
                e.post(Input::ForceFeel([None, Some(Feel::Bossa), Some(Feel::Samba), Some(Feel::Rock), Some(Feel::Funk)][k / 300 % 5]));
            }
            e.fill(&mut buf);
            e.state_into(&mut state);
            let _ = e.beat_clock();
        }
        WATCHING.with(|w| w.set(false));
        assert_eq!(COUNT.with(Cell::get), 0, "{stem}: allocations on the audio path");
    }
}

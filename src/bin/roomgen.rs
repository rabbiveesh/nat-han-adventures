//! The web build's free-play worker: validates rooms off the game's thread
//! (`nat_han_adventures::freeplay::offload`). Trunk builds it as a Web Worker (`index.html`);
//! natively there's nothing for it to do.

#[cfg(target_arch = "wasm32")]
fn main() {
    nat_han_adventures::freeplay::offload::worker_main();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("roomgen is the web build's free-play worker; run the game with `cargo run`.");
}

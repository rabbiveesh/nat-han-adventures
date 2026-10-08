//! Seeded randomness for free play. Everything a run generates comes from its seed through
//! these dice, so a seed means the same rooms on every machine (native and web alike: ChaCha,
//! not the platform-dependent `SmallRng`).

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::adapt::Band;

/// Seeds shown to the player are 6 digits (`000000`..=`999999`): short enough to read out or
/// type with the arrow keys.
pub const SEED_DIGITS: usize = 6;
pub const SEED_MAX: u32 = 999_999;

/// The seed as the player sees it.
pub fn seed_text(seed: u32) -> String {
    format!("{:0w$}", seed % (SEED_MAX + 1), w = SEED_DIGITS)
}

/// Parse a typed seed (digits only, at most [`SEED_DIGITS`] of them).
pub fn parse_seed(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.is_empty() || s.len() > SEED_DIGITS || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// SplitMix64: mixes seeds and indices into well-spread stream seeds.
pub fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A stream seed for one purpose (`what`) of one room of one run.
pub fn stream(seed: u32, what: u64, index: u64) -> u64 {
    mix(mix(seed as u64 ^ (what << 40)) ^ index)
}

/// `band` 1..=10 as 0..=1.
pub fn t(band: Band) -> f32 {
    (band.clamp(1, 10) as f32 - 1.0) / 9.0
}

pub struct Dice(StdRng);

impl Dice {
    pub fn new(seed: u64) -> Dice {
        Dice(StdRng::seed_from_u64(seed))
    }

    /// Uniform in 0..1.
    pub fn u(&mut self) -> f32 {
        self.0.random()
    }

    /// Uniform integer in `lo..=hi`.
    pub fn int(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo { lo } else { self.0.random_range(lo..=hi) }
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.u() < p
    }

    pub fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.0.random_range(0..xs.len())]
    }

    /// A count that grows with the band: `lo` at band 1 to `hi` at band 10, plus 0..=`jitter`.
    pub fn scaled(&mut self, band: Band, lo: f32, hi: f32, jitter: i32) -> i32 {
        (lo + (hi - lo) * t(band)).round() as i32 + self.int(0, jitter)
    }

    /// A float from `lo` (band 1) to `hi` (band 10), ±`spread` (a fraction).
    pub fn scaled_f(&mut self, band: Band, lo: f32, hi: f32, spread: f32) -> f32 {
        (lo + (hi - lo) * t(band)) * (1.0 + spread * (2.0 * self.u() - 1.0))
    }

    /// The underlying generator (for `adapt::next_room`).
    pub fn rng(&mut self) -> &mut StdRng {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_text_round_trips() {
        for s in [0, 7, 4242, 999_999] {
            assert_eq!(parse_seed(&seed_text(s)), Some(s));
        }
        assert_eq!(seed_text(42), "000042");
        assert_eq!(parse_seed("12a"), None);
        assert_eq!(parse_seed("1234567"), None);
        assert_eq!(parse_seed(""), None);
    }

    #[test]
    fn dice_are_deterministic() {
        let a: Vec<i32> = { let mut d = Dice::new(5); (0..20).map(|_| d.int(0, 100)).collect() };
        let b: Vec<i32> = { let mut d = Dice::new(5); (0..20).map(|_| d.int(0, 100)).collect() };
        assert_eq!(a, b);
        assert_ne!(stream(1, 0, 0), stream(1, 0, 1));
        assert_ne!(stream(1, 0, 0), stream(2, 0, 0));
    }
}

//! Sound effects, synthesized from the same NES-style building blocks as the music
//! (pulse, stepped triangle, LFSR noise). All deterministic: randomness comes from fixed seeds
//! (Han's babble gets its variety from several pre-rendered [`han_blip`] variants).

use kira::Frame;

use super::Sfx;
use super::synth::{self, Lfsr, Rendered, SAMPLE_RATE, pulse, triangle};

const SR: f32 = SAMPLE_RATE as f32;
/// Every sfx is normalized to this peak, then scaled by [`level`].
const PEAK: f32 = 0.75;

/// Relative level of each effect (small, frequent ones sit lower).
fn level(sfx: Sfx) -> f32 {
    match sfx {
        Sfx::MenuMove => 0.55,
        Sfx::HanBlip => 0.6,
        Sfx::Jump | Sfx::Land => 0.75,
        Sfx::Nugget | Sfx::Checkpoint | Sfx::MenuSelect => 0.85,
        Sfx::Toot | Sfx::Splat | Sfx::Flush => 1.0,
    }
}

impl Sfx {
    pub const ALL: [Sfx; 10] = [
        Sfx::Jump,
        Sfx::Toot,
        Sfx::Land,
        Sfx::Nugget,
        Sfx::Splat,
        Sfx::Checkpoint,
        Sfx::Flush,
        Sfx::MenuMove,
        Sfx::MenuSelect,
        Sfx::HanBlip,
    ];
}

/// Number of distinct [`han_blip`] variants.
pub const HAN_VARIANTS: usize = 6;

/// Render a sound effect (mono, centred). Never loops.
pub fn render(sfx: Sfx) -> Rendered {
    let mono = match sfx {
        Sfx::Jump => jump(),
        Sfx::Toot => toot(),
        Sfx::Land => land(),
        Sfx::Nugget => nugget(),
        Sfx::Splat => splat(),
        Sfx::Checkpoint => checkpoint(),
        Sfx::Flush => flush(),
        Sfx::MenuMove => menu_move(),
        Sfx::MenuSelect => menu_select(),
        Sfx::HanBlip => return han_blip(0),
    };
    finish(mono, level(sfx))
}

/// One syllable of Han's babble; `variant` (mod [`HAN_VARIANTS`]) picks pitch and contour.
pub fn han_blip(variant: usize) -> Rendered {
    const BASE: [f32; HAN_VARIANTS] = [220.0, 262.0, 196.0, 294.0, 247.0, 175.0];
    // Pitch contour over the syllable: start ratio -> end ratio.
    const CONTOUR: [(f32, f32); HAN_VARIANTS] =
        [(1.0, 1.15), (1.1, 0.9), (0.95, 1.2), (1.2, 1.0), (1.0, 0.85), (0.9, 1.1)];
    let v = variant % HAN_VARIANTS;
    let (a, b) = CONTOUR[v];
    let mut o = Osc::default();
    let n = secs(0.075);
    let mono = (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let f = BASE[v] * (a + (b - a) * t);
            // A touch of "vowel": duty shifts mid-syllable.
            let duty = if t < 0.4 { 0.5 } else { 0.25 };
            o.pulse(f, duty) * 0.5 * env_ar(i, n, 0.004, 0.02)
        })
        .collect();
    finish(mono, level(Sfx::HanBlip))
}

fn secs(s: f32) -> usize {
    (s * SR) as usize
}

/// Oscillator phases for one sound.
#[derive(Default)]
struct Osc {
    phase: f32,
}

impl Osc {
    fn step(&mut self, f: f32) -> f32 {
        let dt = f / SR;
        let p = self.phase;
        self.phase += dt;
        self.phase -= self.phase.floor();
        p
    }
    fn pulse(&mut self, f: f32, duty: f32) -> f32 {
        let p = self.step(f);
        pulse(p, f / SR, duty) * 0.6
    }
    fn tri(&mut self, f: f32) -> f32 {
        triangle(self.step(f))
    }
    fn sine(&mut self, f: f32) -> f32 {
        (self.step(f) * std::f32::consts::TAU).sin()
    }
}

/// Attack/release envelope (linear), in seconds.
fn env_ar(i: usize, n: usize, attack: f32, release: f32) -> f32 {
    let a = (i as f32 / (attack * SR)).min(1.0);
    let r = ((n - i) as f32 / (release * SR)).min(1.0);
    a * r
}

/// Exponential sweep from `a` to `b` as `t` goes 0..1.
fn exp_sweep(a: f32, b: f32, t: f32) -> f32 {
    a * (b / a).powf(t.clamp(0.0, 1.0))
}

/// One-pole low-pass.
struct LowPass(f32);

impl LowPass {
    fn run(&mut self, x: f32, cutoff: f32) -> f32 {
        let a = 1.0 - (-std::f32::consts::TAU * cutoff / SR).exp();
        self.0 += a * (x - self.0);
        self.0
    }
}

fn finish(mono: Vec<f32>, level: f32) -> Rendered {
    let peak = mono.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
    let gain = PEAK * level / peak;
    let mut frames: Vec<Frame> = mono.into_iter().map(|v| Frame::from_mono(v * gain)).collect();
    synth::fade_out(&mut frames, 0.003);
    synth::master(&mut frames);
    Rendered { frames, sample_rate: SAMPLE_RATE, looping: false }
}

fn jump() -> Vec<f32> {
    let n = secs(0.14);
    let mut o = Osc::default();
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let f = exp_sweep(320.0, 760.0, t / 0.8);
            o.pulse(f, 0.25) * env_ar(i, n, 0.002, 0.07)
        })
        .collect()
}

/// "pbbbt": a low, buzzy, wobbling pulse with some flappy noise, a little pop at the front
/// and an upturned end.
fn toot() -> Vec<f32> {
    let n = secs(0.27);
    let (mut o, mut wob) = (Osc::default(), Osc::default());
    let mut noise = Lfsr::with_seed(0x1234);
    let mut lp = LowPass(0.0);
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let flutter = wob.sine(26.0);
            // Pitch: a short "p" pop, then a sagging buzz that lifts at the very end.
            let base = if t < 0.06 { 160.0 } else { exp_sweep(105.0, 78.0, (t - 0.06) / 0.8) };
            let base = if t > 0.86 { base * (1.0 + (t - 0.86) * 2.5) } else { base };
            let f = base * (1.0 + 0.09 * flutter);
            let buzz = o.pulse(f, 0.125) * 1.2;
            // Flappy noise, gated by the flutter so it "bbbb"s.
            let flap = lp.run(noise.next(3_500.0, false), 1_800.0) * (0.5 + 0.5 * flutter).powi(2) * 0.7;
            (buzz + flap) * env_ar(i, n, 0.006, 0.05)
        })
        .collect()
}

fn land() -> Vec<f32> {
    let n = secs(0.12);
    let mut o = Osc::default();
    let mut noise = Lfsr::with_seed(77);
    let mut lp = LowPass(0.0);
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let decay = (-t * 5.0).exp();
            let thump = o.tri(exp_sweep(130.0, 45.0, t * 1.5)) * 0.8;
            let dust = lp.run(noise.next(4_000.0, false), 900.0) * 0.6 * (-t * 12.0).exp();
            (thump + dust) * decay * env_ar(i, n, 0.002, 0.02)
        })
        .collect()
}

/// The classic coin: a short B5 then a ringing E6.
fn nugget() -> Vec<f32> {
    let n = secs(0.45);
    let first = secs(0.065);
    let mut o = Osc::default();
    (0..n)
        .map(|i| {
            let (f, env) = if i < first {
                (987.8, 1.0)
            } else {
                let t = (i - first) as f32 / (n - first) as f32;
                (1318.5, (1.0 - t).powf(1.6))
            };
            o.pulse(f, 0.5) * 0.8 * env * env_ar(i, n, 0.002, 0.01)
        })
        .collect()
}

/// A wet, cartoony squelch: noise sweeping down, a bloopy falling tone, wobbling.
fn splat() -> Vec<f32> {
    let n = secs(0.42);
    let (mut o, mut wob) = (Osc::default(), Osc::default());
    let mut noise = Lfsr::with_seed(0x2bad);
    let mut lp = LowPass(0.0);
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let w = wob.sine(32.0);
            let clock = exp_sweep(24_000.0, 1_200.0, t);
            let cutoff = exp_sweep(7_000.0, 400.0, t) * (1.0 + 0.4 * w);
            let squish = lp.run(noise.next(clock, false), cutoff) * 1.1;
            let bloop = o.tri(exp_sweep(320.0, 55.0, t) * (1.0 + 0.12 * w)) * 0.5;
            (squish + bloop) * (1.0 - t).powf(1.3) * env_ar(i, n, 0.004, 0.03)
        })
        .collect()
}

/// Cheerful rising C-E-G (+ a ringing high C).
fn checkpoint() -> Vec<f32> {
    let notes = [(523.3, 0.07), (659.3, 0.07), (784.0, 0.07), (1046.5, 0.3)];
    let mut o = Osc::default();
    let mut out = Vec::new();
    for (k, &(f, d)) in notes.iter().enumerate() {
        let n = secs(d);
        let last = k == notes.len() - 1;
        for i in 0..n {
            let env = if last { (1.0 - i as f32 / n as f32).powf(1.5) } else { 1.0 };
            out.push(o.pulse(f, 0.25) * 0.75 * env * env_ar(i, n, 0.002, if last { 0.01 } else { 0.004 }));
        }
    }
    // A quiet echo, a 16th later: NES-style "fake reverb".
    let delay = secs(0.09);
    out.resize(out.len() + delay, 0.0);
    for i in (delay..out.len()).rev() {
        out[i] += out[i - delay] * 0.3;
    }
    out
}

/// The flush: a noise swirl whose "filter" sweeps down then back up (~1.2s), then a gurgle of
/// little rising bubble chirps.
fn flush() -> Vec<f32> {
    let swirl = secs(1.2);
    let gurgle = secs(0.5);
    let mut out = Vec::with_capacity(swirl + gurgle);
    let mut noise = Lfsr::with_seed(0x0f1u16);
    let (mut lp1, mut lp2) = (LowPass(0.0), LowPass(0.0));
    let mut wob = Osc::default();
    for i in 0..swirl {
        let t = i as f32 / swirl as f32;
        // Falling for 60%, then rising.
        let cutoff = if t < 0.6 { exp_sweep(6_000.0, 450.0, t / 0.6) } else { exp_sweep(450.0, 2_800.0, (t - 0.6) / 0.4) };
        // The swirl: amplitude and cutoff wobble, speeding up as it drains.
        let w = wob.sine(4.0 + 6.0 * t);
        let c = cutoff * (1.0 + 0.35 * w);
        let x = lp2.run(lp1.run(noise.next(30_000.0, false), c), c);
        let amp = (t / 0.08).min(1.0) * (0.8 + 0.2 * w) * (1.0 - 0.3 * t);
        out.push(x * 1.6 * amp);
    }
    // Gurgle: 6 bubble chirps of varied pitch, each a quick upward triangle blip.
    let bubbles = [(0.00, 240.0), (0.07, 330.0), (0.12, 200.0), (0.2, 380.0), (0.27, 280.0), (0.36, 420.0)];
    let mut g = vec![0.0f32; gurgle];
    for (k, &(at, f0)) in bubbles.iter().enumerate() {
        let start = secs(at);
        let n = secs(0.045);
        let mut o = Osc::default();
        let fade = 1.0 - k as f32 / bubbles.len() as f32 * 0.6;
        for i in 0..n.min(gurgle - start) {
            let t = i as f32 / n as f32;
            g[start + i] += o.tri(exp_sweep(f0, f0 * 2.2, t)) * 0.45 * fade * env_ar(i, n, 0.003, 0.012);
        }
    }
    // Crossfade the swirl's tail into the gurgle.
    let overlap = secs(0.1);
    let tail_start = out.len() - overlap;
    for i in 0..overlap {
        let k = i as f32 / overlap as f32;
        out[tail_start + i] *= 1.0 - k;
        out[tail_start + i] += g[i];
    }
    out.extend_from_slice(&g[overlap..]);
    out
}

fn menu_move() -> Vec<f32> {
    let n = secs(0.035);
    let mut o = Osc::default();
    (0..n).map(|i| o.pulse(1250.0, 0.5) * 0.5 * env_ar(i, n, 0.001, 0.025)).collect()
}

fn menu_select() -> Vec<f32> {
    let a = secs(0.05);
    let n = secs(0.16);
    let mut o = Osc::default();
    (0..n)
        .map(|i| {
            let f = if i < a { 660.0 } else { 990.0 };
            o.pulse(f, 0.25) * 0.7 * env_ar(i, n, 0.002, 0.08)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sfx_is_short_clean_and_audible() {
        for s in Sfx::ALL {
            let r = render(s);
            let peak = r.frames.iter().fold(0.0f32, |m, f| m.max(f.left.abs()).max(f.right.abs()));
            assert!(r.frames.iter().all(|f| f.left.is_finite() && f.right.is_finite()), "{s:?}");
            assert!(peak > 0.1 && peak <= 1.0, "{s:?} peak {peak}");
            assert!(r.duration_secs() < 2.0, "{s:?} {}s", r.duration_secs());
            let (first, last) = (r.frames[0], *r.frames.last().unwrap());
            assert!(first.left.abs() < 0.02 && last.left.abs() < 1e-6, "{s:?} edges {first:?} {last:?}");
        }
        assert!((1.1..1.9).contains(&render(Sfx::Flush).duration_secs()));
        assert!((0.2..0.32).contains(&render(Sfx::Toot).duration_secs()));
    }

    #[test]
    fn han_variants_differ() {
        let a = han_blip(0);
        let b = han_blip(1);
        assert_ne!(a.frames, b.frames);
        assert_eq!(han_blip(HAN_VARIANTS).frames, a.frames);
    }
}

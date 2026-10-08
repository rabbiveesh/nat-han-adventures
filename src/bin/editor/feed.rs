//! The gameplay feed: the game, simulated. Buttons (and the auto-playing Nat) make the same
//! gameplay messages the game makes, turned into the same engine [`Input`]s the game's audio
//! plugin sends (`audio::plugin::forward` and `direct`), with the same director
//! ([`director::Band`]) deciding the filters on the same play clock. So the engine can't tell
//! the editor from the game.
//!
//! The director runs here, not inside the engine ([`EngineConfig::self_directed`] would also
//! do), so its memory (the rolling window, a held summon) survives the editor swapping in a new
//! engine on every edit, as it survives the game's songs changing.
//!
//! [`EngineConfig::self_directed`]: nat_han_adventures::audio::live::EngineConfig::self_directed

use std::collections::VecDeque;

use nat_han_adventures::audio::director::{self, Band, Events};
use nat_han_adventures::audio::live::Input;
use nat_han_adventures::audio::{Filters, Sfx};

/// A gameplay event button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Toot,
    Nugget,
    Death,
    Checkpoint,
    Jump,
    Land,
    WaltzStep,
    Restart,
}

impl Button {
    pub const ALL: [Button; 8] = [
        Button::Toot,
        Button::Nugget,
        Button::Death,
        Button::Checkpoint,
        Button::Jump,
        Button::Land,
        Button::WaltzStep,
        Button::Restart,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Button::Toot => "TOOT",
            Button::Nugget => "NUGGET",
            Button::Death => "DEATH",
            Button::Checkpoint => "CHECKPT",
            Button::Jump => "JUMP",
            Button::Land => "LAND",
            Button::WaltzStep => "WALTZ",
            Button::Restart => "RESTART",
        }
    }

    /// The hotkey (when no text field has focus and REC is off).
    pub fn key(self) -> char {
        match self {
            Button::Toot => 'T',
            Button::Nugget => 'N',
            Button::Death => 'D',
            Button::Checkpoint => 'C',
            Button::Jump => 'J',
            Button::Land => 'L',
            Button::WaltzStep => 'W',
            Button::Restart => 'R',
        }
    }

    /// The sound effect the game plays for it.
    pub fn sfx(self) -> Option<Sfx> {
        match self {
            Button::Toot => Some(Sfx::Toot),
            Button::Nugget => Some(Sfx::Nugget),
            Button::Death => Some(Sfx::Splat),
            Button::Checkpoint => Some(Sfx::Checkpoint),
            Button::Jump => Some(Sfx::Jump),
            Button::Land => Some(Sfx::Land),
            Button::WaltzStep | Button::Restart => None,
        }
    }
}

/// The game's music bookkeeping, without the game.
#[derive(Debug, Clone, Default)]
pub struct Gameplay {
    /// The director's memory of the "level".
    pub band: Band,
    /// Play time, seconds (it runs while the music plays, like the game's level clock).
    pub now: f32,
    /// The director's last decision and why.
    pub decided: Option<(Filters, &'static str)>,
    /// This frame's events, for the director.
    frame: Events,
    restarted: bool,
    started: bool,
    /// What happened lately (play time, label), newest last.
    pub log: VecDeque<(f32, &'static str)>,
}

impl Gameplay {
    /// A button press: the inputs the game's `forward` system would send for that message.
    pub fn press(&mut self, b: Button, out: &mut Vec<Input>) {
        match b {
            Button::Toot => {
                out.push(Input::Toot);
                self.frame.toots += 1;
            }
            Button::Jump => {
                out.push(Input::Jump { on_ground: true });
                self.frame.ground_jumps += 1;
            }
            // A full-height fall.
            Button::Land => out.push(Input::Land { speed: 300.0 }),
            Button::Nugget => {
                out.push(Input::Nugget);
                self.frame.nuggets += 1;
            }
            Button::Death => {
                out.push(Input::Death);
                self.frame.deaths += 1;
            }
            Button::Checkpoint => {
                out.push(Input::Checkpoint);
                self.frame.checkpoints += 1;
            }
            // The director counts it as a jump in threes; `tick` sends the waltz step.
            Button::WaltzStep => self.frame.waltz_steps += 1,
            Button::Restart => {
                out.push(Input::Restart);
                self.restarted = true;
            }
        }
        self.log.push_back((self.now, b.label()));
        while self.log.len() > 12 {
            self.log.pop_front();
        }
    }

    /// `dt` seconds of play: the director hears this frame's events and decides, as the game's
    /// `direct` system does (level start, summons, deaths, checkpoints, every 20 s).
    pub fn tick(&mut self, dt: f32, out: &mut Vec<Input>) {
        self.now += dt.max(0.0);
        let now = self.now;
        let mut decision = None;
        let level_start = !self.started;
        if !self.started || self.restarted {
            decision = Some(self.band.start(now));
            self.started = true;
            self.restarted = false;
        }
        let (before, steps) = (self.band.stats, self.band.steps_taken);
        if let Some(d) = self.band.step(now, std::mem::take(&mut self.frame)) {
            decision = Some(d);
        }
        if let Some(d) = decision {
            self.decided = Some(d);
        }
        if level_start {
            out.push(Input::LevelStart);
        }
        for _ in 0..self.band.steps_taken.saturating_sub(steps) {
            out.push(Input::WaltzStep);
        }
        if self.band.stats != before || decision.is_some() {
            out.push(Input::SetStats(self.band.stats));
        }
        if let Some((f, _)) = decision {
            out.push(Input::SetFilters(f));
        }
    }

    /// Seconds to the director's next periodic check.
    pub fn next_check_in(&self) -> f32 {
        (self.band.next_check() - self.now).max(0.0)
    }

    /// The summon being held, and for how much longer.
    pub fn holding(&self) -> Option<(nat_han_adventures::audio::Harmony, f32)> {
        let h = self.band.held(self.now)?;
        Some((h, self.band.hold.map_or(0.0, |(_, until)| until - self.now)))
    }
}

/// The rules' thresholds, for the stats bars: (label, value, threshold).
pub fn stat_bars(s: &director::PlayStats) -> [(&'static str, u32, u32); 5] {
    [
        ("toots", s.stretch_toots, director::GIANT_STEPS_TOOTS),
        ("nuggets", s.stretch_nuggets, director::FIRED_UP_NUGGETS),
        ("waltz steps", s.stretch_waltzes, 1),
        ("mood deaths", s.mood_deaths, director::NERVOUS_DEATHS),
        ("deaths since cp", s.checkpoint_deaths, director::LAUGHING_DEATHS),
    ]
}

/// A synthetic player: presses the buttons on its own, calm (steady jumps, the odd jump in
/// threes) to chaotic (tooting, grabbing, dying).
#[derive(Debug, Clone)]
pub struct AutoNat {
    pub on: bool,
    /// 0 calm .. 1 chaos.
    pub chaos: f32,
    rng: u64,
    /// Scheduled presses (play time, button).
    queue: Vec<(f32, Button)>,
}

impl Default for AutoNat {
    fn default() -> Self {
        AutoNat { on: false, chaos: 0.5, rng: 0x9E37_79B9_7F4A_7C15, queue: Vec::new() }
    }
}

impl AutoNat {
    fn unit(&mut self) -> f32 {
        // xorshift64*
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        (self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32
    }

    /// The presses for `dt` seconds of play ending at play time `now`.
    pub fn tick(&mut self, now: f32, dt: f32) -> Vec<Button> {
        let mut out = Vec::new();
        if !self.on || dt <= 0.0 {
            return out;
        }
        let c = self.chaos.clamp(0.0, 1.0);
        let rates = [
            (Button::Jump, 0.5 + 0.7 * c),
            (Button::Toot, 0.02 + 0.55 * c * c.sqrt()),
            (Button::Nugget, 0.12 + 0.25 * c),
            (Button::Death, 0.004 + 0.12 * c * c),
            (Button::Checkpoint, 1.0 / 35.0),
        ];
        for (b, rate) in rates {
            if self.unit() < rate * dt {
                self.queue.push((now, b));
            }
        }
        // A calm player sometimes jumps in threes: three takeoffs, evenly spaced.
        if self.unit() < (1.0 - c) * 0.03 * dt {
            let gap = 0.5 + 0.3 * self.unit();
            for k in 0..3 {
                self.queue.push((now + k as f32 * gap, Button::Jump));
            }
        }
        let mut lands = Vec::new();
        self.queue.retain(|&(t, b)| {
            if t <= now {
                out.push(b);
                if b == Button::Jump {
                    lands.push(t + 0.45);
                }
                false
            } else {
                true
            }
        });
        self.queue.extend(lands.into_iter().map(|t| (t, Button::Land)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nat_han_adventures::audio::Harmony;
    use nat_han_adventures::audio::live::{Engine, library};
    use nat_han_adventures::audio::tuning::Tuning;

    fn press(g: &mut Gameplay, b: Button) -> Vec<Input> {
        let mut out = Vec::new();
        g.press(b, &mut out);
        out
    }

    /// Each button sends what the game sends for that message.
    #[test]
    fn buttons_send_the_games_inputs() {
        let mut g = Gameplay::default();
        let mut first = Vec::new();
        g.tick(0.016, &mut first);
        assert_eq!(first[0], Input::LevelStart);
        assert!(first.contains(&Input::SetFilters(Filters::default())));
        assert_eq!(press(&mut g, Button::Toot), [Input::Toot]);
        assert_eq!(press(&mut g, Button::Nugget), [Input::Nugget]);
        assert_eq!(press(&mut g, Button::Death), [Input::Death]);
        assert_eq!(press(&mut g, Button::Checkpoint), [Input::Checkpoint]);
        assert_eq!(press(&mut g, Button::Jump), [Input::Jump { on_ground: true }]);
        assert!(matches!(press(&mut g, Button::Land)[..], [Input::Land { .. }]));
        assert_eq!(press(&mut g, Button::Restart), [Input::Restart]);
        // The waltz step goes through the director, which sends it on.
        assert!(press(&mut g, Button::WaltzStep).is_empty());
        let mut out = Vec::new();
        g.tick(0.016, &mut out);
        assert!(out.contains(&Input::WaltzStep), "{out:?}");
        assert!(out.iter().any(|i| matches!(i, Input::SetStats(_))));
        // The restart re-ran the level start: the waltz step summoned the waltz on top of it.
        assert!(out.contains(&Input::SetFilters(Filters { harmony: Harmony::Waltz, just_intonation: false })), "{out:?}");
    }

    /// Five toots summon Giant Steps, and a headless engine fed the inputs plays it from the
    /// next bar line; two deaths since the checkpoint set the band laughing.
    #[test]
    fn the_feed_drives_a_headless_engine() {
        let song = library::load("sweet_georgia_brown").unwrap();
        let mut engine = Engine::new(&song, 32_000).unwrap();
        let mut g = Gameplay::default();
        let mut buf = vec![kira::Frame::ZERO; 512];
        let mut run = |g: &mut Gameplay, e: &mut Engine, inputs: Vec<Input>, secs: f32| {
            for i in inputs {
                e.post(i);
            }
            let mut t = 0.0;
            while t < secs {
                let mut out = Vec::new();
                g.tick(0.016, &mut out);
                for i in out {
                    e.post(i);
                }
                e.fill(&mut buf);
                t += 512.0 / 32_000.0;
            }
        };
        run(&mut g, &mut engine, Vec::new(), 1.0);
        assert_eq!(engine.state().harmony, Harmony::Original);
        let mut inputs = Vec::new();
        for _ in 0..5 {
            g.press(Button::Toot, &mut inputs);
        }
        assert_eq!(inputs, [Input::Toot; 5]);
        run(&mut g, &mut engine, inputs, 0.05);
        assert_eq!(engine.state().filters.harmony, Harmony::Coltrane);
        assert_eq!(engine.state().stats.last_summon, Some(Harmony::Coltrane));
        // A bar at 184 bpm is 1.3 s: the next uncommitted bar line plays it.
        run(&mut g, &mut engine, Vec::new(), 3.0);
        assert_eq!(engine.state().harmony, Harmony::Coltrane);
        let mut inputs = Vec::new();
        g.press(Button::Death, &mut inputs);
        g.press(Button::Death, &mut inputs);
        run(&mut g, &mut engine, inputs, 3.0);
        assert!(engine.state().filters.just_intonation);
        assert_eq!(engine.state().tuning, Tuning::Medley);
        assert_eq!(g.decided.unwrap().0.harmony, Harmony::Coltrane, "the summon holds through deaths");
    }

    #[test]
    fn auto_nat_plays_like_its_dial() {
        let count = |chaos: f32| {
            let mut a = AutoNat { on: true, chaos, ..AutoNat::default() };
            let mut n = [0u32; 8];
            for k in 0..60 * 60 {
                for b in a.tick(k as f32 / 60.0, 1.0 / 60.0) {
                    n[Button::ALL.iter().position(|x| *x == b).unwrap()] += 1;
                }
            }
            n
        };
        let (calm, wild) = (count(0.0), count(1.0));
        assert!(wild[0] > 4 * calm[0].max(1), "toots {calm:?} {wild:?}");
        assert!(wild[2] > calm[2], "deaths");
        assert!(calm[4] > 20 && calm[5] > 20, "jumps and lands");
    }
}

//! The input timeline of a capture: which keys are down when, in game time, compiled to key
//! presses/releases on exact frames and injected as [`KeyboardInput`] messages (the path real
//! keys take: `ButtonInput<KeyCode>`, then leafwing's `ActionState`).
//!
//! # Format
//! One command per line; `#` starts a comment. A cursor (game seconds from the first recorded
//! frame) starts at 0; commands act at the cursor.
//! - `wait <dur>`: move the cursor on.
//! - `hold <keys...> <dur>`: press the keys, release them `<dur>` later, and move the cursor to
//!   the release (like holding them, then letting go).
//! - `tap <keys...> [<dur>]`: press the keys and release them `<dur>` later (default 0.1 s),
//!   without moving the cursor (for overlapping keys).
//! - `press <keys...>` / `release <keys...>`: latch keys down / up at the cursor.
//! - `at <time>`: put the cursor at an absolute time. `t=<time>` before any command does the
//!   same for that line: `t=4.0 press Space`, `t=4.14 release Space`.
//! - `repeat <n>` ... `end`: the lines between, `n` times (nestable).
//!
//! Times and durations: seconds (`4`, `0.14`, `0.14s`), milliseconds (`140ms`) or frames
//! (`3f`). Keys: Bevy's [`KeyCode`] names (`Space`, `ArrowRight`, `KeyR`, `Enter`,
//! `Escape`, `Digit1`, `F9`, ...), or `Left`/`Right`/`Up`/`Down`/`Esc`, or a single letter
//! (`R`).
//!
//! A held key stays down across back-to-back holds (`hold Right Space 300ms` then
//! `hold Right 450ms` keeps Right down for 750 ms); every hold lasts at least one frame. The
//! timeline lasts until the cursor's final position (or the last release, if later): the
//! capture records exactly that many frames and quits.

use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// A time or duration as written.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Span {
    Secs(f64),
    Frames(u64),
}

impl Span {
    fn secs(self, fps: u32) -> f64 {
        match self {
            Span::Secs(s) => s,
            Span::Frames(f) => f as f64 / fps as f64,
        }
    }
}

/// One parsed command.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Wait(Span),
    At(Span),
    Hold(Vec<KeyCode>, Span),
    Tap(Vec<KeyCode>, Span),
    Press(Vec<KeyCode>),
    Release(Vec<KeyCode>),
    Repeat(u32, Vec<Step>),
}

/// A parsed timeline (frame-rate free).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Script(pub Vec<Step>);

/// One key change on a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub frame: u64,
    pub key: KeyCode,
    pub pressed: bool,
}

/// A timeline compiled at a frame rate: the key changes in frame order, and its length.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Timeline {
    pub events: Vec<KeyEvent>,
    pub frames: u64,
}

impl Timeline {
    /// Its length in game time.
    pub fn duration(&self, fps: u32) -> Duration {
        Duration::from_secs_f64(self.frames as f64 / fps as f64)
    }
}

/// Default `tap` length.
const TAP_SECS: f64 = 0.1;

impl Script {
    /// Parse the format above. Errors name the line.
    pub fn parse(text: &str) -> Result<Script, String> {
        let mut stack: Vec<(u32, Vec<Step>, usize)> = vec![(1, Vec::new(), 0)];
        for (i, raw) in text.lines().enumerate() {
            let n = i + 1;
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let err = |m: String| format!("line {n}: {m}: {raw:?}");
            let mut words: Vec<&str> = line.split_whitespace().collect();
            if let Some(t) = words[0].strip_prefix("t=") {
                let at = span(t).map_err(err)?;
                stack.last_mut().unwrap().1.push(Step::At(at));
                words.remove(0);
                if words.is_empty() {
                    continue;
                }
            }
            let (cmd, args) = (words[0], &words[1..]);
            let step = match cmd {
                "wait" | "pause" => Step::Wait(one_span(args).map_err(err)?),
                "at" => Step::At(one_span(args).map_err(err)?),
                "hold" => {
                    let (keys, d) = keys_then_span(args, false).map_err(err)?;
                    Step::Hold(keys, d.unwrap())
                }
                "tap" => {
                    let (keys, d) = keys_then_span(args, true).map_err(err)?;
                    Step::Tap(keys, d.unwrap_or(Span::Secs(TAP_SECS)))
                }
                "press" => Step::Press(keys(args).map_err(err)?),
                "release" => Step::Release(keys(args).map_err(err)?),
                "repeat" => {
                    let [count] = args else { return Err(err("repeat takes a count".into())) };
                    let c = count.parse::<u32>().map_err(|_| err(format!("bad count {count:?}")))?;
                    stack.push((c, Vec::new(), n));
                    continue;
                }
                "end" => {
                    if !args.is_empty() {
                        return Err(err("end takes nothing".into()));
                    }
                    if stack.len() == 1 {
                        return Err(err("end without repeat".into()));
                    }
                    let (c, body, _) = stack.pop().unwrap();
                    Step::Repeat(c, body)
                }
                other => return Err(err(format!("unknown command {other:?}"))),
            };
            stack.last_mut().unwrap().1.push(step);
        }
        if stack.len() > 1 {
            return Err(format!("line {}: repeat without end", stack.last().unwrap().2));
        }
        Ok(Script(stack.pop().unwrap().1))
    }

    /// The key changes, frame by frame, at `fps`. A time `t` lands on frame `round(t * fps)`.
    pub fn compile(&self, fps: u32) -> Timeline {
        let mut c = Compiler { fps, cursor: 0.0, spans: Vec::new(), latches: Vec::new(), end: 0 };
        c.run(&self.0);
        let frame_of_cursor = c.frame(c.cursor);
        let total = c
            .spans
            .iter()
            .map(|s| s.2)
            .chain(c.latches.iter().map(|l| l.0 + 1))
            .chain([frame_of_cursor, c.end])
            .max()
            .unwrap_or(0);
        let mut keys: Vec<KeyCode> = c.spans.iter().map(|s| s.0).chain(c.latches.iter().map(|l| l.1)).collect();
        keys.sort_by_key(|k| format!("{k:?}"));
        keys.dedup();
        let mut events = Vec::new();
        let mut latched = vec![false; keys.len()];
        let mut down = vec![false; keys.len()];
        for frame in 0..=total {
            for (i, &k) in keys.iter().enumerate() {
                for l in c.latches.iter().filter(|l| l.0 == frame && l.1 == k) {
                    latched[i] = l.2;
                }
                let held = latched[i] || c.spans.iter().any(|s| s.0 == k && s.1 <= frame && frame < s.2);
                if held != down[i] {
                    down[i] = held;
                    events.push(KeyEvent { frame, key: k, pressed: held });
                }
            }
        }
        Timeline { events, frames: total }
    }
}

struct Compiler {
    fps: u32,
    cursor: f64,
    /// (key, first frame down, first frame up).
    spans: Vec<(KeyCode, u64, u64)>,
    /// (frame, key, down), in order.
    latches: Vec<(u64, KeyCode, bool)>,
    end: u64,
}

impl Compiler {
    fn frame(&self, t: f64) -> u64 {
        // A hair over, so a time on a half frame (22.5) always rounds up despite float error.
        (t.max(0.0) * self.fps as f64 + 1e-6).round() as u64
    }

    fn hold(&mut self, keys: &[KeyCode], d: Span) {
        let start = self.frame(self.cursor);
        let stop = self.frame(self.cursor + d.secs(self.fps)).max(start + 1);
        self.spans.extend(keys.iter().map(|&k| (k, start, stop)));
    }

    fn run(&mut self, steps: &[Step]) {
        for s in steps {
            match s {
                Step::Wait(d) => self.cursor += d.secs(self.fps),
                Step::At(t) => self.cursor = t.secs(self.fps),
                Step::Hold(k, d) => {
                    self.hold(k, *d);
                    self.cursor += d.secs(self.fps);
                }
                Step::Tap(k, d) => self.hold(k, *d),
                Step::Press(k) | Step::Release(k) => {
                    let f = self.frame(self.cursor);
                    let down = matches!(s, Step::Press(_));
                    self.latches.extend(k.iter().map(|&k| (f, k, down)));
                }
                Step::Repeat(n, body) => {
                    for _ in 0..*n {
                        self.run(body);
                    }
                }
            }
            self.end = self.end.max(self.frame(self.cursor));
        }
    }
}

fn span(s: &str) -> Result<Span, String> {
    let bad = || format!("bad time {s:?} (try 4, 0.14s, 140ms, 3f)");
    let num = |x: &str| x.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0).ok_or_else(bad);
    if let Some(ms) = s.strip_suffix("ms") {
        Ok(Span::Secs(num(ms)? / 1000.0))
    } else if let Some(f) = s.strip_suffix('f') {
        f.parse::<u64>().map(Span::Frames).map_err(|_| bad())
    } else {
        Ok(Span::Secs(num(s.strip_suffix('s').unwrap_or(s))?))
    }
}

fn one_span(args: &[&str]) -> Result<Span, String> {
    match args {
        [s] => span(s),
        _ => Err("expected one time".into()),
    }
}

fn keys(args: &[&str]) -> Result<Vec<KeyCode>, String> {
    if args.is_empty() {
        return Err("expected keys".into());
    }
    args.iter().map(|k| key_code(k).ok_or_else(|| format!("unknown key {k:?}"))).collect()
}

/// Keys, then a duration (optional if `optional`).
fn keys_then_span(args: &[&str], optional: bool) -> Result<(Vec<KeyCode>, Option<Span>), String> {
    match args.split_last() {
        Some((last, rest)) if key_code(last).is_none() => Ok((keys(rest)?, Some(span(last)?))),
        _ if optional => Ok((keys(args)?, None)),
        _ => Err("expected keys then a duration".into()),
    }
}

/// A key by name (see the module docs).
pub fn key_code(name: &str) -> Option<KeyCode> {
    use KeyCode::*;
    let k = match name {
        "Space" => Space,
        "ArrowLeft" | "Left" => ArrowLeft,
        "ArrowRight" | "Right" => ArrowRight,
        "ArrowUp" | "Up" => ArrowUp,
        "ArrowDown" | "Down" => ArrowDown,
        "Enter" | "Return" => Enter,
        "Escape" | "Esc" => Escape,
        "Backspace" => Backspace,
        "Tab" => Tab,
        "ShiftLeft" | "Shift" => ShiftLeft,
        "ShiftRight" => ShiftRight,
        "ControlLeft" | "Ctrl" => ControlLeft,
        _ => {
            let letter = |c: char| -> Option<KeyCode> {
                const L: [KeyCode; 26] = [
                    KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR,
                    KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
                ];
                const D: [KeyCode; 10] = [Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9];
                match c {
                    'A'..='Z' => Some(L[c as usize - 'A' as usize]),
                    '0'..='9' => Some(D[c as usize - '0' as usize]),
                    _ => None,
                }
            };
            let one = |s: &str| {
                let mut cs = s.chars();
                match (cs.next(), cs.next()) {
                    (Some(c), None) => Some(c),
                    _ => None,
                }
            };
            if let Some(c) = name.strip_prefix("Key").and_then(one).filter(char::is_ascii_uppercase) {
                return letter(c);
            }
            if let Some(c) = name.strip_prefix("Digit").and_then(one) {
                return letter(c).filter(|_| c.is_ascii_digit());
            }
            if let Some(c) = one(name).filter(char::is_ascii_alphabetic) {
                return letter(c.to_ascii_uppercase());
            }
            const F: [KeyCode; 12] = [F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12];
            let n = name.strip_prefix('F')?.parse::<usize>().ok()?;
            return F.get(n.checked_sub(1)?).copied();
        }
    };
    Some(k)
}

/// Plays a [`Timeline`]: each run (one per recorded frame) sends that frame's key changes as
/// [`KeyboardInput`] messages, then moves on a frame. Run it in `PreUpdate` before
/// [`bevy::input::InputSystems`].
#[derive(Resource, Debug, Clone, Default)]
pub struct TimelinePlayer {
    pub timeline: Timeline,
    /// The next frame to play.
    pub frame: u64,
    next: usize,
}

impl TimelinePlayer {
    pub fn new(timeline: Timeline) -> Self {
        TimelinePlayer { timeline, frame: 0, next: 0 }
    }

    /// Every frame of it has played.
    pub fn done(&self) -> bool {
        self.frame >= self.timeline.frames
    }
}

/// The real key path: a winit-shaped [`KeyboardInput`] for the primary window.
pub fn keyboard_input(key: KeyCode, pressed: bool, window: Entity) -> KeyboardInput {
    let logical_key = match key {
        KeyCode::Space => Key::Space,
        KeyCode::Enter => Key::Enter,
        KeyCode::Escape => Key::Escape,
        KeyCode::ArrowLeft => Key::ArrowLeft,
        KeyCode::ArrowRight => Key::ArrowRight,
        KeyCode::ArrowUp => Key::ArrowUp,
        KeyCode::ArrowDown => Key::ArrowDown,
        _ => Key::Unidentified(NativeKey::Unidentified),
    };
    KeyboardInput {
        key_code: key,
        logical_key,
        state: if pressed { ButtonState::Pressed } else { ButtonState::Released },
        text: None,
        repeat: false,
        window,
    }
}

/// See [`TimelinePlayer`].
pub fn play_timeline(
    mut player: ResMut<TimelinePlayer>,
    window: Query<Entity, With<PrimaryWindow>>,
    mut out: MessageWriter<KeyboardInput>,
) {
    let window = window.single().unwrap_or(Entity::PLACEHOLDER);
    let p = &mut *player;
    while let Some(e) = p.timeline.events.get(p.next).filter(|e| e.frame <= p.frame) {
        out.write(keyboard_input(e.key, e.pressed, window));
        p.next += 1;
    }
    p.frame += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use KeyCode::*;

    fn ev(frame: u64, key: KeyCode, pressed: bool) -> KeyEvent {
        KeyEvent { frame, key, pressed }
    }

    #[test]
    fn parses_every_command() {
        let s = Script::parse(
            "# a tour\nwait 4\nhold Space 140ms # jump\nt=5 press Right\nat 6.5s\nrelease ArrowRight\ntap KeyR\ntap Left 3f\nrepeat 2\n  hold Space 0.1\nend\n",
        )
        .unwrap();
        assert_eq!(
            s.0,
            vec![
                Step::Wait(Span::Secs(4.0)),
                Step::Hold(vec![Space], Span::Secs(0.14)),
                Step::At(Span::Secs(5.0)),
                Step::Press(vec![ArrowRight]),
                Step::At(Span::Secs(6.5)),
                Step::Release(vec![ArrowRight]),
                Step::Tap(vec![KeyR], Span::Secs(0.1)),
                Step::Tap(vec![ArrowLeft], Span::Frames(3)),
                Step::Repeat(2, vec![Step::Hold(vec![Space], Span::Secs(0.1))]),
            ]
        );
    }

    #[test]
    fn rejects_mistakes_with_the_line() {
        for (text, needle) in [
            ("wait", "line 1"),
            ("hold Space", "line 1"),
            ("hold Spcae 1", "unknown key"),
            ("wait 1\njump 2", "line 2"),
            ("repeat 2\nwait 1", "repeat without end"),
            ("end", "end without repeat"),
            ("wait -1", "bad time"),
            ("press", "expected keys"),
        ] {
            let e = Script::parse(text).unwrap_err();
            assert!(e.contains(needle), "{text:?}: {e}");
        }
    }

    #[test]
    fn key_names() {
        assert_eq!(key_code("KeyR"), Some(KeyR));
        assert_eq!(key_code("r"), Some(KeyR));
        assert_eq!(key_code("Digit3"), Some(Digit3));
        assert_eq!(key_code("3"), None, "a number is a time");
        assert_eq!(key_code("F9"), Some(F9));
        assert_eq!(key_code("Esc"), Some(Escape));
        assert_eq!(key_code("F13"), None);
        assert_eq!(key_code("Keyr"), None);
        assert_eq!(key_code("Spcae"), None);
    }

    #[test]
    fn compiles_to_frames() {
        // 30 fps: 4 s = frame 120; 140 ms = 4.2 frames -> released on frame 124.
        let t = Script::parse("wait 4\nhold Space 140ms\nwait 120ms\nhold Space 140ms\nwait 1").unwrap().compile(30);
        assert_eq!(t.events, vec![ev(120, Space, true), ev(124, Space, false), ev(128, Space, true), ev(132, Space, false)]);
        // 4 + 0.14 + 0.12 + 0.14 + 1 = 5.4 s.
        assert_eq!(t.frames, 162);
        assert!((t.duration(30).as_secs_f64() - 5.4).abs() < 1e-9);
        // The same at 60 fps.
        let t = Script::parse("wait 4\nhold Space 140ms").unwrap().compile(60);
        assert_eq!(t.events, vec![ev(240, Space, true), ev(248, Space, false)]);
    }

    #[test]
    fn back_to_back_holds_keep_the_key_down() {
        let t = Script::parse("repeat 2\nhold Right Space 300ms\nhold Right 450ms\nend").unwrap().compile(30);
        assert_eq!(
            t.events,
            vec![ev(0, ArrowRight, true), ev(0, Space, true), ev(9, Space, false), ev(23, Space, true), ev(32, Space, false), ev(45, ArrowRight, false)]
        );
        assert_eq!(t.frames, 45);
    }

    #[test]
    fn short_taps_last_a_frame_and_latches_hold() {
        let t = Script::parse("tap Space 1ms\npress Left\nwait 1\nrelease Left\ntap R 2f").unwrap().compile(30);
        assert_eq!(
            t.events,
            vec![ev(0, ArrowLeft, true), ev(0, Space, true), ev(1, Space, false), ev(30, ArrowLeft, false), ev(30, KeyR, true), ev(32, KeyR, false)]
        );
        assert_eq!(t.frames, 32);
    }

    #[test]
    fn the_player_sends_each_frames_keys() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::input::InputPlugin))
            .insert_resource(TimelinePlayer::new(Script::parse("wait 2f\nhold Space 2f").unwrap().compile(30)))
            .add_systems(PreUpdate, play_timeline.before(bevy::input::InputSystems));
        let mut seen = Vec::new();
        for _ in 0..5 {
            app.update();
            let keys = app.world().resource::<ButtonInput<KeyCode>>();
            seen.push((keys.just_pressed(Space), keys.pressed(Space), keys.just_released(Space)));
        }
        assert_eq!(seen, vec![(false, false, false), (false, false, false), (true, true, false), (false, true, false), (false, false, true)]);
        assert!(app.world().resource::<TimelinePlayer>().done());
    }
}

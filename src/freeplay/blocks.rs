//! Side B's building blocks: the pieces Han runs through alone (pits, walls, drip pipes, shafts,
//! trapezes, beat-block doors and bridges, trapdoors, springboards, the forks between them),
//! each drawn with the way through it. The Side B levels are built from them
//! (`examples/side_b_levels.rs` writes `levels/side-b/b1.txt`..`b7.txt` and their routes), and
//! so are free play's Han rooms ([`super::alone`]).
//!
//! A [`Builder`] is a grid of air with a cursor: `x` (the next free column) and `top` (the
//! floor's top row under the cursor). A block draws from the cursor and moves it on, and adds
//! to the *plan* the steps that get Han through it the way it was built: a call (with its lead),
//! a hit (landing on the next beat, so a few columns early; the beat blocks' colours are
//! followed along the plan, so a hit is planned only where one's needed), a hold on the record
//! (Down: Han waits while a call goes in) or a scratch back (Han moonwalks to something he ran
//! past). At a fork the builder *picks* a way ([`Builder::pick`]): a level is built once per
//! route with the same grid, a free play room with its dice.
//!
//! The plan is the text `crate::game::sideb::parse_plan` reads: `col/k` as Han crosses a
//! column, `+secs/k` after the step before (`k`: a set-list call, `20` the hit, `30` / `31` a
//! step of the record back / forward).

use std::fmt::Write as _;

/// The band's calls a block can ask for (the set-list words).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Call {
    Giant,
    Gap,
    Waltz,
    Nervous,
    Melt,
}

impl Call {
    pub const ALL: [Call; 5] = [Call::Giant, Call::Gap, Call::Waltz, Call::Nervous, Call::Melt];

    pub fn word(self) -> &'static str {
        match self {
            Call::Giant => "giant",
            Call::Gap => "gap",
            Call::Waltz => "waltz",
            Call::Nervous => "nervous",
            Call::Melt => "melt",
        }
    }

    /// Columns before its obstacle a call goes in.
    fn lead(self) -> i32 {
        match self {
            Call::Giant => 12,
            Call::Gap => 15,
            Call::Waltz => 20,
            Call::Nervous => 11,
            Call::Melt => 19,
        }
    }
}

/// What a plan step does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Call(Call),
    Hit,
    /// A step of the record back (from playing: held; from held: scratching back).
    Back,
    /// A step forward (from scratching back: held; from held: playing).
    On,
}

/// When a plan step fires: as Han crosses a column, or some seconds after the step before.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum At {
    Col(f32),
    After(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    pub at: At,
    pub act: Act,
}

/// A run of steps that go together (a hold, a scratch back): the first fires at a column, the
/// rest after it. The plan is the groups in column order.
#[derive(Debug, Clone, PartialEq)]
struct Group {
    col: f32,
    steps: Vec<Step>,
}

/// The hit lands on the next beat: this many columns before Han's at what it opens.
const HIT_LEAD: i32 = 5;
/// A call's mode lasts about this many columns of Han's run.
const MODE_COLS: i32 = 36;
/// Melting lasts about this many columns of Han's run.
const MELT_COLS: i32 = 46;
/// A hold on the record: long enough for the call that goes in to take.
const HOLD_SECS: f32 = 1.6;

/// The beat blocks: red (`[`) is there on an even count of hits, blue (`]`) on an odd one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Beat {
    Red,
    Blue,
}

impl Beat {
    fn ch(self) -> u8 {
        match self {
            Beat::Red => b'[',
            Beat::Blue => b']',
        }
    }

    fn there(self, hits: u32) -> bool {
        (hits % 2 == 0) == (self == Beat::Red)
    }

    pub fn other(self) -> Beat {
        match self {
            Beat::Red => Beat::Blue,
            Beat::Blue => Beat::Red,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Builder {
    pub h: usize,
    g: Vec<Vec<u8>>,
    /// The cursor: the next free column, and the floor's top row under it.
    pub x: i32,
    pub top: i32,
    /// The highest and lowest floor the cursor's been on.
    pub min_top: i32,
    pub max_top: i32,
    gates: Vec<String>,
    platforms: Vec<String>,
    groups: Vec<Group>,
    /// The forks' picks, in order (a level's route), or dice values (a room).
    picks: Vec<u32>,
    next_pick: usize,
    /// The hits along the plan so far.
    pub hits: u32,
    /// The last call made, and the column it went in at.
    mode: Option<(Call, i32)>,
    /// The last melting's column.
    melt_at: Option<i32>,
    /// The column the last call's obstacle ends: the next call goes in after it.
    clear_after: i32,
    /// The calls the plan makes.
    pub calls: Vec<Call>,
    /// Draw checkpoints (a level; a free play room has its own at its start).
    pub checkpoints: bool,
    /// Run on where a call would come too soon after the last (a room, built for one way
    /// through); a level, whose routes share a grid, notes it in `tight` instead.
    pub auto_space: bool,
    pub tight: Vec<String>,
    /// The first column a step can be at (Han's start, and a little way on).
    pub first_col: i32,
}

impl Builder {
    /// A grid `h` rows tall, the floor's top at row `top`, the cursor at column `x`.
    pub fn new(h: usize, top: i32, x: i32, picks: Vec<u32>) -> Builder {
        const W: usize = 2400;
        Builder {
            h,
            g: vec![vec![b'.'; W]; h],
            x,
            top,
            min_top: top,
            max_top: top,
            gates: Vec::new(),
            platforms: Vec::new(),
            groups: Vec::new(),
            picks,
            next_pick: 0,
            hits: 0,
            mode: None,
            melt_at: None,
            clear_after: i32::MIN,
            calls: Vec::new(),
            checkpoints: true,
            auto_space: false,
            tight: Vec::new(),
            first_col: x + 3,
        }
    }

    // --- drawing

    pub fn set(&mut self, c: i32, r: i32, ch: u8) {
        if c >= 0 && r >= 0 && (r as usize) < self.h && (c as usize) < self.g[0].len() {
            self.g[r as usize][c as usize] = ch;
        }
    }

    pub fn get(&self, c: i32, r: i32) -> u8 {
        if c < 0 || r < 0 || r as usize >= self.h || c as usize >= self.g[0].len() {
            return b'#';
        }
        self.g[r as usize][c as usize]
    }

    pub fn fill(&mut self, cs: std::ops::Range<i32>, rs: std::ops::Range<i32>, ch: u8) {
        for r in rs {
            for c in cs.clone() {
                self.set(c, r, ch);
            }
        }
    }

    /// Ground from row `top` down, columns `c0..c1`.
    pub fn ground(&mut self, c0: i32, c1: i32, top: i32) {
        let h = self.h as i32;
        self.fill(c0..c1, top..h, b'#');
    }

    pub fn clear(&mut self, c0: i32, c1: i32, r0: i32, r1: i32) {
        self.fill(c0..c1, r0..r1, b'.');
    }

    pub fn gate(&mut self, topic: &str, c0: i32, r0: i32, c1: i32, r1: i32) {
        self.gates.push(format!("gate: {topic} {c0},{r0} {c1},{r1}"));
    }

    fn floor_to(&mut self, top: i32) {
        self.top = top;
        self.min_top = self.min_top.min(top);
        self.max_top = self.max_top.max(top);
    }

    // --- the plan

    /// The next fork's pick of `n` ways.
    pub fn pick(&mut self, n: usize) -> usize {
        let v = self.picks.get(self.next_pick).copied().unwrap_or(0);
        self.next_pick += 1;
        v as usize % n
    }

    fn group(&mut self, col: i32, steps: Vec<Step>) {
        self.groups.push(Group { col: col as f32, steps });
    }

    fn step_at(&mut self, col: i32, act: Act) {
        self.group(col, vec![Step { at: At::Col(col as f32), act }]);
    }

    /// The call `c` for an obstacle from `x0` (columns from the cursor: the obstacle's first)
    /// to `x1`: runs on first if the last call's obstacle isn't behind Han by then (a call
    /// replaces the one before). Skipped when the same call's still on from just before.
    pub fn need(&mut self, c: Call, x0: i32, x1: i32) {
        let still = matches!(self.mode, Some((m, at)) if m == c && self.x + x1 <= at + MODE_COLS);
        if still {
            self.clear_after = self.clear_after.max(self.x + x1);
            return;
        }
        let want = self.x + x0 - c.lead();
        // A leap's no good melted (small, he falls short): fired up waits for the melting to
        // wear off.
        let clear = match (c, self.melt_at) {
            (Call::Gap, Some(m)) => self.clear_after.max(m + MELT_COLS),
            _ => self.clear_after,
        };
        if want < clear + 1 {
            let more = clear + 1 - want;
            self.tight.push(format!("{} call for column {} +{more}", c.word(), self.x + x0));
            if self.auto_space {
                self.filler(more, c.lead() - x0);
            }
        }
        // (Not before Han's on his way: he starts a few columns in.)
        let col = (self.x + x0 - c.lead()).max(self.first_col);
        self.step_at(col, Act::Call(c));
        self.called(c, col);
        self.clear_after = self.x + x1;
    }

    /// The plan's made call `c` at column `col`.
    fn called(&mut self, c: Call, col: i32) {
        self.mode = Some((c, col));
        if c == Call::Melt {
            self.melt_at = Some(col);
        }
        self.calls.push(c);
    }

    /// A hold on the record from column `col`: Han waits while `c` goes in, then runs on.
    pub fn hold(&mut self, col: i32, c: Call) {
        self.group(
            col,
            vec![
                Step { at: At::Col(col as f32), act: Act::Back },
                Step { at: At::After(0.05), act: Act::Call(c) },
                Step { at: At::After(HOLD_SECS), act: Act::On },
            ],
        );
        self.called(c, col);
    }

    /// A hit to land as Han reaches column `col`.
    fn hit_before(&mut self, col: i32) {
        self.step_at(col - HIT_LEAD, Act::Hit);
        self.hits += 1;
    }

    /// The plan, in `parse_plan`'s text, with the calls numbered by `setlist`.
    pub fn plan(&self, setlist: &[Call]) -> String {
        let mut gs = self.groups.clone();
        gs.sort_by(|a, b| a.col.total_cmp(&b.col));
        // Two steps at one column: the second a column on (a step fires as Han crosses its
        // column, one a frame).
        for i in 1..gs.len() {
            if gs[i].col <= gs[i - 1].col {
                let c = gs[i - 1].col + 1.0;
                gs[i].col = c;
                if let Some(Step { at: At::Col(x), .. }) = gs[i].steps.first_mut() {
                    *x = c;
                }
            }
        }
        let mut out = String::new();
        for s in gs.iter().flat_map(|g| &g.steps) {
            let k = match s.act {
                Act::Call(c) => setlist.iter().position(|&x| x == c).unwrap_or_else(|| panic!("{} isn't on the set list", c.word())),
                Act::Hit => 20,
                Act::Back => 30,
                Act::On => 31,
            };
            if !out.is_empty() {
                out.push(' ');
            }
            match s.at {
                At::Col(c) => write!(out, "{c}/{k}"),
                At::After(t) => write!(out, "+{t}/{k}"),
            }
            .unwrap();
        }
        out
    }

    // --- finishing

    /// The grid's rows up to column `w` (exclusive).
    pub fn rows(&self, w: i32) -> Vec<String> {
        self.g.iter().map(|r| String::from_utf8_lossy(&r[..w as usize]).into_owned()).collect()
    }

    /// The header lines: gate marks and platforms.
    pub fn header(&self) -> Vec<String> {
        self.platforms.iter().chain(&self.gates).cloned().collect()
    }

    /// A Side B level's text: the goal near the end, the grid trimmed there.
    pub fn level_text(&mut self, name: &str, setlist: &[Call]) -> String {
        let (x, t) = (self.x, self.top);
        self.set(x - 5, t - 1, b'G');
        let words: Vec<&str> = setlist.iter().map(|c| c.word()).collect();
        let mut s = format!("name: {name}\nworld: 2\nsetlist: {}\n", words.join(" "));
        for l in self.header() {
            s.push_str(&l);
            s.push('\n');
        }
        s.push_str("---\n");
        for r in self.rows(x) {
            s.push_str(&r);
            s.push('\n');
        }
        s
    }

    // --- pieces: plain going

    /// `n` columns of floor, nuggets `d` columns in.
    pub fn run(&mut self, n: i32, nuggets: &[i32]) {
        let (x, t) = (self.x, self.top);
        self.ground(x, x + n, t);
        for &d in nuggets {
            self.set(x + d, t - 1, b'o');
        }
        self.x += n;
    }

    /// `n` columns of going, the last `approach` of them Han's run-up to what's next: floor,
    /// with a hop (spikes or a little pit) before the run-up when there's room, and nuggets.
    pub fn filler(&mut self, n: i32, approach: i32) {
        if n - approach < 8 {
            self.run(n, &[n / 2]);
            return;
        }
        self.run(2, &[]);
        if (self.x / 7) % 2 == 0 {
            self.spikes(2);
            self.run(1, &[]);
        } else {
            self.hop(3);
        }
        let b = n - 5;
        self.run(b, &[b / 2]);
    }

    /// The start: Han's spot, and a run.
    pub fn start(&mut self, n: i32) {
        let t = self.top;
        self.set(self.x + 2, t - 1, b'P');
        self.run(n, &[n / 2, n / 2 + 3]);
    }

    /// A checkpoint on the next tile of run (a level's only).
    pub fn cp(&mut self) {
        if self.checkpoints {
            let t = self.top;
            self.set(self.x + 1, t - 1, b'C');
        }
    }

    pub fn nug(&mut self, dc: i32, dr: i32) {
        let t = self.top;
        self.set(self.x + dc, t - dr, b'o');
    }

    /// The floor moves by `dy` rows (down, positive; up only by 2, a hop) and runs `n`.
    pub fn step(&mut self, dy: i32, n: i32) {
        debug_assert!(dy >= -2, "a step up of {dy} needs Giant Steps: climb");
        self.floor_to(self.top + dy);
        self.run(n, &[n / 2]);
    }

    /// Stairs down: `k` steps of `dy`, `n` columns each.
    pub fn stairs_down(&mut self, k: i32, dy: i32, n: i32) {
        for _ in 0..k {
            self.step(dy, n);
        }
    }

    /// Stairs up: `k` hops of 2, `n` columns each (no call).
    pub fn stairs_up(&mut self, k: i32, n: i32) {
        for _ in 0..k {
            self.step(-2, n);
        }
    }

    /// A pit `w` wide Han hops (up to 7) with no call.
    pub fn hop(&mut self, w: i32) {
        let (x, t) = (self.x, self.top);
        let h = self.h as i32;
        self.clear(x, x + w, t, h);
        self.x += w;
    }

    /// Floor spikes `n` wide (hopped, no call).
    pub fn spikes(&mut self, n: i32) {
        let (x, t) = (self.x, self.top);
        self.ground(x, x + n, t);
        self.fill(x..x + n, t - 1..t, b'^');
        self.x += n;
    }

    // --- pieces: the calls

    /// A pit `w` wide (9: fired up), bottomless or with sewage at its foot.
    pub fn pit(&mut self, w: i32, sewage: bool) {
        self.need(Call::Gap, 0, w);
        self.pit_drawn(w, sewage);
    }

    fn pit_drawn(&mut self, w: i32, sewage: bool) {
        let (x, t) = (self.x, self.top);
        let h = self.h as i32;
        // (One more column than it says, eaten from the run before: Han running flat out
        // clears 9 at a stretch, so it takes 10 to need fired up.)
        self.clear(x - 1, x + w, t, h);
        if sewage {
            self.fill(x..x + w, h - 1..h, b'~');
        }
        self.gate("gap", x, t - 1, x + w - 1, h - 1);
        self.x += w;
    }

    /// A wall `h` tall (6: Giant Steps), `w` wide, then on at the same height.
    pub fn wall(&mut self, h: i32, w: i32) {
        self.need(Call::Giant, 0, w);
        let (x, t) = (self.x, self.top);
        self.ground(x, x + w, t - h);
        self.gate("giant", x, t - h, x + w - 1, t - 1);
        self.x += w;
    }

    /// A step `h` up (Giant Steps), then `n` on along its top.
    pub fn climb(&mut self, h: i32, n: i32, nuggets: &[i32]) {
        self.need(Call::Giant, 0, 3);
        let (x, t) = (self.x, self.top);
        self.gate("giant", x, t - h, x + 2, t - 1);
        self.floor_to(t - h);
        self.run(n, nuggets);
    }

    /// Drips hanging over the floor (melt to pass), under `roof` rows of pipe.
    pub fn drips(&mut self, w: i32, roof: i32) {
        self.need(Call::Melt, 0, w);
        self.drips_drawn(w, roof);
    }

    fn drips_drawn(&mut self, w: i32, roof: i32) {
        let (x, t) = (self.x, self.top);
        self.ground(x, x + w, t);
        self.fill(x..x + w, t - 1 - roof..t - 1, b'#');
        self.fill(x..x + w, t - 1..t, b'v');
        self.x += w;
    }

    /// A cliff `h` tall (nervous climbs it), `w` on along its top.
    pub fn shaft(&mut self, h: i32, w: i32) {
        self.need(Call::Nervous, 0, 1);
        let (x, t) = (self.x, self.top);
        self.ground(x, x + w, t - h);
        self.gate("shaft", x - 8, t - h - 1, x - 1, t - 1);
        self.floor_to(t - h);
        self.x += w;
    }

    /// A trapeze: off an overhang over a pit `w` wide, landing `drop` rows lower (the waltz).
    pub fn trapeze(&mut self, drop: i32, w: i32) {
        self.need(Call::Waltz, 0, w);
        let (x, t) = (self.x, self.top);
        let h = self.h as i32;
        self.fill(x - 2..x + 6, t - 8..t - 6, b'#');
        self.clear(x, x + w, t, h);
        self.fill(x..x + w, h - 1..h, b'~');
        self.gate("trapeze", x - 4, t - 6, x + w - 1, t + drop - 1);
        self.x += w;
        self.floor_to(t + drop);
        let (x, t) = (self.x, self.top);
        self.ground(x, x + 1, t);
    }

    /// A cliff `h` tall over a drip pipe: nervous he climbs its face, melted he goes under.
    pub fn face_over_drips(&mut self, w: i32, h: i32) {
        let way = self.pick(2);
        if way == 0 {
            self.need(Call::Nervous, 0, w);
        } else {
            self.need(Call::Melt, 0, w);
        }
        let (x, t) = (self.x, self.top);
        self.ground(x, x + w, t);
        self.fill(x..x + w, t - h..t - 1, b'#');
        self.fill(x..x + w, t - 1..t, b'v');
        self.gate("shaft", x - 8, t - h - 1, x - 1, t - 1);
        for d in [3, w / 2, w - 3] {
            self.set(x + d, t - h - 1, b'o');
        }
        self.x += w;
        self.clear_after = self.clear_after.max(self.x);
    }

    // --- pieces: the hit

    /// A beat-block door `h` tall (red: there until the first hit).
    pub fn door(&mut self, beat: Beat, h: i32) {
        let x = self.x;
        if beat.there(self.hits) {
            self.hit_before(x);
        }
        let t = self.top;
        self.fill(x..x + 1, t - h..t, beat.ch());
        self.ground(x, x + 1, t);
        self.x += 1;
    }

    /// A pit `w` wide with a beat-block bridge across at floor level.
    pub fn bridge(&mut self, beat: Beat, w: i32) {
        let x = self.x;
        if !beat.there(self.hits) {
            self.hit_before(x - 1);
        }
        let (t, h) = (self.top, self.h as i32);
        self.clear(x - 1, x + w, t, h);
        self.fill(x - 1..x + w, t..t + 1, beat.ch());
        self.x += w;
    }

    /// Bridges to drum across: one of each colour in turn from `first` (a hit for each after
    /// the first), `ledge` columns of floor between them.
    pub fn drum(&mut self, first: Beat, n: i32, w: i32, ledge: i32) {
        let mut beat = first;
        for i in 0..n {
            self.bridge(beat, w);
            beat = beat.other();
            if i + 1 < n {
                self.run(ledge, &[ledge / 2]);
            }
        }
    }

    /// Trapdoor tiles `w` wide over a shaft `depth` rows down (cleared to `depth`).
    fn trapdoor_drawn(&mut self, w: i32, depth: i32) {
        let (x, t) = (self.x, self.top);
        self.fill(x..x + w, t..t + 1, b'-');
        self.clear(x, x + w, t + 1, t + depth);
        self.x += w;
    }

    /// The deck over a pipe (B3's fork): a trapdoor in the deck (a hit as Han's on it drops him
    /// into the pipe; melted for its drips, if it has them) or on along the deck over what's on
    /// it ([`Deck`]: a hole in it drops him into the pipe too). At the deck's end he drops to
    /// the pipe's floor (6 down), and both ways go on from there.
    pub fn deck_fork(&mut self, deck: Deck, drips: bool) {
        let down = self.pick(2) == 0;
        let (t0, t) = (self.x, self.top);
        let p = t + 6;
        if down {
            self.step_at(t0 + 2, Act::Hit);
            self.hits += 1;
            if drips {
                // (Melted before the drop, for the drips in the pipe.)
                self.call_at(t0 + 12 - Call::Melt.lead(), Call::Melt);
            }
        }
        self.trapdoor_drawn(10, 6);
        self.run(8, &[4]);
        match deck {
            Deck::Walls(n) => {
                for _ in 0..n {
                    if !down {
                        self.wall(6, 3);
                    } else {
                        let (x, t) = (self.x, self.top);
                        self.ground(x, x + 3, t - 6);
                        self.gate("giant", x, t - 6, x + 2, t - 1);
                        self.x += 3;
                    }
                    self.run(10, &[5]);
                }
            }
            Deck::Pit => {
                if !down {
                    self.need(Call::Gap, 0, 9);
                }
                let x = self.x;
                self.clear(x - 1, x + 9, t, t + 2);
                self.gate("gap", x, t - 1, x + 8, p - 1);
                self.x += 9;
                self.run(10, &[5]);
            }
            Deck::Trapeze => {
                if !down {
                    self.need(Call::Waltz, 0, 19);
                }
                let x = self.x;
                self.fill(x - 2..x + 6, t - 8..t - 6, b'#');
                self.clear(x, x + 19, t, t + 2);
                self.gate("trapeze", x - 4, t - 6, x + 18, t - 1);
                self.x += 19;
                self.run(10, &[5]);
            }
        }
        let t1 = self.x;
        // The pipe under the deck: the deck a 2-row slab over it.
        self.clear(t0, t1, t + 2, p);
        self.ground(t0, t1, p);
        if drips {
            self.fill(t0 + 12..t1 - 4, t + 2..p - 1, b'#');
            self.fill(t0 + 12..t1 - 4, p - 1..p, b'v');
        }
        for d in (t0 + 2..t0 + 10).step_by(3) {
            self.set(d, p - 1, b'o');
        }
        // At its end the deck stops: off it he drops to the pipe's floor, and both go on there.
        self.clear(t1, t1 + 8, 0, p);
        self.clear_after = t1;
        self.floor_to(p);
        self.run(8, &[4]);
    }

    /// The call `c` at column `col` (a block that knows where it goes).
    fn call_at(&mut self, col: i32, c: Call) {
        self.step_at(col, Act::Call(c));
        self.called(c, col);
    }

    /// The scratch back: a trapdoor Han runs over, then a dead end (a wall to the top). Scratch
    /// the record back to moonwalk him onto the trapdoor, hit, and he drops into the tunnel
    /// under the wall, on at `depth` rows down.
    pub fn scratch_back(&mut self, depth: i32) {
        let (t0, t) = (self.x, self.top);
        self.trapdoor_drawn(6, depth);
        self.run(6, &[3]);
        let w0 = self.x;
        self.fill(w0..w0 + 6, 0..t, b'#');
        self.run(6, &[]);
        let low = t + depth;
        // The tunnel: open under the trapdoor and the wall, its roof the run.
        self.clear(t0, w0 + 12, t + 1, low);
        self.ground(t0, w0 + 14, low);
        self.fill(w0 + 6..w0 + 14, t..low - 3, b'#');
        for d in [t0 + 8, t0 + 12, w0 + 4] {
            self.set(d, low - 1, b'o');
        }
        // Scratch back as he's at the wall, the hit on the trapdoor, then play.
        self.group(
            w0 - 2,
            vec![
                Step { at: At::Col((w0 - 2) as f32), act: Act::Back },
                Step { at: At::After(0.01), act: Act::Back },
                Step { at: At::Col((t0 + 3) as f32), act: Act::Hit },
                Step { at: At::After(0.5), act: Act::On },
                Step { at: At::After(0.01), act: Act::On },
            ],
        );
        self.hits += 1;
        self.clear_after = w0;
        self.x = w0 + 12;
        self.floor_to(low);
        self.ground(self.x - 12, self.x, low);
        self.run(2, &[]);
    }

    /// The springboard: a trampoline in a dip under a beat-block lid. A hit takes the lid off:
    /// Han drops onto the trampoline and springs up onto the high road (a shelf 4 up, `n`
    /// long); with the lid on he runs on under it, where `low` waits: sewage he can't cross
    /// (the hit's a must), or drips (melt).
    pub fn springboard(&mut self, beat: Beat, n: i32, low: Low) {
        let up = match low {
            Low::Sewage => true,
            Low::Drips => self.pick(2) == 0,
        };
        if low == Low::Drips && !up {
            self.need(Call::Melt, 9, 4 + n - 1);
        }
        let (x, t) = (self.x, self.top);
        // Up: the lid has to be off as he gets there; under: on.
        if beat.there(self.hits) == up {
            self.hit_before(x);
        }
        self.clear(x, x + 4, t, t + 1);
        self.fill(x..x + 4, t + 1..t + 2, b'T');
        self.fill(x..x + 4, t..t + 1, beat.ch());
        self.ground(x, x + 4, t + 2);
        self.x += 4;
        let s = self.x + 2;
        self.run(n + 6, &[]);
        self.fill(s..s + n, t - 4..t - 3, b'#');
        for d in (4..n - 2).step_by(4) {
            self.set(s + d, t - 5, b'o');
        }
        match low {
            Low::Sewage => {
                self.clear(s + 2, s + n - 2, t, t + 3);
                self.fill(s + 2..s + n - 2, t + 2..t + 3, b'~');
                self.ground(s + 2, s + n - 2, t + 3);
            }
            Low::Drips => self.fill(s + 3..s + n - 3, t - 1..t, b'v'),
        }
        self.clear_after = self.clear_after.max(s + n);
    }

    /// A springboard with no lid: a trampoline in a dip springs Han up a step of 4 (no call).
    pub fn spring_step(&mut self, n: i32) {
        let (x, t) = (self.x, self.top);
        self.clear(x, x + 3, t, t + 1);
        self.fill(x..x + 3, t + 1..t + 2, b'T');
        self.ground(x, x + 3, t + 2);
        self.x += 3;
        // The step up: 4 rows, a column of air before it to rise past.
        self.run(2, &[]);
        let x = self.x;
        self.ground(x, x + 1, t);
        self.floor_to(t - 4);
        self.run(n, &[n / 2]);
    }

    /// The shelf fork (B1's): a 1-high step, then a shelf 6 up for `n` columns: up on it
    /// (Giant Steps, its 2-high bumps `hops`) or along under it, melted for its drips.
    pub fn shelf_fork(&mut self, n: i32, hops: &[i32], nuggets: &[i32]) {
        let up = self.pick(2) == 0;
        if up {
            // (Giant Steps has to be on well before the step: he jumps from a run.)
            self.need(Call::Giant, -10, n + 2);
        } else {
            self.need(Call::Melt, 6, n - 2);
        }
        let (x, t) = (self.x, self.top);
        self.ground(x, x + n + 2, t);
        self.set(x, t - 1, b'#');
        self.gate("giant", x + 1, t - 6, x + 2, t - 2);
        let s = x + 2;
        self.fill(s..s + n, t - 6..t - 4, b'#');
        for &d in hops {
            self.fill(s + d..s + d + 2, t - 8..t - 6, b'#');
        }
        for &d in nuggets {
            self.set(s + d, t - 7, b'o');
        }
        self.fill(s + 4..s + n - 4, t - 4..t - 1, b'#');
        self.fill(s + 4..s + n - 4, t - 1..t, b'v');
        self.x = s + n;
        // (Either way, the next call waits till he's past it all: the routes share a grid.)
        self.clear_after = self.clear_after.max(self.x);
    }

    /// Up a step (Giant Steps) and straight off its top a pit (fired up): hold the record up
    /// there while the gap goes in.
    pub fn climb_then_pit(&mut self, top_run: i32) {
        self.climb(6, top_run, &[top_run / 2]);
        let x = self.x;
        self.hold(x - top_run + 1, Call::Gap);
        self.pit_drawn(9, false);
        self.clear_after = self.x;
    }

    /// Up a cliff (nervous) and straight off its top a pit (fired up): hold the record up there
    /// while the gap goes in.
    pub fn shaft_then_pit(&mut self, h: i32, top_run: i32) {
        self.shaft(h, top_run);
        let x = self.x;
        self.hold(x - top_run + 1, Call::Gap);
        self.pit_drawn(9, false);
        self.clear_after = self.x;
    }

    /// Drips with a hold before them: Han waits while the melting goes in (after a trapeze or a
    /// climb, when there's no run-up for the call).
    pub fn drips_held(&mut self, w: i32) {
        let x = self.x;
        self.hold(x - 6, Call::Melt);
        self.clear_after = x + w;
        self.drips_drawn(w, 3);
    }

    /// A ferry over a sewage lake `lake` wide (Han waits for it and rides).
    pub fn ferry(&mut self, lake: i32) {
        let n = self.platforms.len() + 1;
        let (x, t) = (self.x, self.top);
        let h = self.h as i32;
        self.clear(x, x + lake, t, h);
        self.fill(x..x + lake, t..h, b'~');
        self.fill(x..x + 3, t..t + 1, b'0' + n as u8);
        self.platforms.push(format!("{n}: dx=5 dy=0 period=3"));
        self.x += lake;
    }

    /// A ferry, and a pit off its landing: hold the record as he steps off while the gap goes
    /// in (no run-up for it on a ferry).
    pub fn ferry_then_pit(&mut self, lake: i32) {
        self.ferry(lake);
        let x = self.x;
        self.run(4, &[]);
        self.hold(x + 1, Call::Gap);
        self.pit_drawn(9, true);
        self.clear_after = self.x;
    }

    /// A tower: up a shaft (nervous), a door and a bridge on top, down the far side by steps.
    pub fn tower(&mut self, h: i32) {
        self.shaft(h, 6);
        self.run(6, &[3]);
        self.door(Beat::Red, 6);
        self.run(6, &[]);
        self.bridge(Beat::Blue, 9);
        self.run(6, &[3]);
        let down = h / 3;
        for _ in 0..3 {
            self.step(down, 5);
        }
    }

    /// Fill to at least `n` columns past `x0` with run (for a set length).
    pub fn pad_to(&mut self, col: i32) {
        if self.x < col {
            let n = col - self.x;
            self.run(n, &[n / 2]);
        }
    }
}

/// What's on the deck of a [`Builder::deck_fork`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deck {
    /// Walls 6 tall (Giant Steps), this many.
    Walls(i32),
    /// A pit (fired up) that drops into the pipe.
    Pit,
    /// A trapeze over a hole into the pipe (the waltz).
    Trapeze,
}

/// What's under a springboard's high road.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Low {
    Sewage,
    Drips,
}

//! Teaching without stopping the game. A new player meets the levels' gadgets (shelves, rides,
//! flies, spray cans, stains, grease) somewhere safe to try, with no words. Words only go to
//! what can't be seen: the band's counters and the call, Han's head (one idea: hop on his head
//! and jump; the band decides how big) and his pool float. Each is a short [`Hint`] line under
//! the counter strip that stays while it matters and goes once you do the thing (or after
//! [`HINT_SECS`]). The first toot is a key bubble over Nat instead. Nothing freezes.
//!
//! Counters are taught at their gate's hint spot (never by filling up early, so level 1 is
//! only about the toot and the horn); the sweat drop just appears when the band gets nervous.
//! The first full counter gets the call's line. Each is taught once per save
//! ([`Progress::taught`], [`Progress::taught_call`], [`Progress::taught_tricks`],
//! [`Progress::taught_tips`]); dying again and again at a hint spot brings its line back
//! (the adaptive engine's `han_hint` lever: [`HintRepeated`]).
//!
//! The pause menu's BAND LESSONS shows the old cards as a manual: there (asked for) the game
//! does wait on them ([`PlayState::Lesson`]); Jump (a tap on touch) dismisses each, after
//! [`ARM_SECS`]. `NATHAN_LESSONS=0` turns the lines off ([`LessonsOff`]).

use std::collections::VecDeque;

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::{palette::*, *};
use crate::audio::{BandMeters, Harmony, director::SUMMON_ORDER};
use crate::events::{HintRepeated, Jumped};
use crate::game::{HanBoosted, HanFloat, HintSpot, Player, Pos};
use crate::input::Action;
use crate::level::Topic;
use crate::save::{BandCounter, HanTrick, Progress};
use crate::state::{AppState, PlayState};
use crate::touch::TouchMode;

/// A card ignores Jump this long after it opens (real seconds).
pub const ARM_SECS: f32 = 0.5;
/// A hint line stays at most this long (play seconds) if you don't do the thing.
pub const HINT_SECS: f32 = 12.0;
/// ...and at least this long, even once done (so it's read).
pub const HINT_MIN_SECS: f32 = 1.5;

pub fn plugin(app: &mut App) {
    if std::env::var("NATHAN_LESSONS").as_deref() == Ok("0") {
        app.insert_resource(LessonsOff);
    }
    app.init_resource::<Lessons>()
        .add_systems(OnExit(AppState::Playing), |mut l: ResMut<Lessons>| {
            *l = Lessons::default()
        })
        .add_systems(
            Update,
            (
                (queue_lessons, run_hints)
                    .chain()
                    .run_if(in_state(PlayState::Running)),
                show_lessons,
                dismiss.run_if(in_state(PlayState::Lesson)),
                draw_card,
                draw_hint,
                draw_key_bubble,
            )
                .chain()
                .run_if(in_state(AppState::Playing)),
        );
}

/// No hint lines (`NATHAN_LESSONS=0`; video capture, unless `NATHAN_LESSONS=1`). The counters
/// are still taught (they show).
#[derive(Resource, Debug, Default)]
pub struct LessonsOff;

/// A lesson card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lesson {
    /// A counter appears.
    Counter(BandCounter),
    /// A counter is full: press Down to call the band (its mode named).
    Call(BandCounter),
    /// One of Han's tricks.
    Trick(HanTrick),
    /// Any other mechanic ([`tip_of`]).
    Tip(Topic),
}

/// The hint lines waiting and the one up; the pause menu's cards waiting and the one up.
#[derive(Resource, Debug, Default)]
pub struct Lessons {
    pub hints: VecDeque<Lesson>,
    pub hint: Option<Hint>,
    pub queue: VecDeque<Lesson>,
    pub showing: Option<Lesson>,
    /// Real time the card went up.
    shown_at: f32,
}

/// A hint line up: what it teaches, how long it's been up (play s), and whether it's been done.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hint {
    pub lesson: Lesson,
    pub age: f32,
    pub done: bool,
}

impl Lessons {
    /// Queue every card the save has been taught (the pause menu's BAND LESSONS).
    pub fn replay(&mut self, progress: &Progress) {
        for c in BandCounter::ALL {
            if progress.taught[c.index()] {
                self.queue.push_back(Lesson::Counter(c));
            }
        }
        if progress.taught_call {
            let first = BandCounter::ALL[..3]
                .iter()
                .copied()
                .find(|c| progress.taught[c.index()])
                .unwrap_or(BandCounter::Horn);
            self.queue.push_back(Lesson::Call(first));
        }
        for t in HanTrick::ALL {
            if progress.taught_tricks[t.index()] {
                self.queue.push_back(Lesson::Trick(t));
            }
        }
        for t in Topic::ALL {
            if progress.taught_tips[t.index()] {
                self.queue.push_back(Lesson::Tip(t));
            }
        }
    }
}

/// The counter a gate's hint teaches.
pub fn counter_of(topic: Topic) -> Option<BandCounter> {
    match topic {
        Topic::Giant => Some(BandCounter::Horn),
        Topic::Gap => Some(BandCounter::Nugget),
        Topic::Waltz => Some(BandCounter::Note),
        Topic::Grip => Some(BandCounter::Sweat),
        _ => None,
    }
}

/// The Han trick a gate's hint teaches.
pub fn trick_of(topic: Topic) -> Option<HanTrick> {
    match topic {
        Topic::Chain => Some(HanTrick::Throw),
        Topic::Mega => Some(HanTrick::Mega),
        Topic::Shaft => Some(HanTrick::Cling),
        Topic::Trapeze => Some(HanTrick::Trapeze),
        Topic::BuddyRaft => Some(HanTrick::Raft),
        _ => None,
    }
}

/// A mechanic with neither a counter nor a Han trick: its own card.
pub fn tip_of(topic: Topic) -> Option<Topic> {
    (counter_of(topic).is_none() && trick_of(topic).is_none()).then_some(topic)
}

/// The cards a hint's topics teach, taught or not (a repeated hint shows them again).
pub fn cards_of(topics: &[Topic]) -> Vec<Lesson> {
    let mut out = Vec::new();
    for &t in topics {
        let l = match (counter_of(t), trick_of(t)) {
            (Some(c), _) => Lesson::Counter(c),
            (_, Some(k)) => Lesson::Trick(k),
            _ => Lesson::Tip(t),
        };
        if !out.contains(&l) {
            out.push(l);
        }
    }
    out
}

fn counter_of_harmony(h: Harmony) -> Option<BandCounter> {
    SUMMON_ORDER
        .iter()
        .position(|&s| s == h)
        .map(|i| BandCounter::ALL[i])
}

/// Han's tricks off his head: one idea (hop on, jump; the band decides how big), so one line
/// teaches them all.
pub const HEAD_TRICKS: [HanTrick; 4] = [
    HanTrick::Throw,
    HanTrick::Mega,
    HanTrick::Cling,
    HanTrick::Trapeze,
];

/// The lessons now due: counters and Han's tricks taught by hint spots just heard, the toot,
/// the first full (shown) counter's call, the sweat drop when the band gets nervous (taught
/// with no line: it shows itself). Marks them taught (once per save). Only lessons with a line
/// ([`hint_text`]) come back.
pub fn due(
    progress: &mut Progress,
    heard: &[Topic],
    meters: &crate::audio::director::Meters,
) -> Vec<Lesson> {
    let mut out = Vec::new();
    for &t in heard {
        if let Some(c) = counter_of(t)
            && !progress.taught[c.index()]
        {
            progress.taught[c.index()] = true;
            if c != BandCounter::Sweat {
                out.push(Lesson::Counter(c));
            }
        }
    }
    for &t in heard {
        let Some(k) = trick_of(t) else { continue };
        if progress.taught_tricks[k.index()] {
            continue;
        }
        if HEAD_TRICKS.contains(&k) {
            for h in HEAD_TRICKS {
                progress.taught_tricks[h.index()] = true;
            }
        } else {
            progress.taught_tricks[k.index()] = true;
        }
        out.push(Lesson::Trick(k));
    }
    for &t in heard {
        if let Some(t) = tip_of(t)
            && !progress.taught_tips[t.index()]
        {
            progress.taught_tips[t.index()] = true;
            if hint_text(Lesson::Tip(t), false).is_some() {
                out.push(Lesson::Tip(t));
            }
        }
    }
    if meters.deaths.full {
        progress.taught[BandCounter::Sweat.index()] = true;
    }
    if !progress.taught_call
        && let Some(c) = meters
            .ready
            .and_then(counter_of_harmony)
            .filter(|c| progress.taught[c.index()])
    {
        progress.taught_call = true;
        out.push(Lesson::Call(c));
    }
    out
}

/// A lesson's hint line, if it has one (the levels show the rest). The toot's is the key
/// bubble over Nat.
pub fn hint_text(l: Lesson, touch: bool) -> Option<String> {
    let jump = if touch { "TAP JUMP" } else { "JUMP" };
    Some(match l {
        Lesson::Counter(BandCounter::Horn) => "TOOTS FILL THE HORN".into(),
        Lesson::Counter(BandCounter::Nugget) => "QUICK NUGGETS FIRE UP THE BAND".into(),
        Lesson::Counter(BandCounter::Note) => "HOP, HOP, HOP IN TIME: A WALTZ".into(),
        Lesson::Counter(BandCounter::Sweat) => return None,
        Lesson::Call(c) => {
            if touch {
                format!("TAP THE {}: CALL THE BAND", c.name().to_uppercase())
            } else {
                "PRESS DOWN: CALL THE BAND".into()
            }
        }
        Lesson::Trick(HanTrick::Raft) => "HOP ON HAN! LEFT/RIGHT ROW".into(),
        Lesson::Trick(_) => format!("HOP ON HAN'S HEAD AND {jump}!"),
        Lesson::Tip(Topic::Toot) => format!("{jump} {jump}"),
        Lesson::Tip(_) => return None,
    })
}

/// The hint line just did its thing: a counter filled, the band was called, Nat jumped off
/// Han's head (or rode his float), Nat tooted.
fn hint_done(
    l: Lesson,
    meters: &crate::audio::director::Meters,
    boosted: bool,
    tooted: bool,
    floating: bool,
) -> bool {
    match l {
        Lesson::Counter(c) => meters
            .summons
            .get(c.index())
            .is_some_and(|m| m.full || m.active.is_some()),
        Lesson::Call(_) => meters.summons.iter().any(|m| m.active.is_some()),
        Lesson::Trick(HanTrick::Raft) => floating,
        Lesson::Trick(_) => boosted,
        Lesson::Tip(_) => tooted,
    }
}

#[allow(clippy::too_many_arguments)]
fn queue_lessons(
    mut lessons: ResMut<Lessons>,
    mut progress: ResMut<Progress>,
    meters: Option<Res<BandMeters>>,
    hints: Query<&HintSpot, Changed<HintSpot>>,
    mut repeated: MessageReader<HintRepeated>,
    off: Option<Res<LessonsOff>>,
) {
    let lessons = &mut *lessons;
    for r in repeated.read() {
        if off.is_some() {
            continue;
        }
        // Stuck here: the line comes back.
        for l in cards_of(&r.topics) {
            let l = match l {
                Lesson::Trick(k) if HEAD_TRICKS.contains(&k) => Lesson::Trick(HanTrick::Throw),
                l => l,
            };
            if hint_text(l, false).is_some()
                && !lessons.hints.contains(&l)
                && lessons.hint.map(|h| h.lesson) != Some(l)
            {
                lessons.hints.push_back(l);
            }
        }
    }
    let heard: Vec<Topic> = hints
        .iter()
        .filter(|h| h.said)
        .flat_map(|h| h.topics.iter().copied())
        .collect();
    let m = meters.map(|m| m.0).unwrap_or_default();
    // (Only touch the save when something's due: it's written on every change.)
    let mut p = progress.bypass_change_detection().clone();
    let new = due(&mut p, &heard, &m);
    if p != *progress {
        *progress = p;
    }
    if off.is_none() {
        lessons.hints.extend(new);
    }
}

#[allow(clippy::too_many_arguments)]
fn run_hints(
    time: Res<Time>,
    mut lessons: ResMut<Lessons>,
    meters: Option<Res<BandMeters>>,
    mut boosted: MessageReader<HanBoosted>,
    mut jumped: MessageReader<Jumped>,
    nat: Query<&crate::game::Body, With<Player>>,
    floats: Query<(), With<HanFloat>>,
) {
    let m = meters.map(|m| m.0).unwrap_or_default();
    let boosted = boosted.read().count() > 0;
    let tooted = jumped.read().any(|j| j.double);
    let floating = nat
        .iter()
        .any(|b| b.riding.is_some_and(|e| floats.contains(e)));
    let dt = time.delta_secs();
    let lessons = &mut *lessons;
    if let Some(h) = &mut lessons.hint {
        h.age += dt;
        h.done |= hint_done(h.lesson, &m, boosted, tooted, floating);
        if (h.done && h.age >= HINT_MIN_SECS) || h.age >= HINT_SECS {
            lessons.hint = None;
        }
    }
    while lessons.hint.is_none() {
        let Some(l) = lessons.hints.pop_front() else {
            break;
        };
        // (Already done by the time it comes up: skip it.)
        if !hint_done(l, &m, boosted, tooted, floating) || matches!(l, Lesson::Trick(_)) {
            lessons.hint = Some(Hint {
                lesson: l,
                age: 0.0,
                done: false,
            });
        }
    }
}

fn show_lessons(
    time: Res<Time<Real>>,
    mut lessons: ResMut<Lessons>,
    play: Res<State<PlayState>>,
    mut next: ResMut<NextState<PlayState>>,
) {
    if *play.get() != PlayState::Running || lessons.showing.is_some() {
        return;
    }
    if let Some(l) = lessons.queue.pop_front() {
        lessons.showing = Some(l);
        lessons.shown_at = time.elapsed_secs();
        next.set(PlayState::Lesson);
    }
}

fn dismiss(
    time: Res<Time<Real>>,
    action: Single<&ActionState<Action>>,
    mut lessons: ResMut<Lessons>,
    mut next: ResMut<NextState<PlayState>>,
    mut sfx_w: MessageWriter<PlaySfx>,
) {
    let now = time.elapsed_secs();
    if now - lessons.shown_at < ARM_SECS || !action.just_pressed(&Action::Jump) {
        return;
    }
    sfx(&mut sfx_w, Sfx::MenuSelect);
    match lessons.queue.pop_front() {
        Some(l) => {
            lessons.showing = Some(l);
            lessons.shown_at = now;
        }
        None => {
            lessons.showing = None;
            next.set(PlayState::Running);
        }
    }
}

/// A card's title and lines.
pub fn card_text(l: Lesson, touch: bool) -> (String, Vec<String>) {
    let (title, lines): (&str, Vec<String>) = match l {
        Lesson::Counter(BandCounter::Horn) => (
            "NEW: THE HORN",
            vec![
                "EVERY TOOT FILLS A PIP.".into(),
                "FILL ALL 5 TO CHARGE".into(),
                "GIANT STEPS.".into(),
            ],
        ),
        Lesson::Counter(BandCounter::Nugget) => (
            "NEW: THE NUGGET",
            vec![
                "GRAB 4 NUGGETS QUICK".into(),
                "TO CHARGE THE".into(),
                "FIRED-UP BAND.".into(),
            ],
        ),
        Lesson::Counter(BandCounter::Note) => (
            "NEW: THE NOTE",
            vec![
                "HOP, HOP, HOP ON EVEN".into(),
                "BEATS TO CHARGE".into(),
                "THE WALTZ.".into(),
            ],
        ),
        Lesson::Counter(BandCounter::Sweat) => (
            "NEW: THE SWEAT DROP",
            vec![
                "EVERY SPLAT ADDS A DROP.".into(),
                "3 AND THE BAND GETS NERVOUS:".into(),
                "GRIPPY FEET.".into(),
            ],
        ),
        Lesson::Call(c) => {
            let (mode, does) = match c {
                BandCounter::Nugget => ("THE FIRED-UP BAND", "AND YOUR FEET GO FAST."),
                BandCounter::Note => ("A WALTZ", "AND YOU JUMP HIGH ON ONE."),
                _ => ("GIANT STEPS", "AND GRAVITY GOES LOW."),
            };
            let call = if touch {
                format!("TAP THE {} TO CALL THE BAND.", c.name().to_uppercase())
            } else {
                "PRESS DOWN TO CALL THE BAND.".into()
            };
            (
                "CHARGED!",
                vec![call, format!("THEY PLAY {mode}"), does.into()],
            )
        }
        Lesson::Trick(HanTrick::Throw) => (
            "HAN'S TRICK: THE THROW",
            vec![
                "BAND FIRED UP? HAN RUNS FAST.".into(),
                "HOP ON HIS HEAD AND JUMP:".into(),
                "HE THROWS YOU FAR!".into(),
            ],
        ),
        Lesson::Trick(HanTrick::Mega) => (
            "HAN'S TRICK: THE MEGA BOOST",
            vec![
                "GIANT STEPS PLAYING? HOP ON".into(),
                "HAN'S HEAD AND JUMP: WAY UP!".into(),
                "TOOT AT THE TOP.".into(),
            ],
        ),
        Lesson::Trick(HanTrick::Cling) => (
            "HAN'S TRICK: THE WALL CLING",
            vec![
                "BAND NERVOUS? IN A SHAFT".into(),
                "HAN CLIMBS THE WALL. HOP ON".into(),
                "HIS HEAD AND JUMP: UP!".into(),
            ],
        ),
        Lesson::Trick(HanTrick::Trapeze) => (
            "HAN'S TRICK: THE TRAPEZE",
            vec![
                "WALTZING? HAN SWINGS FROM".into(),
                "THE CEILING. RIDE HIS CAP,".into(),
                "JUMP ON ONE: FLY FAR!".into(),
            ],
        ),
        Lesson::Trick(HanTrick::Raft) => (
            "HAN'S TRICK: THE POOL FLOAT",
            vec![
                "RUN UP TO A POOL: HAN DIVES".into(),
                "IN. HOP ON, LEFT/RIGHT ROW.".into(),
                "TEENY HOPS UNDER SPIKES!".into(),
            ],
        ),
        Lesson::Tip(t) => tip_text(t, touch),
    };
    (title.into(), lines)
}

/// A [`Lesson::Tip`]'s title and lines.
fn tip_text(t: Topic, touch: bool) -> (&'static str, Vec<String>) {
    let jump = if touch { "TAP JUMP" } else { "JUMP" };
    let lines: Vec<String> = match t {
        Topic::Toot => {
            return (
                "NEW: THE TOOT",
                vec![
                    format!("{jump}, THEN {jump} AGAIN"),
                    "IN MIDAIR: A TOOT!".into(),
                ],
            );
        }
        Topic::OneWay => vec![
            "THIN SHELVES: JUMP UP".into(),
            "THROUGH THEM, LAND ON TOP.".into(),
        ],
        Topic::Platform => vec![
            "HOP ON A FLOATING ROLL:".into(),
            "IT CARRIES YOU. JUMP OFF".into(),
            "AND KEEP ITS SPEED.".into(),
        ],
        Topic::Fly => vec![
            "FLIES HOVER UP THERE.".into(),
            "LITTLE HOPS UNDER THEM!".into(),
        ],
        Topic::Spray => vec![
            "A CAN PUFFS ON A BEAT.".into(),
            "WAIT TILL THE PSSHT STOPS,".into(),
            "THEN GO!".into(),
        ],
        Topic::Stain => vec![
            "TOO WIDE TO JUMP? SPLAT".into(),
            "ON THE SPIKES: YOUR STAIN".into(),
            "IS A STEP!".into(),
        ],
        Topic::Grease => vec![
            "ON GREASE YOU CAN'T STOP".into(),
            "OR JUMP. SPLAT A FEW AND".into(),
            "THE BAND GETS NERVOUS.".into(),
        ],
        Topic::Brick => vec!["JUMP UP INTO A BAR OF".into(), "SOAP: IT SMASHES!".into()],
        Topic::Crumble => vec![
            "SOGGY CARDBOARD SAGS".into(),
            "UNDER YOU. KEEP MOVING!".into(),
        ],
        Topic::Spring => vec!["LAND ON A CUSHION:".into(), "PFFFT! UP YOU GO!".into()],
        Topic::Crusher => vec![
            "A CRUSHER SLAMS, THEN".into(),
            "CREEPS BACK. GO THEN!".into(),
        ],
        Topic::Belt => vec![
            "A CONVEYOR CARRIES YOU.".into(),
            "JUMP OFF IT FOR A BOOST!".into(),
        ],
        Topic::Fan => vec![
            "A FAN BLOWS YOU UP.".into(),
            "FLOAT, THEN STEER OFF!".into(),
        ],
        Topic::Bumper => vec![
            "BUMP A RUBBER DUCK".into(),
            "AND BOING! IT BATS BACK!".into(),
        ],
        Topic::Beat => vec!["RED AND BLUE BLOCKS".into(), "SWAP ON EVERY BAR!".into()],
        Topic::Wind => vec![
            "FANS ON WALLS PUSH YOU.".into(),
            "ON CEILINGS? NO RISING!".into(),
        ],
        Topic::Fling => vec![
            "CUSHIONS ON WALLS FLING".into(),
            "YOU. ON CEILINGS: BONK!".into(),
        ],
        // (The others have a counter or a trick card; a tip for them is never due.)
        _ => vec![],
    };
    let title = match t {
        Topic::OneWay => "NEW: SHELVES",
        Topic::Platform => "NEW: RIDES",
        Topic::Fly => "NEW: FLIES",
        Topic::Spray => "NEW: SPRAY CANS",
        Topic::Stain => "NEW: SPLAT STAINS",
        Topic::Grease => "NEW: GREASE",
        Topic::Brick => "NEW: SOAP BRICKS",
        Topic::Crumble => "NEW: SOGGY BOXES",
        Topic::Spring => "NEW: WHOOPEE CUSHIONS",
        Topic::Crusher => "NEW: CRUSHERS",
        Topic::Belt => "NEW: CONVEYORS",
        Topic::Fan => "NEW: FANS",
        Topic::Bumper => "NEW: BUMPERS",
        Topic::Beat => "NEW: BEAT BLOCKS",
        Topic::Wind => "NEW: SIDEWAYS FANS",
        Topic::Fling => "NEW: SIDEWAYS CUSHIONS",
        _ => "NEW!",
    };
    (title, lines)
}

#[derive(Component)]
struct LessonCard;

fn draw_card(
    mut commands: Commands,
    font: Res<UiFont>,
    lessons: Res<Lessons>,
    touch: Option<Res<TouchMode>>,
    old: Query<Entity, With<LessonCard>>,
) {
    if !lessons.is_changed() {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    let Some(l) = lessons.showing else { return };
    let touch = touch.is_some_and(|t| t.0);
    let (title, lines) = card_text(l, touch);
    let f = &*font;
    commands
        .spawn((
            Name::new("LessonCard"),
            LessonCard,
            DespawnOnExit(AppState::Playing),
            Node {
                justify_content: JustifyContent::Start,
                padding: UiRect::top(px(48.0)),
                ..fullscreen()
            },
            BackgroundColor(OVERLAY),
            // Over the game and the HUD, under the strip (it stays lit).
            GlobalZIndex(30),
        ))
        .with_children(|root| {
            root.spawn(panel(Node {
                padding: UiRect::axes(px(12.0), px(8.0)),
                ..column(8.0, 4.0)
            }))
            .with_children(|p| {
                p.spawn(label(f, title, 8.0, GOLD));
                for line in lines {
                    p.spawn(label(f, line, 8.0, CREAM));
                }
                p.spawn((
                    label(
                        f,
                        if touch {
                            "- TAP JUMP -"
                        } else {
                            "- PRESS JUMP -"
                        },
                        8.0,
                        GOLD,
                    ),
                    Node {
                        margin: UiRect::top(px(4.0)),
                        ..default()
                    },
                ));
            });
        });
}

/// Where the hint line sits (virtual px from the top): under the strip and its CALL arrow.
pub const HINT_TOP: f32 = super::strip::STRIP_TOP + 34.0;

#[derive(Component)]
struct HintLine;

/// The hint line under the strip (not the toot's: that's the key bubble).
fn draw_hint(
    mut commands: Commands,
    font: Res<UiFont>,
    lessons: Res<Lessons>,
    touch: Option<Res<TouchMode>>,
    old: Query<(Entity, &HintLine)>,
    mut shown: Local<Option<Lesson>>,
) {
    let want = lessons
        .hint
        .map(|h| h.lesson)
        .filter(|l| !matches!(l, Lesson::Tip(_)));
    if *shown == want && (want.is_none() || !old.is_empty()) {
        return;
    }
    *shown = want;
    for (e, _) in &old {
        commands.entity(e).despawn();
    }
    let Some(l) = want else { return };
    let Some(text) = hint_text(l, touch.is_some_and(|t| t.0)) else {
        return;
    };
    commands
        .spawn((
            Name::new("HintLine"),
            HintLine,
            DespawnOnExit(AppState::Playing),
            GlobalZIndex(31),
            Node {
                position_type: PositionType::Absolute,
                top: px(HINT_TOP),
                width: percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    padding: UiRect::axes(px(4.0), px(2.0)),
                    ..default()
                },
                BackgroundColor(OVERLAY),
            ))
            .with_children(|p| {
                p.spawn(label(&font, text, 8.0, CREAM));
            });
        });
}

#[derive(Component)]
struct KeyBubble;

/// The toot's key bubble: "JUMP JUMP" floating over Nat until he toots.
#[allow(clippy::type_complexity)]
/// How far above Nat's feet the key bubble sits.
const KEY_BUBBLE_RISE: f32 = 44.0;

fn draw_key_bubble(
    mut commands: Commands,
    font: Option<Res<UiFont>>,
    lessons: Res<Lessons>,
    touch: Option<Res<TouchMode>>,
    nat: Query<&Pos, With<Player>>,
    mut bubble: Query<(Entity, &mut Transform), With<KeyBubble>>,
    window: Option<Single<&Window, With<bevy::window::PrimaryWindow>>>,
) {
    let want = lessons
        .hint
        .is_some_and(|h| matches!(h.lesson, Lesson::Tip(_)));
    // High enough to clear Han's speech bubble when he's beside Nat (his "Toot!" at the spot).
    let Ok(at) = nat.single().map(|p| p.0 + Vec2::new(0.0, KEY_BUBBLE_RISE)) else {
        return;
    };
    match (want, bubble.single_mut()) {
        (false, Ok((e, _))) => commands.entity(e).despawn(),
        (true, Ok((_, mut tf))) => tf.translation = at.extend(10.0),
        (true, Err(_)) => {
            let Some(text) = lessons
                .hint
                .and_then(|h| hint_text(h.lesson, touch.is_some_and(|t| t.0)))
            else {
                return;
            };
            // Rasterized at about screen resolution, scaled down to world size: crisp pixels.
            let k = window.map_or(1.0, |w| {
                (w.resolution.height() / super::VIRTUAL_HEIGHT)
                    .ceil()
                    .max(1.0)
            });
            let size = Vec2::new(text.chars().count() as f32 * 8.0 + 6.0, 12.0);
            commands
                .spawn((
                    Name::new("KeyBubble"),
                    KeyBubble,
                    DespawnOnExit(AppState::Playing),
                    Transform::from_translation(at.extend(10.0)),
                    Visibility::default(),
                ))
                .with_children(|b| {
                    b.spawn((Sprite::from_color(INK, size + 2.0), Transform::default()));
                    b.spawn((
                        Sprite::from_color(CREAM, size),
                        Transform::from_xyz(0.0, 0.0, 0.1),
                    ));
                    b.spawn((
                        Text2d::new(text),
                        TextFont {
                            font: font.map(|f| f.0.clone()).unwrap_or_default().into(),
                            font_size: bevy::text::FontSize::Px(8.0 * k),
                            font_smoothing: bevy::text::FontSmoothing::None,
                            ..default()
                        },
                        TextColor(INK),
                        Transform::from_xyz(0.0, 0.0, 0.2).with_scale(Vec3::splat(1.0 / k)),
                    ));
                });
        }
        (false, Err(_)) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::director::{Meter, Meters};

    #[test]
    fn lessons_come_once_per_save() {
        let mut p = Progress::default();
        let none = Meters::default();
        assert_eq!(
            due(&mut p, &[Topic::Toot, Topic::Giant], &none),
            [Lesson::Counter(BandCounter::Horn), Lesson::Tip(Topic::Toot)]
        );
        assert_eq!(due(&mut p, &[Topic::Giant, Topic::Toot], &none), [], "once");
        // A counter that fills before its hint stays untaught (level 1's nuggets don't teach
        // the nugget counter); the first full counter that shows gets the call.
        let mut full = Meters::default();
        full.summons[1] = Meter {
            lit: 4,
            of: 4,
            full: true,
            active: None,
        };
        full.ready = Some(Harmony::Quartal);
        assert_eq!(due(&mut p, &[], &full), []);
        assert!(!p.taught[BandCounter::Nugget.index()]);
        full.summons[0] = Meter {
            lit: 5,
            of: 5,
            full: true,
            active: None,
        };
        full.ready = Some(Harmony::Coltrane);
        assert_eq!(due(&mut p, &[], &full), [Lesson::Call(BandCounter::Horn)]);
        assert_eq!(due(&mut p, &[], &full), []);
        // The nervous band shows the sweat drop, with no line.
        let mut nervous = Meters::default();
        nervous.deaths = Meter {
            lit: 3,
            of: 3,
            full: true,
            active: None,
        };
        assert_eq!(due(&mut p, &[], &nervous), []);
        assert_eq!(p.taught, [true, false, false, true]);
        assert!(p.taught_call);
        // Han's head: one line for all four tricks.
        assert_eq!(
            due(&mut p, &[Topic::Chain], &none),
            [Lesson::Trick(HanTrick::Throw)]
        );
        assert_eq!(
            due(&mut p, &[Topic::Mega, Topic::Shaft, Topic::Trapeze], &none),
            []
        );
        assert_eq!(
            due(&mut p, &[Topic::BuddyRaft], &none),
            [Lesson::Trick(HanTrick::Raft)]
        );
        // The levels show the gadgets: taught, no line.
        assert_eq!(
            due(&mut p, &[Topic::OneWay, Topic::Spray, Topic::Stain], &none),
            []
        );
        assert!(p.taught_tips[Topic::OneWay.index()]);
        let mut l = Lessons::default();
        l.replay(&p);
        assert!(l.queue.contains(&Lesson::Counter(BandCounter::Horn)));
        assert!(l.queue.contains(&Lesson::Tip(Topic::OneWay)));
    }

    #[test]
    fn hint_lines_fit_under_the_strip() {
        for l in BandCounter::ALL
            .map(Lesson::Counter)
            .into_iter()
            .chain(BandCounter::ALL[..3].iter().map(|&c| Lesson::Call(c)))
            .chain(HanTrick::ALL.map(Lesson::Trick))
            .chain([Lesson::Tip(Topic::Toot)])
        {
            for touch in [false, true] {
                if let Some(t) = hint_text(l, touch) {
                    assert!(t.len() <= 32, "{l:?}: {t:?} is {} long", t.len());
                }
            }
        }
    }

    /// Every mechanic a hint can name has a card (the pause menu's manual) and is taught by it.
    #[test]
    fn every_topic_has_a_card() {
        for t in Topic::ALL {
            let cards = cards_of(&[t]);
            assert_eq!(cards.len(), 1, "{t:?}");
            assert!(!card_text(cards[0], false).1.is_empty(), "{t:?}");
            let mut p = Progress::default();
            let _ = due(&mut p, &[t], &Meters::default());
            assert_ne!(p, Progress::default(), "{t:?} teaches nothing");
        }
        assert_eq!(
            cards_of(&[Topic::Grease, Topic::Grip]),
            [
                Lesson::Tip(Topic::Grease),
                Lesson::Counter(BandCounter::Sweat)
            ]
        );
        assert_eq!(
            cards_of(&[Topic::BuddyRaft]),
            [Lesson::Trick(HanTrick::Raft)]
        );
    }

    #[test]
    fn cards_fit_the_screen() {
        for l in BandCounter::ALL
            .map(Lesson::Counter)
            .into_iter()
            .chain(BandCounter::ALL[..3].iter().map(|&c| Lesson::Call(c)))
            .chain(HanTrick::ALL.map(Lesson::Trick))
            .chain(Topic::ALL.into_iter().filter_map(tip_of).map(Lesson::Tip))
        {
            for touch in [false, true] {
                let (title, lines) = card_text(l, touch);
                assert!(!lines.is_empty(), "{l:?} says nothing");
                for s in std::iter::once(&title).chain(&lines) {
                    // 8 px a character; a 4:3 screen is 288 virtual px wide, less the card's rim.
                    assert!(s.len() <= 32, "{l:?}: {s:?} is {} long", s.len());
                }
            }
        }
    }
}

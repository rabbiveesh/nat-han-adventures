//! Lesson cards: every mechanic is taught by a card, since Han only barks (1-3 words). The
//! band's counters teach themselves, Han's tricks ([`HanTrick`]) and the other mechanics (a
//! [`Lesson::Tip`]: a toot, shelves, rides, flies, cans, stains, grease) come up at their first
//! hint spot, and again when Han repeats that hint after repeated deaths there. The first time a counter matters (Nat
//! reaches the hint spot of its gate, or the counter fills or, for the sweat drop, the band gets
//! nervous) the game freezes ([`PlayState::Lesson`]), dims, and a card explains it, the strip
//! still lit above. The first full counter also gets "press Down to call the band". Jump (a
//! tap on touch) dismisses a card, after [`ARM_SECS`] so a jump in flight doesn't skip it.
//! Each card shows once per save ([`Progress::taught`], [`Progress::taught_call`],
//! [`Progress::taught_tricks`]); the pause
//! menu's BAND LESSONS shows the taught ones again. `NATHAN_LESSONS=0` turns them off
//! ([`LessonsOff`]).

use std::collections::VecDeque;

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::{palette::*, *};
use crate::audio::{BandMeters, Harmony, director::SUMMON_ORDER};
use crate::events::HintRepeated;
use crate::game::HintSpot;
use crate::input::Action;
use crate::level::Topic;
use crate::save::{BandCounter, HanTrick, Progress};
use crate::state::{AppState, PlayState};
use crate::touch::TouchMode;

/// A card ignores Jump this long after it opens (real seconds).
pub const ARM_SECS: f32 = 0.5;

pub fn plugin(app: &mut App) {
    if std::env::var("NATHAN_LESSONS").as_deref() == Ok("0") {
        app.insert_resource(LessonsOff);
    }
    app.init_resource::<Lessons>()
        .add_systems(OnExit(AppState::Playing), |mut l: ResMut<Lessons>| *l = Lessons::default())
        .add_systems(
            Update,
            (queue_lessons.run_if(in_state(PlayState::Running)), show_lessons, dismiss.run_if(in_state(PlayState::Lesson)), draw_card)
                .chain()
                .run_if(in_state(AppState::Playing)),
        );
}

/// No lesson cards (`NATHAN_LESSONS=0`; video capture, unless `NATHAN_LESSONS=1`): scripted
/// runs aren't frozen by a card they don't expect. The counters are still taught (they show).
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

/// The cards waiting, and the one up.
#[derive(Resource, Debug, Default)]
pub struct Lessons {
    pub queue: VecDeque<Lesson>,
    pub showing: Option<Lesson>,
    /// Real time the card went up.
    shown_at: f32,
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
            let first = BandCounter::ALL[..3].iter().copied().find(|c| progress.taught[c.index()]).unwrap_or(BandCounter::Horn);
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
    SUMMON_ORDER.iter().position(|&s| s == h).map(|i| BandCounter::ALL[i])
}

/// The lessons now due: counters and Han's tricks taught by hint spots just heard, full
/// counters, a nervous band, and the first call. Marks them taught (once per save).
pub fn due(progress: &mut Progress, heard: &[Topic], meters: &crate::audio::director::Meters) -> Vec<Lesson> {
    let mut out = Vec::new();
    let mut teach = |c: BandCounter, out: &mut Vec<Lesson>| {
        if !progress.taught[c.index()] {
            progress.taught[c.index()] = true;
            out.push(Lesson::Counter(c));
        }
    };
    for &t in heard {
        if let Some(c) = counter_of(t) {
            teach(c, &mut out);
        }
    }
    for &t in heard {
        if let Some(t) = trick_of(t)
            && !progress.taught_tricks[t.index()]
        {
            progress.taught_tricks[t.index()] = true;
            out.push(Lesson::Trick(t));
        }
    }
    for &t in heard {
        if let Some(t) = tip_of(t)
            && !progress.taught_tips[t.index()]
        {
            progress.taught_tips[t.index()] = true;
            out.push(Lesson::Tip(t));
        }
    }
    for (i, m) in meters.summons.iter().enumerate() {
        if m.full {
            teach(BandCounter::ALL[i], &mut out);
        }
    }
    if meters.deaths.full {
        teach(BandCounter::Sweat, &mut out);
    }
    if !progress.taught_call
        && let Some(c) = meters.ready.and_then(counter_of_harmony)
    {
        progress.taught_call = true;
        out.push(Lesson::Call(c));
    }
    out
}

fn queue_lessons(
    mut lessons: ResMut<Lessons>,
    mut progress: ResMut<Progress>,
    meters: Option<Res<BandMeters>>,
    hints: Query<&HintSpot, Changed<HintSpot>>,
    mut repeated: MessageReader<HintRepeated>,
    off: Option<Res<LessonsOff>>,
) {
    for r in repeated.read() {
        if off.is_none() {
            for l in cards_of(&r.topics) {
                if !lessons.queue.contains(&l) && lessons.showing != Some(l) {
                    lessons.queue.push_back(l);
                }
            }
        }
    }
    let heard: Vec<Topic> = hints.iter().filter(|h| h.said).flat_map(|h| h.topics.iter().copied()).collect();
    let m = meters.map(|m| m.0).unwrap_or_default();
    // (Only touch the save when something's due: it's written on every change.)
    let mut p = progress.bypass_change_detection().clone();
    let new = due(&mut p, &heard, &m);
    if !new.is_empty() {
        *progress = p;
        if off.is_none() {
            lessons.queue.extend(new);
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
        Lesson::Counter(BandCounter::Horn) => {
            ("NEW: THE HORN", vec!["EVERY TOOT FILLS A PIP.".into(), "FILL ALL 5 TO CHARGE".into(), "GIANT STEPS.".into()])
        }
        Lesson::Counter(BandCounter::Nugget) => (
            "NEW: THE NUGGET",
            vec!["GRAB 4 NUGGETS QUICK".into(), "TO CHARGE THE".into(), "FIRED-UP BAND.".into()],
        ),
        Lesson::Counter(BandCounter::Note) => {
            ("NEW: THE NOTE", vec!["HOP, HOP, HOP ON EVEN".into(), "BEATS TO CHARGE".into(), "THE WALTZ.".into()])
        }
        Lesson::Counter(BandCounter::Sweat) => (
            "NEW: THE SWEAT DROP",
            vec!["EVERY SPLAT ADDS A DROP.".into(), "3 AND THE BAND GETS NERVOUS:".into(), "GRIPPY FEET.".into()],
        ),
        Lesson::Call(c) => {
            let (mode, does) = match c {
                BandCounter::Nugget => ("THE FIRED-UP BAND", "AND YOUR FEET GO FAST."),
                BandCounter::Note => ("A WALTZ", "AND YOU JUMP HIGH ON ONE."),
                _ => ("GIANT STEPS", "AND GRAVITY GOES LOW."),
            };
            let call = if touch { format!("TAP THE {} TO CALL THE BAND.", c.name().to_uppercase()) } else { "PRESS DOWN TO CALL THE BAND.".into() };
            ("CHARGED!", vec![call, format!("THEY PLAY {mode}"), does.into()])
        }
        Lesson::Trick(HanTrick::Throw) => (
            "HAN'S TRICK: THE THROW",
            vec!["BAND FIRED UP? HAN RUNS FAST.".into(), "HOP ON HIS HEAD AND JUMP:".into(), "HE THROWS YOU FAR!".into()],
        ),
        Lesson::Trick(HanTrick::Mega) => (
            "HAN'S TRICK: THE MEGA BOOST",
            vec!["GIANT STEPS PLAYING? HOP ON".into(), "HAN'S HEAD AND JUMP: WAY UP!".into(), "TOOT AT THE TOP.".into()],
        ),
        Lesson::Trick(HanTrick::Cling) => (
            "HAN'S TRICK: THE WALL CLING",
            vec!["BAND NERVOUS? IN A SHAFT".into(), "HAN CLIMBS THE WALL. HOP ON".into(), "HIS HEAD AND JUMP: UP!".into()],
        ),
        Lesson::Trick(HanTrick::Trapeze) => (
            "HAN'S TRICK: THE TRAPEZE",
            vec!["WALTZING? HAN SWINGS FROM".into(), "THE CEILING. RIDE HIS CAP,".into(), "JUMP ON ONE: FLY FAR!".into()],
        ),
        Lesson::Trick(HanTrick::Raft) => (
            "HAN'S TRICK: THE RAFT",
            vec!["SPIKES OVER SEWAGE? STAND".into(), "STILL BY IT: HAN WADES IN".into(), "AND LEAVES A BIG RAFT.".into()],
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
            return ("NEW: THE TOOT", vec![format!("{jump}, THEN {jump} AGAIN"), "IN MIDAIR: A TOOT!".into()]);
        }
        Topic::OneWay => vec!["THIN SHELVES: JUMP UP".into(), "THROUGH THEM, LAND ON TOP.".into()],
        Topic::Platform => vec!["HOP ON A FLOATING ROLL:".into(), "IT CARRIES YOU. JUMP OFF".into(), "AND KEEP ITS SPEED.".into()],
        Topic::Fly => vec!["FLIES HOVER UP THERE.".into(), "LITTLE HOPS UNDER THEM!".into()],
        Topic::Spray => vec!["A CAN PUFFS ON A BEAT.".into(), "WAIT TILL THE PSSHT STOPS,".into(), "THEN GO!".into()],
        Topic::Stain => vec!["TOO WIDE TO JUMP? SPLAT".into(), "ON THE SPIKES: YOUR STAIN".into(), "IS A STEP!".into()],
        Topic::Grease => vec!["ON GREASE YOU CAN'T STOP".into(), "OR JUMP. SPLAT A FEW AND".into(), "THE BAND GETS NERVOUS.".into()],
        Topic::Brick => vec!["JUMP UP INTO A BAR OF".into(), "SOAP: IT SMASHES!".into()],
        Topic::Crumble => vec!["SOGGY CARDBOARD SAGS".into(), "UNDER YOU. KEEP MOVING!".into()],
        Topic::Spring => vec!["LAND ON A CUSHION:".into(), "PFFFT! UP YOU GO!".into()],
        Topic::Crusher => vec!["A CRUSHER SLAMS, THEN".into(), "CREEPS BACK. GO THEN!".into()],
        Topic::Belt => vec!["A CONVEYOR CARRIES YOU.".into(), "JUMP OFF IT FOR A BOOST!".into()],
        Topic::Fan => vec!["A FAN BLOWS YOU UP.".into(), "FLOAT, THEN STEER OFF!".into()],
        Topic::Bumper => vec!["BUMP A RUBBER DUCK".into(), "AND BOING! IT BATS BACK!".into()],
        Topic::Beat => vec!["RED AND BLUE BLOCKS".into(), "SWAP ON EVERY BAR!".into()],
        Topic::Wind => vec!["FANS ON WALLS PUSH YOU.".into(), "ON CEILINGS? NO RISING!".into()],
        Topic::Fling => vec!["CUSHIONS ON WALLS FLING".into(), "YOU. ON CEILINGS: BONK!".into()],
        Topic::Boom => vec!["HOP ON! ARROWS STEER,".into(), "JUMP HOPS OFF.".into(), "BEANS FILL ITS TANK!".into()],
        Topic::Diaper => vec!["DIAPERS AIM AHEAD.".into(), "HEAR THE WHISTLE?".into(), "STOP OR TURN BACK!".into()],
        Topic::Helper => vec!["HE SAYS HE'LL HELP.".into(), "HE SNICKERS? HE'LL".into(), "LET GO! JUMP OFF FIRST.".into()],
        Topic::Blast => vec!["JUMP ON THE BUTTON".into(), "AND THE BAND BLASTS!".into(), "IT BLASTS YOU UP TOO.".into()],
        Topic::Stop => vec!["A STOP SIGN!".into(), "TOUCH IT AND THE LEVEL".into(), "STOPS RIGHT THERE.".into()],
        Topic::House => vec![format!("{} AT A DOOR: GO IN.", if touch { "STICK UP" } else { "UP" }), "NOBODY HOME? CHIMNEY!".into(), "SMOKE? IT'S A CHEF...".into()],
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
        Topic::Boom => "NEW: THE BOOM",
        Topic::Diaper => "NEW: SPLOOSH DIAPERS",
        Topic::Helper => "NEW: A \"HELPER\"",
        Topic::Blast => "NEW: THE BLAST BUTTON",
        Topic::Stop => "NEW: STOP SIGNS",
        Topic::House => "NEW: HOUSES",
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
            Node { justify_content: JustifyContent::Start, padding: UiRect::top(px(48.0)), ..fullscreen() },
            BackgroundColor(OVERLAY),
            // Over the game and the HUD, under the strip (it stays lit).
            GlobalZIndex(30),
        ))
        .with_children(|root| {
            root.spawn(panel(Node { padding: UiRect::axes(px(12.0), px(8.0)), ..column(8.0, 4.0) })).with_children(|p| {
                p.spawn(label(f, title, 8.0, GOLD));
                for line in lines {
                    p.spawn(label(f, line, 8.0, CREAM));
                }
                p.spawn((
                    label(f, if touch { "- TAP JUMP -" } else { "- PRESS JUMP -" }, 8.0, GOLD),
                    Node { margin: UiRect::top(px(4.0)), ..default() },
                ));
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::director::{Meter, Meters};

    #[test]
    fn lessons_come_once_per_save() {
        let mut p = Progress::default();
        let none = Meters::default();
        assert_eq!(due(&mut p, &[Topic::Toot, Topic::Giant], &none), [Lesson::Counter(BandCounter::Horn), Lesson::Tip(Topic::Toot)]);
        assert_eq!(due(&mut p, &[Topic::Giant, Topic::Toot], &none), [], "once");
        // A counter that fills before its hint teaches itself, and the first full one the call.
        let mut full = Meters::default();
        full.summons[1] = Meter { lit: 4, of: 4, full: true, active: None };
        full.ready = Some(Harmony::Quartal);
        assert_eq!(due(&mut p, &[], &full), [Lesson::Counter(BandCounter::Nugget), Lesson::Call(BandCounter::Nugget)]);
        assert_eq!(due(&mut p, &[], &full), []);
        // The nervous band teaches the sweat drop.
        let mut nervous = Meters::default();
        nervous.deaths = Meter { lit: 3, of: 3, full: true, active: None };
        assert_eq!(due(&mut p, &[], &nervous), [Lesson::Counter(BandCounter::Sweat)]);
        assert_eq!(p.taught, [true, true, false, true]);
        assert!(p.taught_call);
        // Han's chasm hint teaches his throw.
        assert_eq!(due(&mut p, &[Topic::Chain], &none), [Lesson::Trick(HanTrick::Throw)]);
        assert_eq!(due(&mut p, &[Topic::Chain], &none), []);
        let mut l = Lessons::default();
        l.replay(&p);
        assert_eq!(
            Vec::from(l.queue),
            [
                Lesson::Counter(BandCounter::Horn),
                Lesson::Counter(BandCounter::Nugget),
                Lesson::Counter(BandCounter::Sweat),
                Lesson::Call(BandCounter::Horn),
                Lesson::Trick(HanTrick::Throw),
                Lesson::Tip(Topic::Toot)
            ]
        );
    }

    /// Han only barks, so every mechanic a hint can name has a card, and a repeated hint brings
    /// its cards back.
    #[test]
    fn every_topic_has_a_card() {
        for t in Topic::ALL {
            let cards = cards_of(&[t]);
            assert_eq!(cards.len(), 1, "{t:?}");
            assert!(!card_text(cards[0], false).1.is_empty(), "{t:?}");
            let mut p = Progress::default();
            assert!(!due(&mut p, &[t], &Meters::default()).is_empty(), "{t:?} teaches nothing");
        }
        assert_eq!(cards_of(&[Topic::Grease, Topic::Grip]), [Lesson::Tip(Topic::Grease), Lesson::Counter(BandCounter::Sweat)]);
        assert_eq!(cards_of(&[Topic::BuddyRaft]), [Lesson::Trick(HanTrick::Raft)]);
    }

    #[test]
    fn cards_fit_the_screen() {
        for l in BandCounter::ALL.map(Lesson::Counter).into_iter().chain(BandCounter::ALL[..3].iter().map(|&c| Lesson::Call(c)))
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

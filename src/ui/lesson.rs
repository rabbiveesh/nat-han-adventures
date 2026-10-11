//! Lesson cards: the band's counters teach themselves, and Han's tricks are taught the same way
//! (a [`HanTrick`]'s card comes up at its first hint spot). The first time a counter matters (Nat
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
        _ => None,
    }
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
    off: Option<Res<LessonsOff>>,
) {
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
    };
    (title.into(), lines)
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
        assert_eq!(due(&mut p, &[Topic::Toot, Topic::Giant], &none), [Lesson::Counter(BandCounter::Horn)]);
        assert_eq!(due(&mut p, &[Topic::Giant], &none), [], "once");
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
                Lesson::Trick(HanTrick::Throw)
            ]
        );
    }

    #[test]
    fn cards_fit_the_screen() {
        for l in BandCounter::ALL.map(Lesson::Counter).into_iter().chain(BandCounter::ALL[..3].iter().map(|&c| Lesson::Call(c)))
            .chain(HanTrick::ALL.map(Lesson::Trick))
        {
            for touch in [false, true] {
                let (title, lines) = card_text(l, touch);
                for s in std::iter::once(&title).chain(&lines) {
                    // 8 px a character; a 4:3 screen is 288 virtual px wide, less the card's rim.
                    assert!(s.len() <= 32, "{l:?}: {s:?} is {} long", s.len());
                }
            }
        }
    }
}

//! Pause menu (the game enters [`PlayState::Paused`] on Back), with AUDIO DELAY: tap Jump on
//! the beat (or nudge with left/right) to tell the game how late the player's speakers or
//! headphones sound beyond what the platform reports ([`Progress::audio_delay_ms`]).

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::{palette::*, *};
use crate::audio::{Calibrating, LiveClock};
use crate::game::RestartLevel;
use crate::save::{MAX_AUDIO_DELAY_MS, Progress};
use crate::input::Action;
use crate::state::{AppState, PlayState};

const OPTIONS: [&str; 4] = ["RESUME", "RESTART", "AUDIO DELAY", "LEVEL SELECT"];
const AUDIO_DELAY: usize = 2;
/// Free play's last option: end the run (to its results card).
const END_RUN: &str = "END RUN";

fn option(i: usize, free: bool) -> &'static str {
    if free && i == OPTIONS.len() - 1 { END_RUN } else { OPTIONS[i] }
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(PlayState::Paused), spawn)
        .add_systems(OnExit(PlayState::Paused), |mut commands: Commands| {
            commands.remove_resource::<Calib>();
            commands.remove_resource::<Calibrating>();
        })
        .add_systems(
            Update,
            (input.run_if(not(resource_exists::<Calib>)), calibrate.run_if(resource_exists::<Calib>), highlight)
                .chain()
                .run_if(in_state(PlayState::Paused)),
        );
}

#[derive(Resource, Default)]
struct PauseCursor(usize);

#[derive(Component)]
struct PauseOption(usize);

fn spawn(mut commands: Commands, font: Res<UiFont>, free: Option<Res<crate::freeplay::FreePlayRun>>) {
    commands.insert_resource(PauseCursor(0));
    let f = &*font;
    commands
        .spawn((
            Name::new("PauseMenu"),
            DespawnOnExit(PlayState::Paused),
            fullscreen(),
            BackgroundColor(OVERLAY),
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            root.spawn(panel(Node { padding: UiRect::axes(px(24.0), px(16.0)), ..column(16.0, 8.0) }))
                .with_children(|p| {
                    p.spawn((label(f, "PAUSED", 16.0, GOLD), Node { margin: UiRect::bottom(px(8.0)), ..default() }));
                    for i in 0..OPTIONS.len() {
                        p.spawn((label(f, option(i, free.is_some()), 8.0, CREAM), PauseOption(i)));
                    }
                    if let Some(run) = &free {
                        p.spawn((
                            label(f, format!("SEED {}", run.seed_text()), 8.0, GOLD),
                            Node { margin: UiRect::top(px(8.0)), ..default() },
                        ));
                    }
                    p.spawn((
                        label(f, "(HOLD IT IN...)", 8.0, DIM_CREAM),
                        Node { margin: UiRect::top(px(8.0)), ..default() },
                    ));
                });
        });
}

#[allow(clippy::too_many_arguments)]
fn input(
    action: Single<&ActionState<Action>>,
    mut cursor: ResMut<PauseCursor>,
    mut play: ResMut<NextState<PlayState>>,
    mut app_state: ResMut<NextState<AppState>>,
    mut restart: MessageWriter<RestartLevel>,
    mut sfx_w: MessageWriter<PlaySfx>,
    free: Option<Res<crate::freeplay::FreePlayRun>>,
    mut commands: Commands,
    font: Res<UiFont>,
    progress: Res<Progress>,
) {
    let d = nav(&action).y;
    if d != 0 {
        cursor.0 = (cursor.0 as i32 + d).rem_euclid(OPTIONS.len() as i32) as usize;
        sfx(&mut sfx_w, Sfx::MenuMove);
    }
    if action.just_pressed(&Action::Back) {
        sfx(&mut sfx_w, Sfx::MenuSelect);
        play.set(PlayState::Running);
    } else if action.just_pressed(&Action::Confirm) {
        sfx(&mut sfx_w, Sfx::MenuSelect);
        match cursor.0 {
            0 => play.set(PlayState::Running),
            1 => {
                restart.write(RestartLevel);
                play.set(PlayState::Running);
            }
            AUDIO_DELAY => open_calibration(&mut commands, &font, progress.audio_delay_ms),
            _ if free.is_some() => app_state.set(AppState::LevelComplete),
            _ => app_state.set(AppState::LevelSelect),
        }
    }
}

fn highlight(
    cursor: Res<PauseCursor>,
    free: Option<Res<crate::freeplay::FreePlayRun>>,
    mut q: Query<(&PauseOption, &mut Text, &mut TextColor)>,
) {
    for (o, mut text, mut color) in &mut q {
        let on = o.0 == cursor.0;
        let name = option(o.0, free.is_some());
        let s = if on { format!("> {name} <") } else { name.to_string() };
        if text.0 != s {
            text.0 = s;
        }
        color.0 = if on { GOLD } else { CREAM };
    }
}

// ─── AUDIO DELAY ─────────────────────────────────────────────────────────────

/// Taps per measurement.
const TAPS: usize = 8;
/// Left/right nudge.
const NUDGE_MS: u32 = 10;

/// The AUDIO DELAY panel is open.
#[derive(Resource)]
struct Calib {
    root: Entity,
    /// The delay when it opened (BACK puts it back). Changes apply at once, so the next taps
    /// are heard with them.
    before: u32,
    /// Where in the beat each tap landed (0..1, on the clock as heard with the delay set).
    taps: Vec<f64>,
    note: &'static str,
    /// Skips the frame it opened on (its Confirm press is the one that opened it).
    armed: bool,
}

#[derive(Component)]
enum CalibText {
    Taps,
    Delay,
    Note,
}

fn open_calibration(commands: &mut Commands, font: &UiFont, ms: u32) {
    let f = font;
    let root = commands
        .spawn((
            Name::new("AudioDelay"),
            DespawnOnExit(PlayState::Paused),
            fullscreen(),
            BackgroundColor(OVERLAY),
            GlobalZIndex(11),
        ))
        .with_children(|root| {
            root.spawn(panel(Node { padding: UiRect::axes(px(24.0), px(16.0)), ..column(16.0, 8.0) }))
                .with_children(|p| {
                    p.spawn((label(f, "AUDIO DELAY", 16.0, GOLD), Node { margin: UiRect::bottom(px(8.0)), ..default() }));
                    p.spawn(label(f, "TAP JUMP ON THE BEAT", 8.0, CREAM));
                    p.spawn((label(f, "", 8.0, CREAM), CalibText::Taps));
                    p.spawn((label(f, "", 8.0, GOLD), CalibText::Delay));
                    p.spawn((label(f, "", 8.0, GREEN), CalibText::Note));
                    p.spawn((
                        label(f, "< > NUDGE   OK DONE   BACK UNDO", 8.0, DIM_CREAM),
                        Node { margin: UiRect::top(px(8.0)), ..default() },
                    ));
                });
        })
        .id();
    commands.insert_resource(Calib { root, before: ms, taps: Vec::new(), note: "", armed: false });
    commands.insert_resource(Calibrating);
}

#[allow(clippy::too_many_arguments)]
fn calibrate(
    action: Single<&ActionState<Action>>,
    mut calib: ResMut<Calib>,
    mut progress: ResMut<Progress>,
    clock: Option<Res<LiveClock>>,
    time: Res<Time<Real>>,
    mut commands: Commands,
    mut sfx_w: MessageWriter<PlaySfx>,
    mut texts: Query<(&CalibText, &mut Text)>,
) {
    if !calib.armed {
        calib.armed = true;
    } else if action.just_pressed(&Action::Back) {
        sfx(&mut sfx_w, Sfx::MenuSelect);
        progress.audio_delay_ms = calib.before;
        close(&mut commands, calib.root);
        return;
    } else if action.just_pressed(&Action::Jump) {
        // Space is Jump and Confirm: a tap, never a save.
        if let Some(c) = clock.as_deref().filter(|c| c.clock.bpm > 0.0) {
            // The clock was read last frame, and the press came some time this one: about
            // half a frame after the clock's reading.
            let beat_secs = 60.0 / c.clock.bpm as f64;
            let at = c.clock.phase + 0.5 * time.delta_secs_f64() / beat_secs;
            calib.taps.push(at.rem_euclid(1.0));
            calib.note = "";
            if calib.taps.len() >= TAPS {
                match delay_from_taps(progress.audio_delay_ms, &calib.taps, beat_secs) {
                    Some(ms) => {
                        progress.audio_delay_ms = ms;
                        calib.note = "MEASURED!";
                    }
                    None => calib.note = "TOO SCATTERED: AGAIN",
                }
                calib.taps.clear();
            }
        } else {
            calib.note = "NO MUSIC TO TAP TO";
        }
    } else if action.just_pressed(&Action::Confirm) {
        sfx(&mut sfx_w, Sfx::MenuSelect);
        close(&mut commands, calib.root);
        return;
    } else {
        let d = nav(&action).x;
        if d != 0 {
            let ms = progress.audio_delay_ms as i32 + d * NUDGE_MS as i32;
            progress.audio_delay_ms = ms.clamp(0, MAX_AUDIO_DELAY_MS as i32) as u32;
            calib.note = "";
            sfx(&mut sfx_w, Sfx::MenuMove);
        }
    }
    for (t, mut text) in &mut texts {
        let s = match t {
            CalibText::Taps => format!("TAPS {}/{TAPS}", calib.taps.len()),
            CalibText::Delay => format!("DELAY {} MS", progress.audio_delay_ms),
            CalibText::Note => calib.note.to_string(),
        };
        if text.0 != s {
            text.0 = s;
        }
    }
}

fn close(commands: &mut Commands, root: Entity) {
    commands.entity(root).despawn();
    commands.remove_resource::<Calib>();
    commands.remove_resource::<Calibrating>();
}

/// The delay that would put the taps on the beat, from the one they were heard with: taps
/// that land `x` of a beat late on that clock mean the sound reaches the ear `x` beats later
/// than it allows for. Averaged on the circle (a tap just before the beat and one just after
/// average to the beat, not to half a beat). `None` when the taps are too scattered to trust.
fn delay_from_taps(ms: u32, taps: &[f64], beat_secs: f64) -> Option<u32> {
    use std::f64::consts::TAU;
    if taps.is_empty() {
        return None;
    }
    let (sin, cos) = taps.iter().fold((0.0, 0.0), |(s, c), p| (s + (TAU * p).sin(), c + (TAU * p).cos()));
    let n = taps.len() as f64;
    // Resultant length: 1 for taps at one spot, 0 for taps all around the beat.
    if (sin * sin + cos * cos).sqrt() / n < 0.6 {
        return None;
    }
    let late = sin.atan2(cos) / TAU; // -0.5..0.5 of a beat
    let new = ms as f64 + late * beat_secs * 1000.0;
    Some((new.round().max(0.0) as u32).min(MAX_AUDIO_DELAY_MS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taps_late_on_the_clock_add_delay() {
        // 120 bpm: a beat is 500 ms. Taps 0.36 of a beat late (±0.02) → 180 ms more.
        let taps = [0.34, 0.38, 0.36, 0.35, 0.37, 0.36, 0.34, 0.38];
        assert_eq!(delay_from_taps(0, &taps, 0.5), Some(180));
        assert_eq!(delay_from_taps(100, &taps, 0.5), Some(280));
    }

    #[test]
    fn taps_around_the_beat_average_to_it() {
        // Just before and just after the beat: no change, not half a beat.
        let taps = [0.98, 0.02, 0.99, 0.01, 0.97, 0.03, 0.0, 0.0];
        assert_eq!(delay_from_taps(150, &taps, 0.5), Some(150));
        // Early taps take delay off, never below zero.
        assert_eq!(delay_from_taps(40, &[0.8; 8], 0.5), Some(0));
        assert_eq!(delay_from_taps(200, &[0.8; 8], 0.5), Some(100));
    }

    #[test]
    fn scattered_taps_are_refused() {
        let taps = [0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875];
        assert_eq!(delay_from_taps(0, &taps, 0.5), None);
        assert_eq!(delay_from_taps(0, &[], 0.5), None);
    }
}

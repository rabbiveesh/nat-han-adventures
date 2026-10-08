//! Free play's screens: the setup screen (endless or 8 rooms; a new seed, the last one, or one
//! typed in) and the results card. Keyboard and the phone's touch buttons (arrows, OK, II)
//! drive both; digits can also be typed on a keyboard. The HUD's room counter and the pause
//! menu's seed line live with the HUD and the pause menu (`crate::ui`).

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use super::dice::{SEED_DIGITS, seed_text};
use super::run::{FIXED_ROOMS, FreePlayRun, FreePlaySettings, StartFreePlay, random_seed};
use crate::art::SpriteId;
use crate::audio::Sfx;
use crate::events::PlaySfx;
use crate::game::LevelRun;
use crate::input::Action;
use crate::state::AppState;
use crate::ui::{Blink, UiFont, column, format_time, fullscreen, icon, label, nav, palette::*, panel, sfx};

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::FreePlaySetup), (init_setup, spawn_setup).chain())
        .add_systems(Update, (setup_input, refresh_setup).chain().run_if(in_state(AppState::FreePlaySetup)))
        .add_systems(OnEnter(AppState::LevelComplete), spawn_results.run_if(resource_exists::<FreePlayRun>))
        .add_systems(
            Update,
            results_input.run_if(in_state(AppState::LevelComplete).and_then(resource_exists::<FreePlayRun>)),
        );
}

/// Where the run's seed comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedSource {
    /// A fresh one (already rolled, so it's shown before the run).
    New,
    /// The last run's: replay it.
    Last,
    /// Typed in.
    Typed,
}

/// The setup screen's state.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Setup {
    /// 0: run length, 1: seed, 2: GO.
    pub row: usize,
    pub endless: bool,
    pub source: SeedSource,
    pub new_seed: u32,
    pub last_seed: Option<u32>,
    pub typed: [u8; SEED_DIGITS],
    /// Editing the typed seed: the digit under the cursor.
    pub editing: Option<usize>,
}

impl Setup {
    pub fn seed(&self) -> u32 {
        match self.source {
            SeedSource::New => self.new_seed,
            SeedSource::Last => self.last_seed.unwrap_or(self.new_seed),
            SeedSource::Typed => self.typed.iter().fold(0, |a, &d| a * 10 + d as u32),
        }
    }

    fn digits_of(seed: u32) -> [u8; SEED_DIGITS] {
        let s = seed_text(seed);
        let mut out = [0; SEED_DIGITS];
        for (o, b) in out.iter_mut().zip(s.bytes()) {
            *o = b - b'0';
        }
        out
    }

    /// Next seed source (left/right on the seed row); `Last` only once there's been a run.
    fn cycle(&mut self, d: i32) {
        let mut all = vec![SeedSource::New];
        if self.last_seed.is_some() {
            all.push(SeedSource::Last);
        }
        all.push(SeedSource::Typed);
        let i = all.iter().position(|s| *s == self.source).unwrap_or(0) as i32;
        self.source = all[(i + d).rem_euclid(all.len() as i32) as usize];
        if self.source == SeedSource::Typed && self.typed == [0; SEED_DIGITS] {
            self.typed = Self::digits_of(self.seed_for_typing());
        }
    }

    fn seed_for_typing(&self) -> u32 {
        self.last_seed.unwrap_or(self.new_seed)
    }

    /// Apply navigation / confirm / back. Returns the seed and length when the run should start,
    /// `Err(())` to leave the screen.
    pub fn press(&mut self, d: IVec2, confirm: bool, back: bool) -> Result<Option<(u32, bool)>, ()> {
        if let Some(i) = self.editing {
            if confirm || back {
                self.editing = None;
            } else if d.x != 0 {
                self.editing = Some((i as i32 + d.x).clamp(0, SEED_DIGITS as i32 - 1) as usize);
            } else if d.y != 0 {
                // Up = +1.
                self.typed[i] = (self.typed[i] as i32 - d.y).rem_euclid(10) as u8;
            }
            return Ok(None);
        }
        if back {
            return Err(());
        }
        if d.y != 0 {
            self.row = (self.row as i32 + d.y).rem_euclid(3) as usize;
        }
        match self.row {
            0 if d.x != 0 || confirm => self.endless = !self.endless,
            1 if d.x != 0 => self.cycle(d.x),
            1 if confirm => {
                if self.source != SeedSource::Typed {
                    self.typed = Self::digits_of(self.seed());
                    self.source = SeedSource::Typed;
                }
                self.editing = Some(0);
            }
            2 if confirm => return Ok(Some((self.seed(), self.endless))),
            _ => {}
        }
        Ok(None)
    }

    /// A digit typed on a keyboard: goes in at the cursor (or shifts in from the right).
    pub fn type_digit(&mut self, digit: u8) {
        if self.source != SeedSource::Typed {
            self.source = SeedSource::Typed;
            self.typed = [0; SEED_DIGITS];
        }
        match self.editing {
            Some(i) => {
                self.typed[i] = digit;
                self.editing = Some((i + 1).min(SEED_DIGITS - 1));
            }
            None => {
                self.typed.rotate_left(1);
                self.typed[SEED_DIGITS - 1] = digit;
            }
        }
        self.row = 1;
    }

    /// The three rows' text.
    pub fn lines(&self) -> [String; 3] {
        let run = if self.endless { "ENDLESS".to_string() } else { format!("{FIXED_ROOMS} ROOMS") };
        let digits: String = self
            .typed
            .iter()
            .enumerate()
            .map(|(k, d)| if self.editing == Some(k) { format!("[{d}]") } else { d.to_string() })
            .collect();
        let seed = match self.source {
            SeedSource::New => format!("NEW {}", seed_text(self.new_seed)),
            SeedSource::Last => format!("LAST {}", seed_text(self.seed())),
            SeedSource::Typed => format!("TYPED {digits}"),
        };
        let arrows = |on: bool, s: String| if on && self.editing.is_none() { format!("< {s} >") } else { s };
        [
            format!("RUN  {}", arrows(self.row == 0, run)),
            format!("SEED {}", arrows(self.row == 1, seed)),
            if self.row == 2 { "> FLUSH! <".to_string() } else { "FLUSH!".to_string() },
        ]
    }
}

#[derive(Component)]
struct SetupRow(usize);

#[derive(Component)]
struct SetupHelp;

fn init_setup(mut commands: Commands, settings: Res<FreePlaySettings>, progress: Option<Res<crate::save::Progress>>) {
    commands.insert_resource(Setup {
        row: 2,
        endless: settings.endless,
        source: SeedSource::New,
        new_seed: random_seed(),
        last_seed: settings.last_seed.or(progress.and_then(|p| p.last_seed)),
        typed: [0; SEED_DIGITS],
        editing: None,
    });
}

fn spawn_setup(mut commands: Commands, font: Res<UiFont>, sprites: Option<Res<crate::art::Sprites>>) {
    let f = &*font;
    commands
        .spawn((Name::new("FreePlaySetup"), DespawnOnExit(AppState::FreePlaySetup), fullscreen()))
        .with_children(|root| {
            root.spawn((label(f, "FREE PLAY", 16.0, GOLD), Node { margin: UiRect::bottom(px(4.0)), ..default() }));
            root.spawn((
                label(f, "NEW PIPES EVERY TIME", 8.0, DIM_CREAM),
                Node { margin: UiRect::bottom(px(10.0)), ..default() },
            ));
            root.spawn(Node { column_gap: px(24.0), margin: UiRect::bottom(px(10.0)), ..default() }).with_children(|r| {
                r.spawn(icon(sprites.as_deref(), SpriteId::PooIdle, 24.0, 24.0));
                r.spawn(icon(sprites.as_deref(), SpriteId::HanIdle, 24.0, 24.0));
            });
            root.spawn(panel(Node { width: px(272.0), align_items: AlignItems::Start, ..column(10.0, 10.0) }))
                .with_children(|p| {
                    for k in 0..3 {
                        p.spawn((label(f, "", 8.0, CREAM), SetupRow(k)));
                    }
                });
            root.spawn((
                label(f, "", 8.0, DIM_CREAM),
                SetupHelp,
                Node { margin: UiRect::top(px(10.0)), ..default() },
            ));
        });
}

const DIGIT_KEYS: [KeyCode; 10] = [
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

fn setup_input(
    action: Single<&ActionState<Action>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut setup: ResMut<Setup>,
    mut start: MessageWriter<StartFreePlay>,
    mut next: ResMut<NextState<AppState>>,
    mut sfx_w: MessageWriter<PlaySfx>,
) {
    if let Some(keys) = keys {
        for (d, k) in DIGIT_KEYS.iter().enumerate() {
            if keys.just_pressed(*k) {
                setup.type_digit(d as u8);
                sfx(&mut sfx_w, Sfx::MenuMove);
            }
        }
    }
    let d = nav(&action);
    let (confirm, back) = (action.just_pressed(&Action::Confirm), action.just_pressed(&Action::Back));
    if d == IVec2::ZERO && !confirm && !back {
        return;
    }
    match setup.press(d, confirm, back) {
        Err(()) => {
            sfx(&mut sfx_w, Sfx::MenuMove);
            next.set(AppState::Title);
        }
        Ok(Some((seed, endless))) => {
            sfx(&mut sfx_w, Sfx::MenuSelect);
            start.write(StartFreePlay { seed, endless });
        }
        Ok(None) => sfx(&mut sfx_w, if confirm { Sfx::MenuSelect } else { Sfx::MenuMove }),
    }
}

fn refresh_setup(
    setup: Res<Setup>,
    mut rows: Query<(&SetupRow, &mut Text, &mut TextColor)>,
    mut help: Query<&mut Text, (With<SetupHelp>, Without<SetupRow>)>,
) {
    let lines = setup.lines();
    for (r, mut text, mut color) in &mut rows {
        if text.0 != lines[r.0] {
            text.0 = lines[r.0].clone();
        }
        color.0 = if r.0 == setup.row { GOLD } else { CREAM };
    }
    let h = if setup.editing.is_some() {
        "UP/DOWN:DIGIT  LEFT/RIGHT:MOVE  OK:DONE"
    } else if setup.row == 1 {
        "LEFT/RIGHT:SEED  OK:TYPE ONE  ESC:BACK"
    } else {
        "ARROWS:PICK  OK:GO  ESC:BACK"
    };
    for mut t in &mut help {
        if t.0 != h {
            t.0 = h.to_string();
        }
    }
}

/// The results card's lines.
pub fn result_lines(run: &FreePlayRun, level_run: &LevelRun) -> Vec<String> {
    let rooms = match run.total() {
        Some(t) => format!("ROOMS {}/{t}", run.cleared),
        None => format!("ROOMS {}", run.cleared),
    };
    vec![
        rooms,
        format!("NUGGETS {}", level_run.nuggets),
        format!("TIME {}", format_time(level_run.time)),
        format!("SEED {}", run.seed_text()),
    ]
}

fn spawn_results(mut commands: Commands, font: Res<UiFont>, run: Res<FreePlayRun>, level_run: Res<LevelRun>) {
    let f = &*font;
    let header = if run.finished { "FLUSHED IT ALL!" } else { "PIPE DREAMS!" };
    commands
        .spawn((
            Name::new("FreePlayResults"),
            DespawnOnExit(AppState::LevelComplete),
            fullscreen(),
            BackgroundColor(OVERLAY),
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            root.spawn(panel(Node { min_width: px(240.0), padding: UiRect::axes(px(16.0), px(12.0)), ..column(16.0, 8.0) }))
                .with_children(|p| {
                    p.spawn(label(f, header, 16.0, GOLD));
                    p.spawn((label(f, "FREE PLAY", 8.0, CREAM), Node { margin: UiRect::bottom(px(8.0)), ..default() }));
                    let lines = result_lines(&run, &level_run);
                    for (k, l) in lines.into_iter().enumerate() {
                        p.spawn(label(f, l, 8.0, if k == 3 { GOLD } else { CREAM }));
                    }
                    p.spawn((label(f, "SHARE THE SEED! NASTY ONES TOO.", 8.0, DIM_CREAM), Blink(1.2)));
                    p.spawn((
                        label(f, "ENTER:AGAIN  ESC:MENU", 8.0, DIM_CREAM),
                        Node { margin: UiRect::top(px(8.0)), ..default() },
                    ));
                });
        });
}

fn results_input(
    action: Single<&ActionState<Action>>,
    mut next: ResMut<NextState<AppState>>,
    mut sfx_w: MessageWriter<PlaySfx>,
) {
    if action.just_pressed(&Action::Confirm) {
        sfx(&mut sfx_w, Sfx::MenuSelect);
        next.set(AppState::FreePlaySetup);
    } else if action.just_pressed(&Action::Back) {
        sfx(&mut sfx_w, Sfx::MenuMove);
        next.set(AppState::Title);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Setup {
        Setup {
            row: 2,
            endless: false,
            source: SeedSource::New,
            new_seed: 123,
            last_seed: Some(424242),
            typed: [0; SEED_DIGITS],
            editing: None,
        }
    }

    #[test]
    fn seed_sources_and_typing() {
        let mut s = setup();
        assert_eq!(s.press(IVec2::ZERO, true, false), Ok(Some((123, false))));
        s.row = 1;
        s.press(IVec2::X, false, false).unwrap();
        assert_eq!((s.source, s.seed()), (SeedSource::Last, 424242));
        // Type a seed with the arrows: OK edits, up raises the digit.
        s.press(IVec2::ZERO, true, false).unwrap();
        assert_eq!(s.editing, Some(0));
        s.press(IVec2::new(0, -1), false, false).unwrap();
        assert_eq!(s.seed(), 524242);
        s.press(IVec2::X, false, false).unwrap();
        s.press(IVec2::new(0, 1), false, false).unwrap();
        assert_eq!(s.seed(), 514242);
        s.press(IVec2::ZERO, true, false).unwrap();
        assert_eq!(s.editing, None);
        // Keyboard digits shift in from the right.
        let mut s = setup();
        for d in [9, 8, 7] {
            s.type_digit(d);
        }
        assert_eq!(s.seed(), 987);
        assert!(s.lines()[1].contains("000987"));
        assert_eq!(s.press(IVec2::ZERO, false, true), Err(()));
    }

    #[test]
    fn run_length_toggles() {
        let mut s = setup();
        s.row = 0;
        s.press(IVec2::X, false, false).unwrap();
        assert!(s.endless && s.lines()[0].contains("ENDLESS"));
    }
}

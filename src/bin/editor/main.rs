//! The music editor: `cargo run --bin editor`.
//!
//! A native app that runs the game's real-time music engine with no game: the gameplay is
//! simulated by buttons and dials that send the engine the inputs the game sends, so what you
//! hear is what the game would play. Edit a song as a tracker, a piano roll or text (three
//! views of the same `.song` text); every edit that parses is swapped into the running engine.
//! Save writes `music/<song>.song` (checked with the real parser first).
//!
//! egui on the game's own Bevy (`bevy_egui`): one wgpu, one winit, the Bevy already built; and
//! Bevy can screenshot itself for headless checks. Native only (the `editor` feature).
//!
//! Options (for scripts and screenshots):
//! `--song <stem>`, `--view tracker|piano|text`, `--play`, `--from-bar <n>`, `--toots <n>`
//! (press TOOT n times at the start), `--force <harmony>`, `--feel swing|bossa|samba|rock|funk`
//! (force the band's feel), `--chorus head|two-feel|blowing|stop-time|breaks|riffs|strolling|soli|shout`
//! (force the chorus), `--auto <chaos 0..1>`,
//! `--rec`, `--as-played` (the tracker shows the band's version), `--replace FIND=>WITH` (edit the text first), `--size <w>x<h>`, `--screenshot <file.png>` (after `--wait <secs>`, then quit),
//! `--check` (no window: load and reformat every song, then quit). `NATHAN_AUDIO=headless`
//! renders without a sound card.

mod app;
mod feed;
mod model;
mod played;
mod player;
mod theme;
mod views;

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::WindowResolution;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use nat_han_adventures::audio::live::chorus::{Call as ChorusCall, Chorus};
use nat_han_adventures::audio::live::feel::Feel;
use nat_han_adventures::audio::{AudioOutput, Filters};

use app::{Editor, View, repo_root};

#[derive(Resource, Debug, Clone, Default)]
struct Args {
    song: Option<String>,
    view: Option<View>,
    play: bool,
    from_bar: usize,
    toots: u32,
    force: Option<Filters>,
    feel: Option<Feel>,
    chorus: Option<Chorus>,
    auto: Option<f32>,
    rec: bool,
    as_played: bool,
    size: (u32, u32),
    screenshot: Option<PathBuf>,
    wait: f32,
    check: bool,
    /// `--replace FIND=>WITH`: edit the text at the start (demos, screenshots of errors).
    replace: Vec<(String, String)>,
}

fn args() -> Result<Args, String> {
    let mut a = Args { size: (1440, 900), wait: 4.0, ..Args::default() };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = || it.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--song" => a.song = Some(val()?),
            "--view" => a.view = Some(View::parse(&val()?).ok_or("--view tracker|piano|text")?),
            "--play" => a.play = true,
            "--from-bar" => a.from_bar = val()?.parse::<usize>().map_err(|e| e.to_string())?.saturating_sub(1),
            "--toots" => a.toots = val()?.parse().map_err(|e: std::num::ParseIntError| e.to_string())?,
            "--force" => a.force = Some(Filters::parse(&val()?).ok_or("--force coltrane|quartal|melodic|waltz|original[+ji]")?),
            "--feel" => a.feel = Some(Feel::parse(&val()?).ok_or("--feel swing|bossa|samba|rock|funk")?),
            "--chorus" => a.chorus = Some(Chorus::parse(&val()?).ok_or("--chorus head|two-feel|blowing|stop-time|breaks|riffs|strolling|soli|shout")?),
            "--auto" => a.auto = Some(val()?.parse().map_err(|e: std::num::ParseFloatError| e.to_string())?),
            "--rec" => a.rec = true,
            "--as-played" => a.as_played = true,
            "--size" => {
                let v = val()?;
                let (w, h) = v.split_once('x').ok_or("--size WxH")?;
                a.size = (w.parse().map_err(|_| "--size WxH")?, h.parse().map_err(|_| "--size WxH")?);
            }
            "--screenshot" => a.screenshot = Some(PathBuf::from(val()?)),
            "--wait" => a.wait = val()?.parse().map_err(|e: std::num::ParseFloatError| e.to_string())?,
            "--check" => a.check = true,
            "--replace" => {
                let v = val()?;
                let (f, w) = v.split_once("=>").ok_or("--replace FIND=>WITH")?;
                a.replace.push((f.to_string(), w.to_string()));
            }
            "--help" | "-h" => {
                println!("{}", include_str!("main.rs").lines().take_while(|l| l.starts_with("//!")).map(|l| l.trim_start_matches("//!").trim_start()).collect::<Vec<_>>().join("\n"));
                std::process::exit(0);
            }
            other => return Err(format!("unknown option {other} (try --help)")),
        }
    }
    Ok(a)
}

/// `--check`: every song through the model and the formatter, no window.
fn check() -> Result<String, String> {
    let root = repo_root();
    let songs = app::list_songs(&root);
    for s in &songs {
        let text = app::read_song(&root, &s.stem).ok_or(format!("can't read {}", s.stem))?;
        let mut doc = model::SongDoc::new(&s.stem, &text);
        if let Some(e) = doc.error() {
            return Err(format!("music/{}.song: {e}", s.stem));
        }
        for ch in 0..4 {
            doc.canonicalize(ch).map_err(|e| format!("{} channel {ch}: {e}", s.stem))?;
        }
    }
    Ok(format!("{} songs ok", songs.len()))
}

fn main() {
    let a = match args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("editor: {e}");
            std::process::exit(2);
        }
    };
    if a.check {
        match check() {
            Ok(m) => println!("{m}"),
            Err(e) => {
                eprintln!("editor: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let mut ed = Editor::new(repo_root(), a.song.as_deref(), AudioOutput::from_env());
    if let Some(v) = a.view {
        ed.view = v;
    }
    if let Some(f) = a.force {
        ed.dials.force_harmony = Some(f.harmony);
        if f.just_intonation {
            ed.dials.force_tuning = Some(nat_han_adventures::audio::tuning::Tuning::Medley);
        }
    }
    ed.dials.force_feel = a.feel;
    ed.dials.force_chorus = a.chorus.map(|chorus| ChorusCall {
        chorus,
        key_up: false,
    });
    if let Some(c) = a.auto {
        ed.auto.on = true;
        ed.auto.chaos = c;
    }
    ed.tracker.rec = a.rec;
    ed.tracker.show_played = a.as_played;
    for (f, w) in &a.replace {
        let t = ed.doc.text.replacen(f.as_str(), w, 1);
        ed.set_text(t);
    }
    ed.tracker.row = a.from_bar * ed.rows_per_bar();
    ed.piano.first_bar = a.from_bar;
    App::new()
        .insert_resource(ClearColor(Color::srgb_u8(0x14, 0x10, 0x0c)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Nat Han Adventures · music editor".into(),
                resolution: WindowResolution::from(a.size),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .insert_non_send(ed)
        .insert_resource(a)
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
        })
        .add_systems(EguiPrimaryContextPass, ui)
        .add_systems(Update, script)
        .run();
}

fn ui(mut contexts: EguiContexts, mut ed: NonSendMut<Editor>, time: Res<Time<Real>>, mut styled: Local<bool>) -> Result {
    let ctx = contexts.ctx_mut()?;
    if !*styled {
        // New fonts bind from the next pass.
        theme::install(ctx);
        *styled = true;
        return Ok(());
    }
    ed.update(time.elapsed_secs_f64(), time.delta_secs_f64());
    views::frame(&mut ed, ctx);
    Ok(())
}

/// The command line's script: start playing, toot, screenshot, quit.
fn script(
    mut commands: Commands,
    time: Res<Time<Real>>,
    args: Res<Args>,
    mut ed: NonSendMut<Editor>,
    mut stage: Local<u8>,
    mut exit: MessageWriter<AppExit>,
) {
    let t = time.elapsed_secs();
    if *stage == 0 && t > 0.3 {
        *stage = 1;
        if args.play {
            ed.play_from(args.from_bar);
        }
    }
    if *stage == 1 && t > 1.0 {
        *stage = 2;
        for _ in 0..args.toots {
            ed.press(feed::Button::Toot);
        }
    }
    if let Some(path) = &args.screenshot {
        if *stage == 2 && t > args.wait {
            *stage = 3;
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path.clone()));
        }
        if *stage == 3 && t > args.wait + 2.0 {
            exit.write(AppExit::Success);
        }
    }
}

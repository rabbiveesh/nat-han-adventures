//! The level editor: `cargo run --bin level-editor [-- <levels/file.txt>]`.
//!
//! Paint a level with the game's own sprites, mark its gates, write Han's lines, paint per-tile
//! physics tweaks, set a Side B set list; Validate (F7) runs the level validator and draws its
//! reach map over the grid; Play (F5) opens the real game on the level (a second window, which
//! reloads whenever the editor saves or plays again). Files are the plain level format
//! (`src/level.rs`): what you save is what `include_str!` reads, byte for byte the same for
//! anything you didn't touch. New levels go in `levels/custom/`.
//!
//! Options: `<file>` opens it, `--validate` validates at the start, `--tool tiles|tweaks|gates|lines|select`,
//! `--select <col>,<row>`, `--zoom <z>`, `--size <w>x<h>`, `--screenshot <file.png>` (after
//! `--wait <secs>`, then quit), `--check` (no window: every level file through the editor's
//! document and back, unchanged, then quit). `--play` runs the game itself (with
//! `NATHAN_LEVEL=<file>`: what the Play button starts).

// A downloaded (release) editor on Windows opens no console window beside it.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
#[path = "../editor/theme.rs"]
#[allow(dead_code)]
mod theme;
mod ui;

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::WindowResolution;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, EguiTextureHandle};
use nat_han_adventures::art::{SpriteId, Sprites};

use app::{Editor, Tool, repo_root};

#[derive(Resource, Debug, Clone, Default)]
struct Args {
    file: Option<PathBuf>,
    validate: bool,
    tool: Option<Tool>,
    select: Option<(usize, usize)>,
    zoom: Option<f32>,
    size: (u32, u32),
    screenshot: Option<PathBuf>,
    wait: f32,
    check: bool,
    play: bool,
}

fn args() -> Result<Args, String> {
    let mut a = Args { size: (1600, 900), wait: 3.0, ..Args::default() };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = || it.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--validate" => a.validate = true,
            "--tool" => {
                a.tool = Some(match val()?.as_str() {
                    "tiles" => Tool::Tiles,
                    "tweaks" => Tool::Tweaks,
                    "gates" => Tool::Gates,
                    "lines" => Tool::Spots,
                    "select" => Tool::Select,
                    _ => return Err("--tool tiles|tweaks|gates|lines|select".into()),
                })
            }
            "--select" => {
                let v = val()?;
                let (c, r) = v.split_once(',').ok_or("--select col,row")?;
                a.select = Some((c.parse().map_err(|_| "--select col,row")?, r.parse().map_err(|_| "--select col,row")?));
            }
            "--zoom" => a.zoom = Some(val()?.parse().map_err(|_| "--zoom <z>")?),
            "--size" => {
                let v = val()?;
                let (w, h) = v.split_once('x').ok_or("--size WxH")?;
                a.size = (w.parse().map_err(|_| "--size WxH")?, h.parse().map_err(|_| "--size WxH")?);
            }
            "--screenshot" => a.screenshot = Some(PathBuf::from(val()?)),
            "--wait" => a.wait = val()?.parse().map_err(|_| "--wait <secs>")?,
            "--check" => a.check = true,
            "--play" => a.play = true,
            "--help" | "-h" => {
                println!("{}", include_str!("main.rs").lines().take_while(|l| l.starts_with("//!")).map(|l| l.trim_start_matches("//!").trim_start()).collect::<Vec<_>>().join("\n"));
                std::process::exit(0);
            }
            f if !f.starts_with("--") => a.file = Some(PathBuf::from(f)),
            other => return Err(format!("unknown option {other} (try --help)")),
        }
    }
    Ok(a)
}

/// `--check`: every level file through the document and back.
fn check() -> Result<String, String> {
    let root = repo_root();
    let files = app::list_files(&root);
    for f in &files {
        let text = std::fs::read_to_string(root.join(&f.path)).map_err(|e| format!("{}: {e}", f.path.display()))?;
        let doc = nat_han_adventures::level::doc::LevelDoc::parse(&text).map_err(|e| format!("{}: {e}", f.path.display()))?;
        if doc.text() != text {
            return Err(format!("{}: doesn't round-trip", f.path.display()));
        }
        app::parse(&doc).level.map_err(|e| format!("{}: {e}", f.path.display()))?;
    }
    Ok(format!("{} level files ok", files.len()))
}

fn main() {
    let a = match args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("level-editor: {e}");
            std::process::exit(2);
        }
    };
    if a.play {
        nat_han_adventures::run_game();
        return;
    }
    if a.check {
        match check() {
            Ok(m) => println!("{m}"),
            Err(e) => {
                eprintln!("level-editor: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let mut ed = Editor::new(repo_root(), a.file.as_deref());
    if let Some(t) = a.tool {
        ed.tool = t;
    }
    if let Some(z) = a.zoom {
        ed.zoom = z;
    }
    if let Some(c) = a.select {
        ed.selected = Some(c);
        ed.center_on = Some(c);
    }
    if a.validate {
        ed.validate();
    }
    App::new()
        .insert_resource(ClearColor(Color::srgb_u8(0x14, 0x10, 0x0c)))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Nat Han Adventures · level editor".into(),
                        resolution: WindowResolution::from(a.size),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins((EguiPlugin::default(), nat_han_adventures::art::plugin))
        .insert_non_send(ed)
        .insert_resource(a)
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
        })
        .add_systems(EguiPrimaryContextPass, draw)
        .add_systems(Update, script)
        .run();
}

fn draw(
    mut contexts: EguiContexts,
    mut ed: NonSendMut<Editor>,
    sprites: Option<Res<Sprites>>,
    mut icons: Local<HashMap<SpriteId, bevy_egui::egui::TextureId>>,
    mut styled: Local<bool>,
) -> Result {
    if let Some(sprites) = sprites {
        let world = ed.parsed.level.as_ref().map_or(1, |l| l.world);
        for id in ui::wanted(world) {
            if !icons.contains_key(&id) {
                let tex = contexts.add_image(EguiTextureHandle::Strong(sprites.get(id)));
                icons.insert(id, tex);
            }
        }
    }
    let ctx = contexts.ctx_mut()?;
    if !*styled {
        theme::install(ctx);
        *styled = true;
        return Ok(());
    }
    ed.poll();
    ui::frame(&mut ed, ctx, &icons);
    if ed.validating() || ed.playing() {
        ctx.request_repaint();
    }
    Ok(())
}

/// The command line's script: screenshot and quit (once a validation asked for is in).
fn script(mut commands: Commands, time: Res<Time<Real>>, args: Res<Args>, ed: NonSend<Editor>, mut stage: Local<u8>, mut at: Local<f32>, mut exit: MessageWriter<AppExit>) {
    let t = time.elapsed_secs();
    if let Some(path) = &args.screenshot {
        if *stage == 0 && t > args.wait && !ed.validating() {
            *stage = 1;
            *at = t;
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path.clone()));
        }
        if *stage == 1 && t > *at + 2.0 {
            exit.write(AppExit::Success);
        }
    }
}

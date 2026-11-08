//! Debug dump: press F9 for a plain-text report of everything relevant: build info, every
//! reflected game resource (found through the type registry, so new ones show up for free),
//! the player's and Han's components, extra `Debug` sections other modules register, the raw
//! save, and a log of recent gameplay events. On the web it downloads as a `.txt`; on native
//! it's written next to the save file and the path is logged.

use std::collections::VecDeque;
use std::fmt::{Debug, Write as _};

use bevy::ecs::reflect::{ReflectComponent, ReflectResource};
use bevy::prelude::*;

use crate::events::*;
use crate::game::{Han, Player, RestartLevel};

pub fn plugin(app: &mut App) {
    app.init_resource::<EventLog>()
        .init_resource::<DebugSections>()
        .add_systems(Update, dump.run_if(f9_pressed))
        .add_systems(Startup, register_builtin_sections);
    log_messages::<Jumped>(app);
    log_messages::<Landed>(app);
    log_messages::<PlayerDied>(app);
    log_messages::<PlayerRespawned>(app);
    log_messages::<NuggetCollected>(app);
    log_messages::<CheckpointReached>(app);
    log_messages::<LevelCompleted>(app);
    log_messages::<HanSays>(app);
    log_messages::<PlaySfx>(app);
    log_messages::<RestartLevel>(app);
    log_messages::<crate::audio::MusicStarted>(app);
    log_messages::<crate::audio::MusicChanged>(app);
}

/// How many recent gameplay events the dump includes.
const LOG_LEN: usize = 300;

/// Recent gameplay events: (seconds since startup, type, `Debug` text).
#[derive(Resource, Default)]
pub struct EventLog(VecDeque<(f32, &'static str, String)>);

/// Extra sections for state that isn't reflected: `(title, fn(&World) -> text)`. Any module can
/// push one (e.g. the music director, the adaptive-difficulty profile).
#[derive(Resource, Default)]
pub struct DebugSections(pub Vec<(&'static str, fn(&World) -> String)>);

fn log_messages<M: Message + Debug>(app: &mut App) {
    let name = std::any::type_name::<M>().rsplit("::").next().unwrap_or("?");
    app.add_systems(Last, move |mut r: MessageReader<M>, mut log: ResMut<EventLog>, time: Res<Time<Real>>| {
        for m in r.read() {
            if log.0.len() == LOG_LEN {
                log.0.pop_front();
            }
            log.0.push_back((time.elapsed_secs(), name, format!("{m:?}")));
        }
    });
}

fn f9_pressed(keys: Option<Res<ButtonInput<KeyCode>>>) -> bool {
    keys.is_some_and(|k| k.just_pressed(KeyCode::F9))
}

fn register_builtin_sections(mut sections: ResMut<DebugSections>) {
    sections.0.push(("music director", |w| {
        w.get_resource::<crate::audio::Director>().map_or("(none)".into(), |d| format!("{d:#?}"))
    }));
    sections.0.push(("saved data (raw)", |_| {
        crate::save::storage::read().unwrap_or_else(|| "(nothing saved)".into())
    }));
}

/// Build the whole report.
pub fn report(world: &mut World) -> String {
    let mut out = String::new();
    let secs = world.resource::<Time<Real>>().elapsed_secs();
    let _ = writeln!(out, "Nat Han Adventures debug dump");
    let _ = writeln!(
        out,
        "version {} · commit {} · {} · uptime {secs:.1}s",
        env!("CARGO_PKG_VERSION"),
        option_env!("GITHUB_SHA").unwrap_or("local build"),
        if cfg!(target_arch = "wasm32") { "web" } else { "native" },
    );
    if let Some(state) = world.get_resource::<State<crate::state::AppState>>() {
        let _ = writeln!(out, "app state: {:?}", state.get());
    }
    if let Some(state) = world.get_resource::<State<crate::state::PlayState>>() {
        let _ = writeln!(out, "play state: {:?}", state.get());
    }

    // Every reflected resource of ours.
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let mut ours: Vec<_> = registry
        .iter()
        .filter(|r| r.type_info().type_path().starts_with("nat_han_adventures::"))
        .collect();
    ours.sort_by_key(|r| r.type_info().type_path());
    section(&mut out, "resources");
    // (Bevy 0.19 keeps resources as components on their own entities.)
    for reg in &ours {
        let tid = reg.type_id();
        if reg.data::<ReflectResource>().is_some()
            && let Some(cid) = world.components().get_valid_id(tid)
            && let Some(e) = world.resource_entities().get(cid)
            && let Ok(value) = world.get_reflect(e, tid)
        {
            let _ = writeln!(out, "{} = {:#?}", short(reg.type_info().type_path()), value.as_partial_reflect());
        }
    }

    // The player and Han: all their reflected components.
    for (title, entity) in [
        ("player", single_with::<Player>(world)),
        ("han", single_with::<Han>(world)),
    ] {
        section(&mut out, title);
        let Some(e) = entity else {
            let _ = writeln!(out, "(not spawned)");
            continue;
        };
        for reg in registry.iter() {
            if reg.data::<ReflectComponent>().is_some()
                && let Ok(value) = world.get_reflect(e, reg.type_id())
            {
                let path = reg.type_info().type_path();
                if path.starts_with("nat_han_adventures::") || path.ends_with("::Transform") {
                    let _ = writeln!(out, "{} = {:#?}", short(path), value.as_partial_reflect());
                }
            }
        }
    }
    drop(registry);

    let extra: Vec<_> = world.resource::<DebugSections>().0.clone();
    for (title, f) in extra {
        section(&mut out, title);
        let _ = writeln!(out, "{}", f(world));
    }

    section(&mut out, "recent events (oldest first)");
    for (t, name, text) in &world.resource::<EventLog>().0 {
        let _ = writeln!(out, "{t:9.2}s  {name:<18} {text}");
    }
    out
}

fn section(out: &mut String, title: &str) {
    let _ = writeln!(out, "\n=== {title} ===");
}

fn short(path: &str) -> &str {
    path.strip_prefix("nat_han_adventures::").unwrap_or(path)
}

fn single_with<C: Component>(world: &mut World) -> Option<Entity> {
    world.query_filtered::<Entity, With<C>>().iter(world).next()
}

fn dump(world: &mut World) {
    let text = report(world);
    let secs = world.resource::<Time<Real>>().elapsed_secs() as u64;
    match save_report(&text, &format!("nat-han-debug-{secs}.txt")) {
        Ok(where_) => info!("debug dump saved: {where_}"),
        Err(e) => warn!("debug dump failed: {e}"),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_report(text: &str, name: &str) -> Result<String, String> {
    let dir = crate::save::storage::path().parent().map(std::path::PathBuf::from).unwrap_or_default();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let p = dir.join(name);
    std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(p.display().to_string())
}

/// Web: hand the report to the browser as a file download.
#[cfg(target_arch = "wasm32")]
fn save_report(text: &str, name: &str) -> Result<String, String> {
    use wasm_bindgen::JsCast;
    let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let parts = js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(text));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("text/plain");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts).map_err(err)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(err)?;
    let a: web_sys::HtmlAnchorElement =
        document.create_element("a").map_err(err)?.dyn_into().map_err(|_| "not an anchor")?;
    a.set_href(&url);
    a.set_download(name);
    a.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(format!("downloaded {name}"))
}

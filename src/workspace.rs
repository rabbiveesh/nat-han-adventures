//! Where the native editors read and write their files (`levels/`, `music/`): the repository
//! when they run from it (`cargo run --bin ...`), else a folder of the player's own, filled with
//! the game's levels and songs the first time, so a downloaded editor has them to start from.

use std::path::{Path, PathBuf};

/// The folder a downloaded editor keeps its files in (under Documents).
pub const FOLDER: &str = "Nat Han Adventures";

/// `$NATHAN_HOME`; else the working directory if it has `levels/` or `music/`; else the
/// repository the editor was built from, if it's still there; else `Documents/Nat Han
/// Adventures` (made and filled on first use).
pub fn root() -> PathBuf {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(p) = env("NATHAN_HOME") {
        return p;
    }
    if let Ok(cwd) = std::env::current_dir()
        && is_workspace(&cwd)
    {
        return cwd;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if repo.join("Cargo.toml").is_file() && is_workspace(&repo) {
        return repo;
    }
    let home = env("HOME").or_else(|| env("USERPROFILE"));
    let dir = home
        .map(|h| if h.join("Documents").is_dir() { h.join("Documents") } else { h })
        .or_else(|| std::env::current_exe().ok()?.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(FOLDER);
    fill(&dir);
    dir
}

fn is_workspace(dir: &Path) -> bool {
    dir.join("levels").is_dir() || dir.join("music").is_dir()
}

/// Write the game's own levels and songs into `dir`, leaving any file already there alone.
pub fn fill(dir: &Path) {
    let story = crate::level::LEVEL_SOURCES.iter().enumerate().map(|(i, t)| (format!("levels/{:02}.txt", i + 1), *t));
    let side_b = crate::game::sideb::SIDE_B_SOURCES.iter().enumerate().map(|(i, t)| (format!("levels/side-b/b{}.txt", i + 1), *t));
    let songs = crate::audio::live::library::FILES.iter().map(|(stem, t)| (format!("music/{stem}.song"), *t));
    for (rel, text) in story.chain(side_b).chain(songs) {
        let p = dir.join(rel);
        if p.exists() {
            continue;
        }
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(p, text);
    }
    let _ = std::fs::create_dir_all(dir.join("levels/custom"));
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_fresh_folder_gets_every_level_and_song() {
        let dir = std::env::temp_dir().join(format!("nathan-workspace-{}", std::process::id()));
        super::fill(&dir);
        assert!(super::is_workspace(&dir));
        assert_eq!(std::fs::read_to_string(dir.join("levels/01.txt")).unwrap(), crate::level::LEVEL_SOURCES[0]);
        assert!(dir.join("levels/side-b/b10.txt").is_file());
        assert!(dir.join("music/tiger_rag.song").is_file());
        assert!(dir.join("levels/custom").is_dir());
        // A second fill leaves the player's edits alone.
        std::fs::write(dir.join("levels/01.txt"), "mine").unwrap();
        super::fill(&dir);
        assert_eq!(std::fs::read_to_string(dir.join("levels/01.txt")).unwrap(), "mine");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

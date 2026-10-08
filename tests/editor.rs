//! The music editor binary (`src/bin/editor`, the `editor` feature) builds and runs: `--check`
//! loads every song in `music/` through its model and formatter, with no window.
#![cfg(feature = "editor")]

#[test]
fn the_editor_builds_and_checks_every_song() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_editor")).arg("--check").output().expect("the editor runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}{}", String::from_utf8_lossy(&out.stderr));
    let n = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/music")).unwrap().count();
    assert_eq!(stdout.trim(), format!("{n} songs ok"));
}

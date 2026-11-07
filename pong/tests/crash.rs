//! Crash files (spec 1.7.2, plan 4.4): a forced panic produces the file.
//!
//! The panic hook is process-wide and writes once per process, so each test
//! needs a process of its own: nextest gives every test one (`just test`).

use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use bevy::app::Update;
use pong::crash::{self, CrashContext};
use pong_core::testing::{frame, headless_app};

/// An empty directory of this test's own under the system temp directory.
fn scratch_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bevy_pong-{test}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// The text of every file in `dir`, which must exist.
fn crash_files(dir: &Path) -> Vec<(String, String)> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .expect("the hook created the crash directory")
        .map(|entry| {
            let path = entry.expect("a readable directory entry").path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("a UTF-8 file name")
                .to_owned();
            let text = fs::read_to_string(&path).expect("a readable crash file");
            (name, text)
        })
        .collect();
    files.sort();
    files
}

#[test]
fn a_forced_panic_writes_one_crash_file_naming_the_panic_and_the_seed() {
    let dir = scratch_dir("forced-panic");
    crash::install(dir.clone(), CrashContext { seed: 7 });

    let first = panic::catch_unwind(|| panic!("forced for the crash file test"));
    let second = panic::catch_unwind(|| panic!("a second panic in the same process"));
    assert!(first.is_err() && second.is_err());

    let files = crash_files(&dir);
    assert_eq!(files.len(), 1, "one crash, one file: {files:?}");
    let (name, text) = &files[0];
    assert!(
        name.starts_with("crash-") && Path::new(name).extension().is_some_and(|ext| ext == "txt"),
        "{name}"
    );
    assert!(
        text.contains("message: forced for the crash file test"),
        "{text}"
    );
    assert!(text.contains("seed: 7"), "{text}");
    assert!(text.contains("location: pong/tests/crash.rs:"), "{text}");
    assert!(text.contains("backtrace:"), "{text}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_panic_inside_a_bevy_system_writes_a_crash_file() {
    let dir = scratch_dir("system-panic");
    crash::install(dir.clone(), CrashContext { seed: 11 });

    let mut app = headless_app(1);
    app.add_systems(Update, || panic!("forced inside a system"));
    let result = panic::catch_unwind(AssertUnwindSafe(|| frame(&mut app)));
    assert!(result.is_err(), "the system's panic reaches the caller");

    let files = crash_files(&dir);
    assert_eq!(files.len(), 1, "{files:?}");
    assert!(
        files[0].1.contains("message: forced inside a system"),
        "{}",
        files[0].1
    );
    let _ = fs::remove_dir_all(&dir);
}

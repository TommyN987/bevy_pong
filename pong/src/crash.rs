//! Crash files (spec 1.7.2; docs/architecture.md, "Error policy", uncaught
//! panic): a panic hook writes what went wrong to the XDG state directory, so
//! a panic can be read afterwards.
//!
//! One code path in both builds: the file is written, then the hook that was
//! installed before runs, so a dev build still prints the panic as usual.
//!
//! A panic inside a system does not always end the game: Bevy catches it and
//! hands it to the error policy, which in release logs it and goes on
//! (phase 4 report). The file is written either way.

use std::backtrace::Backtrace;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::panic::{self, PanicHookInfo};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// What a crash file records beyond the panic itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrashContext {
    /// The core's seed, so the launch can be replayed.
    pub seed: u64,
}

/// Set by the first panic of the process: one crash, one file, even if
/// unwinding panics again.
static WRITTEN: AtomicBool = AtomicBool::new(false);

/// Installs the panic hook that writes crash files into `dir`, chained in
/// front of the hook installed before it.
///
/// Call it right after adding `DefaultPlugins`: Bevy's panic handler wraps
/// the hook it finds and skips it for some panics, so this hook goes outside
/// it. The hook is process-wide: install it once.
pub fn install(dir: PathBuf, context: CrashContext) {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if !WRITTEN.swap(true, Ordering::SeqCst) {
            // The log may not exist yet, or any more; stderr always does.
            match write_crash_file(&dir, &report(info, context)) {
                Ok(path) => eprintln!("crash file written to {}", path.display()),
                Err(error) => eprintln!("no crash file in {}: {error}", dir.display()),
            }
        }
        previous(info);
    }));
}

/// The crash file's text: what panicked where, the launch it happened in,
/// and the backtrace, captured whatever `RUST_BACKTRACE` says.
fn report(info: &PanicHookInfo<'_>, context: CrashContext) -> String {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let thread = std::thread::current();
    let location = info.location().map_or_else(
        || "unknown".to_owned(),
        |location| {
            format!(
                "{}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            )
        },
    );
    format!(
        "bevy_pong panicked\n\
         version: {version} ({profile})\n\
         seed: {seed}\n\
         time: {time} (seconds since 1970-01-01 UTC)\n\
         thread: {thread}\n\
         location: {location}\n\
         message: {message}\n\
         \n\
         backtrace:\n\
         {backtrace}\n",
        version = env!("CARGO_PKG_VERSION"),
        seed = context.seed,
        time = unix_seconds(),
        thread = thread.name().unwrap_or("unnamed"),
        message = info.payload_as_str().unwrap_or("(not text)"),
        backtrace = Backtrace::force_capture(),
    )
}

/// Seconds since the Unix epoch; zero for a clock set before it.
fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Writes `contents` to a new `crash-<seconds>-<pid>.txt` in `dir`, creating
/// `dir` if needed, and returns its path. Never overwrites a file.
///
/// # Errors
/// Any I/O error creating the directory or the file.
fn write_crash_file(dir: &Path, contents: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = dir.join(format!(
        "crash-{}-{}.txt",
        unix_seconds(),
        std::process::id()
    ));
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?
        .write_all(contents.as_bytes())?;
    Ok(path)
}

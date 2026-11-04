//! The game's directories under the XDG Base Directory specification (spec
//! 1.6.1, docs/architecture.md "Persistence"): settings in the config
//! directory, profiles in the data directory, crash files in the state
//! directory (spec 1.7.2). Linux only, per the frame; no crate needed.
//!
//! The resolver computes paths and creates nothing: whoever writes a file
//! creates its directory, and reports its own failure.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The directory name under each XDG base.
pub const APP_DIR: &str = "bevy_pong";

/// Where the game keeps its files, one directory per XDG base.
///
/// Invariant: every path is absolute and ends in [`APP_DIR`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDirs {
    config: PathBuf,
    data: PathBuf,
    state: PathBuf,
}

/// No XDG base could be found: a needed `XDG_*_HOME` is unusable and `HOME`
/// is unset, empty, or relative. There is nowhere to keep settings, profiles,
/// or crash files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("no home directory: HOME is unset, empty, or not an absolute path")]
pub struct NoHomeDirectory;

impl AppDirs {
    /// The directories from this process's environment.
    ///
    /// # Errors
    /// [`NoHomeDirectory`], see [`AppDirs::resolve`].
    pub fn from_env() -> Result<Self, NoHomeDirectory> {
        Self::resolve(|name| std::env::var_os(name))
    }

    /// The directories from the environment `env` describes.
    ///
    /// Each base is its `XDG_*_HOME` variable if that holds an absolute path,
    /// else its default under `HOME`. The specification says a relative path
    /// in one of these variables is invalid and ignored; an empty one is
    /// relative. `HOME` is consulted only for a base that needs it.
    ///
    /// # Errors
    /// [`NoHomeDirectory`] when a base falls back to `HOME` and `HOME` is not
    /// an absolute path.
    pub fn resolve(env: impl Fn(&str) -> Option<OsString>) -> Result<Self, NoHomeDirectory> {
        let absolute = |name: &str| {
            env(name)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
        };
        let home = absolute("HOME");
        let base = |variable: &str, default_under_home: &str| {
            absolute(variable)
                .or_else(|| home.as_ref().map(|home| home.join(default_under_home)))
                .map(|base| base.join(APP_DIR))
                .ok_or(NoHomeDirectory)
        };
        Ok(Self {
            config: base("XDG_CONFIG_HOME", ".config")?,
            data: base("XDG_DATA_HOME", ".local/share")?,
            state: base("XDG_STATE_HOME", ".local/state")?,
        })
    }

    /// Per-device settings (spec 1.4.9).
    #[must_use]
    pub fn config(&self) -> &Path {
        &self.config
    }

    /// Profiles and their records (spec 1.4.9).
    #[must_use]
    pub fn data(&self) -> &Path {
        &self.data
    }

    /// Crash files (spec 1.7.2).
    #[must_use]
    pub fn state(&self) -> &Path {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::path::Path;

    use super::{AppDirs, NoHomeDirectory};

    /// An environment holding exactly `vars`.
    fn env(vars: &[(&str, OsString)]) -> impl Fn(&str) -> Option<OsString> {
        let vars = vars.to_vec();
        move |name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.clone())
        }
    }

    fn os(text: &str) -> OsString {
        OsString::from(text)
    }

    #[test]
    fn with_only_home_set_each_base_takes_its_default_under_home() {
        let dirs = AppDirs::resolve(env(&[("HOME", os("/home/kid"))])).expect("HOME is absolute");
        assert_eq!(dirs.config(), Path::new("/home/kid/.config/bevy_pong"));
        assert_eq!(dirs.data(), Path::new("/home/kid/.local/share/bevy_pong"));
        assert_eq!(dirs.state(), Path::new("/home/kid/.local/state/bevy_pong"));
    }

    #[test]
    fn an_absolute_xdg_variable_overrides_its_default() {
        let dirs = AppDirs::resolve(env(&[
            ("HOME", os("/home/kid")),
            ("XDG_CONFIG_HOME", os("/etc/kid")),
            ("XDG_DATA_HOME", os("/srv/data")),
            ("XDG_STATE_HOME", os("/var/state")),
        ]))
        .expect("HOME is absolute");
        assert_eq!(dirs.config(), Path::new("/etc/kid/bevy_pong"));
        assert_eq!(dirs.data(), Path::new("/srv/data/bevy_pong"));
        assert_eq!(dirs.state(), Path::new("/var/state/bevy_pong"));
    }

    #[test]
    fn a_relative_or_empty_xdg_variable_is_ignored() {
        let dirs = AppDirs::resolve(env(&[
            ("HOME", os("/home/kid")),
            ("XDG_CONFIG_HOME", os("relative/config")),
            ("XDG_DATA_HOME", os("")),
        ]))
        .expect("HOME is absolute");
        assert_eq!(dirs.config(), Path::new("/home/kid/.config/bevy_pong"));
        assert_eq!(dirs.data(), Path::new("/home/kid/.local/share/bevy_pong"));
    }

    #[test]
    fn without_a_usable_home_a_base_that_needs_it_fails() {
        assert_eq!(AppDirs::resolve(env(&[])), Err(NoHomeDirectory));
        assert_eq!(
            AppDirs::resolve(env(&[("HOME", os("")), ("XDG_CONFIG_HOME", os("/c"))])),
            Err(NoHomeDirectory)
        );
        assert_eq!(
            AppDirs::resolve(env(&[("HOME", os("home/kid"))])),
            Err(NoHomeDirectory)
        );
    }

    #[test]
    fn the_error_reads_as_one_sentence_naming_home() {
        assert_eq!(
            NoHomeDirectory.to_string(),
            "no home directory: HOME is unset, empty, or not an absolute path"
        );
    }

    #[test]
    fn home_is_not_needed_when_every_base_is_set() {
        let dirs = AppDirs::resolve(env(&[
            ("XDG_CONFIG_HOME", os("/c")),
            ("XDG_DATA_HOME", os("/d")),
            ("XDG_STATE_HOME", os("/s")),
        ]))
        .expect("every base is set");
        assert_eq!(dirs.state(), Path::new("/s/bevy_pong"));
    }

    #[test]
    fn a_path_that_is_not_utf8_is_kept_byte_for_byte() {
        let home = OsString::from_vec(b"/home/k\xffd".to_vec());
        let dirs = AppDirs::resolve(env(&[("HOME", home)])).expect("HOME is absolute");
        assert_eq!(
            dirs.config().as_os_str().as_encoded_bytes(),
            b"/home/k\xffd/.config/bevy_pong"
        );
    }
}

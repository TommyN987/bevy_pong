//! Where the save files live and how they are written (spec 1.4.9,
//! docs/architecture.md "Persistence"): plain `std::fs` through the
//! [`SavePaths`] port, never half-written.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use bevy::ecs::resource::Resource;

/// The settings file's name in its directory.
pub const SETTINGS_FILE: &str = "settings.ron";

/// The profiles file's name in its directory.
pub const PROFILES_FILE: &str = "profiles.ron";

/// Port: the directories the save files live in. The shell inserts it from
/// the XDG config and data directories; tests insert a temporary directory.
/// The core names the files: their format is the core's.
///
/// Precondition: both directories are absolute. They need not exist yet; the
/// writer creates them.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct SavePaths {
    settings_dir: PathBuf,
    profiles_dir: PathBuf,
}

impl SavePaths {
    /// Settings in `settings_dir`, profiles in `profiles_dir`.
    pub fn new(settings_dir: impl Into<PathBuf>, profiles_dir: impl Into<PathBuf>) -> Self {
        Self {
            settings_dir: settings_dir.into(),
            profiles_dir: profiles_dir.into(),
        }
    }

    /// The settings file.
    #[must_use]
    pub fn settings_file(&self) -> PathBuf {
        self.settings_dir.join(SETTINGS_FILE)
    }

    /// The profiles file.
    #[must_use]
    pub fn profiles_file(&self) -> PathBuf {
        self.profiles_dir.join(PROFILES_FILE)
    }
}

/// Why a save file was not written.
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    /// The file system refused: disk full, no permission, a bad path.
    #[error("could not write {}: {source}", path.display())]
    Io {
        /// The file that was to be written.
        path: PathBuf,
        /// What the file system said.
        source: std::io::Error,
    },
}

/// Replaces the contents of `path` with `bytes`, never leaving a torn file
/// (docs/architecture.md, error policy rule 5).
///
/// See [`replace_file`] for the steps and the contract.
///
/// # Errors
/// [`SaveError::Io`] if any step fails; `path` is then as it was.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), SaveError> {
    replace_file(path, |file| file.write_all(bytes))
}

/// Creates the parent directory, lets `fill` write a temporary file beside
/// `path` (`<name>.tmp`, on the same file system), syncs it to disk, and
/// renames it over `path`.
///
/// Precondition: `path` has a parent directory and a file name; the files of
/// [`SavePaths`] always do.
/// Postcondition: on `Ok`, `path` holds what `fill` wrote. On `Err`, `path`
/// is as it was, the old complete file or none, and the temporary file is
/// removed as far as the file system allows; a leftover one is harmless,
/// since the next write truncates it.
///
/// The parent directory is not synced, so a power cut just after the rename
/// may bring back the old file, but always a complete one.
fn replace_file(
    path: &Path,
    fill: impl FnOnce(&mut File) -> std::io::Result<()>,
) -> Result<(), SaveError> {
    let failed = |source| SaveError::Io {
        path: path.to_owned(),
        source,
    };
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(failed(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "a save file needs a directory and a name",
        )));
    };
    std::fs::create_dir_all(dir).map_err(failed)?;
    let mut temp_name = name.to_owned();
    temp_name.push(".tmp");
    let temp = dir.join(temp_name);

    let written = File::create(&temp).and_then(|mut file| {
        fill(&mut file)?;
        file.sync_all()
    });
    if let Err(source) = written.and_then(|()| std::fs::rename(&temp, path)) {
        let _ = std::fs::remove_file(&temp);
        return Err(failed(source));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::{Path, PathBuf};

    use super::{SaveError, SavePaths, replace_file, write_atomically};
    use crate::testing::TempSaveDir;

    #[test]
    fn each_file_is_named_in_its_own_directory() {
        let paths = SavePaths::new(
            "/home/zoe/.config/bevy_pong",
            "/home/zoe/.local/share/bevy_pong",
        );
        assert_eq!(
            paths.settings_file(),
            PathBuf::from("/home/zoe/.config/bevy_pong/settings.ron")
        );
        assert_eq!(
            paths.profiles_file(),
            PathBuf::from("/home/zoe/.local/share/bevy_pong/profiles.ron")
        );
    }

    #[test]
    fn writing_creates_the_missing_directory_and_the_file() {
        let temp = TempSaveDir::new();
        let file = temp.save_paths().profiles_file();
        write_atomically(&file, b"(version: 1, profiles: [])").expect("a writable temp dir");
        assert_eq!(
            std::fs::read(&file).expect("the file exists"),
            b"(version: 1, profiles: [])"
        );
    }

    #[test]
    fn writing_replaces_the_whole_old_file() {
        let temp = TempSaveDir::new();
        let file = temp.save_paths().settings_file();
        write_atomically(&file, b"a much longer old file").expect("first write");
        write_atomically(&file, b"new").expect("second write");
        assert_eq!(std::fs::read(&file).expect("the file exists"), b"new");
    }

    /// Spec 1.4.9: an injected failing writer leaves the original intact.
    #[test]
    fn a_writer_that_fails_halfway_leaves_the_original_intact_and_no_temp_file() {
        let temp = TempSaveDir::new();
        let file = temp.save_paths().profiles_file();
        write_atomically(&file, b"the original").expect("first write");

        let failed = replace_file(&file, |out| {
            out.write_all(b"the half")?;
            Err(std::io::Error::other("disk full"))
        });

        assert!(matches!(failed, Err(SaveError::Io { ref path, .. }) if path == &file));
        assert_eq!(
            std::fs::read(&file).expect("the original remains"),
            b"the original"
        );
        let left: Vec<PathBuf> = std::fs::read_dir(file.parent().expect("a parent"))
            .expect("the directory exists")
            .map(|entry| entry.expect("an entry").path())
            .collect();
        assert_eq!(left, vec![file]);
    }

    #[test]
    fn a_path_without_a_file_name_is_refused_not_written() {
        assert!(matches!(
            write_atomically(Path::new("/"), b"nothing"),
            Err(SaveError::Io { .. })
        ));
    }
}

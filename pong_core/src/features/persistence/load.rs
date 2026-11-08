//! The load policy (spec 1.4.9, docs/architecture.md error table): a save
//! file that cannot be used is set aside under a timestamped name, never
//! overwritten, and the game starts fresh.

use std::path::{Path, PathBuf};

use super::document::{Corruption, DecodeError};

/// Why a save file could not be used (docs/architecture.md, error table).
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// Not a document of any version this game reads.
    #[error("it is damaged: {0}")]
    Corrupt(Corruption),
    /// Written by a newer version of the game.
    #[error("it was written by a newer version of the game (file version {version})")]
    NewerVersion {
        /// The file's version.
        version: u32,
    },
    /// The file system would not hand over the file.
    #[error("it could not be read: {0}")]
    Io(std::io::Error),
}

impl From<DecodeError> for LoadError {
    fn from(error: DecodeError) -> Self {
        match error {
            DecodeError::NewerVersion { version } => Self::NewerVersion { version },
            DecodeError::Corrupt(corruption) => Self::Corrupt(corruption),
        }
    }
}

/// What loading one save file found.
#[derive(Debug)]
pub enum Loaded<M> {
    /// The file decoded to this model.
    Read(M),
    /// There is no file: a first start.
    Absent,
    /// The file was unusable and now lies at `backup`; the game starts fresh
    /// and may write the file again.
    SetAside {
        /// Why it was unusable.
        reason: LoadError,
        /// Where it was moved.
        backup: PathBuf,
    },
    /// The file was unusable and could not be moved; the game starts fresh
    /// and must not write over it this session ("never overwrite").
    Stuck {
        /// Why it was unusable.
        reason: LoadError,
        /// Why it could not be moved.
        rename: std::io::Error,
    },
}

/// Reads and decodes the save file at `path`, setting it aside if it cannot
/// be used. `seconds` (Unix time) names the backup.
///
/// Postcondition: unless the result is [`Loaded::Stuck`], nothing remains at
/// `path` that the game could not read, so writing `path` loses nothing.
pub fn load<M>(
    path: &Path,
    decode: impl FnOnce(&str) -> Result<M, DecodeError>,
    seconds: u64,
) -> Loaded<M> {
    let reason = match std::fs::read_to_string(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Loaded::Absent,
        Err(error) => LoadError::Io(error),
        Ok(text) => match decode(&text) {
            Ok(model) => return Loaded::Read(model),
            Err(error) => error.into(),
        },
    };
    let backup = backup_path(path, seconds);
    match std::fs::rename(path, &backup) {
        Ok(()) => Loaded::SetAside { reason, backup },
        Err(rename) => Loaded::Stuck { reason, rename },
    }
}

/// Where an unusable file is set aside: `<name>.<seconds>.bak` beside it, or
/// `<name>.<seconds>-1.bak`, `-2`, and so on if that is taken, so two
/// failures in one second keep both files. `seconds` is Unix time.
fn backup_path(path: &Path, seconds: u64) -> PathBuf {
    let named = |suffix: String| {
        let mut name = path.as_os_str().to_owned();
        name.push(format!(".{seconds}{suffix}.bak"));
        PathBuf::from(name)
    };
    let first = named(String::new());
    if !first.exists() {
        return first;
    }
    (1..u32::MAX)
        .map(|n| named(format!("-{n}")))
        .find(|candidate| !candidate.exists())
        .unwrap_or(first)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    use super::{LoadError, Loaded, backup_path, load};
    use crate::features::persistence::document::{decode_profiles, encode_profiles};
    use crate::features::persistence::model::Profiles;
    use crate::testing::TempSaveDir;

    const NOW: u64 = 1_762_615_800;

    fn fixture(name: &str) -> String {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/load_policy")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
    }

    /// A temp directory holding `profiles.ron` with `text`.
    fn profiles_file(text: &str) -> (TempSaveDir, PathBuf) {
        let temp = TempSaveDir::new();
        let file = temp.path().join("profiles.ron");
        std::fs::write(&file, text).expect("writable");
        (temp, file)
    }

    fn set_read_only(dir: &Path, read_only: bool) {
        let mode = if read_only { 0o555 } else { 0o755 };
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode))
            .expect("the test owns the directory");
    }

    #[test]
    fn a_backup_is_named_after_the_file_and_the_second() {
        let temp = TempSaveDir::new();
        let file = temp.path().join("profiles.ron");
        assert_eq!(
            backup_path(&file, 1_762_615_800),
            temp.path().join("profiles.ron.1762615800.bak")
        );
    }

    #[test]
    fn a_second_backup_in_the_same_second_gets_its_own_name() {
        let temp = TempSaveDir::new();
        let file = temp.path().join("profiles.ron");
        let taken = temp.path().join("profiles.ron.1762615800.bak");
        std::fs::write(&taken, "first").expect("writable");
        assert_eq!(
            backup_path(&file, 1_762_615_800),
            temp.path().join("profiles.ron.1762615800-1.bak")
        );
        std::fs::write(temp.path().join("profiles.ron.1762615800-1.bak"), "second")
            .expect("writable");
        assert_eq!(
            backup_path(&file, 1_762_615_800),
            temp.path().join("profiles.ron.1762615800-2.bak")
        );
    }

    #[test]
    fn no_file_is_a_first_start() {
        let temp = TempSaveDir::new();
        let loaded = load(&temp.path().join("profiles.ron"), decode_profiles, NOW);
        assert!(matches!(loaded, Loaded::Absent), "{loaded:?}");
    }

    #[test]
    fn a_valid_file_is_read() {
        let saved = encode_profiles(&Profiles::default()).expect("encodes");
        let (_temp, file) = profiles_file(&saved);
        let loaded = load(&file, decode_profiles, NOW);
        assert!(
            matches!(loaded, Loaded::Read(ref p) if *p == Profiles::default()),
            "{loaded:?}"
        );
    }

    /// Spec 1.4.9: a corrupt file is backed up under a timestamp, never lost.
    #[test]
    fn a_corrupt_file_is_set_aside_under_a_timestamp_and_its_bytes_kept() {
        let corrupt = fixture("corrupt.ron");
        let (temp, file) = profiles_file(&corrupt);
        let loaded = load(&file, decode_profiles, NOW);
        let Loaded::SetAside { reason, backup } = loaded else {
            panic!("expected SetAside, got {loaded:?}");
        };
        assert!(matches!(reason, LoadError::Corrupt(_)), "{reason:?}");
        assert_eq!(backup, temp.path().join("profiles.ron.1762615800.bak"));
        assert_eq!(
            std::fs::read_to_string(&backup).expect("the backup exists"),
            corrupt
        );
        assert!(!file.exists(), "the corrupt file is still in the way");
    }

    /// Spec 1.4.9: a file from a newer game is set aside the same way.
    #[test]
    fn a_file_from_a_newer_game_is_set_aside_as_newer() {
        let (_temp, file) = profiles_file(&fixture("newer_version.ron"));
        let loaded = load(&file, decode_profiles, NOW);
        assert!(
            matches!(
                loaded,
                Loaded::SetAside {
                    reason: LoadError::NewerVersion { version: 99 },
                    ..
                }
            ),
            "{loaded:?}"
        );
        assert!(!file.exists());
    }

    /// "Never overwrite": a file that cannot be moved stays where it is.
    #[test]
    fn a_corrupt_file_that_cannot_be_moved_is_left_in_place() {
        let corrupt = fixture("corrupt.ron");
        let (temp, file) = profiles_file(&corrupt);
        set_read_only(temp.path(), true);
        let loaded = load(&file, decode_profiles, NOW);
        set_read_only(temp.path(), false);
        assert!(
            matches!(
                loaded,
                Loaded::Stuck {
                    reason: LoadError::Corrupt(_),
                    ..
                }
            ),
            "{loaded:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&file).expect("still there"),
            corrupt
        );
    }
}

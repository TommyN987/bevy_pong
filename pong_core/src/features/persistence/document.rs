//! The save documents as they are on disk (spec 1.4.9, docs/architecture.md
//! "Persistence"): RON, one file per kind, each with its own version.
//!
//! A file names its version in a top-level `version` field. Decoding reads
//! that field alone first, then parses the file strictly as that version's
//! frozen shape, wraps it in the kind's versioned enum, and migrates it one
//! version at a time to the current model through the model's validating
//! constructors. Reading the version first is what tells a file from a newer
//! game apart from a damaged one.
//!
//! Every type in [`v1`] is the released version 1 format and never changes.
//! A new version adds its shape to that version's module, a variant, and a
//! step from the version before. Each kind versions on its own: profiles are
//! at version 2 ([`v2`], M2 spec 2.8.3); settings at 3 ([`v3`], M2 spec 2.8.2).

use bevy::color::Srgba;
use ron::ser::PrettyConfig;
use serde::Deserialize;

use crate::domain::{Mode, ProfileId};

use super::model::{
    InvalidSaveData, Profile, ProfileName, Profiles, Records, RecordsByMode, Settings, Volume,
    WindowMode,
};

/// The profiles file version this game writes.
pub const PROFILES_VERSION: u32 = 2;

/// The settings file version this game writes.
pub const SETTINGS_VERSION: u32 = 3;

/// Why a document could not become a model.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    /// Written by a newer game: its version is above the one this game writes.
    #[error("file version {version} is newer than this game reads")]
    NewerVersion {
        /// The file's version.
        version: u32,
    },
    /// Not a document of any version this game reads.
    #[error("file is corrupt: {0}")]
    Corrupt(#[from] Corruption),
}

/// What is wrong with a corrupt document.
#[derive(Debug, thiserror::Error)]
pub enum Corruption {
    /// Not RON, or not the shape its version promises.
    #[error("{0}")]
    Syntax(#[from] ron::de::SpannedError),
    /// A value its model refuses.
    #[error("{0}")]
    Invalid(#[from] InvalidSaveData),
    /// A version this game never wrote, below the current one.
    #[error("no version {version} was ever released")]
    UnknownVersion {
        /// The file's version.
        version: u32,
    },
}

/// Why a model could not be written as a document.
#[derive(Debug, thiserror::Error)]
#[error("could not write the document: {0}")]
pub struct EncodeError(#[from] ron::Error);

/// The profiles as a current-version document.
///
/// # Errors
/// [`EncodeError`] if RON cannot write the document.
pub fn encode_profiles(profiles: &Profiles) -> Result<String, EncodeError> {
    let entry = |records: Records| v1::RecordsEntry {
        wins: records.wins,
        losses: records.losses,
        longest_rally: records.longest_rally,
    };
    let document = v2::ProfilesFile {
        version: PROFILES_VERSION,
        profiles: profiles
            .as_slice()
            .iter()
            .map(|profile| {
                let color = profile.color();
                let records = profile.records_by_mode();
                v2::ProfileEntry {
                    id: profile.id().0,
                    name: profile.name().as_str().to_owned(),
                    color: v1::Color {
                        red: color.red,
                        green: color.green,
                        blue: color.blue,
                    },
                    records: v2::RecordsByModeEntry {
                        classic: entry(records.classic),
                        chaos: entry(records.chaos),
                    },
                }
            })
            .collect(),
    };
    Ok(ron::ser::to_string_pretty(&document, PrettyConfig::new())?)
}

/// The profiles a document of any released version holds.
///
/// # Errors
/// [`DecodeError::NewerVersion`] for a file from a newer game;
/// [`DecodeError::Corrupt`] for anything else that is not a valid document.
pub fn decode_profiles(text: &str) -> Result<Profiles, DecodeError> {
    let document = match version_of(text)? {
        1 => ProfilesDocument::V1(strict(text)?),
        2 => ProfilesDocument::V2(strict(text)?),
        version => return Err(unreadable_version(version, PROFILES_VERSION)),
    };
    Ok(document.into_current().map_err(Corruption::from)?)
}

/// The settings as a current-version document.
///
/// # Errors
/// [`EncodeError`] if RON cannot write the document.
pub fn encode_settings(settings: Settings) -> Result<String, EncodeError> {
    let document = v3::SettingsFile {
        version: SETTINGS_VERSION,
        volume_percent: settings.volume.percent(),
        window_mode: match settings.window_mode {
            WindowMode::BorderlessFullscreen => v1::WindowModeEntry::BorderlessFullscreen,
            WindowMode::Windowed => v1::WindowModeEntry::Windowed,
        },
        points_to_win: settings
            .points_to_win
            .map_or(v2::PointsEntry::Unpicked, |points| {
                v2::PointsEntry::Picked(points.get())
            }),
        mode: match settings.mode {
            Mode::Classic => v3::ModeEntry::Classic,
            Mode::Chaos { cap } => v3::ModeEntry::Chaos { cap: cap.get() },
        },
    };
    Ok(ron::ser::to_string_pretty(&document, PrettyConfig::new())?)
}

/// The settings a document of any released version holds.
///
/// # Errors
/// [`DecodeError::NewerVersion`] for a file from a newer game;
/// [`DecodeError::Corrupt`] for anything else that is not a valid document.
pub fn decode_settings(text: &str) -> Result<Settings, DecodeError> {
    let document = match version_of(text)? {
        1 => SettingsDocument::V1(strict(text)?),
        2 => SettingsDocument::V2(strict(text)?),
        3 => SettingsDocument::V3(strict(text)?),
        version => return Err(unreadable_version(version, SETTINGS_VERSION)),
    };
    Ok(document.into_current().map_err(Corruption::from)?)
}

/// A profiles file of a released version, before migration.
enum ProfilesDocument {
    V1(v1::ProfilesFile),
    V2(v2::ProfilesFile),
}

impl ProfilesDocument {
    /// Migrates to the current model, one version at a time: a version 1
    /// file becomes a version 2 file, which becomes the model.
    fn into_current(self) -> Result<Profiles, InvalidSaveData> {
        match self {
            Self::V1(file) => v2::ProfilesFile::from(file).into_model(),
            Self::V2(file) => file.into_model(),
        }
    }
}

/// A settings file of a released version, before migration.
enum SettingsDocument {
    V1(v1::SettingsFile),
    V2(v2::SettingsFile),
    V3(v3::SettingsFile),
}

impl SettingsDocument {
    /// Migrates to the current model, one version at a time: a version 1
    /// file becomes a version 2 file, that a version 3 file, which becomes
    /// the model. A new version adds one step at the end; no step is rewritten.
    fn into_current(self) -> Result<Settings, InvalidSaveData> {
        let v2 = match self {
            Self::V1(file) => v2::SettingsFile::from(file),
            Self::V2(file) => file,
            Self::V3(file) => return file.into_model(),
        };
        v3::SettingsFile::from(v2).into_model()
    }
}

/// Reads only a document's top-level `version`, ignoring every other field,
/// so it reads a file of any version.
#[derive(Deserialize)]
struct VersionProbe {
    version: u32,
}

fn version_of(text: &str) -> Result<u32, Corruption> {
    Ok(ron::from_str::<VersionProbe>(text)?.version)
}

/// `text` parsed as exactly the shape `T`: unknown fields are refused.
fn strict<'a, T: Deserialize<'a>>(text: &'a str) -> Result<T, Corruption> {
    Ok(ron::from_str(text)?)
}

/// The error for a version this game cannot read, given the one it writes.
fn unreadable_version(version: u32, current: u32) -> DecodeError {
    if version > current {
        DecodeError::NewerVersion { version }
    } else {
        Corruption::UnknownVersion { version }.into()
    }
}

/// Released version 1. Frozen: never edit these types; add a `v2` instead.
mod v1 {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct ProfilesFile {
        pub(super) version: u32,
        pub(super) profiles: Vec<ProfileEntry>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct ProfileEntry {
        pub(super) id: u32,
        pub(super) name: String,
        pub(super) color: Color,
        pub(super) records: RecordsEntry,
    }

    /// sRGB channels, the game's own shape rather than Bevy's colour
    /// serialization, so a Bevy upgrade cannot change the file.
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct Color {
        pub(super) red: f32,
        pub(super) green: f32,
        pub(super) blue: f32,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct RecordsEntry {
        pub(super) wins: u32,
        pub(super) losses: u32,
        pub(super) longest_rally: u32,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct SettingsFile {
        pub(super) version: u32,
        pub(super) volume_percent: u8,
        pub(super) window_mode: WindowModeEntry,
    }

    #[derive(Serialize, Deserialize)]
    pub(super) enum WindowModeEntry {
        BorderlessFullscreen,
        Windowed,
    }
}

/// Released version 2 of each kind, frozen once released, as [`v1`] is.
///
/// - Settings (plan 10.2): version 1 plus the last points-to-win picked. It
///   reuses v1's window mode, whose shape it shares.
/// - Profiles (M2 spec 2.8.3): version 1 with each profile's records kept
///   per mode. It reuses v1's colour and v1's records shape, once per mode.
mod v2 {
    use serde::{Deserialize, Serialize};

    use super::v1::{self, WindowModeEntry};
    use super::{
        InvalidSaveData, Profile, ProfileId, ProfileName, Profiles, Records, RecordsByMode, Srgba,
    };

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct ProfilesFile {
        pub(super) version: u32,
        pub(super) profiles: Vec<ProfileEntry>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct ProfileEntry {
        pub(super) id: u32,
        pub(super) name: String,
        pub(super) color: v1::Color,
        pub(super) records: RecordsByModeEntry,
    }

    /// One field per mode, by name: a third mode is a new version.
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct RecordsByModeEntry {
        pub(super) classic: v1::RecordsEntry,
        pub(super) chaos: v1::RecordsEntry,
    }

    /// The `v1 → v2` step: every record version 1 kept was Classic's, the
    /// only mode there was; Chaos starts empty.
    impl From<v1::ProfilesFile> for ProfilesFile {
        fn from(file: v1::ProfilesFile) -> Self {
            Self {
                version: 2,
                profiles: file
                    .profiles
                    .into_iter()
                    .map(|entry| ProfileEntry {
                        id: entry.id,
                        name: entry.name,
                        color: entry.color,
                        records: RecordsByModeEntry {
                            classic: entry.records,
                            chaos: v1::RecordsEntry {
                                wins: 0,
                                losses: 0,
                                longest_rally: 0,
                            },
                        },
                    })
                    .collect(),
            }
        }
    }

    impl ProfilesFile {
        pub(super) fn into_model(self) -> Result<Profiles, InvalidSaveData> {
            let records = |entry: v1::RecordsEntry| Records {
                wins: entry.wins,
                losses: entry.losses,
                longest_rally: entry.longest_rally,
            };
            let profiles = self
                .profiles
                .into_iter()
                .map(|entry| {
                    Profile::new(
                        ProfileId(entry.id),
                        ProfileName::new(&entry.name)?,
                        Srgba::rgb(entry.color.red, entry.color.green, entry.color.blue),
                        RecordsByMode {
                            classic: records(entry.records.classic),
                            chaos: records(entry.records.chaos),
                        },
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Profiles::new(profiles)
        }
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct SettingsFile {
        pub(super) version: u32,
        pub(super) volume_percent: u8,
        pub(super) window_mode: WindowModeEntry,
        pub(super) points_to_win: PointsEntry,
    }

    /// The game's own shape, not an `Option`: serde would read a missing
    /// `Option` field as `None`, and a released shape is exact.
    #[derive(Serialize, Deserialize)]
    pub(super) enum PointsEntry {
        Unpicked,
        Picked(u8),
    }

    /// The `v1 → v2` step: the values kept, no pick, since version 1 saved
    /// none.
    impl From<v1::SettingsFile> for SettingsFile {
        fn from(file: v1::SettingsFile) -> Self {
            Self {
                version: 2,
                volume_percent: file.volume_percent,
                window_mode: file.window_mode,
                points_to_win: PointsEntry::Unpicked,
            }
        }
    }
}

/// Released version 3 of the settings (M2 spec 2.8.2): version 2 plus the
/// last mode picked. Frozen once released; it reuses the shapes of the
/// versions before for the fields it shares with them.
mod v3 {
    use std::num::NonZeroU8;

    use serde::{Deserialize, Serialize};

    use super::v1::WindowModeEntry;
    use super::v2::{self, PointsEntry};
    use super::{InvalidSaveData, Mode, Settings, Volume, WindowMode};

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct SettingsFile {
        pub(super) version: u32,
        pub(super) volume_percent: u8,
        pub(super) window_mode: WindowModeEntry,
        pub(super) points_to_win: PointsEntry,
        pub(super) mode: ModeEntry,
    }

    /// The game's own shape, not the domain's `Mode`: renaming a variant
    /// there must not change the file.
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub(super) enum ModeEntry {
        Classic,
        Chaos { cap: u8 },
    }

    /// The `v2 → v3` step: the values kept, and Classic, the only mode a
    /// version 2 game played.
    impl From<v2::SettingsFile> for SettingsFile {
        fn from(file: v2::SettingsFile) -> Self {
            Self {
                version: 3,
                volume_percent: file.volume_percent,
                window_mode: file.window_mode,
                points_to_win: file.points_to_win,
                mode: ModeEntry::Classic,
            }
        }
    }

    impl SettingsFile {
        pub(super) fn into_model(self) -> Result<Settings, InvalidSaveData> {
            Ok(Settings {
                volume: Volume::new(self.volume_percent)?,
                window_mode: match self.window_mode {
                    WindowModeEntry::BorderlessFullscreen => WindowMode::BorderlessFullscreen,
                    WindowModeEntry::Windowed => WindowMode::Windowed,
                },
                points_to_win: match self.points_to_win {
                    PointsEntry::Unpicked => None,
                    PointsEntry::Picked(points) => {
                        Some(NonZeroU8::new(points).ok_or(InvalidSaveData::NoPointsToWin)?)
                    }
                },
                mode: match self.mode {
                    ModeEntry::Classic => Mode::Classic,
                    ModeEntry::Chaos { cap } => Mode::Chaos {
                        cap: NonZeroU8::new(cap).ok_or(InvalidSaveData::NoChaosCap)?,
                    },
                },
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;

    use bevy::color::Srgba;

    use super::{
        Corruption, DecodeError, decode_profiles, decode_settings, encode_profiles, encode_settings,
    };
    use crate::domain::{Mode, ProfileId};
    use crate::features::persistence::model::{
        InvalidSaveData, Profile, ProfileName, Profiles, Records, RecordsByMode, Settings, Volume,
        WindowMode,
    };
    use crate::features::tuning::{EMBEDDED_RON, Tuning};

    fn profile(id: u32, name: &str, color: Srgba, records: RecordsByMode) -> Profile {
        Profile::new(
            ProfileId(id),
            ProfileName::new(name).expect("a valid name"),
            color,
            records,
        )
        .expect("a valid profile")
    }

    fn two_children() -> Profiles {
        Profiles::new(vec![
            profile(
                1,
                "Zoë",
                Srgba::rgb(1.0, 0.3, 0.25),
                RecordsByMode {
                    classic: Records {
                        wins: 4,
                        losses: 2,
                        longest_rally: 17,
                    },
                    chaos: Records {
                        wins: 1,
                        losses: 3,
                        longest_rally: 9,
                    },
                },
            ),
            profile(
                2,
                "Max",
                Srgba::rgb(0.2, 0.8, 0.9),
                RecordsByMode::default(),
            ),
        ])
        .expect("distinct ids")
    }

    #[test]
    fn profiles_come_back_from_their_document_unchanged() {
        let profiles = two_children();
        let text = encode_profiles(&profiles).expect("profiles encode");
        assert_eq!(
            decode_profiles(&text).expect("their own document decodes"),
            profiles
        );
    }

    /// M2 spec 2.8.3: the profiles this game writes are version 2.
    #[test]
    fn profiles_are_written_as_version_2() {
        let text = encode_profiles(&two_children()).expect("profiles encode");
        assert!(text.contains("version: 2"), "{text}");
    }

    /// M2 spec 2.8.3: M1's records become Classic's.
    #[test]
    fn a_version_1_profiles_file_migrates_with_its_records_under_classic() {
        let v1 = "(version: 1, profiles: [(id: 1, name: \"Zoë\", \
            color: (red: 1.0, green: 0.3, blue: 0.25), \
            records: (wins: 4, losses: 2, longest_rally: 17))])";
        let profiles = decode_profiles(v1).expect("version 1 decodes");
        assert_eq!(
            profiles.as_slice()[0].records_by_mode(),
            RecordsByMode {
                classic: Records {
                    wins: 4,
                    losses: 2,
                    longest_rally: 17,
                },
                chaos: Records::default(),
            }
        );
    }

    /// Exact like version 1: a version 2 profile without a mode's records
    /// is corrupt, not empty there.
    #[test]
    fn a_version_2_profile_missing_a_modes_records_is_corrupt() {
        let missing = "(version: 2, profiles: [(id: 1, name: \"Zoë\", \
            color: (red: 1.0, green: 0.3, blue: 0.25), \
            records: (classic: (wins: 4, losses: 2, longest_rally: 17)))])";
        assert!(matches!(
            decode_profiles(missing),
            Err(DecodeError::Corrupt(Corruption::Syntax(_)))
        ));
    }

    #[test]
    fn settings_come_back_from_their_document_unchanged() {
        for settings in [
            Settings::default(),
            Settings {
                volume: Volume::new(30).expect("in range"),
                window_mode: WindowMode::Windowed,
                points_to_win: NonZeroU8::new(15),
                mode: Mode::Classic,
            },
            Settings {
                mode: Mode::Chaos {
                    cap: NonZeroU8::new(2).expect("above zero"),
                },
                ..Settings::default()
            },
        ] {
            let text = encode_settings(settings).expect("settings encode");
            assert_eq!(
                decode_settings(&text).expect("their own document decodes"),
                settings
            );
        }
    }

    /// M2 spec 2.8.2: the settings this game writes are version 3.
    #[test]
    fn settings_are_written_as_version_3() {
        let text = encode_settings(Settings::default()).expect("settings encode");
        assert!(text.contains("version: 3"), "{text}");
    }

    /// The second migration: a version 2 file keeps its values and names
    /// Classic, the only mode a version 2 game played.
    #[test]
    fn a_version_2_settings_file_migrates_with_classic_as_its_mode() {
        let v2 =
            "(version: 2, volume_percent: 40, window_mode: Windowed, points_to_win: Picked(15))";
        assert_eq!(
            decode_settings(v2).expect("version 2 decodes"),
            Settings {
                volume: Volume::new(40).expect("in range"),
                window_mode: WindowMode::Windowed,
                points_to_win: NonZeroU8::new(15),
                mode: Mode::Classic,
            }
        );
    }

    #[test]
    fn a_version_3_file_missing_its_mode_field_is_corrupt() {
        let missing =
            "(version: 3, volume_percent: 60, window_mode: Windowed, points_to_win: Unpicked)";
        assert!(matches!(
            decode_settings(missing),
            Err(DecodeError::Corrupt(Corruption::Syntax(_)))
        ));
    }

    /// A released shape is exact inside a variant too.
    #[test]
    fn a_version_3_mode_with_a_field_version_3_never_had_is_corrupt() {
        let extra = "(version: 3, volume_percent: 60, window_mode: Windowed, \
            points_to_win: Unpicked, mode: Chaos(cap: 2, speed: 9))";
        assert!(matches!(
            decode_settings(extra),
            Err(DecodeError::Corrupt(Corruption::Syntax(_)))
        ));
    }

    /// `Mode::Chaos` cannot hold a cap of zero, so a file that saves one is
    /// corrupt.
    #[test]
    fn a_version_3_file_saving_a_chaos_cap_of_zero_is_corrupt() {
        let zero = "(version: 3, volume_percent: 60, window_mode: Windowed, \
            points_to_win: Unpicked, mode: Chaos(cap: 0))";
        assert!(matches!(
            decode_settings(zero),
            Err(DecodeError::Corrupt(Corruption::Invalid(
                InvalidSaveData::NoChaosCap
            )))
        ));
    }

    /// The first migration: a version 1 file keeps its values and has no
    /// points-to-win pick, since version 1 never saved one.
    #[test]
    fn a_version_1_settings_file_migrates_with_no_points_pick() {
        let v1 = "(version: 1, volume_percent: 60, window_mode: Windowed)";
        assert_eq!(
            decode_settings(v1).expect("version 1 decodes"),
            Settings {
                volume: Volume::new(60).expect("in range"),
                window_mode: WindowMode::Windowed,
                points_to_win: None,
                mode: Mode::Classic,
            }
        );
    }

    /// Exact like version 1: a version 2 file without its pick field is
    /// corrupt, not unpicked.
    #[test]
    fn a_version_2_file_missing_its_points_field_is_corrupt() {
        let missing = "(version: 2, volume_percent: 60, window_mode: Windowed)";
        assert!(matches!(
            decode_settings(missing),
            Err(DecodeError::Corrupt(Corruption::Syntax(_)))
        ));
    }

    #[test]
    fn a_version_2_file_saving_zero_points_is_corrupt() {
        let zero =
            "(version: 2, volume_percent: 60, window_mode: Windowed, points_to_win: Picked(0))";
        assert!(matches!(
            decode_settings(zero),
            Err(DecodeError::Corrupt(Corruption::Invalid(
                InvalidSaveData::NoPointsToWin
            )))
        ));
    }

    /// Plan 7.2's newer-version policy needs this told apart from corruption.
    #[test]
    fn a_file_from_a_newer_game_is_newer_not_corrupt_whatever_else_it_holds() {
        let future = "(version: 99, rooms: [\"crypt\"], profiles: 7)";
        assert!(matches!(
            decode_profiles(future),
            Err(DecodeError::NewerVersion { version: 99 })
        ));
        assert!(matches!(
            decode_settings(future),
            Err(DecodeError::NewerVersion { version: 99 })
        ));
    }

    #[test]
    fn a_version_never_released_is_corrupt() {
        assert!(matches!(
            decode_settings("(version: 0)"),
            Err(DecodeError::Corrupt(Corruption::UnknownVersion {
                version: 0
            }))
        ));
    }

    #[test]
    fn text_that_is_not_a_document_is_corrupt() {
        for text in ["", "not ron at all", "(profiles: [])", "[1, 2, 3]"] {
            assert!(
                matches!(
                    decode_profiles(text),
                    Err(DecodeError::Corrupt(Corruption::Syntax(_)))
                ),
                "{text:?}"
            );
        }
    }

    /// A released shape is exact: no serde defaults, no extra fields.
    #[test]
    fn a_version_1_file_with_a_field_version_1_never_had_is_corrupt() {
        let extra = "(version: 1, volume_percent: 80, window_mode: Windowed, theme: Dark)";
        assert!(matches!(
            decode_settings(extra),
            Err(DecodeError::Corrupt(Corruption::Syntax(_)))
        ));
        let missing = "(version: 1, volume_percent: 80)";
        assert!(matches!(
            decode_settings(missing),
            Err(DecodeError::Corrupt(Corruption::Syntax(_)))
        ));
    }

    /// Docs/architecture.md error table: invalid save data is corrupt.
    #[test]
    fn a_well_formed_file_holding_a_value_its_model_refuses_is_corrupt() {
        let long_name = "(version: 1, profiles: [(id: 1, name: \"Thirteen char\", \
            color: (red: 1.0, green: 0.0, blue: 0.0), \
            records: (wins: 0, losses: 0, longest_rally: 0))])";
        assert!(matches!(
            decode_profiles(long_name),
            Err(DecodeError::Corrupt(Corruption::Invalid(
                InvalidSaveData::NameTooLong { .. }
            )))
        ));
        assert!(matches!(
            decode_settings("(version: 1, volume_percent: 101, window_mode: Windowed)"),
            Err(DecodeError::Corrupt(Corruption::Invalid(
                InvalidSaveData::VolumeAboveFull { percent: 101 }
            )))
        ));
    }

    /// Spec 1.4.8: profiles store the colour, not a palette index.
    #[test]
    fn a_profile_keeps_its_colour_after_the_palette_is_reordered() {
        let first = "(red: 1.00, green: 0.30, blue: 0.25),";
        let third = "(red: 1.00, green: 0.90, blue: 0.20),";
        let tuning = Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid");
        let picked = tuning.palette()[2];
        let saved = encode_profiles(
            &Profiles::new(vec![profile(1, "Mia", picked, RecordsByMode::default())])
                .expect("one profile"),
        )
        .expect("profiles encode");

        let swapped = EMBEDDED_RON
            .replace(first, "FIRST")
            .replace(third, first)
            .replace("FIRST", third);
        let reordered = Tuning::from_ron(&swapped).expect("a reordered palette is valid");
        assert_ne!(
            reordered.palette()[2],
            picked,
            "the palette was not reordered"
        );

        let loaded = decode_profiles(&saved).expect("the saved profiles decode");
        assert_eq!(loaded.as_slice()[0].color(), picked);
        assert_eq!(reordered.palette()[0], picked);
    }
}

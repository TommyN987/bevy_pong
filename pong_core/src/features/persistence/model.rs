//! The save models as the game holds them (spec 1.4.9). Constructors check
//! every value, so a model in hand is valid; a file that would build an
//! invalid one is corrupt (docs/architecture.md, error table).

use std::collections::HashSet;

use bevy::color::{Alpha, Srgba};
use bevy::ecs::resource::Resource;

use crate::domain::ProfileId;

/// A save value a model refuses.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InvalidSaveData {
    /// A profile name with no characters.
    #[error("a profile name must have at least one character")]
    EmptyName,
    /// A profile name longer than [`ProfileName::MAX_CHARS`].
    #[error("profile name \"{name}\" has {chars} characters; the most is 12")]
    NameTooLong {
        /// The name refused.
        name: String,
        /// Its length in characters.
        chars: usize,
    },
    /// A profile colour channel that is negative, NaN, or infinite.
    #[error("profile colour {channel} is {value}; it must be finite and not negative")]
    ColorChannel {
        /// `red`, `green`, or `blue`.
        channel: &'static str,
        /// The value refused.
        value: f32,
    },
    /// Two profiles with one id.
    #[error("profile id {id} is used twice")]
    DuplicateProfileId {
        /// The id seen twice.
        id: u32,
    },
    /// A new profile after one that holds the highest id there is.
    #[error("every profile id is taken")]
    NoProfileIdLeft,
    /// A master volume above full.
    #[error("volume {percent}% is above 100%")]
    VolumeAboveFull {
        /// The percent refused.
        percent: u8,
    },
}

/// A child's chosen name: one to twelve characters (spec 1.6.3), counted as
/// characters, not bytes. Two profiles may share a name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileName(String);

impl ProfileName {
    /// The longest name, in characters.
    pub const MAX_CHARS: usize = 12;

    /// `name`, if it has one to [`Self::MAX_CHARS`] characters.
    ///
    /// # Errors
    /// [`InvalidSaveData::EmptyName`] or [`InvalidSaveData::NameTooLong`].
    pub fn new(name: &str) -> Result<Self, InvalidSaveData> {
        let chars = name.chars().count();
        if chars == 0 {
            Err(InvalidSaveData::EmptyName)
        } else if chars > Self::MAX_CHARS {
            Err(InvalidSaveData::NameTooLong {
                name: name.to_owned(),
                chars,
            })
        } else {
            Ok(Self(name.to_owned()))
        }
    }

    /// The name as typed.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What a profile has won, lost, and rallied (spec 1.4.9). Unsigned, so a
/// negative count cannot exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Records {
    /// Matches won.
    pub wins: u32,
    /// Matches lost.
    pub losses: u32,
    /// Most paddle hits in one rally.
    pub longest_rally: u32,
}

impl Records {
    /// One more match won; stays at the maximum rather than wrapping.
    pub const fn add_win(&mut self) {
        self.wins = self.wins.saturating_add(1);
    }

    /// One more match lost; stays at the maximum rather than wrapping.
    pub const fn add_loss(&mut self) {
        self.losses = self.losses.saturating_add(1);
    }

    /// Keeps the longer of the recorded rally and `hits`.
    pub const fn record_rally(&mut self, hits: u32) {
        if hits > self.longest_rally {
            self.longest_rally = hits;
        }
    }
}

/// One child's profile. The colour is the value the child picked, not a
/// palette index, so reordering the palette leaves it alone (spec 1.4.8).
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    id: ProfileId,
    name: ProfileName,
    color: Srgba,
    records: Records,
}

impl Profile {
    /// A profile, if `color`'s channels are finite and not negative, as the
    /// palette's are (spec 1.4.8). Alpha is not kept: profiles are opaque.
    ///
    /// # Errors
    /// [`InvalidSaveData::ColorChannel`] naming the first bad channel.
    pub fn new(
        id: ProfileId,
        name: ProfileName,
        color: Srgba,
        records: Records,
    ) -> Result<Self, InvalidSaveData> {
        for (channel, value) in [
            ("red", color.red),
            ("green", color.green),
            ("blue", color.blue),
        ] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(InvalidSaveData::ColorChannel { channel, value });
            }
        }
        Ok(Self {
            id,
            name,
            color: color.with_alpha(1.0),
            records,
        })
    }

    /// Its identity.
    #[must_use]
    pub const fn id(&self) -> ProfileId {
        self.id
    }

    /// Its name.
    #[must_use]
    pub const fn name(&self) -> &ProfileName {
        &self.name
    }

    /// Its colour, opaque.
    #[must_use]
    pub const fn color(&self) -> Srgba {
        self.color
    }

    /// Its records.
    #[must_use]
    pub const fn records(&self) -> Records {
        self.records
    }
}

/// Every profile on this device, in the order they were made. Invariant: no
/// two share an id. A resource from `Boot` on (plan 7.2).
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct Profiles(Vec<Profile>);

impl Profiles {
    /// `profiles`, if no two share an id.
    ///
    /// # Errors
    /// [`InvalidSaveData::DuplicateProfileId`] naming the first id seen twice.
    pub fn new(profiles: Vec<Profile>) -> Result<Self, InvalidSaveData> {
        let mut seen = HashSet::new();
        if let Some(twice) = profiles.iter().find(|profile| !seen.insert(profile.id)) {
            return Err(InvalidSaveData::DuplicateProfileId { id: twice.id.0 });
        }
        Ok(Self(profiles))
    }

    /// The profiles, in order.
    #[must_use]
    pub fn as_slice(&self) -> &[Profile] {
        &self.0
    }

    /// Adds a profile named `name` in `color` with no records, last in the
    /// order, and returns its id: one above the highest id held, or 1 for
    /// the first profile.
    ///
    /// # Errors
    /// [`InvalidSaveData::ColorChannel`] as [`Profile::new`], or
    /// [`InvalidSaveData::NoProfileIdLeft`]; nothing is added then.
    pub fn create(
        &mut self,
        name: ProfileName,
        color: Srgba,
    ) -> Result<ProfileId, InvalidSaveData> {
        let id = match self.0.iter().map(Profile::id).max() {
            None => ProfileId(1),
            Some(ProfileId(highest)) => ProfileId(
                highest
                    .checked_add(1)
                    .ok_or(InvalidSaveData::NoProfileIdLeft)?,
            ),
        };
        self.0
            .push(Profile::new(id, name, color, Records::default())?);
        Ok(id)
    }

    /// Names the profile with `id` `name` and returns its old name; `None`,
    /// changing nothing, if no profile has `id`. Its id, colour, records, and
    /// place stay.
    pub fn rename(&mut self, id: ProfileId, name: ProfileName) -> Option<ProfileName> {
        let profile = self.0.iter_mut().find(|profile| profile.id == id)?;
        Some(std::mem::replace(&mut profile.name, name))
    }

    /// Takes the profile with `id` out and hands it back; `None`, changing
    /// nothing, if no profile has `id`. The others keep their order. The id
    /// is free again: [`create`](Self::create) reuses it if it was the
    /// highest, which nothing outside this list notices in M1.
    pub fn remove(&mut self, id: ProfileId) -> Option<Profile> {
        let index = self.0.iter().position(|profile| profile.id == id)?;
        Some(self.0.remove(index))
    }

    /// The records of the profile with `id`, to change; `None` if no profile
    /// has it. Ids cannot change through this, so they stay unique.
    pub fn records_mut(&mut self, id: ProfileId) -> Option<&mut Records> {
        self.0
            .iter_mut()
            .find(|profile| profile.id == id)
            .map(|profile| &mut profile.records)
    }
}

/// Master volume in whole percent, `0..=100`. The settings screen steps it
/// by ten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Volume(u8);

impl Volume {
    /// `percent`, if it is at most 100.
    ///
    /// # Errors
    /// [`InvalidSaveData::VolumeAboveFull`].
    pub const fn new(percent: u8) -> Result<Self, InvalidSaveData> {
        if percent > 100 {
            Err(InvalidSaveData::VolumeAboveFull { percent })
        } else {
            Ok(Self(percent))
        }
    }

    /// The volume in whole percent.
    #[must_use]
    pub const fn percent(self) -> u8 {
        self.0
    }
}

/// How the window shows the game (spec 1.6.6). The core's own type: the
/// core never depends on Bevy's window crate; the shell maps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMode {
    /// The whole screen, no border; the default on first start.
    BorderlessFullscreen,
    /// A window on the desktop.
    Windowed,
}

/// This device's settings (spec 1.4.9): master volume and window mode only.
/// A resource from `Boot` on (plan 7.2).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// Master volume.
    pub volume: Volume,
    /// How the window shows the game.
    pub window_mode: WindowMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: Volume(80),
            window_mode: WindowMode::BorderlessFullscreen,
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::color::Srgba;

    use super::{
        InvalidSaveData, Profile, ProfileId, ProfileName, Profiles, Records, Settings, Volume,
        WindowMode,
    };

    fn profile(id: u32, color: Srgba) -> Result<Profile, InvalidSaveData> {
        Profile::new(
            ProfileId(id),
            ProfileName::new("Mia").expect("a valid name"),
            color,
            Records::default(),
        )
    }

    #[test]
    fn a_name_of_one_to_twelve_characters_is_kept_as_typed() {
        for name in ["A", "Twelve chars"] {
            let kept = ProfileName::new(name).expect("a name in range");
            assert_eq!(kept.as_str(), name);
        }
    }

    #[test]
    fn a_thirteenth_character_is_refused() {
        assert_eq!(
            ProfileName::new("Thirteen char"),
            Err(InvalidSaveData::NameTooLong {
                name: "Thirteen char".to_owned(),
                chars: 13
            })
        );
    }

    #[test]
    fn an_empty_name_is_refused() {
        assert_eq!(ProfileName::new(""), Err(InvalidSaveData::EmptyName));
    }

    /// "Zoë Ångström" is twelve characters but fourteen bytes.
    #[test]
    fn length_is_counted_in_characters_not_bytes() {
        assert!(ProfileName::new("Zoë Ångström").is_ok());
    }

    #[test]
    fn volume_is_kept_from_silent_to_full() {
        for percent in [0, 80, 100] {
            assert_eq!(Volume::new(percent).map(Volume::percent), Ok(percent));
        }
    }

    #[test]
    fn volume_above_full_is_refused() {
        assert_eq!(
            Volume::new(101),
            Err(InvalidSaveData::VolumeAboveFull { percent: 101 })
        );
    }

    #[test]
    fn a_profile_keeps_the_colour_it_was_given_opaque() {
        let picked = Srgba::new(1.0, 0.3, 0.25, 0.5);
        let kept = profile(1, picked).expect("a valid colour");
        assert_eq!(kept.color(), Srgba::new(1.0, 0.3, 0.25, 1.0));
    }

    #[test]
    fn a_colour_channel_that_is_negative_or_not_finite_is_refused() {
        assert_eq!(
            profile(1, Srgba::new(1.0, -0.1, 0.0, 1.0)),
            Err(InvalidSaveData::ColorChannel {
                channel: "green",
                value: -0.1
            })
        );
        assert!(matches!(
            profile(1, Srgba::new(0.0, 0.0, f32::NAN, 1.0)),
            Err(InvalidSaveData::ColorChannel {
                channel: "blue",
                ..
            })
        ));
    }

    #[test]
    fn profiles_keep_their_order() {
        let red = profile(2, Srgba::RED).expect("valid");
        let blue = profile(1, Srgba::BLUE).expect("valid");
        let profiles = Profiles::new(vec![red.clone(), blue.clone()]).expect("distinct ids");
        assert_eq!(profiles.as_slice(), [red, blue]);
    }

    #[test]
    fn two_profiles_with_one_id_are_refused() {
        let first = profile(3, Srgba::RED).expect("valid");
        let second = profile(3, Srgba::BLUE).expect("valid");
        assert_eq!(
            Profiles::new(vec![first, second]),
            Err(InvalidSaveData::DuplicateProfileId { id: 3 })
        );
    }

    fn named(name: &str) -> ProfileName {
        ProfileName::new(name).expect("a valid name")
    }

    #[test]
    fn the_first_profile_created_gets_id_one_with_no_records() {
        let mut profiles = Profiles::default();
        let id = profiles.create(named("Zoë"), Srgba::RED).expect("created");
        assert_eq!(id, ProfileId(1));
        let [zoe] = profiles.as_slice() else {
            panic!("expected one profile, got {profiles:?}");
        };
        assert_eq!(
            (zoe.id(), zoe.name().as_str(), zoe.color(), zoe.records()),
            (ProfileId(1), "Zoë", Srgba::RED, Records::default())
        );
    }

    /// Ids are never handed out twice while their profile exists, gaps or not.
    #[test]
    fn a_created_profile_gets_the_id_after_the_highest_and_goes_last() {
        let mut profiles = Profiles::new(vec![
            profile(5, Srgba::RED).expect("valid"),
            profile(2, Srgba::BLUE).expect("valid"),
        ])
        .expect("distinct ids");
        let id = profiles
            .create(named("Max"), Srgba::GREEN)
            .expect("created");
        assert_eq!(id, ProfileId(6));
        let ids: Vec<ProfileId> = profiles.as_slice().iter().map(Profile::id).collect();
        assert_eq!(ids, [ProfileId(5), ProfileId(2), ProfileId(6)]);
    }

    #[test]
    fn creating_with_a_bad_colour_is_refused_and_adds_nothing() {
        let mut profiles = Profiles::default();
        let refused = profiles.create(named("Mia"), Srgba::new(f32::INFINITY, 0.0, 0.0, 1.0));
        assert!(matches!(
            refused,
            Err(InvalidSaveData::ColorChannel { channel: "red", .. })
        ));
        assert_eq!(profiles, Profiles::default());
    }

    #[test]
    fn creating_after_the_highest_possible_id_is_refused_and_adds_nothing() {
        let last = profile(u32::MAX, Srgba::RED).expect("valid");
        let mut profiles = Profiles::new(vec![last]).expect("one profile");
        let before = profiles.clone();
        assert_eq!(
            profiles.create(named("Mia"), Srgba::BLUE),
            Err(InvalidSaveData::NoProfileIdLeft)
        );
        assert_eq!(profiles, before);
    }

    fn mia_and_max() -> Profiles {
        let mut profiles = Profiles::default();
        profiles.create(named("Mia"), Srgba::RED).expect("created");
        profiles.create(named("Max"), Srgba::BLUE).expect("created");
        profiles
    }

    /// Spec 1.6.3 SHOULD: rename a profile; its id, colour, and records stay.
    #[test]
    fn a_renamed_profile_keeps_its_id_colour_records_and_place() {
        let mut profiles = mia_and_max();
        profiles
            .records_mut(ProfileId(1))
            .expect("Mia exists")
            .add_win();
        assert_eq!(
            profiles.rename(ProfileId(1), named("Zoë")),
            Some(named("Mia"))
        );
        let [zoe, max] = profiles.as_slice() else {
            panic!("expected two profiles, got {profiles:?}");
        };
        assert_eq!(
            (
                zoe.id(),
                zoe.name().as_str(),
                zoe.color(),
                zoe.records().wins
            ),
            (ProfileId(1), "Zoë", Srgba::RED, 1)
        );
        assert_eq!(max.name().as_str(), "Max");
    }

    /// Spec 1.6.3 SHOULD: delete a profile; the others keep their order.
    #[test]
    fn a_removed_profile_is_handed_back_and_the_rest_keep_their_order() {
        let mut profiles = mia_and_max();
        profiles
            .create(named("Ida"), Srgba::GREEN)
            .expect("created");
        let removed = profiles.remove(ProfileId(2)).expect("Max exists");
        assert_eq!(removed.name().as_str(), "Max");
        let ids: Vec<ProfileId> = profiles.as_slice().iter().map(Profile::id).collect();
        assert_eq!(ids, [ProfileId(1), ProfileId(3)]);
    }

    #[test]
    fn an_unknown_id_renames_and_removes_nothing() {
        let mut profiles = mia_and_max();
        let before = profiles.clone();
        assert_eq!(profiles.rename(ProfileId(9), named("Zoë")), None);
        assert_eq!(profiles.remove(ProfileId(9)), None);
        assert_eq!(profiles, before);
    }

    /// Spec 1.6.6: the first start is fullscreen.
    #[test]
    fn first_start_settings_are_borderless_fullscreen_at_eighty_percent() {
        let settings = Settings::default();
        assert_eq!(settings.window_mode, WindowMode::BorderlessFullscreen);
        assert_eq!(settings.volume.percent(), 80);
    }

    #[test]
    fn a_profiles_records_change_through_its_id_and_an_unknown_id_finds_nothing() {
        let mut profiles = Profiles::new(vec![
            profile(1, Srgba::RED).expect("valid"),
            profile(2, Srgba::BLUE).expect("valid"),
        ])
        .expect("distinct ids");
        profiles
            .records_mut(ProfileId(2))
            .expect("profile 2 exists")
            .add_win();
        assert_eq!(profiles.as_slice()[1].records().wins, 1);
        assert_eq!(profiles.as_slice()[0].records(), Records::default());
        assert!(profiles.records_mut(ProfileId(9)).is_none());
    }

    #[test]
    fn wins_and_losses_count_up_and_stop_at_the_maximum() {
        let mut records = Records::default();
        records.add_win();
        records.add_loss();
        records.add_loss();
        assert_eq!((records.wins, records.losses), (1, 2));
        let mut full = Records {
            wins: u32::MAX,
            losses: u32::MAX,
            longest_rally: 0,
        };
        full.add_win();
        full.add_loss();
        assert_eq!((full.wins, full.losses), (u32::MAX, u32::MAX));
    }

    #[test]
    fn a_rally_record_keeps_the_longer_rally() {
        let mut records = Records::default();
        records.record_rally(5);
        records.record_rally(3);
        assert_eq!(records.longest_rally, 5);
        records.record_rally(8);
        assert_eq!(records.longest_rally, 8);
    }
}

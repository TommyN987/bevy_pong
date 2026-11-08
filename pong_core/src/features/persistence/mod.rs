//! Persistence (spec 1.4.9, docs/architecture.md "Persistence"): the profiles
//! and settings the game keeps between runs, their on-disk documents, and the
//! file IO the core performs through the shell's [`io::SavePaths`] port.
//!
//! - [`model`]: what the game holds in memory; every value valid by construction.
//! - [`document`]: what is on disk; each released version's shape is frozen.
//! - [`io`]: where files live and how they are written without tearing.

use std::time::{SystemTime, UNIX_EPOCH};

use std::path::Path;

use bevy::app::{App, Plugin, Update};
use bevy::ecs::error::BevyError;
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::schedule::common_conditions::resource_exists;
use bevy::ecs::system::{Commands, Res, ResMut};
use bevy::state::state::OnEnter;

use self::document::{
    EncodeError, decode_profiles, decode_settings, encode_profiles, encode_settings,
};
use self::io::{SavePaths, write_atomically};
use self::load::{LoadError, Loaded, load};
use self::model::{Profiles, Settings};
use crate::domain::{MatchConfig, MatchEnded, Notice, Player};
use crate::flow::states::AppState;

pub mod document;
pub mod io;
pub mod load;
pub mod model;

/// Loads the settings and profiles on entering `Boot` and writes them when
/// a match ends (spec 1.4.9).
///
/// Load policy, per file: a missing file starts fresh; a file that cannot be
/// used is set aside as `<name>.<seconds>.bak` and the game starts fresh with
/// one [`Notice`]; if it cannot even be moved, the game starts fresh and never
/// writes that file this session. Without [`SavePaths`] nothing is written.
///
/// Precondition: the shell inserts [`SavePaths`] before the app runs, or
/// not at all.
pub struct PersistencePlugin;

impl Plugin for PersistencePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Boot), load_saves)
            .add_systems(
                Update,
                (record_match_result, write_saves_at_match_end)
                    .chain()
                    .run_if(resource_exists::<SaveState>),
            );
    }
}

/// Which save file, for the player's sentences.
#[derive(Debug, Clone, Copy)]
enum SaveFile {
    Profiles,
    Settings,
}

impl SaveFile {
    const fn label(self) -> &'static str {
        match self {
            Self::Profiles => "Saved profiles",
            Self::Settings => "Saved settings",
        }
    }
}

/// What the game may still do with each save file this session.
#[derive(Resource, Debug)]
struct SaveState {
    profiles: FileState,
    settings: FileState,
    /// Whether this session has told the player a write failed (spec 1.4.9:
    /// one `Notice` per session).
    write_failure_announced: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum FileState {
    /// The game may write the file; `last_written` is what the file holds as
    /// far as the game knows, so an unchanged model is not written again.
    Writable { last_written: String },
    /// No `SavePaths`, or an unusable file that could not be moved aside.
    Unwritable,
}

impl FileState {
    /// Writable, as if `encoded` were already on disk. A model that cannot be
    /// encoded starts from nothing, so the next match end tries to write it.
    fn writable(encoded: Result<String, EncodeError>) -> Self {
        Self::Writable {
            last_written: encoded.unwrap_or_default(),
        }
    }
}

impl FileState {
    /// Writes `encoded` to `path` if this file is writable and it differs
    /// from what was last written. Returns whether a write was due and failed;
    /// a failure leaves `last_written` alone, so the next call retries.
    fn write_if_changed(&mut self, path: &Path, encoded: Result<String, EncodeError>) -> bool {
        let Self::Writable { last_written } = self else {
            return false;
        };
        let Ok(text) = encoded else {
            return true;
        };
        if *last_written == text {
            return false;
        }
        if write_atomically(path, text.as_bytes()).is_err() {
            return true;
        }
        *last_written = text;
        false
    }
}

const COULD_NOT_SAVE: &str = "The game could not save. It will try again after the next match.";

const NOWHERE_TO_SAVE: &str =
    "There is nowhere to save on this computer, so nothing will be kept after the game closes.";

/// The sentence for a file the game could not use.
fn unusable_notice(file: SaveFile, reason: &LoadError, set_aside: bool) -> Notice {
    let what = match reason {
        LoadError::NewerVersion { .. } => "come from a newer version of the game",
        LoadError::Corrupt(_) | LoadError::Io(_) => "could not be read",
    };
    let then = if set_aside {
        "so the game starts fresh. The old file was kept."
    } else {
        "so the game starts fresh and will not save over them."
    };
    Notice {
        text: format!("{} {what}, {then}", file.label()),
    }
}

/// The model `loaded` gives, and what may be written later, telling the
/// player through `notices` when a file could not be used.
fn settle<M: Default>(
    loaded: Loaded<M>,
    file: SaveFile,
    encode: impl Fn(&M) -> Result<String, EncodeError>,
    notices: &mut MessageWriter<Notice>,
) -> (M, FileState) {
    match loaded {
        Loaded::Read(model) => {
            let state = FileState::writable(encode(&model));
            (model, state)
        }
        Loaded::Absent => {
            let model = M::default();
            let state = FileState::writable(encode(&model));
            (model, state)
        }
        Loaded::SetAside { reason, .. } => {
            notices.write(unusable_notice(file, &reason, true));
            let model = M::default();
            let state = FileState::writable(encode(&model));
            (model, state)
        }
        Loaded::Stuck { reason, .. } => {
            notices.write(unusable_notice(file, &reason, false));
            (M::default(), FileState::Unwritable)
        }
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Reads both files once, on entering `Boot`; the Boot gate waits for the
/// resources this inserts (spec 1.4.10).
fn load_saves(
    mut commands: Commands,
    paths: Option<Res<SavePaths>>,
    mut notices: MessageWriter<Notice>,
) {
    let Some(paths) = paths else {
        notices.write(Notice {
            text: NOWHERE_TO_SAVE.to_owned(),
        });
        commands.insert_resource(Profiles::default());
        commands.insert_resource(Settings::default());
        commands.insert_resource(SaveState {
            profiles: FileState::Unwritable,
            settings: FileState::Unwritable,
            write_failure_announced: false,
        });
        return;
    };
    let seconds = unix_seconds();
    let (profiles, profiles_state) = settle(
        load(&paths.profiles_file(), decode_profiles, seconds),
        SaveFile::Profiles,
        encode_profiles,
        &mut notices,
    );
    let (settings, settings_state) = settle(
        load(&paths.settings_file(), decode_settings, seconds),
        SaveFile::Settings,
        |settings: &Settings| encode_settings(*settings),
        &mut notices,
    );
    commands.insert_resource(profiles);
    commands.insert_resource(settings);
    commands.insert_resource(SaveState {
        profiles: profiles_state,
        settings: settings_state,
        write_failure_announced: false,
    });
}

/// Credits a finished match to the profiles that played it (spec 1.4.9): a
/// win to the winning side's player, a loss to the losing side's. A Guest or
/// a computer side records nothing; one profile on both seats records
/// nothing at all (decided 2026-11-08). Runs before the match-end write, so
/// the file holds the result.
///
/// # Errors
/// A `MatchEnded` while no [`MatchConfig`] exists, or one naming a profile
/// that [`Profiles`] lacks: the menu built the match wrong. Nothing is
/// recorded then.
fn record_match_result(
    mut ended: MessageReader<MatchEnded>,
    config: Option<Res<MatchConfig>>,
    mut profiles: ResMut<Profiles>,
) -> Result<(), BevyError> {
    let Some(&MatchEnded { winner }) = ended.read().last() else {
        return Ok(());
    };
    let Some(config) = config else {
        return Err(BevyError::from("a match ended with no MatchConfig"));
    };
    let profile_on = |side| match config.player_on(side) {
        Some(Player::Profile(id)) => Some(id),
        Some(Player::Guest) | None => None,
    };
    let (winner_id, loser_id) = (profile_on(winner), profile_on(winner.opposite()));
    if winner_id.is_some() && winner_id == loser_id {
        return Ok(());
    }
    if let Some(unknown) = [winner_id, loser_id]
        .into_iter()
        .flatten()
        .find(|&id| profiles.records_mut(id).is_none())
    {
        return Err(BevyError::from(format!(
            "the match named {unknown:?}, which no profile has"
        )));
    }
    if let Some(records) = winner_id.and_then(|id| profiles.records_mut(id)) {
        records.add_win();
    }
    if let Some(records) = loser_id.and_then(|id| profiles.records_mut(id)) {
        records.add_loss();
    }
    Ok(())
}

/// Writes each writable file whose model changed since its last write, once
/// per [`MatchEnded`] (spec 1.4.9: records are written at match end only). A
/// failure keeps the model in memory and is retried at the next match end;
/// the first failure of the session sends one [`Notice`].
fn write_saves_at_match_end(
    mut ended: MessageReader<MatchEnded>,
    paths: Option<Res<SavePaths>>,
    profiles: Res<Profiles>,
    settings: Res<Settings>,
    mut state: ResMut<SaveState>,
    mut notices: MessageWriter<Notice>,
) {
    if ended.read().count() == 0 {
        return;
    }
    let Some(paths) = paths else {
        return;
    };
    let state = &mut *state;
    let profiles_failed = state
        .profiles
        .write_if_changed(&paths.profiles_file(), encode_profiles(&profiles));
    let settings_failed = state
        .settings
        .write_if_changed(&paths.settings_file(), encode_settings(*settings));
    if (profiles_failed || settings_failed) && !state.write_failure_announced {
        state.write_failure_announced = true;
        notices.write(Notice {
            text: COULD_NOT_SAVE.to_owned(),
        });
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    use bevy::app::App;
    use bevy::color::Srgba;
    use bevy::math::Vec2;

    use super::document::{decode_profiles, decode_settings, encode_profiles};
    use super::io::SavePaths;
    use super::model::{Profile, ProfileName, Profiles, Records, Settings, Volume, WindowMode};
    use crate::domain::{
        Controller, Difficulty, MatchConfig, Notice, Player, ProfileId, Seat, SeatPlayers,
    };
    use crate::features::tuning::Tuning;
    use crate::flow::states::{AppState, MatchState};
    use crate::kernel::geometry::Side;
    use crate::testing::{
        app_state, boot, frame, headless_app, leave_match, match_state, place_ball, record,
        recorded, run_until, start_match, take_recorded,
    };

    const COULD_NOT_SAVE: &str = "The game could not save. It will try again after the next match.";

    fn set_read_only(dir: &Path, read_only: bool) {
        let mode = if read_only { 0o555 } else { 0o755 };
        std::fs::create_dir_all(dir).expect("writable");
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode))
            .expect("the test owns the directory");
    }

    /// One match of two guests to a single point, from the menu back to it.
    fn play_a_match_to_its_end(app: &mut App) {
        left_side_wins(
            app,
            Controller::Human(Seat::Left),
            Controller::Human(Seat::Right),
            SeatPlayers::GUESTS,
        );
    }

    /// One match to a single point that the left side wins, from the menu
    /// back to the menu.
    fn left_side_wins(app: &mut App, left: Controller, right: Controller, players: SeatPlayers) {
        start_match(
            app,
            MatchConfig {
                left,
                right,
                players,
                points_to_win: NonZeroU8::MIN,
            },
        );
        let goal = app.world().resource::<Tuning>().arena().goal_x(Side::Right);
        place_ball(app, Vec2::new(goal + 2.0, 0.0), Vec2::new(60.0, 0.0));
        run_until(
            app,
            |app| match_state(app) == MatchState::MatchOver,
            10,
            "the match never ended",
        );
        leave_match(app);
    }

    fn windowed_at_thirty() -> Settings {
        Settings {
            volume: Volume::new(30).expect("in range"),
            window_mode: WindowMode::Windowed,
        }
    }

    fn fixture(name: &str) -> String {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/load_policy")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
    }

    fn save_paths(app: &App) -> SavePaths {
        app.world().resource::<SavePaths>().clone()
    }

    /// Writes `text` where the app will look for the file `pick` names.
    fn place(app: &App, pick: impl Fn(&SavePaths) -> PathBuf, text: &str) -> PathBuf {
        let file = pick(&save_paths(app));
        std::fs::create_dir_all(file.parent().expect("a parent")).expect("writable");
        std::fs::write(&file, text).expect("writable");
        file
    }

    fn notices(app: &App) -> Vec<String> {
        recorded::<Notice>(app)
            .iter()
            .map(|n| n.text.clone())
            .collect()
    }

    fn booted() -> App {
        let app = headless_app(1);
        boot_app(app)
    }

    fn boot_app(mut app: App) -> App {
        record::<Notice>(&mut app);
        boot(&mut app);
        // One more frame, so a fixed step has recorded any startup notice.
        frame(&mut app);
        app
    }

    #[test]
    fn a_first_start_boots_into_the_menu_with_default_saves_and_no_notice() {
        let app = booted();
        assert_eq!(app_state(&app), AppState::Menu);
        assert_eq!(*app.world().resource::<Profiles>(), Profiles::default());
        assert_eq!(*app.world().resource::<Settings>(), Settings::default());
        assert_eq!(recorded::<Notice>(&app), &[]);
    }

    /// Spec 1.4.9 load policy, corrupt fixture.
    #[test]
    fn corrupt_profiles_are_set_aside_and_the_game_starts_fresh_with_one_notice() {
        let app = headless_app(1);
        let corrupt = fixture("corrupt.ron");
        let file = place(&app, SavePaths::profiles_file, &corrupt);
        let app = boot_app(app);
        assert_eq!(app_state(&app), AppState::Menu);
        assert_eq!(*app.world().resource::<Profiles>(), Profiles::default());
        assert_eq!(
            notices(&app),
            ["Saved profiles could not be read, so the game starts fresh. The old file was kept."]
        );
        assert!(!file.exists());
        let backups: Vec<String> = std::fs::read_dir(file.parent().expect("a parent"))
            .expect("the directory exists")
            .map(|entry| {
                std::fs::read_to_string(entry.expect("an entry").path()).expect("readable")
            })
            .collect();
        assert_eq!(backups, [corrupt]);
    }

    /// Spec 1.4.9 load policy, version-99 fixture.
    #[test]
    fn settings_from_a_newer_game_are_set_aside_with_one_notice() {
        let app = headless_app(1);
        place(
            &app,
            SavePaths::settings_file,
            &fixture("newer_version.ron"),
        );
        let app = boot_app(app);
        assert_eq!(*app.world().resource::<Settings>(), Settings::default());
        assert_eq!(
            notices(&app),
            [
                "Saved settings come from a newer version of the game, so the game starts fresh. The old file was kept."
            ]
        );
    }

    #[test]
    fn without_save_paths_the_game_starts_fresh_and_says_nothing_will_be_kept() {
        let mut app = headless_app(1);
        app.world_mut().remove_resource::<SavePaths>();
        let app = boot_app(app);
        assert_eq!(app_state(&app), AppState::Menu);
        assert_eq!(
            notices(&app),
            [
                "There is nowhere to save on this computer, so nothing will be kept after the game closes."
            ]
        );
    }

    /// Spec 1.4.9 SHOULD: records are written at `MatchEnded` only.
    #[test]
    fn a_changed_model_is_written_when_the_match_ends_and_not_during_play() {
        let mut app = booted();
        let paths = save_paths(&app);
        app.insert_resource(windowed_at_thirty());
        for _ in 0..30 {
            frame(&mut app);
        }
        assert!(
            !paths.settings_file().exists(),
            "written before any match ended"
        );

        play_a_match_to_its_end(&mut app);
        let written = std::fs::read_to_string(paths.settings_file()).expect("written at match end");
        assert_eq!(
            decode_settings(&written).expect("decodes"),
            windowed_at_thirty()
        );
        assert!(
            !paths.profiles_file().exists(),
            "unchanged profiles were written"
        );
    }

    /// Spec 1.4.9 write policy, unwritable path.
    #[test]
    fn a_failed_write_keeps_the_model_tells_once_and_retries_at_the_next_match_end() {
        let mut app = booted();
        take_recorded::<Notice>(&mut app);
        let paths = save_paths(&app);
        let dir = paths.settings_file().parent().expect("a parent").to_owned();
        set_read_only(&dir, true);
        app.insert_resource(windowed_at_thirty());

        play_a_match_to_its_end(&mut app);
        assert_eq!(notices(&app), [COULD_NOT_SAVE]);
        assert_eq!(*app.world().resource::<Settings>(), windowed_at_thirty());
        play_a_match_to_its_end(&mut app);
        assert_eq!(notices(&app), [COULD_NOT_SAVE], "told twice in one session");
        assert!(!paths.settings_file().exists());

        set_read_only(&dir, false);
        play_a_match_to_its_end(&mut app);
        let written = std::fs::read_to_string(paths.settings_file()).expect("the retry wrote it");
        assert_eq!(
            decode_settings(&written).expect("decodes"),
            windowed_at_thirty()
        );
    }

    /// "Never overwrite" (docs/architecture.md): a file that could not be
    /// set aside keeps its bytes for the whole session.
    #[test]
    fn a_file_that_could_not_be_set_aside_is_never_written_over() {
        let app = headless_app(1);
        let corrupt = fixture("corrupt.ron");
        let file = place(&app, SavePaths::profiles_file, &corrupt);
        let dir = file.parent().expect("a parent").to_owned();
        set_read_only(&dir, true);
        let mut app = boot_app(app);
        set_read_only(&dir, false);
        assert_eq!(
            notices(&app),
            [
                "Saved profiles could not be read, so the game starts fresh and will not save over them."
            ]
        );

        let mia = Profile::new(
            ProfileId(1),
            ProfileName::new("Mia").expect("valid"),
            Srgba::RED,
            Records::default(),
        )
        .expect("valid");
        app.insert_resource(Profiles::new(vec![mia]).expect("one profile"));
        play_a_match_to_its_end(&mut app);
        assert_eq!(
            std::fs::read_to_string(&file).expect("still there"),
            corrupt
        );
        assert_ne!(
            encode_profiles(app.world().resource::<Profiles>()).expect("encodes"),
            corrupt
        );
    }

    #[test]
    fn without_save_paths_a_match_end_writes_nothing() {
        let mut app = headless_app(1);
        let paths = save_paths(&app);
        app.world_mut().remove_resource::<SavePaths>();
        let mut app = boot_app(app);
        app.insert_resource(windowed_at_thirty());
        play_a_match_to_its_end(&mut app);
        assert!(!paths.settings_file().exists());
        assert!(!paths.profiles_file().exists());
    }

    const ZOE: ProfileId = ProfileId(1);
    const MAX: ProfileId = ProfileId(2);

    /// A booted app whose profiles are Zoë and Max, with no records yet.
    fn zoe_and_max() -> App {
        let mut app = booted();
        let child = |id: ProfileId, name: &str| {
            Profile::new(
                id,
                ProfileName::new(name).expect("valid"),
                Srgba::RED,
                Records::default(),
            )
            .expect("valid")
        };
        app.insert_resource(
            Profiles::new(vec![child(ZOE, "Zoë"), child(MAX, "Max")]).expect("distinct ids"),
        );
        app
    }

    fn records(app: &App, id: ProfileId) -> Records {
        app.world()
            .resource::<Profiles>()
            .as_slice()
            .iter()
            .find(|profile| profile.id() == id)
            .expect("the profile exists")
            .records()
    }

    fn seats(left_seat: Player, right_seat: Player) -> SeatPlayers {
        SeatPlayers {
            left_seat,
            right_seat,
        }
    }

    fn wins_losses(app: &App, id: ProfileId) -> (u32, u32) {
        let records = records(app, id);
        (records.wins, records.losses)
    }

    /// Spec 1.4.9: a Guest side records nothing; the other side records
    /// normally, and the file holds it.
    #[test]
    fn a_guest_side_records_nothing_and_the_other_side_records_normally() {
        let mut app = zoe_and_max();
        let humans = (
            Controller::Human(Seat::Left),
            Controller::Human(Seat::Right),
        );
        left_side_wins(
            &mut app,
            humans.0,
            humans.1,
            seats(Player::Profile(ZOE), Player::Guest),
        );
        left_side_wins(
            &mut app,
            humans.0,
            humans.1,
            seats(Player::Guest, Player::Profile(ZOE)),
        );
        assert_eq!(wins_losses(&app, ZOE), (1, 1));
        assert_eq!(wins_losses(&app, MAX), (0, 0));

        let saved = std::fs::read_to_string(save_paths(&app).profiles_file()).expect("written");
        assert_eq!(
            decode_profiles(&saved).expect("decodes"),
            *app.world().resource::<Profiles>()
        );
    }

    #[test]
    fn the_winners_profile_gains_a_win_and_the_losers_a_loss() {
        let mut app = zoe_and_max();
        left_side_wins(
            &mut app,
            Controller::Human(Seat::Left),
            Controller::Human(Seat::Right),
            seats(Player::Profile(ZOE), Player::Profile(MAX)),
        );
        assert_eq!(wins_losses(&app, ZOE), (1, 0));
        assert_eq!(wins_losses(&app, MAX), (0, 1));
    }

    /// The side a seat plays decides, not the seat's name: here the right
    /// seat's keys play the left side.
    #[test]
    fn against_the_computer_only_the_human_records_through_the_seat_it_plays() {
        let mut app = zoe_and_max();
        left_side_wins(
            &mut app,
            Controller::Ai(Difficulty::ThreeStars),
            Controller::Human(Seat::Left),
            seats(Player::Profile(MAX), Player::Profile(ZOE)),
        );
        assert_eq!(wins_losses(&app, MAX), (0, 1));
        assert_eq!(wins_losses(&app, ZOE), (0, 0));
    }

    /// Decision 2 (2026-11-08): a child playing themselves moves no record.
    #[test]
    fn one_profile_on_both_seats_records_nothing() {
        let mut app = zoe_and_max();
        left_side_wins(
            &mut app,
            Controller::Human(Seat::Left),
            Controller::Human(Seat::Right),
            seats(Player::Profile(ZOE), Player::Profile(ZOE)),
        );
        assert_eq!(wins_losses(&app, ZOE), (0, 0));
    }

    /// A match naming a profile that does not exist is a menu bug.
    #[test]
    #[should_panic(expected = "ProfileId(9)")]
    fn under_the_dev_policy_a_match_with_an_unknown_profile_panics_at_its_end() {
        let mut app = zoe_and_max();
        left_side_wins(
            &mut app,
            Controller::Human(Seat::Left),
            Controller::Human(Seat::Right),
            seats(Player::Profile(ZOE), Player::Profile(ProfileId(9))),
        );
    }
}

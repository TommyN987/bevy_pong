//! The menus (spec 1.6.3, plan 8.1): from the main menu through the mode,
//! points, and profile picks into a match, and the settings screen.
//! Headless: the core harness plus `InputPlugin`, with keys written as the
//! window backend writes them.

mod common;

use std::num::NonZeroU8;

use bevy::app::{App, AppExit};
use bevy::camera::visibility::Visibility;
use bevy::color::{Color, Srgba};
use bevy::ecs::change_detection::Mut;
use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::input::InputPlugin;
use bevy::input::keyboard::KeyCode;
use bevy::state::state::State;
use bevy::text::EditableText;
use bevy::ui::widget::Text;
use bevy::ui::{AlignItems, BorderColor, Display, Node};
use common::tap;
use pong::ui::menu::name_entry::NameField;
use pong::ui::menu::view::{MenuHeading, MenuRow, MenuScreenRoot, ScrollMark, SeatCue};
use pong::ui::menu::{MenuItem, MenuList, MenuPlugin, MenuScreen, SHOWN_ROWS};
use pong::ui::style::KeyHint;
use pong_core::domain::{
    Controller, Difficulty, MatchConfig, Player, ProfileId, Seat, SeatPlayers,
};
use pong_core::features::persistence::document::{decode_profiles, decode_settings};
use pong_core::features::persistence::io::SavePaths;
use pong_core::features::persistence::model::{
    Profile, ProfileName, Profiles, Records, Settings, Volume, WindowMode,
};
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::AppState;
use pong_core::kernel::geometry::Side;
use pong_core::testing::{app_state, boot, frame, headless_app, record, recorded};

/// Left seat: W up, S down, D confirm, A back. Right seat: the arrows.
const LEFT_DOWN: KeyCode = KeyCode::KeyS;
const LEFT_UP: KeyCode = KeyCode::KeyW;
const LEFT_CONFIRM: KeyCode = KeyCode::KeyD;
const LEFT_BACK: KeyCode = KeyCode::KeyA;
const RIGHT_DOWN: KeyCode = KeyCode::ArrowDown;
const RIGHT_CONFIRM: KeyCode = KeyCode::ArrowRight;

const ZOE: ProfileId = ProfileId(1);
const MAX: ProfileId = ProfileId(2);

fn menu_app() -> App {
    let mut app = headless_app(1);
    app.add_plugins((InputPlugin, MenuPlugin));
    boot(&mut app);
    frame(&mut app);
    app
}

/// The menu with Zoë and Max saved.
fn menu_app_with_zoe_and_max() -> App {
    let mut app = menu_app();
    let profile = |id, name| {
        Profile::new(
            id,
            ProfileName::new(name).expect("valid"),
            Srgba::RED,
            Records::default(),
        )
        .expect("valid")
    };
    app.insert_resource(
        Profiles::new(vec![profile(ZOE, "Zoë"), profile(MAX, "Max")]).expect("distinct ids"),
    );
    app
}

fn screen(app: &App) -> MenuScreen {
    *app.world().resource::<State<MenuScreen>>().get()
}

fn list(app: &App) -> &MenuList {
    app.world().resource::<MenuList>()
}

fn under_cursor(app: &App) -> MenuItem {
    list(app).items()[list(app).cursor()]
}

/// Moves the cursor with `down` until it rests on `item`, then confirms
/// with `confirm`.
fn pick(app: &mut App, item: MenuItem, (down, confirm): (KeyCode, KeyCode)) {
    let rows = list(app).items().len();
    let up = if down == RIGHT_DOWN {
        KeyCode::ArrowUp
    } else {
        LEFT_UP
    };
    for _ in 0..rows {
        tap(app, up);
    }
    for _ in 0..rows {
        if under_cursor(app) == item {
            tap(app, confirm);
            return;
        }
        tap(app, down);
    }
    panic!(
        "{item:?} is not on {:?}: {:?}",
        screen(app),
        list(app).items()
    );
}

const LEFT: (KeyCode, KeyCode) = (LEFT_DOWN, LEFT_CONFIRM);
const RIGHT: (KeyCode, KeyCode) = (RIGHT_DOWN, RIGHT_CONFIRM);

fn points(n: u8) -> NonZeroU8 {
    NonZeroU8::new(n).expect("not zero")
}

fn match_config(app: &App) -> MatchConfig {
    *app.world().resource::<MatchConfig>()
}

#[test]
fn the_game_boots_into_the_main_menu_offering_play_settings_and_quit() {
    let app = menu_app();
    assert_eq!(screen(&app), MenuScreen::Main);
    assert_eq!(
        list(&app).items(),
        [MenuItem::Play, MenuItem::Settings, MenuItem::Quit]
    );
    assert_eq!(list(&app).cursor(), 0);
}

#[test]
fn either_seat_drives_a_shared_screen() {
    let mut app = menu_app();
    tap(&mut app, LEFT_DOWN);
    assert_eq!(under_cursor(&app), MenuItem::Settings);
    tap(&mut app, RIGHT_DOWN);
    assert_eq!(under_cursor(&app), MenuItem::Quit);
}

#[test]
fn the_cursor_stops_at_the_first_and_last_rows() {
    let mut app = menu_app();
    tap(&mut app, LEFT_UP);
    assert_eq!(under_cursor(&app), MenuItem::Play);
    for _ in 0..5 {
        tap(&mut app, LEFT_DOWN);
    }
    assert_eq!(under_cursor(&app), MenuItem::Quit);
}

#[test]
fn quit_ends_the_game() {
    let mut app = menu_app();
    record::<AppExit>(&mut app);
    pick(&mut app, MenuItem::Quit, LEFT);
    frame(&mut app);
    assert_eq!(recorded::<AppExit>(&app), [AppExit::Success]);
}

/// Spec 1.4.5: the menu offers the tuning's list with its default preselected.
#[test]
fn points_to_win_offers_the_tuned_choices_with_the_default_under_the_cursor() {
    let mut app = menu_app();
    pick(&mut app, MenuItem::Play, LEFT);
    assert_eq!(screen(&app), MenuScreen::Opponent);
    assert_eq!(
        list(&app).items(),
        [
            MenuItem::Friend,
            MenuItem::Computer(Difficulty::OneStar),
            MenuItem::Computer(Difficulty::TwoStars),
            MenuItem::Computer(Difficulty::ThreeStars),
        ]
    );
    pick(&mut app, MenuItem::Friend, LEFT);
    assert_eq!(screen(&app), MenuScreen::PointsToWin);
    assert_eq!(
        list(&app).items(),
        [5, 10, 15, 21].map(|n| MenuItem::PointsToWin(points(n)))
    );
    assert_eq!(under_cursor(&app), MenuItem::PointsToWin(points(10)));
}

#[test]
fn two_friends_pick_left_seat_first_then_right_and_the_match_starts() {
    let mut app = menu_app();
    pick(&mut app, MenuItem::Play, LEFT);
    pick(&mut app, MenuItem::Friend, RIGHT);
    pick(&mut app, MenuItem::PointsToWin(points(5)), LEFT);
    assert_eq!(screen(&app), MenuScreen::PickProfile(Seat::Left));
    pick(&mut app, MenuItem::Player(Player::Guest), LEFT);
    assert_eq!(screen(&app), MenuScreen::PickProfile(Seat::Right));
    pick(&mut app, MenuItem::Player(Player::Guest), RIGHT);
    assert_eq!(app_state(&app), AppState::InMatch);
    assert_eq!(
        match_config(&app),
        MatchConfig {
            left: Controller::Human(Seat::Left),
            right: Controller::Human(Seat::Right),
            players: SeatPlayers::GUESTS,
            points_to_win: points(5),
        }
    );
}

/// Spec 1.6.3: during a seat's turn only that seat's keys move the cursor.
#[test]
fn during_a_seats_pick_the_other_seats_keys_do_nothing() {
    let mut app = menu_app_with_zoe_and_max();
    pick(&mut app, MenuItem::Play, LEFT);
    pick(&mut app, MenuItem::Friend, LEFT);
    pick(&mut app, MenuItem::PointsToWin(points(10)), LEFT);
    assert_eq!(screen(&app), MenuScreen::PickProfile(Seat::Left));
    for key in [
        RIGHT_DOWN,
        KeyCode::ArrowUp,
        RIGHT_CONFIRM,
        KeyCode::ArrowLeft,
    ] {
        tap(&mut app, key);
        assert_eq!(list(&app).cursor(), 0, "{key:?}");
        assert_eq!(screen(&app), MenuScreen::PickProfile(Seat::Left), "{key:?}");
    }
}

/// Plan 8.1 (ADR 0008): no profile is offered to both seats; Guest is.
#[test]
fn the_right_seat_is_not_offered_the_left_seats_profile_and_both_get_it_in_the_match() {
    let mut app = menu_app_with_zoe_and_max();
    pick(&mut app, MenuItem::Play, LEFT);
    pick(&mut app, MenuItem::Friend, LEFT);
    pick(&mut app, MenuItem::PointsToWin(points(10)), LEFT);
    assert_eq!(
        list(&app).items(),
        [
            MenuItem::Player(Player::Guest),
            MenuItem::Player(Player::Profile(ZOE)),
            MenuItem::Player(Player::Profile(MAX)),
            MenuItem::NewProfile,
            MenuItem::EditProfiles,
        ]
    );
    pick(&mut app, MenuItem::Player(Player::Profile(ZOE)), LEFT);
    assert_eq!(
        list(&app).items(),
        [
            MenuItem::Player(Player::Guest),
            MenuItem::Player(Player::Profile(MAX)),
            MenuItem::NewProfile,
            MenuItem::EditProfiles,
        ]
    );
    pick(&mut app, MenuItem::Player(Player::Profile(MAX)), RIGHT);
    assert_eq!(
        match_config(&app).players,
        SeatPlayers {
            left_seat: Player::Profile(ZOE),
            right_seat: Player::Profile(MAX),
        }
    );
}

/// Decision 4: the seat that confirms the computer plays it, on its own side,
/// and makes every later pick.
#[test]
fn the_seat_that_confirms_the_computer_plays_it_on_its_side_and_picks_alone() {
    let mut app = menu_app_with_zoe_and_max();
    pick(&mut app, MenuItem::Play, LEFT);
    pick(&mut app, MenuItem::Computer(Difficulty::TwoStars), RIGHT);
    assert_eq!(screen(&app), MenuScreen::PointsToWin);
    tap(&mut app, LEFT_DOWN);
    assert_eq!(under_cursor(&app), MenuItem::PointsToWin(points(10)));
    pick(&mut app, MenuItem::PointsToWin(points(15)), RIGHT);
    assert_eq!(screen(&app), MenuScreen::PickProfile(Seat::Right));
    pick(&mut app, MenuItem::Player(Player::Profile(MAX)), RIGHT);
    assert_eq!(app_state(&app), AppState::InMatch);
    let config = match_config(&app);
    assert_eq!(
        (config.left, config.right, config.points_to_win),
        (
            Controller::Ai(Difficulty::TwoStars),
            Controller::Human(Seat::Right),
            points(15)
        )
    );
    assert_eq!(config.player_on(Side::Right), Some(Player::Profile(MAX)));
}

#[test]
fn back_and_escape_return_to_the_screen_before_and_the_new_path_is_the_one_played() {
    let mut app = menu_app_with_zoe_and_max();
    pick(&mut app, MenuItem::Play, LEFT);
    pick(&mut app, MenuItem::Friend, LEFT);
    pick(&mut app, MenuItem::PointsToWin(points(10)), LEFT);
    pick(&mut app, MenuItem::Player(Player::Profile(ZOE)), LEFT);
    assert_eq!(screen(&app), MenuScreen::PickProfile(Seat::Right));
    tap(&mut app, KeyCode::ArrowLeft);
    assert_eq!(screen(&app), MenuScreen::PickProfile(Seat::Left));
    tap(&mut app, LEFT_BACK);
    assert_eq!(screen(&app), MenuScreen::PointsToWin);
    tap(&mut app, KeyCode::Escape);
    assert_eq!(screen(&app), MenuScreen::Opponent);

    pick(&mut app, MenuItem::Computer(Difficulty::OneStar), RIGHT);
    pick(&mut app, MenuItem::PointsToWin(points(5)), RIGHT);
    pick(&mut app, MenuItem::Player(Player::Profile(MAX)), RIGHT);
    assert_eq!(
        match_config(&app),
        MatchConfig {
            left: Controller::Ai(Difficulty::OneStar),
            right: Controller::Human(Seat::Right),
            // Zoë's pick on the abandoned path does not reach the match.
            players: SeatPlayers {
                left_seat: Player::Guest,
                right_seat: Player::Profile(MAX),
            },
            points_to_win: points(5),
        }
    );
}

#[test]
fn escape_on_the_main_menu_stays_there() {
    let mut app = menu_app();
    tap(&mut app, KeyCode::Escape);
    assert_eq!(screen(&app), MenuScreen::Main);
    assert_eq!(app_state(&app), AppState::Menu);
}

fn saved_settings(app: &App) -> Settings {
    let file = app.world().resource::<SavePaths>().settings_file();
    decode_settings(&std::fs::read_to_string(file).expect("settings written")).expect("decodes")
}

/// Spec 1.6.3: volume changes and persists. Steps of ten, the current one
/// under the cursor.
#[test]
fn a_volume_picked_in_settings_changes_and_is_saved() {
    let mut app = menu_app();
    pick(&mut app, MenuItem::Settings, LEFT);
    assert_eq!(
        list(&app).items(),
        [MenuItem::OpenVolume, MenuItem::OpenWindowMode]
    );
    pick(&mut app, MenuItem::OpenVolume, LEFT);
    let volume = |percent| MenuItem::SetVolume(Volume::new(percent).expect("in range"));
    assert_eq!(
        list(&app).items(),
        (0..=10).map(|tens| volume(tens * 10)).collect::<Vec<_>>()
    );
    assert_eq!(under_cursor(&app), volume(80));
    tap(&mut app, LEFT_UP);
    tap(&mut app, LEFT_CONFIRM);
    assert_eq!(screen(&app), MenuScreen::Settings);
    assert_eq!(app.world().resource::<Settings>().volume.percent(), 70);
    frame(&mut app);
    assert_eq!(saved_settings(&app).volume.percent(), 70);
}

/// Spec 1.6.3: window mode changes and persists.
#[test]
fn a_window_mode_picked_in_settings_changes_and_is_saved() {
    let mut app = menu_app();
    pick(&mut app, MenuItem::Settings, RIGHT);
    pick(&mut app, MenuItem::OpenWindowMode, RIGHT);
    assert_eq!(
        list(&app).items(),
        [
            MenuItem::SetWindowMode(WindowMode::BorderlessFullscreen),
            MenuItem::SetWindowMode(WindowMode::Windowed),
        ]
    );
    pick(
        &mut app,
        MenuItem::SetWindowMode(WindowMode::Windowed),
        RIGHT,
    );
    assert_eq!(screen(&app), MenuScreen::Settings);
    assert_eq!(
        app.world().resource::<Settings>().window_mode,
        WindowMode::Windowed
    );
    frame(&mut app);
    assert_eq!(saved_settings(&app).window_mode, WindowMode::Windowed);
    tap(&mut app, KeyCode::Escape);
    assert_eq!(screen(&app), MenuScreen::Main);
}

fn name_field(app: &mut App) -> Mut<'_, EditableText> {
    let mut fields = app
        .world_mut()
        .query_filtered::<&mut EditableText, With<NameField>>();
    fields
        .single_mut(app.world_mut())
        .expect("one name field on the name screen")
}

/// Types `name` as the name field would hold it after the keys.
fn type_name(app: &mut App, name: &str) {
    let mut field = name_field(app);
    let max_characters = field.max_characters;
    *field = EditableText {
        max_characters,
        ..EditableText::new(name)
    };
}

/// From the main menu to the left seat's name screen, against the computer.
fn to_the_left_name_screen(app: &mut App) {
    pick(app, MenuItem::Play, LEFT);
    pick(app, MenuItem::Computer(Difficulty::OneStar), LEFT);
    pick(app, MenuItem::PointsToWin(points(10)), LEFT);
    pick(app, MenuItem::NewProfile, LEFT);
    assert_eq!(screen(app), MenuScreen::NameProfile(Seat::Left));
}

/// Spec 1.6.3: the seat key filter is suspended during text entry.
#[test]
fn on_the_name_screen_seat_keys_type_and_never_navigate() {
    let mut app = menu_app();
    to_the_left_name_screen(&mut app);
    for key in [
        LEFT_UP,
        LEFT_DOWN,
        LEFT_CONFIRM,
        LEFT_BACK,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        RIGHT_CONFIRM,
        KeyCode::ArrowLeft,
    ] {
        tap(&mut app, key);
        assert_eq!(screen(&app), MenuScreen::NameProfile(Seat::Left), "{key:?}");
    }
}

/// Spec 1.6.3: the name cap, and a name from the left seat's own keys.
#[test]
fn the_name_field_takes_twelve_characters_at_most() {
    let mut app = menu_app();
    to_the_left_name_screen(&mut app);
    assert_eq!(
        name_field(&mut app).max_characters,
        Some(ProfileName::MAX_CHARS)
    );
}

/// Spec 1.6.3: a new profile appears in the file. "Dawid" uses three of
/// the left seat's keys.
#[test]
fn a_named_and_coloured_profile_is_saved_and_waits_under_the_cursor() {
    let mut app = menu_app_with_zoe_and_max();
    to_the_left_name_screen(&mut app);
    type_name(&mut app, "Dawid");
    tap(&mut app, KeyCode::Enter);
    assert_eq!(screen(&app), MenuScreen::PickColor(Seat::Left));
    let palette = app.world().resource::<Tuning>().palette().to_vec();
    assert_eq!(
        list(&app).items(),
        palette
            .iter()
            .map(|&c| MenuItem::Color(c))
            .collect::<Vec<_>>()
    );
    pick(&mut app, MenuItem::Color(palette[1]), LEFT);

    assert_eq!(screen(&app), MenuScreen::PickProfile(Seat::Left));
    let dawid = ProfileId(3);
    assert_eq!(under_cursor(&app), MenuItem::Player(Player::Profile(dawid)));
    frame(&mut app);
    let file = app.world().resource::<SavePaths>().profiles_file();
    let saved = decode_profiles(&std::fs::read_to_string(file).expect("profiles written"))
        .expect("decodes");
    let made = saved.as_slice().last().expect("three profiles");
    assert_eq!(
        (made.id(), made.name().as_str(), made.color()),
        (dawid, "Dawid", palette[1])
    );
}

#[test]
fn an_empty_or_blank_name_is_not_taken() {
    let mut app = menu_app();
    to_the_left_name_screen(&mut app);
    tap(&mut app, KeyCode::Enter);
    assert_eq!(screen(&app), MenuScreen::NameProfile(Seat::Left));
    type_name(&mut app, "   ");
    tap(&mut app, KeyCode::Enter);
    assert_eq!(screen(&app), MenuScreen::NameProfile(Seat::Left));
}

#[test]
fn escape_on_the_name_screen_returns_to_the_profile_list_and_back_from_colour_keeps_the_name() {
    let mut app = menu_app();
    to_the_left_name_screen(&mut app);
    tap(&mut app, KeyCode::Escape);
    assert_eq!(screen(&app), MenuScreen::PickProfile(Seat::Left));

    pick(&mut app, MenuItem::NewProfile, LEFT);
    type_name(&mut app, "Mia");
    tap(&mut app, KeyCode::Enter);
    tap(&mut app, LEFT_BACK);
    assert_eq!(screen(&app), MenuScreen::NameProfile(Seat::Left));
    assert_eq!(name_field(&mut app).value().to_string(), "Mia");
}

/// Spec 1.6.3: duplicates allowed.
#[test]
fn two_profiles_with_one_name_coexist() {
    let mut app = menu_app();
    to_the_left_name_screen(&mut app);
    for _ in 0..2 {
        type_name(&mut app, "Mia");
        tap(&mut app, KeyCode::Enter);
        tap(&mut app, LEFT_CONFIRM);
        pick(&mut app, MenuItem::NewProfile, LEFT);
    }
    let names: Vec<(ProfileId, String)> = app
        .world()
        .resource::<Profiles>()
        .as_slice()
        .iter()
        .map(|p| (p.id(), p.name().as_str().to_owned()))
        .collect();
    assert_eq!(
        names,
        [
            (ProfileId(1), "Mia".to_owned()),
            (ProfileId(2), "Mia".to_owned())
        ]
    );
}

/// Each row's index and whether the cursor mark is on it, top to bottom.
fn rows(app: &mut App) -> Vec<(usize, bool)> {
    let mut query = app.world_mut().query::<(&MenuRow, &BorderColor)>();
    let mut rows: Vec<(usize, bool)> = query
        .iter(app.world())
        .map(|(row, border)| (row.0, border.top != Color::NONE))
        .collect();
    rows.sort_unstable();
    rows
}

fn count<C: Component>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<(), With<C>>()
        .iter(app.world())
        .count()
}

#[test]
fn the_screen_shows_one_row_per_item_and_marks_the_cursor_row() {
    let mut app = menu_app();
    assert_eq!(rows(&mut app), [(0, true), (1, false), (2, false)]);
    tap(&mut app, LEFT_DOWN);
    assert_eq!(rows(&mut app), [(0, false), (1, true), (2, false)]);
    pick(&mut app, MenuItem::Settings, LEFT);
    assert_eq!(rows(&mut app), [(0, true), (1, false)]);
}

#[test]
fn only_the_current_screen_is_on_show() {
    let mut app = menu_app();
    to_the_left_name_screen(&mut app);
    assert_eq!(count::<MenuScreenRoot>(&mut app), 1);
    assert_eq!(count::<NameField>(&mut app), 1);
    tap(&mut app, KeyCode::Escape);
    assert_eq!(count::<MenuScreenRoot>(&mut app), 1);
    assert_eq!(count::<NameField>(&mut app), 0);
}

fn seat_cue(app: &mut App) -> Vec<String> {
    let mut query = app.world_mut().query_filtered::<&Text, With<SeatCue>>();
    query.iter(app.world()).map(|text| text.0.clone()).collect()
}

fn screen_alignment(app: &mut App) -> AlignItems {
    let mut query = app
        .world_mut()
        .query_filtered::<&Node, With<MenuScreenRoot>>();
    query.single(app.world()).expect("one screen").align_items
}

/// Spec 1.6.3: profile select per seat with a seat cue.
#[test]
fn a_seats_screen_sits_on_its_half_under_its_keys() {
    let mut app = menu_app();
    pick(&mut app, MenuItem::Play, LEFT);
    assert_eq!(seat_cue(&mut app), Vec::<String>::new());
    assert_eq!(screen_alignment(&mut app), AlignItems::Center);
    pick(&mut app, MenuItem::Friend, LEFT);
    pick(&mut app, MenuItem::PointsToWin(points(10)), LEFT);
    assert_eq!(seat_cue(&mut app), ["W↑ S↓ D✓"]);
    assert_eq!(screen_alignment(&mut app), AlignItems::FlexStart);
    pick(&mut app, MenuItem::Player(Player::Guest), LEFT);
    assert_eq!(seat_cue(&mut app), ["↑ ↓ →✓"]);
    assert_eq!(screen_alignment(&mut app), AlignItems::FlexEnd);
}

/// The seat's keys type on the name screen, so no key cue says otherwise;
/// the screen still sits on the seat's half.
#[test]
fn the_name_screen_sits_on_its_seats_half_without_a_key_cue() {
    let mut app = menu_app();
    to_the_left_name_screen(&mut app);
    assert_eq!(screen_alignment(&mut app), AlignItems::FlexStart);
    assert_eq!(seat_cue(&mut app), Vec::<String>::new());
}

/// From the main menu to the left seat's profile list, against the computer.
fn to_the_left_profile_list(app: &mut App) {
    pick(app, MenuItem::Play, LEFT);
    pick(app, MenuItem::Computer(Difficulty::OneStar), LEFT);
    pick(app, MenuItem::PointsToWin(points(10)), LEFT);
    assert_eq!(screen(app), MenuScreen::PickProfile(Seat::Left));
}

/// From the left seat's profile list to the choice of what to do with `id`.
fn to_the_actions_for(app: &mut App, id: ProfileId) {
    to_the_left_profile_list(app);
    pick(app, MenuItem::EditProfiles, LEFT);
    assert_eq!(screen(app), MenuScreen::EditProfiles(Seat::Left));
    pick(app, MenuItem::EditProfile(id), LEFT);
    assert_eq!(screen(app), MenuScreen::ProfileActions(Seat::Left));
    assert_eq!(
        list(app).items(),
        [
            MenuItem::RenameProfile,
            MenuItem::RecolorProfile,
            MenuItem::DeleteProfile
        ]
    );
}

fn profile_names(app: &App) -> Vec<String> {
    app.world()
        .resource::<Profiles>()
        .as_slice()
        .iter()
        .map(|profile| profile.name().as_str().to_owned())
        .collect()
}

fn saved_profile_names(app: &App) -> Vec<String> {
    let file = app.world().resource::<SavePaths>().profiles_file();
    decode_profiles(&std::fs::read_to_string(file).expect("profiles written"))
        .expect("decodes")
        .as_slice()
        .iter()
        .map(|profile| profile.name().as_str().to_owned())
        .collect()
}

fn heading(app: &mut App) -> Vec<String> {
    let mut query = app.world_mut().query_filtered::<&Text, With<MenuHeading>>();
    query.iter(app.world()).map(|text| text.0.clone()).collect()
}

/// Spec 1.6.3 SHOULD: rename and delete reachable from profile select.
#[test]
fn edit_is_offered_below_new_only_when_the_seat_has_a_profile_to_edit() {
    let mut app = menu_app();
    to_the_left_profile_list(&mut app);
    assert_eq!(
        list(&app).items(),
        [MenuItem::Player(Player::Guest), MenuItem::NewProfile]
    );
}

#[test]
fn a_profile_renamed_from_profile_select_shows_there_and_is_saved() {
    let mut app = menu_app_with_zoe_and_max();
    to_the_actions_for(&mut app, MAX);
    pick(&mut app, MenuItem::RenameProfile, LEFT);
    assert_eq!(screen(&app), MenuScreen::RenameProfile(Seat::Left));
    assert_eq!(name_field(&mut app).value().to_string(), "Max");
    type_name(&mut app, "Maxi");
    tap(&mut app, KeyCode::Enter);
    assert_eq!(screen(&app), MenuScreen::PickProfile(Seat::Left));
    assert_eq!(under_cursor(&app), MenuItem::Player(Player::Profile(MAX)));
    assert_eq!(profile_names(&app), ["Zoë", "Maxi"]);
    frame(&mut app);
    assert_eq!(saved_profile_names(&app), ["Zoë", "Maxi"]);
}

#[test]
fn escape_while_renaming_keeps_the_name() {
    let mut app = menu_app_with_zoe_and_max();
    to_the_actions_for(&mut app, MAX);
    pick(&mut app, MenuItem::RenameProfile, LEFT);
    type_name(&mut app, "Maxi");
    tap(&mut app, KeyCode::Escape);
    assert_eq!(screen(&app), MenuScreen::ProfileActions(Seat::Left));
    assert_eq!(profile_names(&app), ["Zoë", "Max"]);
}

/// Decision 2: a habit of confirming loses nothing.
#[test]
fn delete_asks_first_with_no_under_the_cursor_and_no_or_back_keeps_the_profile() {
    let mut app = menu_app_with_zoe_and_max();
    to_the_actions_for(&mut app, MAX);
    pick(&mut app, MenuItem::DeleteProfile, LEFT);
    assert_eq!(screen(&app), MenuScreen::DeleteProfile(Seat::Left));
    assert_eq!(heading(&mut app), ["Delete Max?"]);
    assert_eq!(
        list(&app).items(),
        [MenuItem::KeepProfile, MenuItem::ConfirmDelete]
    );
    assert_eq!(under_cursor(&app), MenuItem::KeepProfile);
    tap(&mut app, LEFT_CONFIRM);
    assert_eq!(screen(&app), MenuScreen::ProfileActions(Seat::Left));
    pick(&mut app, MenuItem::DeleteProfile, LEFT);
    tap(&mut app, LEFT_BACK);
    assert_eq!(screen(&app), MenuScreen::ProfileActions(Seat::Left));
    assert_eq!(profile_names(&app), ["Zoë", "Max"]);
}

#[test]
fn a_profile_deleted_after_yes_is_gone_from_the_list_and_the_file() {
    let mut app = menu_app_with_zoe_and_max();
    to_the_actions_for(&mut app, MAX);
    pick(&mut app, MenuItem::DeleteProfile, LEFT);
    pick(&mut app, MenuItem::ConfirmDelete, LEFT);
    assert_eq!(screen(&app), MenuScreen::PickProfile(Seat::Left));
    assert_eq!(
        list(&app).items(),
        [
            MenuItem::Player(Player::Guest),
            MenuItem::Player(Player::Profile(ZOE)),
            MenuItem::NewProfile,
            MenuItem::EditProfiles,
        ]
    );
    frame(&mut app);
    assert_eq!(saved_profile_names(&app), ["Zoë"]);
}

/// Decision 5: the right seat cannot edit the left seat's pick.
#[test]
fn the_other_seats_pick_cannot_be_edited() {
    let mut app = menu_app_with_zoe_and_max();
    pick(&mut app, MenuItem::Play, LEFT);
    pick(&mut app, MenuItem::Friend, LEFT);
    pick(&mut app, MenuItem::PointsToWin(points(10)), LEFT);
    pick(&mut app, MenuItem::Player(Player::Profile(ZOE)), LEFT);
    pick(&mut app, MenuItem::EditProfiles, RIGHT);
    assert_eq!(list(&app).items(), [MenuItem::EditProfile(MAX)]);
    tap(&mut app, KeyCode::ArrowLeft);
    assert_eq!(screen(&app), MenuScreen::PickProfile(Seat::Right));
}

fn color_of(app: &App, id: ProfileId) -> Srgba {
    app.world()
        .resource::<Profiles>()
        .as_slice()
        .iter()
        .find(|profile| profile.id() == id)
        .expect("the profile")
        .color()
}

fn palette(app: &App) -> Vec<Srgba> {
    app.world().resource::<Tuning>().palette().to_vec()
}

/// Plan 8.4 (the user's addition to spec 1.6.3): a profile's colour changes
/// from profile select too.
#[test]
fn a_profile_recoloured_from_profile_select_shows_there_and_is_saved() {
    let mut app = menu_app_with_zoe_and_max();
    let palette = palette(&app);
    to_the_actions_for(&mut app, MAX);
    pick(&mut app, MenuItem::RecolorProfile, LEFT);
    assert_eq!(screen(&app), MenuScreen::RecolorProfile(Seat::Left));
    assert_eq!(
        list(&app).items(),
        palette
            .iter()
            .map(|&c| MenuItem::Color(c))
            .collect::<Vec<_>>()
    );
    pick(&mut app, MenuItem::Color(palette[2]), LEFT);
    assert_eq!(screen(&app), MenuScreen::PickProfile(Seat::Left));
    assert_eq!(under_cursor(&app), MenuItem::Player(Player::Profile(MAX)));
    assert_eq!(color_of(&app, MAX), palette[2]);
    frame(&mut app);
    let file = app.world().resource::<SavePaths>().profiles_file();
    let saved = decode_profiles(&std::fs::read_to_string(file).expect("profiles written"))
        .expect("decodes");
    assert_eq!(saved.as_slice()[1].color(), palette[2]);

    pick(&mut app, MenuItem::EditProfiles, LEFT);
    pick(&mut app, MenuItem::EditProfile(MAX), LEFT);
    pick(&mut app, MenuItem::RecolorProfile, LEFT);
    assert_eq!(
        under_cursor(&app),
        MenuItem::Color(palette[2]),
        "the current colour"
    );
}

#[test]
fn back_from_the_colours_keeps_the_colour() {
    let mut app = menu_app_with_zoe_and_max();
    to_the_actions_for(&mut app, MAX);
    pick(&mut app, MenuItem::RecolorProfile, LEFT);
    tap(&mut app, LEFT_DOWN);
    tap(&mut app, KeyCode::Escape);
    assert_eq!(screen(&app), MenuScreen::ProfileActions(Seat::Left));
    assert_eq!(color_of(&app, MAX), Srgba::RED);
}

/// Plan 8.4 (R2): the name screens say how a name is taken.
#[test]
fn the_name_screens_hint_that_enter_takes_the_name() {
    let mut app = menu_app_with_zoe_and_max();
    to_the_actions_for(&mut app, MAX);
    pick(&mut app, MenuItem::RenameProfile, LEFT);
    assert_eq!(texts_of::<KeyHint>(&mut app), ["Enter ✓"]);
    tap(&mut app, KeyCode::Escape);
    tap(&mut app, KeyCode::Escape);
    tap(&mut app, KeyCode::Escape);
    pick(&mut app, MenuItem::NewProfile, LEFT);
    assert_eq!(screen(&app), MenuScreen::NameProfile(Seat::Left));
    assert_eq!(texts_of::<KeyHint>(&mut app), ["Enter ✓"]);
}

fn texts_of<C: Component>(app: &mut App) -> Vec<String> {
    let mut query = app.world_mut().query_filtered::<&Text, With<C>>();
    query.iter(app.world()).map(|text| text.0.clone()).collect()
}

/// The rows laid out, by index: a hidden row takes no space.
fn rows_on_screen(app: &mut App) -> Vec<usize> {
    let mut query = app.world_mut().query::<(&MenuRow, &Node)>();
    let mut rows: Vec<usize> = query
        .iter(app.world())
        .filter(|(_, node)| node.display != Display::None)
        .map(|(row, _)| row.0)
        .collect();
    rows.sort_unstable();
    rows
}

/// Which scroll marks show: (▲, ▼).
fn scroll_marks(app: &mut App) -> (bool, bool) {
    let mut query = app.world_mut().query::<(&ScrollMark, &Visibility)>();
    let marks: Vec<_> = query.iter(app.world()).map(|(m, v)| (*m, *v)).collect();
    assert_eq!(marks.len(), 2, "one mark above the rows, one below");
    let shows = |wanted| {
        marks
            .iter()
            .any(|&(mark, visibility)| mark == wanted && visibility != Visibility::Hidden)
    };
    (shows(ScrollMark::Above), shows(ScrollMark::Below))
}

/// A list that fits shows every row and no scroll mark.
#[test]
fn a_short_list_shows_every_row_and_no_scroll_mark() {
    let mut app = menu_app();
    pick(&mut app, MenuItem::Settings, LEFT);
    assert_eq!(rows_on_screen(&mut app), [0, 1]);
    assert_eq!(scroll_marks(&mut app), (false, false));
}

/// The user's hand check (2026-11-08): 90 and 100 ran off the screen. A
/// long list shows [`SHOWN_ROWS`] around the cursor, marking what is hidden.
#[test]
fn a_long_list_opens_around_the_cursor_and_marks_what_is_hidden() {
    let mut app = menu_app();
    pick(&mut app, MenuItem::Settings, LEFT);
    pick(&mut app, MenuItem::OpenVolume, LEFT);
    assert_eq!(list(&app).items().len(), 11);
    assert_eq!(list(&app).cursor(), 8, "80 under the cursor");
    assert_eq!(rows_on_screen(&mut app), (5..11).collect::<Vec<_>>());
    assert_eq!(scroll_marks(&mut app), (true, false));
}

/// The shown rows move only when the cursor would leave them, one row at a
/// time, and never past either end.
#[test]
fn the_shown_rows_follow_the_cursor_one_row_at_a_time() {
    let mut app = menu_app();
    pick(&mut app, MenuItem::Settings, LEFT);
    pick(&mut app, MenuItem::OpenVolume, LEFT);
    for _ in 0..3 {
        tap(&mut app, LEFT_UP);
    }
    assert_eq!(list(&app).cursor(), 5);
    assert_eq!(
        rows_on_screen(&mut app),
        (5..11).collect::<Vec<_>>(),
        "the cursor is still on"
    );
    tap(&mut app, LEFT_UP);
    assert_eq!(rows_on_screen(&mut app), (4..10).collect::<Vec<_>>());
    assert_eq!(scroll_marks(&mut app), (true, true));
    for _ in 0..10 {
        tap(&mut app, LEFT_UP);
    }
    assert_eq!(
        rows_on_screen(&mut app),
        (0..SHOWN_ROWS).collect::<Vec<_>>()
    );
    assert_eq!(scroll_marks(&mut app), (false, true));
    tap(&mut app, LEFT_DOWN);
    assert_eq!(
        rows_on_screen(&mut app),
        (0..SHOWN_ROWS).collect::<Vec<_>>()
    );
    for _ in 0..20 {
        tap(&mut app, LEFT_DOWN);
    }
    assert_eq!(rows_on_screen(&mut app), (5..11).collect::<Vec<_>>());
    assert_eq!(list(&app).shown(), 5..11);
}

/// The cursor near an end does not shorten the list: at 100 the last
/// [`SHOWN_ROWS`] show, not the cursor's row and the few after it.
#[test]
fn a_list_opened_near_its_end_still_shows_a_full_window() {
    let mut app = menu_app();
    app.world_mut().resource_mut::<Settings>().volume = Volume::new(100).expect("in range");
    pick(&mut app, MenuItem::Settings, LEFT);
    pick(&mut app, MenuItem::OpenVolume, LEFT);
    assert_eq!(list(&app).cursor(), 10, "100 under the cursor");
    assert_eq!(rows_on_screen(&mut app), (5..11).collect::<Vec<_>>());
}

/// Plan 10.2: the points picked are kept in the settings and saved, and the
/// points screen opens on them next time, here after backing out to it.
#[test]
fn the_points_picked_are_saved_and_under_the_cursor_next_time() {
    let mut app = menu_app();
    pick(&mut app, MenuItem::Play, LEFT);
    pick(&mut app, MenuItem::Friend, LEFT);
    pick(&mut app, MenuItem::PointsToWin(points(15)), LEFT);
    assert_eq!(
        app.world().resource::<Settings>().points_to_win,
        Some(points(15))
    );
    frame(&mut app);
    assert_eq!(saved_settings(&app).points_to_win, Some(points(15)));
    tap(&mut app, KeyCode::Escape);
    assert_eq!(screen(&app), MenuScreen::PointsToWin);
    assert_eq!(under_cursor(&app), MenuItem::PointsToWin(points(15)));
}

/// A pick the tuning no longer offers: the tuned default, not the first row.
#[test]
fn a_saved_pick_no_longer_offered_opens_on_the_tuned_default() {
    let mut app = menu_app();
    app.world_mut().resource_mut::<Settings>().points_to_win = Some(points(7));
    pick(&mut app, MenuItem::Play, LEFT);
    pick(&mut app, MenuItem::Friend, LEFT);
    assert_eq!(under_cursor(&app), MenuItem::PointsToWin(points(10)));
}

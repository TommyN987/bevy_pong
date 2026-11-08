//! The menu screens (spec 1.6.3, plan 8.1): a list per screen, a cursor, and
//! the picks that become the match.
//!
//! Every screen is a vertical list and confirm picks the row under the
//! cursor, since the back key (A/Left) leaves no key to change a value
//! sideways. Shared screens take either seat's keys; a seat's own screens
//! take only its keys; Escape always backs out.
//!
//! The flow: `Main` → `Opponent` → `PointsToWin` → `PickProfile` per human seat →
//! the match. Two friends pick left seat first, then right. Against the
//! computer, the seat that confirmed the computer plays on its own side and
//! makes every later pick. New on a profile list leads to `NameProfile`, then
//! `PickColor`, and back to the list with the new profile under the cursor.
//! Edit on it (spec 1.6.3 SHOULD) lists the profiles the seat may pick; one
//! of them leads to `ProfileActions`: `RenameProfile` or `RecolorProfile`,
//! each back to the list with it under the cursor, or `DeleteProfile`, which
//! asks first with No under the cursor.

use std::num::NonZeroU8;

use bevy::app::{App, AppExit, Plugin, Update};
use bevy::color::Srgba;
use bevy::ecs::error::BevyError;
use bevy::ecs::message::{Message, MessageReader, MessageWriter};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::schedule::common_conditions::resource_changed;
use bevy::ecs::system::{Commands, Res, ResMut, SystemParam};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::state::app::AppExtStates;
use bevy::state::condition::{in_state, state_changed};
use bevy::state::state::{NextState, State, SubStates};
use pong_core::domain::{
    Controller, Difficulty, MatchConfig, Player, ProfileId, Seat, SeatPlayers,
};
use pong_core::features::persistence::model::{
    Profile, ProfileName, Profiles, Settings, Volume, WindowMode,
};
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::AppState;

use super::menu_input::{MenuDrivers, MenuKey, MenuPress, menu_press};

pub mod name_entry;
pub mod view;

/// The menu's screens and their flow.
///
/// Preconditions: Bevy's `InputPlugin` and the core plugin group, which
/// provide `ButtonInput<KeyCode>`, `Tuning`, `Settings`, and `Profiles` in
/// `Menu`.
pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_sub_state::<MenuScreen>()
            .add_message::<MenuChoice>()
            .init_resource::<MenuList>()
            .init_resource::<MatchDraft>()
            .add_systems(
                Update,
                (
                    (list_screen, view::show_screen)
                        .chain()
                        .run_if(state_changed::<MenuScreen>),
                    navigate_menu,
                    name_entry::confirm_name,
                    act_on_choice,
                    view::mark_cursor.run_if(resource_changed::<MenuList>),
                )
                    .chain()
                    .run_if(in_state(AppState::Menu)),
            );
    }
}

/// One menu screen; one state per screen (docs/architecture.md, rule 8).
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[source(AppState = AppState::Menu)]
pub enum MenuScreen {
    /// Play, settings, quit.
    #[default]
    Main,
    /// Volume and window mode.
    Settings,
    /// Master volume in steps of ten.
    Volume,
    /// Fullscreen or a window.
    WindowMode,
    /// A friend or the computer at one to three stars.
    Opponent,
    /// The tuning's points-to-win choices.
    PointsToWin,
    /// Who the seat plays as: Guest, a saved profile, or a new one.
    PickProfile(Seat),
    /// The seat types a new profile's name.
    NameProfile(Seat),
    /// The seat picks the new profile's colour from the palette.
    PickColor(Seat),
    /// The profiles the seat may rename or delete.
    EditProfiles(Seat),
    /// Rename or delete the profile picked to edit.
    ProfileActions(Seat),
    /// The seat retypes the profile's name.
    RenameProfile(Seat),
    /// The seat picks the profile's new colour from the palette.
    RecolorProfile(Seat),
    /// Whether to delete the profile, for good.
    DeleteProfile(Seat),
}

impl MenuScreen {
    /// The seat whose turn the screen is, if it belongs to one.
    const fn seat(self) -> Option<Seat> {
        match self {
            Self::PickProfile(seat)
            | Self::NameProfile(seat)
            | Self::PickColor(seat)
            | Self::EditProfiles(seat)
            | Self::ProfileActions(seat)
            | Self::RenameProfile(seat)
            | Self::RecolorProfile(seat)
            | Self::DeleteProfile(seat) => Some(seat),
            Self::Main
            | Self::Settings
            | Self::Volume
            | Self::WindowMode
            | Self::Opponent
            | Self::PointsToWin => None,
        }
    }
}

/// One row of a menu list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuItem {
    /// Main: on to the opponent.
    Play,
    /// Main: the settings screen.
    Settings,
    /// Main: ends the game.
    Quit,
    /// Settings: the volume list.
    OpenVolume,
    /// Settings: the window mode list.
    OpenWindowMode,
    /// Volume: this volume.
    SetVolume(Volume),
    /// Window mode: this mode.
    SetWindowMode(WindowMode),
    /// Opponent: two humans.
    Friend,
    /// Opponent: the computer at this difficulty.
    Computer(Difficulty),
    /// Points to win: this target.
    PointsToWin(NonZeroU8),
    /// A seat's profile list: play as this player.
    Player(Player),
    /// A seat's profile list: make a new profile.
    NewProfile,
    /// A seat's colour list: the new profile's colour, or the edited one's.
    Color(Srgba),
    /// A seat's profile list: rename or delete one.
    EditProfiles,
    /// The edit list: this profile.
    EditProfile(ProfileId),
    /// A profile's actions: retype its name.
    RenameProfile,
    /// A profile's actions: pick its colour again.
    RecolorProfile,
    /// A profile's actions: delete it, after asking.
    DeleteProfile,
    /// The delete question: no, keep it.
    KeepProfile,
    /// The delete question: yes, delete it.
    ConfirmDelete,
}

/// The current screen's rows and the cursor. Invariant: the cursor is on a
/// row, or 0 on a screen without rows.
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct MenuList {
    items: Vec<MenuItem>,
    cursor: usize,
}

impl MenuList {
    /// `items` with the cursor on `preselected`, or on the first row if it
    /// is absent.
    fn new(items: Vec<MenuItem>, preselected: Option<MenuItem>) -> Self {
        let cursor = preselected
            .and_then(|wanted| items.iter().position(|&item| item == wanted))
            .unwrap_or(0);
        Self { items, cursor }
    }

    /// The rows, top to bottom.
    #[must_use]
    pub fn items(&self) -> &[MenuItem] {
        &self.items
    }

    /// The row the cursor is on.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The cursor one row up; it stays on the first row.
    const fn up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// The cursor one row down; it stays on the last row.
    const fn down(&mut self) {
        if self.cursor + 1 < self.items.len() {
            self.cursor += 1;
        }
    }

    fn under_cursor(&self) -> Option<MenuItem> {
        self.items.get(self.cursor).copied()
    }
}

/// Who the other side is, once picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opponent {
    /// Two humans, left seat on the left side.
    Friend,
    /// The computer, against `human`'s seat on that seat's own side.
    Computer { difficulty: Difficulty, human: Seat },
}

impl Opponent {
    /// The seat that picks a profile first.
    const fn first_seat(self) -> Seat {
        match self {
            Self::Friend => Seat::Left,
            Self::Computer { human, .. } => human,
        }
    }

    /// The seat that picks after `seat`, if any.
    const fn seat_after(self, seat: Seat) -> Option<Seat> {
        match (self, seat) {
            (Self::Friend, Seat::Left) => Some(Seat::Right),
            (Self::Friend, Seat::Right) | (Self::Computer { .. }, _) => None,
        }
    }

    /// The seat that picked before `seat`, if any.
    const fn seat_before(self, seat: Seat) -> Option<Seat> {
        match (self, seat) {
            (Self::Friend, Seat::Right) => Some(Seat::Left),
            (Self::Friend, Seat::Left) | (Self::Computer { .. }, _) => None,
        }
    }

    /// Whose keys drive the shared picks after this one.
    const fn drivers(self) -> MenuDrivers {
        match self {
            Self::Friend => MenuDrivers::EitherSeat,
            Self::Computer { human, .. } => MenuDrivers::Seat(human),
        }
    }

    fn plays(self, seat: Seat) -> bool {
        match self {
            Self::Friend => true,
            Self::Computer { human, .. } => human == seat,
        }
    }

    /// The left and right sides' controllers.
    const fn controllers(self) -> (Controller, Controller) {
        match self {
            Self::Friend => (
                Controller::Human(Seat::Left),
                Controller::Human(Seat::Right),
            ),
            Self::Computer {
                difficulty,
                human: Seat::Left,
            } => (Controller::Human(Seat::Left), Controller::Ai(difficulty)),
            Self::Computer {
                difficulty,
                human: Seat::Right,
            } => (Controller::Ai(difficulty), Controller::Human(Seat::Right)),
        }
    }
}

/// The picks so far. Cleared when Play is picked.
#[derive(Resource, Debug, Clone)]
struct MatchDraft {
    opponent: Option<Opponent>,
    points_to_win: Option<NonZeroU8>,
    players: SeatPlayers,
    /// The name typed for the profile being made.
    new_name: Option<ProfileName>,
    /// The profile to put the cursor on when the seat's list shows next: the
    /// one just made or renamed.
    cursor_profile: Option<ProfileId>,
    /// The profile being renamed or deleted.
    editing: Option<ProfileId>,
}

impl Default for MatchDraft {
    fn default() -> Self {
        Self {
            opponent: None,
            points_to_win: None,
            players: SeatPlayers::GUESTS,
            new_name: None,
            cursor_profile: None,
            editing: None,
        }
    }
}

impl MatchDraft {
    /// The match the picks describe. A seat the computer leaves empty is
    /// Guest, so a pick from an abandoned path never reaches the match.
    fn match_config(&self) -> Result<MatchConfig, BevyError> {
        let (Some(opponent), Some(points_to_win)) = (self.opponent, self.points_to_win) else {
            return Err(BevyError::from(
                "the match started before its opponent and points were picked",
            ));
        };
        let (left, right) = opponent.controllers();
        let player = |seat| {
            if opponent.plays(seat) {
                self.players.seat(seat)
            } else {
                Player::Guest
            }
        };
        Ok(MatchConfig {
            left,
            right,
            players: SeatPlayers {
                left_seat: player(Seat::Left),
                right_seat: player(Seat::Right),
            },
            points_to_win,
        })
    }

    fn set_player(&mut self, seat: Seat, player: Player) {
        match seat {
            Seat::Left => self.players.left_seat = player,
            Seat::Right => self.players.right_seat = player,
        }
    }

    /// The profile `seat` may not pick: the one the seat before it holds
    /// (plan 8.1, ADR 0008). Guest is never withheld.
    fn withheld_from(&self, seat: Seat) -> Option<ProfileId> {
        let before = self.opponent?.seat_before(seat)?;
        match self.players.seat(before) {
            Player::Profile(id) => Some(id),
            Player::Guest => None,
        }
    }

    /// The saved profiles `seat` may pick, rename, or delete: all but the one
    /// [`withheld_from`](Self::withheld_from) it.
    fn offered_profiles<'p>(
        &self,
        seat: Seat,
        profiles: &'p Profiles,
    ) -> impl Iterator<Item = ProfileId> + 'p {
        let withheld = self.withheld_from(seat);
        profiles
            .as_slice()
            .iter()
            .map(Profile::id)
            .filter(move |&id| Some(id) != withheld)
    }

    /// The profile being edited.
    ///
    /// # Errors
    /// None is: the edit list was skipped.
    fn editing(&self) -> Result<ProfileId, BevyError> {
        self.editing
            .ok_or_else(|| BevyError::from("a profile action with no profile picked to edit"))
    }

    /// Whose keys drive `screen`; `None` while a name is typed, so seat keys
    /// type letters instead (spec 1.6.3).
    fn drivers(&self, screen: MenuScreen) -> Option<MenuDrivers> {
        match screen {
            MenuScreen::Main
            | MenuScreen::Settings
            | MenuScreen::Volume
            | MenuScreen::WindowMode
            | MenuScreen::Opponent => Some(MenuDrivers::EitherSeat),
            MenuScreen::PointsToWin => Some(
                self.opponent
                    .map_or(MenuDrivers::EitherSeat, Opponent::drivers),
            ),
            MenuScreen::PickProfile(seat)
            | MenuScreen::PickColor(seat)
            | MenuScreen::EditProfiles(seat)
            | MenuScreen::ProfileActions(seat)
            | MenuScreen::RecolorProfile(seat)
            | MenuScreen::DeleteProfile(seat) => Some(MenuDrivers::Seat(seat)),
            MenuScreen::NameProfile(_) | MenuScreen::RenameProfile(_) => None,
        }
    }

    /// The screen backing out of `screen` returns to; none from Main.
    fn screen_before(&self, screen: MenuScreen) -> Option<MenuScreen> {
        Some(match screen {
            MenuScreen::Main => return None,
            MenuScreen::Settings | MenuScreen::Opponent => MenuScreen::Main,
            MenuScreen::Volume | MenuScreen::WindowMode => MenuScreen::Settings,
            MenuScreen::PointsToWin => MenuScreen::Opponent,
            MenuScreen::PickProfile(seat) => self
                .opponent
                .and_then(|opponent| opponent.seat_before(seat))
                .map_or(MenuScreen::PointsToWin, MenuScreen::PickProfile),
            MenuScreen::NameProfile(seat) | MenuScreen::EditProfiles(seat) => {
                MenuScreen::PickProfile(seat)
            }
            MenuScreen::PickColor(seat) => MenuScreen::NameProfile(seat),
            MenuScreen::ProfileActions(seat) => MenuScreen::EditProfiles(seat),
            MenuScreen::RenameProfile(seat)
            | MenuScreen::RecolorProfile(seat)
            | MenuScreen::DeleteProfile(seat) => MenuScreen::ProfileActions(seat),
        })
    }
}

/// What `screen` lists, and where the cursor starts.
fn list_for(
    screen: MenuScreen,
    draft: &MatchDraft,
    profiles: &Profiles,
    settings: Settings,
    tuning: &Tuning,
) -> MenuList {
    match screen {
        MenuScreen::Main => MenuList::new(
            vec![MenuItem::Play, MenuItem::Settings, MenuItem::Quit],
            None,
        ),
        MenuScreen::Settings => {
            MenuList::new(vec![MenuItem::OpenVolume, MenuItem::OpenWindowMode], None)
        }
        MenuScreen::Volume => MenuList::new(
            (0..=10)
                .filter_map(|tens| Volume::new(tens * 10).ok())
                .map(MenuItem::SetVolume)
                .collect(),
            Some(MenuItem::SetVolume(settings.volume)),
        ),
        MenuScreen::WindowMode => MenuList::new(
            [WindowMode::BorderlessFullscreen, WindowMode::Windowed]
                .map(MenuItem::SetWindowMode)
                .to_vec(),
            Some(MenuItem::SetWindowMode(settings.window_mode)),
        ),
        MenuScreen::Opponent => MenuList::new(
            [
                MenuItem::Friend,
                MenuItem::Computer(Difficulty::OneStar),
                MenuItem::Computer(Difficulty::TwoStars),
                MenuItem::Computer(Difficulty::ThreeStars),
            ]
            .to_vec(),
            None,
        ),
        MenuScreen::PointsToWin => {
            let flow = tuning.match_flow();
            MenuList::new(
                flow.points_to_win_options()
                    .iter()
                    .map(|&points| MenuItem::PointsToWin(points))
                    .collect(),
                Some(MenuItem::PointsToWin(flow.points_to_win_default())),
            )
        }
        MenuScreen::PickProfile(seat) => {
            let saved: Vec<MenuItem> = draft
                .offered_profiles(seat, profiles)
                .map(|id| MenuItem::Player(Player::Profile(id)))
                .collect();
            let edit = (!saved.is_empty()).then_some(MenuItem::EditProfiles);
            MenuList::new(
                std::iter::once(MenuItem::Player(Player::Guest))
                    .chain(saved)
                    .chain(std::iter::once(MenuItem::NewProfile))
                    .chain(edit)
                    .collect(),
                draft
                    .cursor_profile
                    .map(|id| MenuItem::Player(Player::Profile(id))),
            )
        }
        MenuScreen::EditProfiles(seat) => MenuList::new(
            draft
                .offered_profiles(seat, profiles)
                .map(MenuItem::EditProfile)
                .collect(),
            draft.editing.map(MenuItem::EditProfile),
        ),
        MenuScreen::ProfileActions(_) => MenuList::new(
            vec![
                MenuItem::RenameProfile,
                MenuItem::RecolorProfile,
                MenuItem::DeleteProfile,
            ],
            None,
        ),
        MenuScreen::RecolorProfile(_) => MenuList::new(
            palette_items(tuning),
            draft
                .editing
                .and_then(|id| {
                    profiles
                        .as_slice()
                        .iter()
                        .find(|profile| profile.id() == id)
                })
                .map(|profile| MenuItem::Color(profile.color())),
        ),
        MenuScreen::DeleteProfile(_) => {
            MenuList::new(vec![MenuItem::KeepProfile, MenuItem::ConfirmDelete], None)
        }
        MenuScreen::NameProfile(_) | MenuScreen::RenameProfile(_) => {
            MenuList::new(Vec::new(), None)
        }
        MenuScreen::PickColor(_) => MenuList::new(palette_items(tuning), None),
    }
}

/// One row per palette colour, in the palette's order.
fn palette_items(tuning: &Tuning) -> Vec<MenuItem> {
    tuning
        .palette()
        .iter()
        .map(|&color| MenuItem::Color(color))
        .collect()
}

/// Lists the screen just entered.
fn list_screen(
    screen: Res<State<MenuScreen>>,
    draft: Res<MatchDraft>,
    profiles: Res<Profiles>,
    settings: Res<Settings>,
    tuning: Res<Tuning>,
    mut list: ResMut<MenuList>,
) {
    *list = list_for(*screen.get(), &draft, &profiles, *settings, &tuning);
}

/// A row confirmed, and by which seat's key.
#[derive(Message, Debug, Clone, Copy)]
struct MenuChoice {
    item: MenuItem,
    by: Seat,
}

/// Moves the cursor, backs out, or reports a confirmed row, for the keys of
/// whoever drives the screen.
fn navigate_menu(
    keys: Res<ButtonInput<KeyCode>>,
    tuning: Res<Tuning>,
    screen: Res<State<MenuScreen>>,
    draft: Res<MatchDraft>,
    mut list: ResMut<MenuList>,
    mut next_screen: ResMut<NextState<MenuScreen>>,
    mut choices: MessageWriter<MenuChoice>,
) {
    let screen = *screen.get();
    let Some(drivers) = draft.drivers(screen) else {
        return;
    };
    match menu_press(&keys, tuning.bindings(), drivers) {
        None => {}
        Some(MenuPress::Seat(_, MenuKey::Up)) => list.up(),
        Some(MenuPress::Seat(_, MenuKey::Down)) => list.down(),
        Some(MenuPress::Seat(_, MenuKey::Back) | MenuPress::Escape) => {
            if let Some(before) = draft.screen_before(screen) {
                next_screen.set(before);
            }
        }
        Some(MenuPress::Seat(by, MenuKey::Confirm)) => {
            if let Some(item) = list.under_cursor() {
                choices.write(MenuChoice { item, by });
            }
        }
    }
}

/// Where a choice leads.
enum Outcome {
    Show(MenuScreen),
    Play(MatchConfig),
    Quit,
}

/// Applies `item`, confirmed by `by` on `screen`, to the picks and the
/// saved models.
///
/// # Errors
/// A row that `screen` does not list, or a pick out of the flow's order:
/// the menu built its lists wrong.
fn choose(
    MenuChoice { item, by }: MenuChoice,
    screen: MenuScreen,
    draft: &mut MatchDraft,
    settings: &mut Settings,
    profiles: &mut Profiles,
) -> Result<Outcome, BevyError> {
    let seat = || {
        screen
            .seat()
            .ok_or_else(|| BevyError::from(format!("{item:?} on {screen:?}, which no seat owns")))
    };
    Ok(match item {
        MenuItem::Play => {
            *draft = MatchDraft::default();
            Outcome::Show(MenuScreen::Opponent)
        }
        MenuItem::Settings => Outcome::Show(MenuScreen::Settings),
        MenuItem::Quit => Outcome::Quit,
        MenuItem::OpenVolume => Outcome::Show(MenuScreen::Volume),
        MenuItem::OpenWindowMode => Outcome::Show(MenuScreen::WindowMode),
        MenuItem::SetVolume(volume) => {
            settings.volume = volume;
            Outcome::Show(MenuScreen::Settings)
        }
        MenuItem::SetWindowMode(mode) => {
            settings.window_mode = mode;
            Outcome::Show(MenuScreen::Settings)
        }
        MenuItem::Friend => {
            draft.opponent = Some(Opponent::Friend);
            Outcome::Show(MenuScreen::PointsToWin)
        }
        MenuItem::Computer(difficulty) => {
            draft.opponent = Some(Opponent::Computer {
                difficulty,
                human: by,
            });
            Outcome::Show(MenuScreen::PointsToWin)
        }
        MenuItem::PointsToWin(points) => {
            let opponent = draft
                .opponent
                .ok_or_else(|| BevyError::from("points picked before the opponent"))?;
            draft.points_to_win = Some(points);
            Outcome::Show(MenuScreen::PickProfile(opponent.first_seat()))
        }
        MenuItem::Player(player) => {
            let seat = seat()?;
            let opponent = draft
                .opponent
                .ok_or_else(|| BevyError::from("a profile picked before the opponent"))?;
            draft.set_player(seat, player);
            draft.cursor_profile = None;
            match opponent.seat_after(seat) {
                Some(next) => Outcome::Show(MenuScreen::PickProfile(next)),
                None => Outcome::Play(draft.match_config()?),
            }
        }
        MenuItem::NewProfile => Outcome::Show(MenuScreen::NameProfile(seat()?)),
        MenuItem::Color(color) if matches!(screen, MenuScreen::RecolorProfile(_)) => {
            let seat = seat()?;
            let id = draft.editing()?;
            profiles.recolor(id, color)?.ok_or_else(|| {
                BevyError::from(format!("recolouring {id:?}, which no profile has"))
            })?;
            draft.cursor_profile = Some(id);
            Outcome::Show(MenuScreen::PickProfile(seat))
        }
        MenuItem::Color(color) => {
            let seat = seat()?;
            let name = draft
                .new_name
                .take()
                .ok_or_else(|| BevyError::from("a colour picked before a name was typed"))?;
            draft.cursor_profile = Some(profiles.create(name, color)?);
            Outcome::Show(MenuScreen::PickProfile(seat))
        }
        MenuItem::EditProfiles => Outcome::Show(MenuScreen::EditProfiles(seat()?)),
        MenuItem::EditProfile(id) => {
            draft.editing = Some(id);
            Outcome::Show(MenuScreen::ProfileActions(seat()?))
        }
        MenuItem::RenameProfile => Outcome::Show(MenuScreen::RenameProfile(seat()?)),
        MenuItem::RecolorProfile => Outcome::Show(MenuScreen::RecolorProfile(seat()?)),
        MenuItem::DeleteProfile => Outcome::Show(MenuScreen::DeleteProfile(seat()?)),
        MenuItem::KeepProfile => Outcome::Show(MenuScreen::ProfileActions(seat()?)),
        MenuItem::ConfirmDelete => {
            let seat = seat()?;
            let id = draft.editing()?;
            profiles
                .remove(id)
                .ok_or_else(|| BevyError::from(format!("deleting {id:?}, which no profile has")))?;
            draft.editing = None;
            if draft.cursor_profile == Some(id) {
                draft.cursor_profile = None;
            }
            Outcome::Show(MenuScreen::PickProfile(seat))
        }
    })
}

/// Where the menu goes next.
#[derive(SystemParam)]
struct MenuExits<'w, 's> {
    commands: Commands<'w, 's>,
    next_screen: ResMut<'w, NextState<MenuScreen>>,
    next_app: ResMut<'w, NextState<AppState>>,
    exit: MessageWriter<'w, AppExit>,
}

/// Acts on each confirmed row.
///
/// # Errors
/// As [`choose`].
fn act_on_choice(
    mut choices: MessageReader<MenuChoice>,
    screen: Res<State<MenuScreen>>,
    mut draft: ResMut<MatchDraft>,
    mut settings: ResMut<Settings>,
    mut profiles: ResMut<Profiles>,
    mut exits: MenuExits,
) -> Result<(), BevyError> {
    for &choice in choices.read() {
        match choose(
            choice,
            *screen.get(),
            &mut draft,
            &mut settings,
            &mut profiles,
        )? {
            Outcome::Show(next) => exits.next_screen.set(next),
            Outcome::Play(config) => {
                exits.commands.insert_resource(config);
                exits.next_app.set(AppState::InMatch);
            }
            Outcome::Quit => {
                exits.exit.write(AppExit::Success);
            }
        }
    }
    Ok(())
}

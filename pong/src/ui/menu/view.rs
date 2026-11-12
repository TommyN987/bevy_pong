//! What the menu screens show (spec 1.6.3): a heading, the seat cue on a
//! seat's own screens, and one row per list item with the cursor's row
//! marked, at most `SHOWN_ROWS` of them laid out at once, with ▲ and ▼ where
//! rows are hidden. A seat's screens sit on that seat's half of the screen.

use bevy::camera::visibility::Visibility;
use bevy::color::{Color, Srgba};
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::state::state::State;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BorderColor, Display, FlexDirection, JustifyContent, Node, UiRect, Val,
};
use pong_core::domain::{Player, ProfileId, Seat};
use pong_core::features::persistence::model::{Profile, Profiles, Settings, WindowMode};
use pong_core::features::tuning::{SeatKeys, Tuning};

use super::name_entry::name_field;
use super::{MatchDraft, MenuItem, MenuList, MenuScreen};
use crate::ui::menu_input::{MenuDrivers, key_doing};
use crate::ui::mode_label;
use crate::ui::style::{DIM, cursor_frame, font, hint, row};
use crate::window::TITLE;

/// Marks a screen's outermost node; despawned, with all it holds, when the
/// screen is left.
#[derive(Component, Debug, Default)]
pub struct MenuScreenRoot;

/// One list row, by its index in [`MenuList::items`].
#[derive(Component, Debug)]
pub struct MenuRow(pub usize);

/// The screen's heading.
#[derive(Component, Debug, Default)]
pub struct MenuHeading;

/// The keys of the seat whose turn it is (spec 1.6.3, seat cue).
#[derive(Component, Debug, Default)]
pub struct SeatCue;

/// Marks rows hidden above or below the shown ones (▲, ▼); hidden itself,
/// keeping its place, when none are.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollMark {
    /// Rows hidden above.
    Above,
    /// Rows hidden below.
    Below,
}

/// A row's words and colour.
pub(super) fn label(item: MenuItem, profiles: &Profiles, settings: Settings) -> (String, Srgba) {
    let window = |mode| match mode {
        WindowMode::BorderlessFullscreen => "Fullscreen",
        WindowMode::Windowed => "Window",
    };
    let white = |text: &str| (text.to_owned(), Srgba::WHITE);
    match item {
        MenuItem::Play => white("Play"),
        MenuItem::Settings => white("Settings"),
        MenuItem::Quit => white("Quit"),
        MenuItem::OpenVolume => white(&format!("Volume {}", settings.volume.percent())),
        MenuItem::OpenWindowMode => white(window(settings.window_mode)),
        MenuItem::SetVolume(volume) => white(&volume.percent().to_string()),
        MenuItem::SetWindowMode(mode) => white(window(mode)),
        MenuItem::Mode(mode) => white(&mode_label(mode)),
        MenuItem::Friend => white("Friend"),
        MenuItem::Computer(difficulty) => white(&"★".repeat(usize::from(difficulty.stars()))),
        MenuItem::PointsToWin(points) => white(&points.to_string()),
        MenuItem::Player(Player::Guest) => white("Guest"),
        MenuItem::Player(Player::Profile(id)) | MenuItem::EditProfile(id) => find(profiles, id)
            .map_or_else(
                || white("?"),
                |profile| (profile.name().as_str().to_owned(), profile.color()),
            ),
        MenuItem::NewProfile => white("New"),
        MenuItem::Color(color) => ("●●●".to_owned(), color),
        MenuItem::EditProfiles => white("Edit"),
        MenuItem::RenameProfile => white("Rename"),
        MenuItem::RecolorProfile => white("Color"),
        MenuItem::DeleteProfile => white("Delete"),
        MenuItem::KeepProfile => white("No"),
        MenuItem::ConfirmDelete => white("Yes"),
    }
}

fn find(profiles: &Profiles, id: ProfileId) -> Option<&Profile> {
    profiles
        .as_slice()
        .iter()
        .find(|profile| profile.id() == id)
}

/// A screen's heading: one word, or the question about the profile being
/// edited.
fn heading(screen: MenuScreen, editing: Option<&Profile>) -> String {
    let name = editing.map_or("?", |profile| profile.name().as_str());
    match screen {
        MenuScreen::Main => TITLE.to_owned(),
        MenuScreen::Settings => "Settings".to_owned(),
        MenuScreen::Volume => "Volume".to_owned(),
        MenuScreen::WindowMode => "Screen".to_owned(),
        MenuScreen::Mode => "Mode".to_owned(),
        MenuScreen::Opponent => "vs".to_owned(),
        MenuScreen::PointsToWin => "Points".to_owned(),
        MenuScreen::PickProfile(_) => "Who?".to_owned(),
        MenuScreen::NameProfile(_) | MenuScreen::RenameProfile(_) => "Name".to_owned(),
        MenuScreen::PickColor(_) | MenuScreen::RecolorProfile(_) => "Color".to_owned(),
        MenuScreen::EditProfiles(_) => "Edit".to_owned(),
        MenuScreen::ProfileActions(_) => name.to_owned(),
        MenuScreen::DeleteProfile(_) => format!("Delete {name}?"),
    }
}

/// The seat cue: the seat's up, down, and confirm keys, each with what it
/// does (plan 8.4): "W↑ S↓ D✓", or "↑ ↓ →✓" where a key is its arrow.
fn seat_cue(keys: SeatKeys) -> String {
    [(keys.up(), "↑"), (keys.down(), "↓"), (keys.action(), "✓")]
        .map(|(key, meaning)| key_doing(key, meaning))
        .join(" ")
}

/// Under the name field: how a name is taken (plan 8.4).
const NAME_HINT: &str = "Enter ✓";

/// Shows the screen just entered: its heading, the seat cue while one seat's
/// keys drive it, the name field on a name screen, and its rows. A screen
/// that one seat drives or types on sits on that seat's half.
pub(super) fn show_screen(
    mut commands: Commands,
    screen: Res<State<MenuScreen>>,
    list: Res<MenuList>,
    draft: Res<MatchDraft>,
    profiles: Res<Profiles>,
    settings: Res<Settings>,
    tuning: Res<Tuning>,
) {
    let screen = *screen.get();
    // Whose keys drive the screen, for the cue; the name screen has none,
    // since its seat's keys type there.
    let cued = match draft.drivers(screen) {
        Some(MenuDrivers::Seat(seat)) => Some(seat),
        Some(MenuDrivers::EitherSeat) | None => None,
    };
    let align_items = match cued.or(screen.seat()) {
        None => AlignItems::Center,
        Some(Seat::Left) => AlignItems::FlexStart,
        Some(Seat::Right) => AlignItems::FlexEnd,
    };
    commands
        .spawn((
            MenuScreenRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items,
                padding: UiRect::axes(Val::Vw(10.0), Val::Px(0.0)),
                row_gap: Val::Vh(1.0),
                ..Node::default()
            },
            DespawnOnExit(screen),
        ))
        .with_children(|root| {
            let editing = draft.editing.and_then(|id| find(&profiles, id));
            root.spawn((
                MenuHeading,
                Text::new(heading(screen, editing)),
                font(9.0),
                TextColor(Color::WHITE),
            ));
            if let Some(seat) = cued {
                root.spawn((
                    SeatCue,
                    Text::new(seat_cue(tuning.bindings().seat(seat))),
                    font(5.0),
                    TextColor(DIM),
                ));
            }
            let typed = match screen {
                MenuScreen::NameProfile(_) => Some(draft.new_name.as_ref()),
                MenuScreen::RenameProfile(_) => Some(editing.map(Profile::name)),
                _ => None,
            };
            if let Some(typed) = typed {
                root.spawn(name_field(typed));
                root.spawn(hint(NAME_HINT));
            }
            // Every row, and both marks hidden: `follow_cursor` shows the
            // right ones this same frame, since entering the screen changed
            // the list.
            root.spawn(scroll_mark(ScrollMark::Above));
            for (index, &item) in list.items().iter().enumerate() {
                let (text, color) = label(item, &profiles, *settings);
                root.spawn((
                    MenuRow(index),
                    row(text, Color::from(color), index == list.cursor()),
                ));
            }
            root.spawn(scroll_mark(ScrollMark::Below));
        });
}

fn scroll_mark(mark: ScrollMark) -> impl Bundle {
    let glyph = match mark {
        ScrollMark::Above => "▲",
        ScrollMark::Below => "▼",
    };
    (
        mark,
        Text::new(glyph),
        font(4.0),
        TextColor(DIM),
        Visibility::Hidden,
    )
}

/// Moves the cursor's mark to the cursor's row and lays out only the shown
/// rows; a scroll mark shows while rows are hidden on its side. Writes a
/// row's `Node` only when its display changes, since any write lays it out
/// again.
pub(super) fn follow_cursor(
    list: Res<MenuList>,
    mut rows: Query<(&MenuRow, &mut BorderColor, &mut Node)>,
    mut marks: Query<(&ScrollMark, &mut Visibility)>,
) {
    let shown = list.shown();
    for (row, mut border, mut node) in &mut rows {
        *border = cursor_frame(row.0 == list.cursor());
        let display = if shown.contains(&row.0) {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    for (mark, mut visibility) in &mut marks {
        let hidden_beyond = match mark {
            ScrollMark::Above => shown.start > 0,
            ScrollMark::Below => shown.end < list.items().len(),
        };
        *visibility = if hidden_beyond {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;

    use bevy::color::Srgba;
    use pong_core::domain::{Difficulty, Mode, Player, ProfileId};
    use pong_core::features::persistence::model::{
        Profile, ProfileName, Profiles, RecordsByMode, Settings, WindowMode,
    };

    use super::{heading, label};
    use crate::ui::menu::{MenuItem, MenuScreen};

    fn mia() -> Profiles {
        let profile = Profile::new(
            ProfileId(4),
            ProfileName::new("Mia").expect("valid"),
            Srgba::BLUE,
            RecordsByMode::default(),
        )
        .expect("valid");
        Profiles::new(vec![profile]).expect("one profile")
    }

    /// Single words or icons where a nine-year-old reads (spec 1.6.3 SHOULD).
    #[test]
    fn rows_read_as_single_words_numbers_or_stars() {
        let settings = Settings {
            window_mode: WindowMode::Windowed,
            ..Settings::default()
        };
        for (item, expected) in [
            (MenuItem::Play, "Play"),
            (MenuItem::Computer(Difficulty::OneStar), "★"),
            (MenuItem::Computer(Difficulty::ThreeStars), "★★★"),
            (MenuItem::OpenVolume, "Volume 80"),
            (MenuItem::OpenWindowMode, "Window"),
            (MenuItem::Player(Player::Guest), "Guest"),
            (MenuItem::Player(Player::Profile(ProfileId(4))), "Mia"),
            (MenuItem::NewProfile, "New"),
            (MenuItem::EditProfiles, "Edit"),
            (MenuItem::EditProfile(ProfileId(4)), "Mia"),
            (MenuItem::RenameProfile, "Rename"),
            (MenuItem::DeleteProfile, "Delete"),
            (MenuItem::KeepProfile, "No"),
            (MenuItem::ConfirmDelete, "Yes"),
        ] {
            assert_eq!(label(item, &mia(), settings).0, expected, "{item:?}");
        }
    }

    /// M2 spec 2.8.1: Classic, Chaos ●, Chaos ●●, Chaos ●●●.
    #[test]
    fn mode_rows_read_as_the_mode_and_a_dot_per_effect_at_once() {
        let chaos = |cap| Mode::Chaos {
            cap: NonZeroU8::new(cap).expect("above zero"),
        };
        for (mode, expected) in [
            (Mode::Classic, "Classic"),
            (chaos(1), "Chaos ●"),
            (chaos(3), "Chaos ●●●"),
        ] {
            assert_eq!(
                label(MenuItem::Mode(mode), &mia(), Settings::default()).0,
                expected
            );
        }
    }

    #[test]
    fn the_mode_screen_is_headed_by_one_word() {
        assert_eq!(heading(MenuScreen::Mode, None), "Mode");
    }

    #[test]
    fn a_profile_and_a_colour_show_in_their_colour() {
        let settings = Settings::default();
        assert_eq!(
            label(
                MenuItem::Player(Player::Profile(ProfileId(4))),
                &mia(),
                settings
            )
            .1,
            Srgba::BLUE
        );
        assert_eq!(
            label(MenuItem::Color(Srgba::RED), &mia(), settings).1,
            Srgba::RED
        );
    }
}

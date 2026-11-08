//! What the menu screens show (spec 1.6.3): a heading, the seat cue on a
//! seat's own screens, and one row per list item with the cursor's row
//! marked. A seat's screens sit on that seat's half of the screen.

use bevy::color::{Color, Srgba};
use bevy::ecs::children;
use bevy::ecs::component::Component;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::input::keyboard::KeyCode;
use bevy::state::state::State;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::text::{FontSize, TextColor, TextFont};
use bevy::ui::widget::Text;
use bevy::ui::{AlignItems, BorderColor, FlexDirection, JustifyContent, Node, UiRect, Val};
use pong_core::domain::{Player, Seat};
use pong_core::features::persistence::model::{Profiles, Settings, WindowMode};
use pong_core::features::tuning::{SeatKeys, Tuning};

use super::name_entry::name_field;
use super::{MatchDraft, MenuItem, MenuList, MenuScreen};
use crate::ui::menu_input::MenuDrivers;
use crate::window::TITLE;

/// Marks a screen's outermost node; despawned, with all it holds, when the
/// screen is left.
#[derive(Component, Debug, Default)]
pub struct MenuScreenRoot;

/// One list row, by its index in [`MenuList::items`].
#[derive(Component, Debug)]
pub struct MenuRow(pub usize);

/// The keys of the seat whose turn it is (spec 1.6.3, seat cue).
#[derive(Component, Debug, Default)]
pub struct SeatCue;

/// The cursor's mark: a white frame around its row.
const CURSOR: Color = Color::WHITE;

const DIM: Color = Color::srgb(0.6, 0.6, 0.7);

fn font(viewport_percent: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Vh(viewport_percent),
        ..TextFont::default()
    }
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
        MenuItem::Friend => white("Friend"),
        MenuItem::Computer(difficulty) => white(&"★".repeat(usize::from(difficulty.stars()))),
        MenuItem::PointsToWin(points) => white(&points.to_string()),
        MenuItem::Player(Player::Guest) => white("Guest"),
        MenuItem::Player(Player::Profile(id)) => profiles
            .as_slice()
            .iter()
            .find(|profile| profile.id() == id)
            .map_or_else(
                || white("?"),
                |profile| (profile.name().as_str().to_owned(), profile.color()),
            ),
        MenuItem::NewProfile => white("New"),
        MenuItem::Color(color) => ("●●●".to_owned(), color),
    }
}

/// A screen's one-word heading.
const fn heading(screen: MenuScreen) -> &'static str {
    match screen {
        MenuScreen::Main => TITLE,
        MenuScreen::Settings => "Settings",
        MenuScreen::Volume => "Volume",
        MenuScreen::WindowMode => "Screen",
        MenuScreen::Opponent => "vs",
        MenuScreen::PointsToWin => "Points",
        MenuScreen::PickProfile(_) => "Who?",
        MenuScreen::NameProfile(_) => "Name",
        MenuScreen::PickColor(_) => "Color",
    }
}

/// A key as printed on it: letters and digits bare, arrows as arrows.
fn key_label(key: KeyCode) -> String {
    match key {
        KeyCode::ArrowUp => "↑".to_owned(),
        KeyCode::ArrowDown => "↓".to_owned(),
        KeyCode::ArrowLeft => "←".to_owned(),
        KeyCode::ArrowRight => "→".to_owned(),
        other => {
            let name = format!("{other:?}");
            name.strip_prefix("Key")
                .or_else(|| name.strip_prefix("Digit"))
                .map_or_else(|| name.clone(), str::to_owned)
        }
    }
}

/// The seat cue: the seat's up, down, and confirm keys.
fn seat_cue(keys: SeatKeys) -> String {
    [keys.up(), keys.down(), keys.action()]
        .map(key_label)
        .join(" ")
}

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
            root.spawn((
                Text::new(heading(screen)),
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
            if let MenuScreen::NameProfile(_) = screen {
                root.spawn(name_field(draft.new_name.as_ref()));
            }
            for (index, &item) in list.items().iter().enumerate() {
                let (text, color) = label(item, &profiles, *settings);
                root.spawn((
                    MenuRow(index),
                    Node {
                        padding: UiRect::axes(Val::Vh(2.0), Val::Vh(0.5)),
                        border: UiRect::all(Val::Vh(0.4)),
                        ..Node::default()
                    },
                    BorderColor::all(row_border(index, list.cursor())),
                    children![(Text::new(text), font(6.0), TextColor(Color::from(color)))],
                ));
            }
        });
}

fn row_border(index: usize, cursor: usize) -> Color {
    if index == cursor { CURSOR } else { Color::NONE }
}

/// Moves the cursor's mark to the cursor's row.
pub(super) fn mark_cursor(list: Res<MenuList>, mut rows: Query<(&MenuRow, &mut BorderColor)>) {
    for (row, mut border) in &mut rows {
        *border = BorderColor::all(row_border(row.0, list.cursor()));
    }
}

#[cfg(test)]
mod tests {
    use bevy::color::Srgba;
    use pong_core::domain::{Difficulty, Player, ProfileId};
    use pong_core::features::persistence::model::{
        Profile, ProfileName, Profiles, Records, Settings, WindowMode,
    };

    use super::label;
    use crate::ui::menu::MenuItem;

    fn mia() -> Profiles {
        let profile = Profile::new(
            ProfileId(4),
            ProfileName::new("Mia").expect("valid"),
            Srgba::BLUE,
            Records::default(),
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
        ] {
            assert_eq!(label(item, &mia(), settings).0, expected, "{item:?}");
        }
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

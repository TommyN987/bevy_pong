//! Menu keys (spec 1.6.3, decisions "Keys"): each seat's up and down move
//! the cursor, its action key (D/Right) confirms, its back key (A/Left) backs
//! out; Escape backs out for anyone.

use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use pong_core::domain::Seat;
use pong_core::features::tuning::BindingsTuning;

/// What a seat's menu key means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKey {
    /// The cursor one row up.
    Up,
    /// The cursor one row down.
    Down,
    /// Picks the row under the cursor.
    Confirm,
    /// The screen before.
    Back,
}

/// One key press a menu acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuPress {
    /// One of `Seat`'s keys.
    Seat(Seat, MenuKey),
    /// Escape, which backs out whoever presses it.
    Escape,
}

/// Whose keys drive a screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuDrivers {
    /// A shared screen: both seats' keys.
    EitherSeat,
    /// That seat's turn: only its keys (spec 1.6.3).
    Seat(Seat),
}

impl MenuDrivers {
    fn includes(self, seat: Seat) -> bool {
        match self {
            Self::EitherSeat => true,
            Self::Seat(driver) => driver == seat,
        }
    }
}

/// A key as printed on it: letters and digits bare, arrows as arrows.
#[must_use]
pub fn key_label(key: KeyCode) -> String {
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

/// `key` with what it does, as an icon: "W↑", "D✓"; the icon alone where
/// the key is printed with it, as ↑ is.
#[must_use]
pub fn key_doing(key: KeyCode, meaning: &str) -> String {
    let label = key_label(key);
    if label == meaning {
        label
    } else {
        format!("{label}{meaning}")
    }
}

/// The press `drivers` may make this frame: a key pressed this frame, not
/// one held from before, so a held key moves the cursor once. Escape first,
/// then the left seat's keys, then the right's, each in up, down, confirm,
/// back order.
#[must_use]
pub fn menu_press(
    keys: &ButtonInput<KeyCode>,
    bindings: BindingsTuning,
    drivers: MenuDrivers,
) -> Option<MenuPress> {
    if keys.just_pressed(KeyCode::Escape) {
        return Some(MenuPress::Escape);
    }
    [Seat::Left, Seat::Right]
        .into_iter()
        .filter(|&seat| drivers.includes(seat))
        .find_map(|seat| {
            let seat_keys = bindings.seat(seat);
            [
                (seat_keys.up(), MenuKey::Up),
                (seat_keys.down(), MenuKey::Down),
                (seat_keys.action(), MenuKey::Confirm),
                (seat_keys.back(), MenuKey::Back),
            ]
            .into_iter()
            .find(|&(key, _)| keys.just_pressed(key))
            .map(|(_, meaning)| MenuPress::Seat(seat, meaning))
        })
}

#[cfg(test)]
mod tests {
    use bevy::input::ButtonInput;
    use bevy::input::keyboard::KeyCode;
    use pong_core::domain::Seat;
    use pong_core::features::tuning::{BindingsTuning, EMBEDDED_RON, Tuning};

    use super::{MenuDrivers, MenuKey, MenuPress, menu_press};

    fn bindings() -> BindingsTuning {
        Tuning::from_ron(EMBEDDED_RON)
            .expect("the embedded tuning is valid")
            .bindings()
    }

    fn pressing(key: KeyCode) -> ButtonInput<KeyCode> {
        let mut keys = ButtonInput::default();
        keys.press(key);
        keys
    }

    #[test]
    fn each_seats_keys_move_confirm_and_back_out_naming_the_seat() {
        for seat in [Seat::Left, Seat::Right] {
            let keys = bindings().seat(seat);
            for (key, meaning) in [
                (keys.up(), MenuKey::Up),
                (keys.down(), MenuKey::Down),
                (keys.action(), MenuKey::Confirm),
                (keys.back(), MenuKey::Back),
            ] {
                assert_eq!(
                    menu_press(&pressing(key), bindings(), MenuDrivers::EitherSeat),
                    Some(MenuPress::Seat(seat, meaning)),
                    "{key:?}"
                );
            }
        }
    }

    /// Spec 1.6.3: during a seat's turn the other seat's keys do nothing.
    #[test]
    fn during_one_seats_turn_the_other_seats_keys_do_nothing() {
        let right = bindings().seat(Seat::Right);
        for key in [right.up(), right.down(), right.action(), right.back()] {
            assert_eq!(
                menu_press(&pressing(key), bindings(), MenuDrivers::Seat(Seat::Left)),
                None,
                "{key:?}"
            );
        }
        let left = bindings().seat(Seat::Left);
        assert_eq!(
            menu_press(
                &pressing(left.down()),
                bindings(),
                MenuDrivers::Seat(Seat::Left)
            ),
            Some(MenuPress::Seat(Seat::Left, MenuKey::Down))
        );
    }

    #[test]
    fn escape_backs_out_during_any_turn() {
        for drivers in [
            MenuDrivers::EitherSeat,
            MenuDrivers::Seat(Seat::Left),
            MenuDrivers::Seat(Seat::Right),
        ] {
            assert_eq!(
                menu_press(&pressing(KeyCode::Escape), bindings(), drivers),
                Some(MenuPress::Escape),
                "{drivers:?}"
            );
        }
    }

    /// One press, one step: a held key does not repeat.
    #[test]
    fn a_key_held_from_an_earlier_frame_or_none_at_all_does_nothing() {
        let mut keys = pressing(bindings().seat(Seat::Left).down());
        keys.clear();
        assert_eq!(menu_press(&keys, bindings(), MenuDrivers::EitherSeat), None);
        assert_eq!(
            menu_press(&ButtonInput::default(), bindings(), MenuDrivers::EitherSeat),
            None
        );
    }
}

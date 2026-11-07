//! Keyboard adapter (spec 1.6.1): each seat's keys, bound in tuning, become
//! that seat's paddle intent in the core's `SeatIntents` port.

use bevy::ecs::system::{Local, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::log::debug;
use pong_core::domain::Seat;
use pong_core::features::seats::{PaddleIntent, SeatIntents};
use pong_core::features::tuning::{SeatKeys, Tuning};

/// What `seat`'s keys ask for this frame.
///
/// A key counts if it is held, or was pressed and released within this frame
/// (`just_pressed` without `pressed`): the seats contract reports such a tap as
/// held for its frame, so the core's latch can keep it. Up and down together
/// cancel to `Still`.
#[must_use]
pub fn seat_intent(keys: &ButtonInput<KeyCode>, seat: SeatKeys) -> PaddleIntent {
    let active = |key: KeyCode| keys.pressed(key) || keys.just_pressed(key);
    match (active(seat.up()), active(seat.down())) {
        (true, false) => PaddleIntent::Up,
        (false, true) => PaddleIntent::Down,
        (true, true) | (false, false) => PaddleIntent::Still,
    }
}

/// The bound keys held now: the left seat's first, each seat's in up, down,
/// action, back order. The keyboard ghosting check (spec 1.8.2) reads this.
#[must_use]
pub fn held_seat_keys(keys: &ButtonInput<KeyCode>, tuning: &Tuning) -> Vec<KeyCode> {
    [Seat::Left, Seat::Right]
        .into_iter()
        .map(|seat| tuning.bindings().seat(seat))
        .flat_map(|seat| [seat.up(), seat.down(), seat.action(), seat.back()])
        .filter(|&key| keys.pressed(key))
        .collect()
}

/// Reports both seats every frame in which the paddles move. A seat the AI
/// holds is reported too; the core ignores it.
pub(super) fn report_seat_intents(
    keys: Res<ButtonInput<KeyCode>>,
    tuning: Res<Tuning>,
    mut seats: ResMut<SeatIntents>,
) {
    let bindings = tuning.bindings();
    for seat in [Seat::Left, Seat::Right] {
        seats.report(seat, seat_intent(&keys, bindings.seat(seat)));
    }
}

/// Logs the bound keys held at debug level whenever they change, in every
/// state once tuning has loaded: the keyboard ghosting check (spec 1.8.2,
/// `just key-check`). A key that is held but missing from the line was lost
/// by the keyboard. Silent unless `RUST_LOG` enables this module's debug.
pub(super) fn log_held_seat_keys(
    keys: Res<ButtonInput<KeyCode>>,
    tuning: Res<Tuning>,
    mut last: Local<Vec<KeyCode>>,
) {
    let held = held_seat_keys(&keys, &tuning);
    if held != *last {
        debug!("held seat keys ({}): {held:?}", held.len());
        *last = held;
    }
}

#[cfg(test)]
mod tests {
    use bevy::input::ButtonInput;
    use bevy::input::keyboard::KeyCode;
    use pong_core::domain::Seat;
    use pong_core::features::seats::PaddleIntent;
    use pong_core::features::tuning::{EMBEDDED_RON, SeatKeys, Tuning};

    use super::{held_seat_keys, seat_intent};

    fn tuning() -> Tuning {
        Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid")
    }

    fn keys_of(seat: Seat) -> SeatKeys {
        tuning().bindings().seat(seat)
    }

    /// Spec 1.8.2: up, down, and action of both seats, six keys at once.
    #[test]
    fn all_six_ghosting_check_keys_held_are_listed_and_unbound_keys_are_not() {
        let (left, right) = (keys_of(Seat::Left), keys_of(Seat::Right));
        let six = [
            left.up(),
            left.down(),
            left.action(),
            right.up(),
            right.down(),
            right.action(),
        ];
        let mut keys = ButtonInput::<KeyCode>::default();
        for key in six {
            keys.press(key);
        }
        keys.press(KeyCode::Space);
        assert_eq!(held_seat_keys(&keys, &tuning()), six);
    }

    #[test]
    fn a_held_key_maps_to_its_intent() {
        let seat = keys_of(Seat::Left);
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(seat.up());
        assert_eq!(seat_intent(&keys, seat), PaddleIntent::Up);
        keys.release(seat.up());
        keys.clear();
        keys.press(seat.down());
        assert_eq!(seat_intent(&keys, seat), PaddleIntent::Down);
    }

    #[test]
    fn a_key_pressed_and_released_within_the_frame_still_counts() {
        let seat = keys_of(Seat::Right);
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(seat.down());
        keys.release(seat.down());
        assert!(!keys.pressed(seat.down()));
        assert_eq!(seat_intent(&keys, seat), PaddleIntent::Down);
    }

    #[test]
    fn up_and_down_together_or_neither_is_still() {
        let seat = keys_of(Seat::Left);
        let mut keys = ButtonInput::<KeyCode>::default();
        assert_eq!(seat_intent(&keys, seat), PaddleIntent::Still);
        keys.press(seat.up());
        keys.press(seat.down());
        assert_eq!(seat_intent(&keys, seat), PaddleIntent::Still);
    }

    #[test]
    fn one_seats_keys_do_not_move_the_other_seat() {
        let left = keys_of(Seat::Left);
        let right = keys_of(Seat::Right);
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(left.up());
        keys.press(left.action());
        keys.press(left.back());
        assert_eq!(seat_intent(&keys, right), PaddleIntent::Still);
    }
}

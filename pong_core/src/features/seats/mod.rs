//! Seats and intents (spec 1.4.1): the port through which a seated human's keys
//! become a paddle intent, and the per-side intents the paddles consume.

use bevy::app::{App, FixedUpdate, Plugin};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Res, ResMut};
use bevy::state::state::OnEnter;

use crate::domain::{Controller, MatchConfig, Seat};
use crate::flow::phases::FixedStepPhase;
use crate::flow::states::MatchState;
use crate::kernel::geometry::Side;

/// Registers the [`SeatIntents`] port and the [`PaddleIntents`] output,
/// resolves human seats onto their sides in the intent phase, and forgets
/// every reported intent on entering a countdown.
///
/// Precondition: [`MatchConfig`] exists whenever the phases run; the menu
/// inserts it before entering a match.
///
/// Contract for the shell adapter: report every `Update` while a match is in
/// progress, and only then; report a key pressed and released within one frame
/// (Bevy's `just_pressed`) as held for that frame, or the tap is lost before
/// the latch sees it; call [`SeatIntents::clear`] on focus loss (spec 1.6.1).
pub struct SeatsPlugin;

impl Plugin for SeatsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SeatIntents>()
            .init_resource::<PaddleIntents>()
            .add_systems(OnEnter(MatchState::Countdown), forget_reported_intents)
            .add_systems(
                FixedUpdate,
                resolve_human_intents.in_set(FixedStepPhase::Intent),
            );
    }
}

/// A key press from the menu or the last rally must not become the first
/// paddle move of the next serve.
fn forget_reported_intents(mut seats: ResMut<SeatIntents>) {
    seats.clear();
}

/// What each side's paddle is asked to do this fixed step. Written in the
/// intent phase: by this feature for sides a human holds, by the AI feature
/// for sides it holds. Read by the paddle feature in the integrate phase.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PaddleIntents {
    left: PaddleIntent,
    right: PaddleIntent,
}

impl PaddleIntents {
    /// The intent for the paddle on `side`.
    #[must_use]
    pub const fn side(self, side: Side) -> PaddleIntent {
        match side {
            Side::Left => self.left,
            Side::Right => self.right,
        }
    }

    /// Sets the intent for the paddle on `side`.
    pub const fn set(&mut self, side: Side, intent: PaddleIntent) {
        match side {
            Side::Left => self.left = intent,
            Side::Right => self.right = intent,
        }
    }
}

/// Each side held by a human takes that seat's intent for this step.
fn resolve_human_intents(
    config: Res<MatchConfig>,
    mut seats: ResMut<SeatIntents>,
    mut paddles: ResMut<PaddleIntents>,
) {
    for side in [Side::Left, Side::Right] {
        match config.controller(side) {
            Controller::Human(seat) => paddles.set(side, seats.consume(seat)),
            Controller::Ai(_) => {}
        }
    }
}

/// What a paddle is asked to do this fixed step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaddleIntent {
    /// Stay where it is.
    #[default]
    Still,
    /// Move toward `+y`.
    Up,
    /// Move toward `-y`.
    Down,
}

/// Port: what each seat's keys are asking for. The shell's keyboard adapter
/// (or a test) calls [`report`](Self::report) every `Update`; the intent phase
/// calls [`consume`](Self::consume) every fixed step.
///
/// Two things are kept per seat. `held` is the level: whatever was reported
/// last, so a held key moves the paddle on every fixed step however many run
/// in a frame. `tapped` latches any non-still report until a fixed step
/// consumes it, so a press shorter than one frame still moves the paddle, once.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SeatIntents {
    left: SeatIntent,
    right: SeatIntent,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct SeatIntent {
    held: PaddleIntent,
    tapped: PaddleIntent,
}

impl SeatIntents {
    /// Records what `seat`'s keys ask for right now. Call once per `Update`.
    pub fn report(&mut self, seat: Seat, held: PaddleIntent) {
        let entry = self.entry_mut(seat);
        entry.held = held;
        if held != PaddleIntent::Still {
            entry.tapped = held;
        }
    }

    /// Forgets everything for both seats, for focus loss and pause.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// The intent for this fixed step: the held level if any, else the latched
    /// tap, which is spent by this call.
    pub fn consume(&mut self, seat: Seat) -> PaddleIntent {
        let entry = self.entry_mut(seat);
        let tapped = std::mem::take(&mut entry.tapped);
        if entry.held == PaddleIntent::Still {
            tapped
        } else {
            entry.held
        }
    }

    const fn entry_mut(&mut self, seat: Seat) -> &mut SeatIntent {
        match seat {
            Seat::Left => &mut self.left,
            Seat::Right => &mut self.right,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;

    use bevy::app::App;

    use bevy::state::state::NextState;

    use super::{PaddleIntent, PaddleIntents, SeatIntents};
    use crate::domain::{Controller, Difficulty, MatchConfig, Seat};
    use crate::flow::states::MatchState;
    use crate::kernel::geometry::Side;
    use crate::testing::{
        boot, enter_match, frame, headless_app, match_state, report_intent, run_until, start_match,
    };

    fn two_humans() -> MatchConfig {
        MatchConfig {
            left: Controller::Human(Seat::Left),
            right: Controller::Human(Seat::Right),
            points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
        }
    }

    fn remaining_intent(app: &mut App, seat: Seat) -> PaddleIntent {
        app.world_mut().resource_mut::<SeatIntents>().consume(seat)
    }

    #[test]
    fn a_key_press_from_the_menu_is_forgotten_on_entering_the_first_countdown() {
        let mut app = headless_app(1);
        boot(&mut app);
        report_intent(&mut app, Seat::Left, PaddleIntent::Up);
        enter_match(&mut app, two_humans());
        assert_eq!(match_state(&app), MatchState::Countdown);
        assert_eq!(remaining_intent(&mut app, Seat::Left), PaddleIntent::Still);
    }

    #[test]
    fn a_key_press_from_the_last_rally_is_forgotten_on_entering_every_later_countdown() {
        let mut app = headless_app(1);
        boot(&mut app);
        start_match(&mut app, two_humans());
        report_intent(&mut app, Seat::Right, PaddleIntent::Down);
        app.world_mut()
            .resource_mut::<NextState<MatchState>>()
            .set(MatchState::Countdown);
        run_until(
            &mut app,
            |app| match_state(app) == MatchState::Countdown,
            2,
            "no countdown",
        );
        assert_eq!(remaining_intent(&mut app, Seat::Right), PaddleIntent::Still);
    }

    fn right_seat_plays_left_side_against_ai() -> MatchConfig {
        MatchConfig {
            left: Controller::Human(Seat::Right),
            right: Controller::Ai(Difficulty::TwoStars),
            points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
        }
    }

    fn report(app: &mut App, seat: Seat, intent: PaddleIntent) {
        app.world_mut()
            .resource_mut::<SeatIntents>()
            .report(seat, intent);
    }

    #[test]
    fn a_human_seat_drives_its_side_and_an_ai_side_is_left_alone() {
        let mut app = headless_app(1);
        boot(&mut app);
        start_match(&mut app, right_seat_plays_left_side_against_ai());
        report(&mut app, Seat::Right, PaddleIntent::Up);
        report(&mut app, Seat::Left, PaddleIntent::Down);
        frame(&mut app);
        let intents = *app.world().resource::<PaddleIntents>();
        assert_eq!(intents.side(Side::Left), PaddleIntent::Up);
        assert_eq!(intents.side(Side::Right), PaddleIntent::Still);
    }

    #[test]
    fn outside_a_match_reports_do_not_reach_the_paddles() {
        let mut app = headless_app(1);
        boot(&mut app);
        report(&mut app, Seat::Left, PaddleIntent::Up);
        frame(&mut app);
        let intents = *app.world().resource::<PaddleIntents>();
        assert_eq!(intents.side(Side::Left), PaddleIntent::Still);
    }

    #[test]
    fn a_held_key_moves_on_every_fixed_step_of_the_frame() {
        let mut port = SeatIntents::default();
        port.report(Seat::Left, PaddleIntent::Up);
        assert_eq!(port.consume(Seat::Left), PaddleIntent::Up);
        assert_eq!(port.consume(Seat::Left), PaddleIntent::Up);
    }

    #[test]
    fn a_tap_shorter_than_a_frame_is_not_missed_and_moves_exactly_once() {
        let mut port = SeatIntents::default();
        // Pressed and released with no fixed step in between.
        port.report(Seat::Left, PaddleIntent::Down);
        port.report(Seat::Left, PaddleIntent::Still);
        assert_eq!(port.consume(Seat::Left), PaddleIntent::Down);
        assert_eq!(port.consume(Seat::Left), PaddleIntent::Still);
    }

    #[test]
    fn a_tap_consumed_during_its_frame_is_not_repeated_after_release() {
        let mut port = SeatIntents::default();
        port.report(Seat::Right, PaddleIntent::Up);
        assert_eq!(port.consume(Seat::Right), PaddleIntent::Up);
        port.report(Seat::Right, PaddleIntent::Still);
        assert_eq!(port.consume(Seat::Right), PaddleIntent::Still);
    }

    #[test]
    fn clear_drops_held_and_latched_intents_for_both_seats() {
        let mut port = SeatIntents::default();
        port.report(Seat::Left, PaddleIntent::Up);
        port.report(Seat::Right, PaddleIntent::Down);
        port.clear();
        assert_eq!(port.consume(Seat::Left), PaddleIntent::Still);
        assert_eq!(port.consume(Seat::Right), PaddleIntent::Still);
    }

    #[test]
    fn seats_do_not_leak_into_each_other() {
        let mut port = SeatIntents::default();
        port.report(Seat::Left, PaddleIntent::Up);
        assert_eq!(port.consume(Seat::Right), PaddleIntent::Still);
        assert_eq!(port.consume(Seat::Left), PaddleIntent::Up);
    }
}

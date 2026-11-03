//! Seats and intents (spec 1.4.1): the port through which a seated human's keys
//! become a paddle intent, and the per-side intents the paddles consume.

use bevy::app::{App, FixedUpdate, Plugin};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::schedule::common_conditions::resource_exists;
use bevy::ecs::system::{Res, ResMut};

use crate::app::phases::FixedStepPhase;
use crate::domain::{Controller, MatchConfig, Seat};
use crate::kernel::geometry::Side;

/// Registers the [`SeatIntents`] port and the [`PaddleIntents`] output, and
/// resolves human seats onto their sides in the intent phase.
///
/// Contract for the shell adapter: report intents only while a match is in
/// progress, and call [`SeatIntents::clear`] on focus loss (spec 1.6.6).
pub struct SeatsPlugin;

impl Plugin for SeatsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SeatIntents>()
            .init_resource::<PaddleIntents>()
            .add_systems(
                FixedUpdate,
                resolve_human_intents
                    .in_set(FixedStepPhase::Intent)
                    .run_if(resource_exists::<MatchConfig>),
            );
    }
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
#[expect(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are taken by value"
)]
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

    use bevy::MinimalPlugins;
    use bevy::app::{App, FixedUpdate};

    use super::{PaddleIntent, PaddleIntents, SeatIntents, SeatsPlugin};
    use crate::app::phases::{PhasesPlugin, unphased_fixed_step_systems};
    use crate::domain::{Controller, Difficulty, MatchConfig, Seat};
    use crate::kernel::geometry::Side;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, PhasesPlugin, SeatsPlugin));
        app
    }

    fn right_seat_plays_left_side_against_ai() -> MatchConfig {
        MatchConfig {
            left: Controller::Human(Seat::Right),
            right: Controller::Ai(Difficulty::TwoStars),
            points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
        }
    }

    #[test]
    fn a_human_seat_drives_its_side_and_an_ai_side_is_left_alone() {
        let mut app = app();
        app.insert_resource(right_seat_plays_left_side_against_ai());
        app.world_mut()
            .resource_mut::<SeatIntents>()
            .report(Seat::Right, PaddleIntent::Up);
        app.world_mut()
            .resource_mut::<SeatIntents>()
            .report(Seat::Left, PaddleIntent::Down);
        app.world_mut().run_schedule(FixedUpdate);
        let intents = *app.world().resource::<PaddleIntents>();
        assert_eq!(intents.side(Side::Left), PaddleIntent::Up);
        assert_eq!(intents.side(Side::Right), PaddleIntent::Still);
    }

    #[test]
    fn without_a_match_config_the_fixed_step_runs_and_intents_stay_still() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<SeatIntents>()
            .report(Seat::Left, PaddleIntent::Up);
        app.world_mut().run_schedule(FixedUpdate);
        let intents = *app.world().resource::<PaddleIntents>();
        assert_eq!(intents.side(Side::Left), PaddleIntent::Still);
    }

    #[test]
    fn the_seats_system_declares_its_phase() {
        let mut app = app();
        assert_eq!(unphased_fixed_step_systems(&mut app), Vec::<String>::new());
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

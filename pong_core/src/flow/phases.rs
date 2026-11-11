//! Fixed-step phases (spec 1.4.10, docs/architecture.md rule 4): declared once,
//! in order. Every core system in `FixedUpdate` names its phase and never orders
//! itself against another feature's system.

use bevy::app::{App, FixedFirst, FixedUpdate};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{Res, ResMut};
use bevy::state::state::{NextState, State};

use super::states::{AppState, MatchState};

/// The named sets one fixed step runs, in this order.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedStepPhase {
    /// Port resources become paddle intents (seats, AI).
    Intent,
    /// The effects in play are kept current; a serve starts a flight (ADR 0011).
    Effects,
    /// Paddles and ball move.
    Integrate,
    /// Swept collisions, reflections, hit events.
    Collide,
    /// Scoring, serve, match flow.
    Rules,
    /// Invariant checks; faults heal or panic per profile.
    Sanity,
}

impl FixedStepPhase {
    /// Every phase, in execution order. The single declaration of the order.
    pub const IN_ORDER: [Self; 6] = [
        Self::Intent,
        Self::Effects,
        Self::Integrate,
        Self::Collide,
        Self::Rules,
        Self::Sanity,
    ];
}

/// Fixed-step systems that serve, move, collide, or judge the ball. A system
/// joins this set beside its phase, and then runs only while the ball is in
/// play: in [`MatchState::Playing`], with no transition pending. The rest of a
/// phase also runs in the waits between rallies, where the paddles move and
/// the ball waits (ADR 0004).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BallInPlay;

/// Configures the phases and [`BallInPlay`] in `FixedUpdate`. Features add
/// systems with `.in_set(FixedStepPhase::X)`, plus `.in_set(BallInPlay)` for
/// the ball, and nothing else about ordering or gating.
///
/// Every phase runs only while the [`StepGate`] is open: in a substate whose
/// paddles move ([`MatchState::paddles_move`]), with no transition pending.
/// The simulation's resources and entities exist only in a match, and nothing
/// moves during a pause or after the match. No feature system guards for any
/// of that itself.
pub(super) fn configure(app: &mut App) {
    app.init_resource::<StepGate>()
        .add_systems(FixedFirst, open_or_close_step_gate)
        .configure_sets(FixedUpdate, BallInPlay.run_if(ball_is_in_play));
    for phase in FixedStepPhase::IN_ORDER {
        app.configure_sets(FixedUpdate, phase.run_if(step_gate_is_open));
    }
    for pair in FixedStepPhase::IN_ORDER.windows(2) {
        app.configure_sets(FixedUpdate, pair[1].after(pair[0]));
    }
}

/// Whether the current fixed step runs the phases, and the ball's systems
/// among them (ADR 0003, ADR 0004).
///
/// Decided once per step, in `FixedFirst`, so all phases of a step agree.
/// Invariant: `open` only if the paddles move in the current `MatchState` and
/// neither `MatchState` nor `AppState` has a transition pending;
/// `ball_in_play` only if also `open` and `MatchState` is `Playing`. A
/// transition requested inside a fixed step applies only at the next frame's
/// `StateTransition`; the step that requests it finishes, and every later step
/// of that frame stays shut.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
struct StepGate {
    open: bool,
    ball_in_play: bool,
}

/// Runs in `FixedFirst`, outside the phases it gates: the one fixed-step core
/// system with no phase (architecture rule 4), so the schedule walker, which
/// checks `FixedUpdate`, does not list it.
fn open_or_close_step_gate(
    match_state: Option<Res<State<MatchState>>>,
    next_match_state: Res<NextState<MatchState>>,
    next_app_state: Res<NextState<AppState>>,
    mut gate: ResMut<StepGate>,
) {
    let state = match_state.map(|state| *state.get());
    let settled = matches!(*next_match_state, NextState::Unchanged)
        && matches!(*next_app_state, NextState::Unchanged);
    gate.open = settled && state.is_some_and(MatchState::paddles_move);
    gate.ball_in_play = gate.open && state == Some(MatchState::Playing);
}

fn step_gate_is_open(gate: Res<StepGate>) -> bool {
    gate.open
}

fn ball_is_in_play(gate: Res<StepGate>) -> bool {
    gate.ball_in_play
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;

    use bevy::app::{App, FixedUpdate};
    use bevy::ecs::resource::Resource;
    use bevy::ecs::schedule::IntoScheduleConfigs;
    use bevy::ecs::system::{Local, ResMut};
    use bevy::state::state::NextState;

    use super::{BallInPlay, FixedStepPhase};
    use crate::domain::{Controller, MatchConfig, Mode, Seat, SeatPlayers};
    use crate::flow::states::MatchState;
    use crate::testing::{
        FrameLength, boot, frame, headless_app, match_state, set_frame_length, start_match,
        unphased_fixed_step_systems,
    };

    #[derive(Resource, Default)]
    struct Ran(Vec<FixedStepPhase>);

    /// Stands in for rules ending play: on its first run it asks for a point pause.
    fn stop_play_once(mut done: Local<bool>, mut next: ResMut<NextState<MatchState>>) {
        if !*done {
            next.set(MatchState::PointScored);
            *done = true;
        }
    }

    /// ADR 0003: once a fixed step asks to leave `Playing`, that step finishes
    /// every phase, and no later step of the same frame runs any.
    #[test]
    fn the_step_that_stops_play_finishes_every_phase_and_the_rest_of_its_frame_runs_none() {
        let mut app = app();
        set_frame_length(&mut app, FrameLength::Steps(4));
        app.add_systems(
            FixedUpdate,
            (
                record(FixedStepPhase::Intent).in_set(FixedStepPhase::Intent),
                stop_play_once.in_set(FixedStepPhase::Rules),
                record(FixedStepPhase::Sanity).in_set(FixedStepPhase::Sanity),
            ),
        );
        frame(&mut app);
        assert_eq!(
            app.world().resource::<Ran>().0,
            vec![FixedStepPhase::Intent, FixedStepPhase::Sanity]
        );
    }

    /// A full core app already in play, so the phases run.
    fn app() -> App {
        let mut app = headless_app(1);
        app.init_resource::<Ran>();
        boot(&mut app);
        start_match(
            &mut app,
            MatchConfig {
                left: Controller::Human(Seat::Left),
                right: Controller::Human(Seat::Right),
                players: SeatPlayers::GUESTS,
                points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
                mode: Mode::Classic,
            },
        );
        app
    }

    /// Records `Intent` as any phase system would, and `Collide` as a ball
    /// system would.
    fn record_a_phase_and_a_ball_system(app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                record(FixedStepPhase::Intent).in_set(FixedStepPhase::Intent),
                record(FixedStepPhase::Collide)
                    .in_set(FixedStepPhase::Collide)
                    .in_set(BallInPlay),
            ),
        );
    }

    /// What one frame in `state` runs, entered from play.
    fn ran_in(state: MatchState) -> Vec<FixedStepPhase> {
        let mut app = app();
        record_a_phase_and_a_ball_system(&mut app);
        app.world_mut()
            .resource_mut::<NextState<MatchState>>()
            .set(state);
        frame(&mut app);
        assert_eq!(match_state(&app), state);
        app.world_mut().resource_mut::<Ran>().0.clear();
        frame(&mut app);
        std::mem::take(&mut app.world_mut().resource_mut::<Ran>().0)
    }

    #[test]
    fn in_play_the_phases_and_the_balls_systems_run() {
        let mut app = app();
        record_a_phase_and_a_ball_system(&mut app);
        frame(&mut app);
        assert_eq!(
            app.world().resource::<Ran>().0,
            vec![FixedStepPhase::Intent, FixedStepPhase::Collide]
        );
    }

    /// ADR 0004: between rallies the paddles move, so the phases run, and
    /// the ball waits, so its systems do not.
    #[test]
    fn between_rallies_the_phases_run_without_the_balls_systems() {
        for state in [MatchState::Countdown, MatchState::PointScored] {
            assert_eq!(ran_in(state), vec![FixedStepPhase::Intent], "{state:?}");
        }
    }

    #[test]
    fn paused_or_after_the_match_no_phase_runs() {
        for state in [MatchState::Paused, MatchState::MatchOver] {
            assert_eq!(ran_in(state), Vec::<FixedStepPhase>::new(), "{state:?}");
        }
    }

    #[test]
    fn outside_a_match_no_phase_runs() {
        let mut app = headless_app(1);
        app.init_resource::<Ran>();
        boot(&mut app);
        app.add_systems(
            FixedUpdate,
            record(FixedStepPhase::Intent).in_set(FixedStepPhase::Intent),
        );
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(
            app.world().resource::<Ran>().0,
            Vec::<FixedStepPhase>::new()
        );
    }

    fn record(phase: FixedStepPhase) -> impl FnMut(ResMut<Ran>) {
        move |mut ran: ResMut<Ran>| ran.0.push(phase)
    }

    #[test]
    fn phases_run_in_declared_order_regardless_of_registration_order() {
        let mut app = app();
        app.add_systems(
            FixedUpdate,
            record(FixedStepPhase::Sanity).in_set(FixedStepPhase::Sanity),
        )
        .add_systems(
            FixedUpdate,
            record(FixedStepPhase::Rules).in_set(FixedStepPhase::Rules),
        )
        .add_systems(
            FixedUpdate,
            record(FixedStepPhase::Collide).in_set(FixedStepPhase::Collide),
        )
        .add_systems(
            FixedUpdate,
            record(FixedStepPhase::Integrate).in_set(FixedStepPhase::Integrate),
        )
        .add_systems(
            FixedUpdate,
            record(FixedStepPhase::Effects).in_set(FixedStepPhase::Effects),
        )
        .add_systems(
            FixedUpdate,
            record(FixedStepPhase::Intent).in_set(FixedStepPhase::Intent),
        );
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(
            app.world().resource::<Ran>().0,
            vec![
                FixedStepPhase::Intent,
                FixedStepPhase::Effects,
                FixedStepPhase::Integrate,
                FixedStepPhase::Collide,
                FixedStepPhase::Rules,
                FixedStepPhase::Sanity,
            ]
        );
    }

    fn unphased_system() {}
    fn phased_system() {}

    #[test]
    fn the_walker_names_a_fixed_step_system_that_declares_no_phase() {
        let mut app = app();
        app.add_systems(
            FixedUpdate,
            (unphased_system, phased_system.in_set(FixedStepPhase::Rules)),
        );
        let unphased = unphased_fixed_step_systems(&mut app);
        assert_eq!(unphased.len(), 1, "{unphased:?}");
        assert!(unphased[0].contains("unphased_system"), "{unphased:?}");
    }

    #[test]
    fn the_walker_accepts_an_app_whose_fixed_step_systems_all_declare_a_phase() {
        let mut app = app();
        app.add_systems(FixedUpdate, phased_system.in_set(FixedStepPhase::Integrate));
        assert_eq!(unphased_fixed_step_systems(&mut app), Vec::<String>::new());
    }
}

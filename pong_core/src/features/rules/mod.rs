//! Rules (spec 1.4.5): match flow substates, scoring, serve timing, and the
//! end of the match.

use std::num::NonZeroU8;

use bevy::app::{App, FixedUpdate, Plugin, Update};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::error::BevyError;
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::state::condition::in_state;
use bevy::state::state::{NextState, OnEnter, OnTransition, State};
use bevy::time::{Time, Timer, TimerMode};

use crate::domain::{MatchConfig, MatchEnded, PointScored, Random, Serve, SimFault};
use crate::features::ball::{Ball, Position};
use crate::features::tuning::Tuning;
use crate::flow::phases::{BallInPlay, FixedStepPhase};
use crate::flow::states::{AppState, MatchState, paddles_move};
use crate::kernel::geometry::Side;

/// Match flow: owns every [`MatchState`] transition (ADR 0003), scoring, serve
/// timing, pause and resume through the [`MatchControl`] port, and the end of
/// the match.
pub struct RulesPlugin;

impl Plugin for RulesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PointScored>()
            .add_message::<MatchEnded>()
            // Written by sanity, later in the feature order; registered by its
            // reader here, as ball registers the `Serve` that rules writes.
            .add_message::<SimFault>()
            .init_resource::<Score>()
            .init_resource::<MatchControl>()
            .add_systems(OnEnter(AppState::InMatch), begin_match)
            .add_systems(OnEnter(MatchState::Countdown), begin_countdown)
            .add_systems(
                OnTransition {
                    exited: MatchState::Countdown,
                    entered: MatchState::Playing,
                },
                request_serve,
            )
            .add_systems(OnEnter(MatchState::PointScored), begin_point_pause)
            .add_systems(
                Update,
                (
                    advance_countdown.run_if(in_state(MatchState::Countdown)),
                    advance_point_pause.run_if(in_state(MatchState::PointScored)),
                    // After the timers, so its countdown overrides theirs.
                    restart_rally_after_fault
                        .run_if(paddles_move)
                        .after(advance_countdown)
                        .after(advance_point_pause),
                    // After the timers and the heal, so their transition in
                    // the same frame defers the request rather than being
                    // overwritten.
                    apply_pause_requests
                        .after(advance_countdown)
                        .after(advance_point_pause)
                        .after(restart_rally_after_fault),
                ),
            )
            .add_systems(
                FixedUpdate,
                (score_goal_crossings, end_match_at_target)
                    .chain()
                    .in_set(FixedStepPhase::Rules)
                    .in_set(BallInPlay),
            );
    }
}

/// The side the next serve flies toward: random at match start, then the side
/// that conceded the last point.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
struct NextServeToward(Side);

/// Time left in a timed match state (`Countdown`, `PointScored`).
#[derive(Resource, Debug)]
struct StateTimer(Timer);

/// Port: the shell's pause and resume requests (ADR 0003). The shell asks;
/// rules decides, and the shell never sets [`MatchState`] itself.
///
/// The latest request of a frame wins. Rules acts on it in `Update` once no
/// match transition is pending; a request made outside a match is dropped.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MatchControl {
    request: Option<PauseRequest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PauseRequest {
    Pause,
    Resume,
}

impl MatchControl {
    /// Asks to pause. Honoured in `Countdown`, `Playing`, and `PointScored`;
    /// ignored in `Paused` and `MatchOver`. The shell's focus-loss adapter
    /// asks only while `Playing` (spec 1.6.1).
    pub const fn request_pause(&mut self) {
        self.request = Some(PauseRequest::Pause);
    }

    /// Asks to resume. Honoured in `Paused`, returning to the substate the
    /// pause interrupted; ignored elsewhere.
    pub const fn request_resume(&mut self) {
        self.request = Some(PauseRequest::Resume);
    }
}

/// The substate a pause interrupted, which resume returns to.
/// Invariant: present whenever `MatchState` is `Paused`, since only
/// [`apply_pause_requests`] enters `Paused`.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
struct PausedFrom(MatchState);

fn begin_match(mut commands: Commands, mut random: ResMut<Random>, mut score: ResMut<Score>) {
    *score = Score::default();
    let toward = if random.next_f32_in(0.0, 1.0) < 0.5 {
        Side::Left
    } else {
        Side::Right
    };
    commands.insert_resource(NextServeToward(toward));
}

fn begin_countdown(mut commands: Commands, tuning: Res<Tuning>) {
    commands.insert_resource(StateTimer(Timer::new(
        tuning.match_flow().countdown(),
        TimerMode::Once,
    )));
}

fn advance_countdown(
    time: Res<Time>,
    mut timer: ResMut<StateTimer>,
    mut next: ResMut<NextState<MatchState>>,
) {
    if timer.0.tick(time.delta()).is_finished() {
        next.set(MatchState::Playing);
    }
}

/// Runs only on `Countdown → Playing`, so resuming a paused rally never
/// serves again (ADR 0003).
fn request_serve(toward: Res<NextServeToward>, mut serves: MessageWriter<Serve>) {
    serves.write(Serve { toward: toward.0 });
}

/// Acts on the shell's latest [`MatchControl`] request. A resumed `Countdown`
/// or `PointScored` restarts its timer, because entering it does.
///
/// # Errors
/// `Paused` with no recorded substate to resume to: something other than
/// this system entered `Paused`, against ADR 0003.
fn apply_pause_requests(
    mut commands: Commands,
    mut control: ResMut<MatchControl>,
    state: Option<Res<State<MatchState>>>,
    paused_from: Option<Res<PausedFrom>>,
    mut next: ResMut<NextState<MatchState>>,
) -> Result<(), BevyError> {
    let Some(state) = state else {
        control.request = None;
        return Ok(());
    };
    if !matches!(*next, NextState::Unchanged) {
        // A transition is already on its way; the request waits a frame.
        return Ok(());
    }
    let current = *state.get();
    match (control.request.take(), current) {
        (
            Some(PauseRequest::Pause),
            MatchState::Countdown | MatchState::Playing | MatchState::PointScored,
        ) => {
            commands.insert_resource(PausedFrom(current));
            next.set(MatchState::Paused);
        }
        (Some(PauseRequest::Resume), MatchState::Paused) => {
            let Some(paused_from) = paused_from else {
                return Err(BevyError::from(
                    "MatchState is Paused with no recorded substate to resume to",
                ));
            };
            next.set(paused_from.0);
        }
        _ => {}
    }
    Ok(())
}

/// The release answer to a [`SimFault`] (spec 1.4.7): a fresh rally. Entering
/// `Countdown` puts the ball back on the centre spot and restarts the timer,
/// from `Countdown` itself too, since `set` re-enters the current state; the
/// countdown's end serves as after any point, so `Serve` keeps one emitter
/// (ADR 0002). The score stands.
///
/// A match already ending is left to end. Under the dev error policy the
/// sanity phase panics first and this never runs. Logging is the error
/// handler's: the sanity system returned the fault as its error.
fn restart_rally_after_fault(
    mut faults: MessageReader<SimFault>,
    mut next: ResMut<NextState<MatchState>>,
) {
    if faults.read().count() == 0 {
        return;
    }
    let match_ending = matches!(
        *next,
        NextState::Pending(MatchState::MatchOver)
            | NextState::PendingIfDifferent(MatchState::MatchOver)
    );
    if !match_ending {
        next.set(MatchState::Countdown);
    }
}

fn begin_point_pause(mut commands: Commands, tuning: Res<Tuning>) {
    commands.insert_resource(StateTimer(Timer::new(
        tuning.match_flow().point_pause(),
        TimerMode::Once,
    )));
}

fn advance_point_pause(
    time: Res<Time>,
    mut timer: ResMut<StateTimer>,
    mut next: ResMut<NextState<MatchState>>,
) {
    if timer.0.tick(time.delta()).is_finished() {
        next.set(MatchState::Countdown);
    }
}

/// A ball wholly past a goal line is a point for the other side: the score
/// moves, the point is announced, the next serve goes toward the conceder, and
/// play pauses to show the point.
fn score_goal_crossings(
    tuning: Res<Tuning>,
    mut score: ResMut<Score>,
    mut next_serve: ResMut<NextServeToward>,
    mut points: MessageWriter<PointScored>,
    mut next: ResMut<NextState<MatchState>>,
    balls: Query<&Position, With<Ball>>,
) {
    let arena = tuning.arena();
    let radius = tuning.ball().radius().value();
    for position in &balls {
        let conceder = if position.0.x - radius > arena.goal_x(Side::Right) {
            Side::Right
        } else if position.0.x + radius < arena.goal_x(Side::Left) {
            Side::Left
        } else {
            continue;
        };
        let scorer = conceder.opposite();
        score.award(scorer);
        points.write(PointScored { scorer });
        next_serve.0 = conceder;
        next.set(MatchState::PointScored);
    }
}

/// Runs after scoring: a side at the target ends the match, overriding the
/// point pause requested in the same step.
fn end_match_at_target(
    config: Res<MatchConfig>,
    score: Res<Score>,
    mut ended: MessageWriter<MatchEnded>,
    mut next: ResMut<NextState<MatchState>>,
) {
    if !score.is_changed() {
        return;
    }
    if let Some(winner) = score.winner(config.points_to_win) {
        ended.write(MatchEnded { winner });
        next.set(MatchState::MatchOver);
    }
}

/// Points per side in the current match. Reset on entering a match.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Score {
    left: u8,
    right: u8,
}

impl Score {
    /// Points `side` has.
    #[must_use]
    pub const fn side(self, side: Side) -> u8 {
        match side {
            Side::Left => self.left,
            Side::Right => self.right,
        }
    }

    /// One more point for `side`. Saturates at the `u8` ceiling, which no
    /// points-to-win option approaches.
    pub const fn award(&mut self, side: Side) {
        match side {
            Side::Left => self.left = self.left.saturating_add(1),
            Side::Right => self.right = self.right.saturating_add(1),
        }
    }

    /// The side that has reached `points_to_win`, if any.
    #[must_use]
    pub fn winner(self, points_to_win: NonZeroU8) -> Option<Side> {
        [Side::Left, Side::Right]
            .into_iter()
            .find(|&side| self.side(side) >= points_to_win.get())
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;

    use bevy::app::App;
    use bevy::ecs::change_detection::Mut;
    use bevy::ecs::message::Messages;
    use bevy::math::Vec2;

    use super::{MatchControl, MatchState, Score};
    use crate::domain::{Controller, MatchConfig, MatchEnded, PointScored, Seat, Serve, SimFault};
    use crate::features::tuning::Tuning;
    use crate::kernel::geometry::Side;
    use crate::kernel::test_support::assert_vec_near;
    use crate::testing::{
        FrameLength, ball_state, boot, enter_match, fixed_timestep, frame, headless_app,
        match_state, place_ball, record, run_until, set_frame_length, start_match, take_recorded,
    };

    /// The most whole fixed steps one frame can run at 60 Hz (see
    /// [`set_frame_length`]), as after a long hitch.
    const LONGEST_FRAME: FrameLength = FrameLength::Steps(14);

    fn one() -> NonZeroU8 {
        NonZeroU8::new(1).expect("one is non-zero")
    }

    fn two() -> NonZeroU8 {
        NonZeroU8::new(2).expect("two is non-zero")
    }

    fn ten() -> NonZeroU8 {
        NonZeroU8::new(10).expect("ten is non-zero")
    }

    fn two_humans_to(points_to_win: NonZeroU8) -> MatchConfig {
        MatchConfig {
            left: Controller::Human(Seat::Left),
            right: Controller::Human(Seat::Right),
            points_to_win,
        }
    }

    fn recording_app(seed: u64) -> App {
        let mut app = headless_app(seed);
        record::<Serve>(&mut app);
        record::<PointScored>(&mut app);
        record::<MatchEnded>(&mut app);
        boot(&mut app);
        app
    }

    /// Frames until a timer of `duration` finishes at the harness rate: the
    /// first frame count whose total time reaches the duration.
    fn frames(app: &App, duration: std::time::Duration) -> u32 {
        let timestep = fixed_timestep(app);
        u32::try_from(duration.as_nanos().div_ceil(timestep.as_nanos()))
            .expect("a tuning duration fits in frames")
    }

    /// Puts the ball fully past the right goal line, flying out: left scores.
    fn ball_out_past_the_right_goal(app: &mut App) {
        let arena = app.world().resource::<Tuning>().arena();
        place_ball(
            app,
            Vec2::new(arena.goal_x(Side::Right) + 2.0, 0.0),
            Vec2::new(60.0, 0.0),
        );
    }

    fn playing(app: &App) -> bool {
        match_state(app) == MatchState::Playing
    }

    #[test]
    fn a_match_begins_in_countdown_and_plays_after_the_tuned_countdown() {
        let mut app = recording_app(1);
        enter_match(&mut app, two_humans_to(two()));
        assert_eq!(match_state(&app), MatchState::Countdown);
        let countdown = app.world().resource::<Tuning>().match_flow().countdown();
        for _ in 0..frames(&app, countdown) - 2 {
            frame(&mut app);
        }
        assert_eq!(match_state(&app), MatchState::Countdown);
        run_until(&mut app, playing, 4, "play never began");
        assert_eq!(take_recorded::<Serve>(&mut app).len(), 1);
    }

    /// ADR 0003: the transition to `PointScored` applies only next frame, so
    /// the rest of this frame's fixed steps must not run.
    #[test]
    fn a_goal_in_the_longest_frame_scores_exactly_one_point() {
        let mut app = recording_app(1);
        start_match(&mut app, two_humans_to(ten()));
        set_frame_length(&mut app, LONGEST_FRAME);
        ball_out_past_the_right_goal(&mut app);
        frame(&mut app);
        assert_eq!(app.world().resource::<Score>().side(Side::Left), 1);
        assert_eq!(
            take_recorded::<PointScored>(&mut app),
            vec![PointScored { scorer: Side::Left }]
        );
    }

    #[test]
    fn the_final_point_in_the_longest_frame_ends_the_match_exactly_once() {
        let mut app = recording_app(1);
        start_match(&mut app, two_humans_to(one()));
        set_frame_length(&mut app, LONGEST_FRAME);
        ball_out_past_the_right_goal(&mut app);
        frame(&mut app);
        assert_eq!(
            take_recorded::<MatchEnded>(&mut app),
            vec![MatchEnded { winner: Side::Left }]
        );
        frame(&mut app);
        assert_eq!(match_state(&app), MatchState::MatchOver);
        assert_eq!(app.world().resource::<Score>().side(Side::Left), 1);
    }

    #[test]
    fn a_ball_fully_past_a_goal_line_scores_for_the_other_side_pauses_then_serves_at_the_conceder()
    {
        let mut app = recording_app(1);
        start_match(&mut app, two_humans_to(ten()));
        take_recorded::<Serve>(&mut app);
        ball_out_past_the_right_goal(&mut app);
        frame(&mut app);
        assert_eq!(app.world().resource::<Score>().side(Side::Left), 1);
        assert_eq!(
            take_recorded::<PointScored>(&mut app),
            vec![PointScored { scorer: Side::Left }]
        );
        // The transition set during the fixed step applies on the next frame.
        run_until(
            &mut app,
            |app| match_state(app) == MatchState::PointScored,
            2,
            "no point pause",
        );
        let point_pause = app.world().resource::<Tuning>().match_flow().point_pause();
        for _ in 0..frames(&app, point_pause) - 2 {
            frame(&mut app);
        }
        assert_eq!(match_state(&app), MatchState::PointScored);
        run_until(
            &mut app,
            |app| match_state(app) == MatchState::Countdown,
            4,
            "no countdown after the point",
        );
        run_until(&mut app, playing, 1_000, "no play after the countdown");
        assert_eq!(
            take_recorded::<Serve>(&mut app),
            vec![Serve {
                toward: Side::Right
            }]
        );
    }

    #[test]
    fn the_flow_cycles_to_match_over_at_the_target_and_ends_the_match_exactly_once() {
        let mut app = recording_app(1);
        start_match(&mut app, two_humans_to(two()));
        for _ in 0..2 {
            run_until(&mut app, playing, 1_000, "no play");
            ball_out_past_the_right_goal(&mut app);
            frame(&mut app);
            frame(&mut app);
        }
        assert_eq!(match_state(&app), MatchState::MatchOver);
        assert_eq!(
            take_recorded::<MatchEnded>(&mut app),
            vec![MatchEnded { winner: Side::Left }]
        );
        take_recorded::<Serve>(&mut app);
        for _ in 0..200 {
            frame(&mut app);
        }
        assert_eq!(match_state(&app), MatchState::MatchOver);
        assert_eq!(take_recorded::<Serve>(&mut app), Vec::<Serve>::new());
        assert_eq!(
            take_recorded::<MatchEnded>(&mut app),
            Vec::<MatchEnded>::new()
        );
    }

    #[test]
    fn the_simulation_stands_still_outside_playing() {
        let mut app = recording_app(1);
        enter_match(&mut app, two_humans_to(two()));
        place_ball(&mut app, Vec2::new(10.0, 10.0), Vec2::new(60.0, 60.0));
        frame(&mut app);
        assert_vec_near(ball_state(&mut app).0, Vec2::new(10.0, 10.0), 1e-6);

        run_until(&mut app, playing, 1_000, "no play");
        pause(&mut app);
        place_ball(&mut app, Vec2::new(10.0, 10.0), Vec2::new(60.0, 60.0));
        frame(&mut app);
        assert_vec_near(ball_state(&mut app).0, Vec2::new(10.0, 10.0), 1e-6);
    }

    fn control(app: &mut App) -> Mut<'_, MatchControl> {
        app.world_mut().resource_mut::<MatchControl>()
    }

    /// Requests a pause and runs the frames until it applies.
    fn pause(app: &mut App) {
        control(app).request_pause();
        run_until(
            app,
            |app| match_state(app) == MatchState::Paused,
            2,
            "the pause request was not honoured",
        );
    }

    /// Requests a resume and returns the substate it lands in.
    fn resume(app: &mut App) -> MatchState {
        control(app).request_resume();
        run_until(
            app,
            |app| match_state(app) != MatchState::Paused,
            2,
            "the resume request was not honoured",
        );
        match_state(app)
    }

    /// ADR 0003: only `Countdown → Playing` serves.
    #[test]
    fn resuming_a_paused_rally_neither_serves_nor_moves_the_ball_back() {
        let mut app = recording_app(1);
        start_match(&mut app, two_humans_to(ten()));
        take_recorded::<Serve>(&mut app);
        let velocity = Vec2::new(60.0, 60.0);
        place_ball(&mut app, Vec2::new(10.0, 10.0), velocity);
        pause(&mut app);
        let (paused_at, _) = ball_state(&mut app);
        assert_eq!(resume(&mut app), MatchState::Playing);
        // The frame that applies the resume runs one fixed step of play.
        let dt = fixed_timestep(&app).as_secs_f32();
        let (position, now_moving) = ball_state(&mut app);
        assert_vec_near(position, paused_at + velocity * dt, 1e-4);
        assert_vec_near(now_moving, velocity, 1e-6);
        assert_eq!(take_recorded::<Serve>(&mut app), Vec::<Serve>::new());
    }

    #[test]
    fn a_pause_from_countdown_playing_or_point_scored_resumes_to_that_substate() {
        let mut app = recording_app(1);
        enter_match(&mut app, two_humans_to(ten()));
        pause(&mut app);
        assert_eq!(resume(&mut app), MatchState::Countdown);

        run_until(&mut app, playing, 1_000, "no play");
        pause(&mut app);
        assert_eq!(resume(&mut app), MatchState::Playing);

        ball_out_past_the_right_goal(&mut app);
        run_until(
            &mut app,
            |app| match_state(app) == MatchState::PointScored,
            2,
            "no point pause",
        );
        pause(&mut app);
        assert_eq!(resume(&mut app), MatchState::PointScored);
        assert_eq!(app.world().resource::<Score>().side(Side::Left), 1);
    }

    #[test]
    fn a_pause_request_after_the_match_is_over_is_ignored() {
        let mut app = recording_app(1);
        start_match(&mut app, two_humans_to(one()));
        ball_out_past_the_right_goal(&mut app);
        run_until(
            &mut app,
            |app| match_state(app) == MatchState::MatchOver,
            2,
            "no match over",
        );
        control(&mut app).request_pause();
        for _ in 0..3 {
            frame(&mut app);
        }
        assert_eq!(match_state(&app), MatchState::MatchOver);
    }

    #[test]
    fn a_pause_request_made_outside_a_match_is_dropped_not_kept_for_the_next_match() {
        let mut app = recording_app(1);
        control(&mut app).request_pause();
        frame(&mut app);
        enter_match(&mut app, two_humans_to(ten()));
        for _ in 0..3 {
            frame(&mut app);
        }
        assert_eq!(match_state(&app), MatchState::Countdown);
    }

    #[test]
    fn the_first_serve_side_follows_the_seed() {
        let mut first = recording_app(5);
        start_match(&mut first, two_humans_to(two()));
        let mut second = recording_app(5);
        start_match(&mut second, two_humans_to(two()));
        assert_eq!(
            take_recorded::<Serve>(&mut first),
            take_recorded::<Serve>(&mut second)
        );
    }

    #[test]
    fn a_fresh_score_is_nil_nil_and_awarding_counts_per_side() {
        let mut score = Score::default();
        assert_eq!((score.side(Side::Left), score.side(Side::Right)), (0, 0));
        score.award(Side::Right);
        score.award(Side::Right);
        score.award(Side::Left);
        assert_eq!((score.side(Side::Left), score.side(Side::Right)), (1, 2));
    }

    #[test]
    fn the_winner_is_the_side_that_reached_the_target_and_nobody_before_that() {
        let mut score = Score::default();
        score.award(Side::Left);
        assert_eq!(score.winner(two()), None);
        score.award(Side::Left);
        assert_eq!(score.winner(two()), Some(Side::Left));
    }

    /// What the sanity phase writes; any variant serves here.
    fn report_fault(app: &mut App) {
        app.world_mut()
            .resource_mut::<Messages<SimFault>>()
            .write(SimFault::BallCount { count: 2 });
    }

    /// Spec 1.4.7, ADR 0002: the release answer to a fault is a fresh rally,
    /// served by rules as every rally is.
    #[test]
    fn a_fault_in_play_restarts_the_rally_from_a_countdown_that_ends_in_a_serve() {
        let mut app = recording_app(1);
        start_match(&mut app, two_humans_to(ten()));
        take_recorded::<Serve>(&mut app);
        place_ball(&mut app, Vec2::new(30.0, 10.0), Vec2::new(60.0, 0.0));
        report_fault(&mut app);
        frame(&mut app);
        frame(&mut app);
        assert_eq!(match_state(&app), MatchState::Countdown);
        let (position, velocity) = ball_state(&mut app);
        assert_vec_near(position, Vec2::ZERO, 1e-6);
        assert_vec_near(velocity, Vec2::ZERO, 1e-6);
        assert_eq!(*app.world().resource::<Score>(), Score::default());
        run_until(&mut app, playing, 1_000, "the countdown never ended");
        frame(&mut app);
        assert_eq!(take_recorded::<Serve>(&mut app).len(), 1);
    }

    #[test]
    fn a_fault_during_the_countdown_starts_the_countdown_over() {
        let mut app = recording_app(1);
        enter_match(&mut app, two_humans_to(ten()));
        let countdown = frames(
            &app,
            app.world().resource::<Tuning>().match_flow().countdown(),
        );
        for _ in 0..countdown / 2 {
            frame(&mut app);
        }
        report_fault(&mut app);
        frame(&mut app);
        // More than the half that was left: a timer that went on would be done.
        for _ in 0..countdown * 3 / 4 {
            frame(&mut app);
        }
        assert_eq!(match_state(&app), MatchState::Countdown);
    }

    #[test]
    fn a_fault_in_the_frame_that_ends_the_match_does_not_undo_its_end() {
        let mut app = recording_app(1);
        start_match(&mut app, two_humans_to(one()));
        ball_out_past_the_right_goal(&mut app);
        report_fault(&mut app);
        frame(&mut app);
        frame(&mut app);
        assert_eq!(match_state(&app), MatchState::MatchOver);
    }
}

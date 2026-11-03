//! Rules (spec 1.4.5): match flow substates, scoring, serve timing, and the
//! end of the match.

use std::num::NonZeroU8;

use bevy::app::{App, FixedUpdate, Plugin, Update};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::message::MessageWriter;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::state::app::AppExtStates;
use bevy::state::condition::in_state;
use bevy::state::state::{NextState, OnEnter, SubStates};
use bevy::time::{Time, Timer, TimerMode};

use crate::app::phases::FixedStepPhase;
use crate::app::states::AppState;
use crate::domain::{MatchConfig, MatchEnded, PointScored, Random, Serve};
use crate::features::ball::{Ball, Position};
use crate::features::seats::SeatIntents;
use crate::features::tuning::Tuning;
use crate::kernel::geometry::Side;

/// Where a match is in its flow. Exists only while [`AppState::InMatch`].
///
/// `Countdown` and `PointScored` are timed from tuning and advance by
/// themselves. `Playing` is the only state in which the fixed-step phases run.
/// `Paused` is entered and left by the shell (Escape, focus loss). `MatchOver`
/// is final; the shell returns to the menu from it.
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[source(AppState = AppState::InMatch)]
pub enum MatchState {
    /// The serve cue is running; nothing moves.
    #[default]
    Countdown,
    /// The ball is live.
    Playing,
    /// A point was just scored; the score is shown.
    PointScored,
    /// Play is suspended by the shell.
    Paused,
    /// A side reached the target.
    MatchOver,
}

/// Match flow: substates, scoring, serve timing, and the end of the match.
pub struct RulesPlugin;

impl Plugin for RulesPlugin {
    fn build(&self, app: &mut App) {
        app.add_sub_state::<MatchState>()
            .add_message::<PointScored>()
            .add_message::<MatchEnded>()
            .init_resource::<Score>()
            .add_systems(OnEnter(AppState::InMatch), begin_match)
            .add_systems(OnEnter(MatchState::Countdown), begin_countdown)
            .add_systems(OnEnter(MatchState::Playing), request_serve)
            .add_systems(OnEnter(MatchState::PointScored), begin_point_pause)
            .add_systems(
                Update,
                (
                    advance_countdown.run_if(in_state(MatchState::Countdown)),
                    advance_point_pause.run_if(in_state(MatchState::PointScored)),
                ),
            )
            .add_systems(
                FixedUpdate,
                (score_goal_crossings, end_match_at_target)
                    .chain()
                    .in_set(FixedStepPhase::Rules),
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

fn begin_match(mut commands: Commands, mut random: ResMut<Random>, mut score: ResMut<Score>) {
    *score = Score::default();
    let toward = if random.next_f32_in(0.0, 1.0) < 0.5 {
        Side::Left
    } else {
        Side::Right
    };
    commands.insert_resource(NextServeToward(toward));
}

/// A stale key press from the menu or the last rally must not become the first
/// paddle move (seats contract, plan 3.5).
fn begin_countdown(mut commands: Commands, tuning: Res<Tuning>, mut seats: ResMut<SeatIntents>) {
    seats.clear();
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

fn request_serve(toward: Res<NextServeToward>, mut serves: MessageWriter<Serve>) {
    serves.write(Serve { toward: toward.0 });
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
    use bevy::math::Vec2;
    use bevy::state::state::NextState;
    use bevy::time::{Fixed, Time};

    use super::{MatchState, Score};
    use crate::domain::{Controller, MatchConfig, MatchEnded, PointScored, Seat, Serve};
    use crate::features::seats::{PaddleIntent, SeatIntents};
    use crate::features::tuning::Tuning;
    use crate::kernel::geometry::Side;
    use crate::kernel::test_support::assert_vec_near;
    use crate::testing::{
        STEP_HZ, ball_state, boot, enter_match, headless_app, match_state, place_ball, record,
        run_until, start_match, step, take_recorded,
    };

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
    fn frames(duration: std::time::Duration) -> u32 {
        let timestep = Time::<Fixed>::from_hz(STEP_HZ).timestep();
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
        for _ in 0..frames(countdown) - 2 {
            step(&mut app);
        }
        assert_eq!(match_state(&app), MatchState::Countdown);
        run_until(&mut app, playing, 4, "play never began");
        assert_eq!(take_recorded::<Serve>(&mut app).len(), 1);
    }

    #[test]
    fn a_ball_fully_past_a_goal_line_scores_for_the_other_side_pauses_then_serves_at_the_conceder()
    {
        let mut app = recording_app(1);
        start_match(&mut app, two_humans_to(ten()));
        take_recorded::<Serve>(&mut app);
        ball_out_past_the_right_goal(&mut app);
        step(&mut app);
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
        for _ in 0..frames(point_pause) - 2 {
            step(&mut app);
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
            step(&mut app);
            step(&mut app);
        }
        assert_eq!(match_state(&app), MatchState::MatchOver);
        assert_eq!(
            take_recorded::<MatchEnded>(&mut app),
            vec![MatchEnded { winner: Side::Left }]
        );
        take_recorded::<Serve>(&mut app);
        for _ in 0..200 {
            step(&mut app);
        }
        assert_eq!(match_state(&app), MatchState::MatchOver);
        assert_eq!(take_recorded::<Serve>(&mut app), Vec::<Serve>::new());
        assert_eq!(
            take_recorded::<MatchEnded>(&mut app),
            Vec::<MatchEnded>::new()
        );
    }

    #[test]
    fn entering_countdown_clears_seat_intents() {
        let mut app = recording_app(1);
        start_match(&mut app, two_humans_to(ten()));
        app.world_mut()
            .resource_mut::<SeatIntents>()
            .report(Seat::Left, PaddleIntent::Up);
        ball_out_past_the_right_goal(&mut app);
        step(&mut app);
        run_until(
            &mut app,
            |app| match_state(app) == MatchState::Countdown,
            1_000,
            "no countdown after the point",
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<SeatIntents>()
                .consume(Seat::Left),
            PaddleIntent::Still
        );
    }

    #[test]
    fn the_simulation_stands_still_outside_playing() {
        let mut app = recording_app(1);
        enter_match(&mut app, two_humans_to(two()));
        place_ball(&mut app, Vec2::new(10.0, 10.0), Vec2::new(60.0, 60.0));
        step(&mut app);
        assert_vec_near(ball_state(&mut app).0, Vec2::new(10.0, 10.0), 1e-6);

        run_until(&mut app, playing, 1_000, "no play");
        app.world_mut()
            .resource_mut::<NextState<MatchState>>()
            .set(MatchState::Paused);
        step(&mut app);
        assert_eq!(match_state(&app), MatchState::Paused);
        place_ball(&mut app, Vec2::new(10.0, 10.0), Vec2::new(60.0, 60.0));
        step(&mut app);
        assert_vec_near(ball_state(&mut app).0, Vec2::new(10.0, 10.0), 1e-6);
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
}

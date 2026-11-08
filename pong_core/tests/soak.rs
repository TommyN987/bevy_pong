//! Soak (spec 1.5.2, plan 3.19): matches over at least a hundred thousand fixed
//! steps of live play, panic handler on, no fault. The nextest slow-timeout of
//! thirty seconds (`.config/nextest.toml`) fails a slow run.
//!
//! Both seats mix random intents with tracking aimed by the kernel's crossing
//! prediction, off by a seeded error per approach. Tracking makes long rallies
//! that reach the speed cap; the random intents and the error make points. Each
//! frame runs half a step, one, two, or four, so the step gate (ADR 0003) and
//! the intent latch are exercised as on real displays.
//!
//! The invariants are the sanity phase's to check, after every fixed step
//! (plan 6.1); under the harness's dev error policy a fault panics the run.

use std::num::NonZeroU8;

use bevy::app::{App, FixedUpdate};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::ResMut;
use pong_core::domain::{BallHitPaddle, Controller, MatchConfig, Seat};
use pong_core::features::paddle::Paddle;
use pong_core::features::sanity::SanityPlugin;
use pong_core::features::seats::PaddleIntent;
use pong_core::features::tuning::Tuning;
use pong_core::flow::phases::{BallInPlay, FixedStepPhase};
use pong_core::flow::states::{AppState, MatchState};
use pong_core::kernel::geometry::Side;
use pong_core::kernel::integrate::Motion;
use pong_core::kernel::predict::crossing_y;
use pong_core::kernel::random::Pcg32;
use pong_core::testing::{
    FrameLength, app_state, ball_state, boot, enter_match, fixed_timestep, frame, headless_app,
    leave_match, match_state, record, report_intent, set_frame_length, take_recorded,
};

/// Fixed steps of live play to run, counted where the ball's systems run.
const LIVE_STEPS: u32 = 100_000;

/// Frames allowed to reach [`LIVE_STEPS`]; countdowns and point pauses take
/// frames but no live steps.
const MAX_FRAMES: u32 = 400_000;

/// Paddle hits the seeded run must produce at least, so a change that quietly
/// shortens every rally fails here. The first green run made 1,118.
const MIN_PADDLE_HITS: usize = 1_000;

/// The frame lengths a frame is drawn from.
const FRAME_LENGTHS: [FrameLength; 4] = [
    FrameLength::HalfStep,
    FrameLength::Steps(1),
    FrameLength::Steps(2),
    FrameLength::Steps(4),
];

/// Counts fixed steps in which the phases ran.
#[derive(Resource, Default)]
struct LiveSteps(u32);

fn count_live_step(mut live: ResMut<LiveSteps>) {
    live.0 += 1;
}

fn two_humans_to(points: u8) -> MatchConfig {
    MatchConfig {
        left: Controller::Human(Seat::Left),
        right: Controller::Human(Seat::Right),
        points_to_win: NonZeroU8::new(points).expect("a positive target"),
    }
}

/// A uniformly drawn element of `items`.
fn pick<T: Copy>(random: &mut Pcg32, items: &[T]) -> T {
    let index = usize::try_from(random.next_u32()).expect("u32 fits usize") % items.len();
    items[index]
}

/// One seat's player: aims where the ball will cross its paddle's face, off by
/// an error redrawn each time the ball turns toward it, and presses a random
/// key a quarter of the time.
struct Player {
    side: Side,
    seat: Seat,
    aim_error: f32,
    ball_was_approaching: bool,
}

impl Player {
    /// Largest aim error. A paddle half-height plus the ball radius is 8, so
    /// most approaches are returned and some are missed.
    const MAX_AIM_ERROR: f32 = 9.0;

    fn new(side: Side, seat: Seat) -> Self {
        Self {
            side,
            seat,
            aim_error: 0.0,
            ball_was_approaching: false,
        }
    }

    fn intent(
        &mut self,
        random: &mut Pcg32,
        tuning: &Tuning,
        paddle: Paddle,
        ball: Motion,
        dt: f32,
    ) -> PaddleIntent {
        let approaching = ball.velocity.x * self.side.sign() > 0.0;
        if approaching && !self.ball_was_approaching {
            self.aim_error = random.next_f32_in(-Self::MAX_AIM_ERROR, Self::MAX_AIM_ERROR);
        }
        self.ball_was_approaching = approaching;
        if random.next_u32().is_multiple_of(4) {
            return pick(
                random,
                &[PaddleIntent::Still, PaddleIntent::Up, PaddleIntent::Down],
            );
        }
        let radius = tuning.ball().radius();
        let face = paddle.rect(tuning);
        let face_x = match self.side {
            Side::Left => face.max.x + radius.value(),
            Side::Right => face.min.x - radius.value(),
        };
        let target = crossing_y(tuning.arena(), radius, ball, face_x, dt)
            .map_or(0.0, |y| y + self.aim_error);
        let gap = target - paddle.center_y();
        if gap > 1.0 {
            PaddleIntent::Up
        } else if gap < -1.0 {
            PaddleIntent::Down
        } else {
            PaddleIntent::Still
        }
    }
}

fn paddle(app: &mut App, side: Side) -> Paddle {
    app.world_mut()
        .query::<&Paddle>()
        .iter(app.world())
        .copied()
        .find(|paddle| paddle.side() == side)
        .expect("one paddle per side in a match")
}

#[test]
fn a_hundred_thousand_live_steps_with_mixed_intents_and_frame_lengths_raise_no_fault() {
    let mut app = headless_app(152);
    app.init_resource::<LiveSteps>().add_systems(
        FixedUpdate,
        count_live_step
            .in_set(FixedStepPhase::Sanity)
            .in_set(BallInPlay),
    );
    record::<BallHitPaddle>(&mut app);
    boot(&mut app);
    let config = two_humans_to(5);
    enter_match(&mut app, config);

    assert!(
        app.is_plugin_added::<SanityPlugin>(),
        "the soak relies on the sanity phase to check every step"
    );
    let tuning = app.world().resource::<Tuning>().clone();
    let cap = tuning.ball().speed_rule().cap();
    let dt = fixed_timestep(&app).as_secs_f32();

    let mut random = Pcg32::seed(42);
    let mut players = [
        Player::new(Side::Left, Seat::Left),
        Player::new(Side::Right, Seat::Right),
    ];
    let mut matches_ended = 0;
    let mut paddle_hits = 0;
    let mut top_speed: f32 = 0.0;
    let mut frames = 0;

    while app.world().resource::<LiveSteps>().0 < LIVE_STEPS {
        assert!(
            frames < MAX_FRAMES,
            "only {} live steps in {MAX_FRAMES} frames",
            app.world().resource::<LiveSteps>().0
        );
        frames += 1;
        if match_state(&app) == MatchState::MatchOver {
            matches_ended += 1;
            leave_match(&mut app);
            enter_match(&mut app, config);
        }
        let (position, velocity) = ball_state(&mut app);
        let ball = Motion { position, velocity };
        for player in &mut players {
            let paddle = paddle(&mut app, player.side);
            let intent = player.intent(&mut random, &tuning, paddle, ball, dt);
            report_intent(&mut app, player.seat, intent);
        }
        set_frame_length(&mut app, pick(&mut random, &FRAME_LENGTHS));
        frame(&mut app);
        paddle_hits += take_recorded::<BallHitPaddle>(&mut app).len();

        let (_, velocity) = ball_state(&mut app);
        top_speed = top_speed.max(velocity.length());
    }

    assert_eq!(app_state(&app), AppState::InMatch);
    assert!(matches_ended >= 1, "no match ended in {frames} frames");
    assert!(
        top_speed >= cap - 1e-3,
        "no rally reached the speed cap {cap}; top speed {top_speed}"
    );
    assert!(
        paddle_hits >= MIN_PADDLE_HITS,
        "only {paddle_hits} paddle hits; {matches_ended} matches in {frames} frames"
    );
    // Visible with `--no-capture`: the numbers behind the thresholds above.
    println!(
        "soak: {frames} frames, {matches_ended} matches, {paddle_hits} paddle hits, top speed {top_speed}"
    );
}

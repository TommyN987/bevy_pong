//! Soak (spec 1.5.2, plan 3.19; M2 spec 2.5.3, 2.5.4): matches over at least
//! a hundred thousand fixed steps of live play, panic handler on, no fault.
//! The nextest slow-timeout of thirty seconds (`.config/nextest.toml`) fails
//! a slow run.
//!
//! Classic runs M1's soak. Chaos runs every subset of the effects its bag
//! ships, up to the cap of three, each forced beside the mode's always-on
//! effects with every dial full. One test per subset size, and each test
//! divides the same hundred thousand steps among its subsets, so its running
//! time does not grow with the roster.
//!
//! Both seats mix random intents with tracking aimed by the kernel's crossing
//! prediction under the ball's own flight, off by a seeded error per
//! approach. Tracking makes rallies that reach the speed cap; the random
//! intents and the error make points. Each frame runs half a step, one, two,
//! or four, so the step gate (ADR 0003) and the intent latch are exercised as
//! on real displays.
//!
//! The invariants are the sanity phase's to check, after every fixed step
//! (plan 6.1); under the harness's dev error policy a fault panics the run.

use std::num::NonZeroU8;

use bevy::app::{App, FixedUpdate};
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::ResMut;
use pong_core::domain::{
    BallHitPaddle, Controller, EffectId, MatchConfig, Mode, Seat, SeatPlayers,
};
use pong_core::features::ball::{Ball, Position, Velocity};
use pong_core::features::effects::{ActiveEffects, BallFlight, Sandbox};
use pong_core::features::paddle::Paddle;
use pong_core::features::sanity::SanityPlugin;
use pong_core::features::seats::PaddleIntent;
use pong_core::features::tuning::{EMBEDDED_RON, Tuning};
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

/// Frames allowed per live step asked for; countdowns and point pauses take
/// frames but no live steps.
const MAX_FRAMES_PER_LIVE_STEP: u32 = 4;

/// Paddle hits the seeded Classic run must produce at least, so a change that
/// quietly shortens every rally fails here. The first green run made 1,118.
const MIN_PADDLE_HITS: usize = 1_000;

/// The return count held to put every dial at its full setting, whatever
/// return its ramp is full at: the highest there is.
const EVERY_DIAL_FULL: u16 = u16::MAX;

/// The most effects Chaos schedules at once: the highest cap on the mode
/// screen (M2 spec 2.4.1).
const HIGHEST_CAP: u8 = 3;

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

fn two_humans_to(points: u8, mode: Mode) -> MatchConfig {
    MatchConfig {
        left: Controller::Human(Seat::Left),
        right: Controller::Human(Seat::Right),
        players: SeatPlayers::GUESTS,
        points_to_win: NonZeroU8::new(points).expect("a positive target"),
        mode,
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

    /// What this player presses, given its paddle, the ball's velocity along
    /// x, and where the ball will cross this player's paddle face, if it will.
    fn intent(
        &mut self,
        random: &mut Pcg32,
        paddle: Paddle,
        ball_velocity_x: f32,
        crossing: Option<f32>,
    ) -> PaddleIntent {
        let approaching = ball_velocity_x * self.side.sign() > 0.0;
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
        let target = crossing.map_or(0.0, |y| y + self.aim_error);
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

/// The ball's velocity along x, and where the ball will cross the face of
/// each of `paddles`, predicted under the flight the ball is in: the same
/// flight the physics steps it under, fields and all.
fn sighted(
    app: &mut App,
    tuning: &Tuning,
    paddles: [Paddle; 2],
    dt: f32,
) -> (f32, [Option<f32>; 2]) {
    let radius = tuning.ball().radius();
    let mut balls = app
        .world_mut()
        .query_filtered::<(&Position, &Velocity, &BallFlight), With<Ball>>();
    let (position, velocity, flight) = balls.single(app.world()).expect("exactly one ball");
    let ball = Motion {
        position: position.0,
        velocity: velocity.0,
    };
    let crossings = paddles.map(|paddle| {
        let face = paddle.rect(tuning);
        let face_x = match paddle.side() {
            Side::Left => face.max.x + radius.value(),
            Side::Right => face.min.x - radius.value(),
        };
        crossing_y(tuning.arena(), radius, ball, flight.flight(), face_x, dt)
    });
    (ball.velocity.x, crossings)
}

/// One soak run to be made.
struct Soak {
    /// Seeds the app: its random source and the order of unordered systems.
    app_seed: u64,
    /// Seeds the two players' aim errors, random keys, and frame lengths.
    player_seed: u64,
    mode: Mode,
    /// The whole set of effects the sandbox forces into play, every dial
    /// full; `None` lets the mode's schedule and the rally decide.
    forced_at_full_dial: Option<Vec<EffectId>>,
    /// Fixed steps of live play to run.
    live_steps: u32,
}

/// What a soak run did.
struct SoakRun {
    frames: u32,
    matches_ended: u32,
    paddle_hits: usize,
    top_speed: f32,
    /// The effects in play when the run stopped.
    in_play: Vec<EffectId>,
}

impl Soak {
    /// Plays matches to five points, one after another, until the live steps
    /// asked for have run. Postcondition: the app is still in a match; any
    /// simulation fault would have panicked the run.
    fn run(&self) -> SoakRun {
        let mut app = headless_app(self.app_seed);
        app.init_resource::<LiveSteps>().add_systems(
            FixedUpdate,
            count_live_step
                .in_set(FixedStepPhase::Sanity)
                .in_set(BallInPlay),
        );
        record::<BallHitPaddle>(&mut app);
        boot(&mut app);
        if let Some(effects) = &self.forced_at_full_dial {
            let mut sandbox = app.world_mut().resource_mut::<Sandbox>();
            sandbox.force_effects(effects);
            sandbox.hold_returns(Some(EVERY_DIAL_FULL));
        }
        let config = two_humans_to(5, self.mode);
        enter_match(&mut app, config);

        assert!(
            app.is_plugin_added::<SanityPlugin>(),
            "the soak relies on the sanity phase to check every step"
        );
        let tuning = app.world().resource::<Tuning>().clone();
        let dt = fixed_timestep(&app).as_secs_f32();
        let max_frames = self.live_steps * MAX_FRAMES_PER_LIVE_STEP;

        let mut random = Pcg32::seed(self.player_seed);
        let mut players = [
            Player::new(Side::Left, Seat::Left),
            Player::new(Side::Right, Seat::Right),
        ];
        let mut run = SoakRun {
            frames: 0,
            matches_ended: 0,
            paddle_hits: 0,
            top_speed: 0.0,
            in_play: Vec::new(),
        };

        while app.world().resource::<LiveSteps>().0 < self.live_steps {
            assert!(
                run.frames < max_frames,
                "only {} live steps in {max_frames} frames",
                app.world().resource::<LiveSteps>().0
            );
            run.frames += 1;
            if match_state(&app) == MatchState::MatchOver {
                run.matches_ended += 1;
                leave_match(&mut app);
                enter_match(&mut app, config);
            }
            let paddles = [paddle(&mut app, Side::Left), paddle(&mut app, Side::Right)];
            let (ball_velocity_x, crossings) = sighted(&mut app, &tuning, paddles, dt);
            for ((player, paddle), crossing) in players.iter_mut().zip(paddles).zip(crossings) {
                let intent = player.intent(&mut random, paddle, ball_velocity_x, crossing);
                report_intent(&mut app, player.seat, intent);
            }
            set_frame_length(&mut app, pick(&mut random, &FRAME_LENGTHS));
            frame(&mut app);
            run.paddle_hits += take_recorded::<BallHitPaddle>(&mut app).len();

            let (_, velocity) = ball_state(&mut app);
            run.top_speed = run.top_speed.max(velocity.length());
        }

        assert_eq!(app_state(&app), AppState::InMatch);
        run.in_play = app.world().resource::<ActiveEffects>().ids().to_vec();
        run
    }
}

#[test]
fn a_hundred_thousand_live_steps_with_mixed_intents_and_frame_lengths_raise_no_fault() {
    let SoakRun {
        frames,
        matches_ended,
        paddle_hits,
        top_speed,
        ..
    } = Soak {
        app_seed: 152,
        player_seed: 42,
        mode: Mode::Classic,
        forced_at_full_dial: None,
        live_steps: LIVE_STEPS,
    }
    .run();
    let tuning = Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid");
    let cap = tuning.ball().speed_cap();

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

/// Every way to pick `size` of `items`, each once, in the order given.
fn subsets_of<T: Copy>(items: &[T], size: usize) -> Vec<Vec<T>> {
    if size == 0 {
        return vec![Vec::new()];
    }
    let mut subsets = Vec::new();
    for (index, first) in items.iter().enumerate() {
        for rest in subsets_of(&items[index + 1..], size - 1) {
            let mut subset = vec![*first];
            subset.extend(rest);
            subsets.push(subset);
        }
    }
    subsets
}

fn chaos() -> Mode {
    Mode::Chaos {
        cap: NonZeroU8::new(HIGHEST_CAP).expect("the highest cap is non-zero"),
    }
}

/// M2 spec 2.5.3 and 2.5.4 for the subsets of one size: every choice of
/// `size` effects from Chaos's bag, forced beside the mode's always-on
/// effects with every dial full, soaks without a fault. The test's hundred
/// thousand live steps are divided evenly among its subsets.
///
/// While the bag ships fewer than `size` effects there is no such subset,
/// and the test says so and passes.
fn every_chaos_subset_soaks_without_a_fault(size: usize) {
    let tuning = Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid");
    let mode = tuning.mode(chaos());
    let cap = tuning.ball().speed_cap();
    let subsets = subsets_of(mode.bag(), size);
    if subsets.is_empty() {
        println!(
            "chaos soak: no subset of {size}; the bag ships {}",
            mode.bag().len()
        );
        return;
    }
    let count = u32::try_from(subsets.len()).expect("a handful of subsets");
    let live_steps = LIVE_STEPS / count;
    for (index, subset) in (0_u64..).zip(&subsets) {
        let forced: Vec<EffectId> = EffectId::ALL
            .into_iter()
            .filter(|id| mode.always().contains(id) || subset.contains(id))
            .collect();
        let run = Soak {
            app_seed: 152 + index,
            player_seed: 42 + index,
            mode: chaos(),
            forced_at_full_dial: Some(forced.clone()),
            live_steps,
        }
        .run();
        println!(
            "chaos soak, {forced:?} at full dial: {live_steps} live steps in {} frames, {} matches, {} paddle hits, top speed {}",
            run.frames, run.matches_ended, run.paddle_hits, run.top_speed
        );
        // The forcing took hold: these effects were in play, and a return
        // left at the full dial's speed.
        assert_eq!(run.in_play, forced);
        assert!(
            run.top_speed >= cap - 1e-3,
            "{forced:?}: no return reached the speed cap {cap}; top speed {}",
            run.top_speed
        );
        assert!(
            run.matches_ended >= 1,
            "{forced:?}: no match ended in {} frames",
            run.frames
        );
    }
}

#[test]
fn under_chaos_every_effect_alone_at_full_dial_soaks_without_a_fault() {
    every_chaos_subset_soaks_without_a_fault(1);
}

#[test]
fn under_chaos_every_pair_of_effects_at_full_dial_soaks_without_a_fault() {
    every_chaos_subset_soaks_without_a_fault(2);
}

#[test]
fn under_chaos_every_three_effects_at_full_dial_soak_without_a_fault() {
    every_chaos_subset_soaks_without_a_fault(3);
}

#[test]
fn subsets_list_every_choice_of_a_size_once() {
    let effects = [EffectId::Speed, EffectId::Shrink];
    assert_eq!(
        subsets_of(&effects, 1),
        vec![vec![EffectId::Speed], vec![EffectId::Shrink]]
    );
    assert_eq!(
        subsets_of(&effects, 2),
        vec![vec![EffectId::Speed, EffectId::Shrink]]
    );
    assert_eq!(subsets_of(&effects, 3), Vec::<Vec<EffectId>>::new());
    assert_eq!(subsets_of(&effects, 0), vec![Vec::new()]);
    // Six effects, as the full roster's bag: 6, 15, and 20 subsets.
    let six = [0_u8, 1, 2, 3, 4, 5];
    assert_eq!(subsets_of(&six, 1).len(), 6);
    assert_eq!(subsets_of(&six, 2).len(), 15);
    assert_eq!(subsets_of(&six, 3).len(), 20);
}

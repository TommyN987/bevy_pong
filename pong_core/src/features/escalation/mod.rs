//! Escalation (M2 spec 2.4): which effects are in play, decided between
//! points from the mode's data, the score, and a bag drawn once per match.
//!
//! The match scale sets breadth here: how many scheduled effects, and which.
//! The rally scale sets intensity elsewhere: each effect reads the rally's
//! return count from the flight it contributes to. This feature knows no
//! effect: it reads names from tuning and hands them to the effects feature.

use bevy::app::{App, Plugin};
use bevy::ecs::message::MessageWriter;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Res, ResMut};
use bevy::state::state::OnEnter;

use crate::domain::{EffectEnded, EffectId, EffectStarted, MatchConfig, Mode, Random};
use crate::features::effects::ActiveEffects;
use crate::features::rules::Score;
use crate::features::tuning::Tuning;
use crate::flow::states::{AppState, MatchState};
use crate::kernel::geometry::Side;

pub mod schedule;

use schedule::Rotation;

/// Decides the effects in play: the one writer of [`ActiveEffects`].
///
/// - Entering a match draws the mode's bag in a seeded order.
/// - Entering a countdown brings the effects in play up to date with the
///   score, and writes [`EffectStarted`] and [`EffectEnded`] for the bag
///   effects that joined or left. Nothing changes inside a rally
///   (M2 spec 2.4.3).
///
/// A countdown is entered after every point, and also after a fault and on
/// resuming a paused countdown. The update counts points, not countdowns, so
/// those re-entries change nothing.
pub struct EscalationPlugin;

impl Plugin for EscalationPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<EffectStarted>()
            .add_message::<EffectEnded>()
            .init_resource::<Schedule>()
            // The match's own schedule runs before its first countdown's, as
            // the ball's spawn and its first placing rely on too.
            .add_systems(OnEnter(AppState::InMatch), draw_the_bag)
            .add_systems(OnEnter(MatchState::Countdown), bring_effects_up_to_date);
    }
}

/// The match's bag and how far it has turned.
///
/// Invariant: `rotations_done` is the number of whole rotation intervals in
/// the points scored when the schedule was last brought up to date.
#[derive(Resource, Debug, Default)]
struct Schedule {
    rotation: Rotation<EffectId>,
    rotations_done: u32,
}

/// The most scheduled effects `mode` allows at once.
fn cap(mode: Mode) -> usize {
    match mode {
        // Classic's bag is empty; no cap is needed to keep it so.
        Mode::Classic => usize::MAX,
        Mode::Chaos { cap } => usize::from(cap.get()),
    }
}

/// Shuffles the mode's bag from the seeded source, once per match. A bag of
/// one or none draws nothing, so Classic's random stream is as M1 left it.
fn draw_the_bag(
    config: Res<MatchConfig>,
    tuning: Res<Tuning>,
    mut random: ResMut<Random>,
    mut schedule: ResMut<Schedule>,
) {
    let mut bag = tuning.mode(config.mode).bag().to_vec();
    schedule::shuffle(&mut bag, |choices| random.next_below(choices));
    *schedule = Schedule {
        rotation: Rotation::new(bag),
        rotations_done: 0,
    };
}

/// Sets the effects in play for the rally about to start: the mode's
/// always-on effects, and as many of its bag as the leader's progress, the
/// cap, and the bag allow, rotated once per interval of points scored.
fn bring_effects_up_to_date(
    config: Res<MatchConfig>,
    tuning: Res<Tuning>,
    score: Res<Score>,
    mut schedule: ResMut<Schedule>,
    mut active: ResMut<ActiveEffects>,
    mut started: MessageWriter<EffectStarted>,
    mut ended: MessageWriter<EffectEnded>,
) {
    let mode = tuning.mode(config.mode);
    let (left, right) = (score.side(Side::Left), score.side(Side::Right));
    let count = schedule::count(
        left.max(right),
        config.points_to_win,
        mode.thresholds_percent(),
        cap(config.mode),
        mode.bag().len(),
    );
    let points = u32::from(left) + u32::from(right);
    let rotations_due = points / u32::from(mode.rotate_every_points().get());
    let rotate = rotations_due > schedule.rotations_done;
    schedule.rotations_done = rotations_due;

    let changes = schedule.rotation.advance(count, rotate);
    for effect in changes.ended {
        ended.write(EffectEnded { effect });
    }
    for effect in changes.started {
        started.write(EffectStarted { effect });
    }

    let in_play: Vec<EffectId> = mode
        .always()
        .iter()
        .copied()
        .chain(schedule.rotation.active())
        .collect();
    active.set(&in_play, &tuning, config.mode);
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;

    use bevy::app::App;
    use bevy::math::Vec2;

    use crate::app::error_policy::{ErrorPolicy, ErrorPolicyPlugin};
    use crate::domain::{
        BallHitPaddle, Controller, Difficulty, EffectEnded, EffectId, EffectStarted, MatchConfig,
        Mode, Seat, SeatPlayers, SimFault,
    };
    use crate::features::effects::ActiveEffects;
    use crate::features::rules::{MatchControl, Score};
    use crate::features::tuning::{EMBEDDED_RON, Tuning};
    use crate::flow::states::MatchState;
    use crate::kernel::geometry::Side;
    use crate::kernel::test_support::assert_near;
    use crate::testing::{
        ball_state, boot, enter_match, frame, headless_app, headless_app_with, match_state,
        place_ball, record, run_until, start_match, take_recorded,
    };

    fn chaos(cap: u8) -> Mode {
        Mode::Chaos {
            cap: NonZeroU8::new(cap).expect("a positive cap"),
        }
    }

    fn two_humans(mode: Mode, points: u8) -> MatchConfig {
        MatchConfig {
            left: Controller::Human(Seat::Left),
            right: Controller::Human(Seat::Right),
            players: SeatPlayers::GUESTS,
            points_to_win: NonZeroU8::new(points).expect("a positive target"),
            mode,
        }
    }

    /// The embedded tuning with Chaos given a bag of two and nothing always
    /// on, so the schedule has something to turn over while only two effects
    /// exist. Speed serves as a plain second name here.
    fn two_in_the_bag() -> Tuning {
        let ron = EMBEDDED_RON.replacen(
            "always: [speed],\n            bag: [shrink],",
            "always: [],\n            bag: [shrink, speed],",
            1,
        );
        assert_ne!(ron, EMBEDDED_RON, "Chaos's lists were not found in the RON");
        Tuning::from_ron(&ron).expect("a bag of two is valid")
    }

    /// A match in play in `mode`, to `points`, with `tuning` if given,
    /// recording what starts and ends.
    fn playing(mode: Mode, points: u8, tuning: Option<Tuning>) -> App {
        let mut app = headless_app(1);
        record::<EffectStarted>(&mut app);
        record::<EffectEnded>(&mut app);
        boot(&mut app);
        if let Some(tuning) = tuning {
            app.insert_resource(tuning);
        }
        start_match(&mut app, two_humans(mode, points));
        app
    }

    fn in_play(app: &App) -> Vec<EffectId> {
        app.world().resource::<ActiveEffects>().ids().to_vec()
    }

    fn started(app: &mut App) -> Vec<EffectId> {
        take_recorded::<EffectStarted>(app)
            .into_iter()
            .map(|message| message.effect)
            .collect()
    }

    fn ended(app: &mut App) -> Vec<EffectId> {
        take_recorded::<EffectEnded>(app)
            .into_iter()
            .map(|message| message.effect)
            .collect()
    }

    /// Waits for play, then sends the ball out over the right goal line,
    /// above the paddle: a point for the left. Stops in the point pause.
    fn left_scores(app: &mut App) {
        until(app, MatchState::Playing);
        place_ball(app, Vec2::new(70.0, 30.0), Vec2::new(250.0, 0.0));
        run_until(
            app,
            |app| match_state(app) == MatchState::PointScored,
            30,
            "no point was scored",
        );
    }

    fn until(app: &mut App, state: MatchState) {
        run_until(
            app,
            |app| match_state(app) == state,
            1_000,
            "the state never came",
        );
    }

    /// M2 spec 2.1.2: Classic is Speed alone, and starts nothing.
    #[test]
    fn classic_plays_with_speed_alone_and_announces_nothing() {
        let mut app = playing(Mode::Classic, 5, None);
        assert_eq!(in_play(&app), [EffectId::Speed]);
        left_scores(&mut app);
        until(&mut app, MatchState::Playing);
        assert_eq!(in_play(&app), [EffectId::Speed]);
        assert_eq!(started(&mut app), []);
        assert_eq!(ended(&mut app), []);
    }

    /// Chaos is in play from the first serve: Speed always, one from the bag.
    #[test]
    fn chaos_starts_its_first_bag_effect_before_the_first_serve() {
        let mut app = playing(chaos(3), 5, None);
        assert_eq!(in_play(&app), [EffectId::Speed, EffectId::Shrink]);
        assert_eq!(started(&mut app), [EffectId::Shrink]);
        assert_eq!(ended(&mut app), []);
    }

    /// M2 spec 2.3.7 and 2.4.3: a rotation ends one effect and starts the
    /// next, in that order, between two points and never inside a rally.
    #[test]
    fn a_point_rotates_one_effect_out_and_the_next_in_at_the_countdown() {
        let mut app = playing(chaos(1), 21, Some(two_in_the_bag()));
        let first = in_play(&app);
        assert_eq!(first.len(), 1);
        assert_eq!(started(&mut app), first);

        left_scores(&mut app);
        assert_eq!(in_play(&app), first, "nothing changes before the countdown");
        assert_eq!(started(&mut app), []);

        until(&mut app, MatchState::Countdown);
        let second = in_play(&app);
        assert_eq!(second.len(), 1);
        assert_ne!(second, first);
        assert_eq!(ended(&mut app), first);
        assert_eq!(started(&mut app), second);

        until(&mut app, MatchState::Playing);
        for _ in 0..20 {
            frame(&mut app);
        }
        assert_eq!(in_play(&app), second, "nothing changes inside a rally");
        assert_eq!(started(&mut app), []);
        assert_eq!(ended(&mut app), []);

        // The bag of two has turned once over; the third rally has the first again.
        left_scores(&mut app);
        until(&mut app, MatchState::Countdown);
        assert_eq!(in_play(&app), first);
    }

    /// M2 spec 2.4.1 and 2.4.4 through the systems: to five points the count
    /// rises to two when the leader has two, and then one joins and none leaves.
    #[test]
    fn the_count_rises_with_the_leaders_score_and_a_rise_only_adds() {
        let mut app = playing(chaos(2), 5, Some(two_in_the_bag()));
        assert_eq!(in_play(&app).len(), 1);
        started(&mut app);

        left_scores(&mut app);
        until(&mut app, MatchState::Countdown);
        assert_eq!(
            in_play(&app).len(),
            1,
            "one point is under thirty percent of five"
        );
        started(&mut app);
        ended(&mut app);

        left_scores(&mut app);
        until(&mut app, MatchState::Countdown);
        assert_eq!(in_play(&app).len(), 2);
        assert_eq!(started(&mut app).len(), 1);
        assert_eq!(ended(&mut app), [], "a rise only adds");
    }

    #[test]
    fn the_cap_holds_the_count_however_far_the_leader_gets() {
        let mut app = playing(chaos(1), 5, Some(two_in_the_bag()));
        for _ in 0..4 {
            left_scores(&mut app);
            until(&mut app, MatchState::Countdown);
            assert_eq!(in_play(&app).len(), 1);
        }
    }

    /// M2 spec 2.4.4: a fault restart enters the countdown again and rotates nothing.
    #[test]
    fn a_fault_restart_rotates_nothing() {
        let mut app =
            headless_app_with(1, |core| core.set(ErrorPolicyPlugin(ErrorPolicy::Release)));
        record::<EffectStarted>(&mut app);
        record::<EffectEnded>(&mut app);
        record::<SimFault>(&mut app);
        boot(&mut app);
        app.insert_resource(two_in_the_bag());
        start_match(&mut app, two_humans(chaos(1), 21));
        left_scores(&mut app);
        until(&mut app, MatchState::Playing);
        let before = in_play(&app);
        started(&mut app);
        ended(&mut app);

        place_ball(&mut app, Vec2::ZERO, Vec2::new(f32::NAN, 0.0));
        until(&mut app, MatchState::Countdown);
        assert_eq!(take_recorded::<SimFault>(&mut app).len(), 1);
        until(&mut app, MatchState::Playing);
        assert_eq!(in_play(&app), before);
        assert_eq!(started(&mut app), []);
        assert_eq!(ended(&mut app), []);
    }

    /// Resuming a paused countdown enters the countdown again and rotates nothing.
    #[test]
    fn resuming_a_paused_countdown_rotates_nothing() {
        let mut app = playing(chaos(1), 21, Some(two_in_the_bag()));
        left_scores(&mut app);
        until(&mut app, MatchState::Countdown);
        let before = in_play(&app);
        started(&mut app);
        ended(&mut app);

        app.world_mut()
            .resource_mut::<MatchControl>()
            .request_pause();
        until(&mut app, MatchState::Paused);
        app.world_mut()
            .resource_mut::<MatchControl>()
            .request_resume();
        until(&mut app, MatchState::Countdown);
        until(&mut app, MatchState::Playing);
        assert_eq!(in_play(&app), before);
        assert_eq!(started(&mut app), []);
        assert_eq!(ended(&mut app), []);
    }

    /// A new match draws its bag afresh and starts from one effect again.
    #[test]
    fn a_second_match_starts_its_schedule_over() {
        let mut app = playing(chaos(2), 5, Some(two_in_the_bag()));
        for _ in 0..2 {
            left_scores(&mut app);
            until(&mut app, MatchState::Countdown);
        }
        assert_eq!(in_play(&app).len(), 2);
        crate::testing::leave_match(&mut app);
        started(&mut app);
        enter_match(&mut app, two_humans(chaos(2), 5));
        assert_eq!(in_play(&app).len(), 1);
        assert_eq!(started(&mut app).len(), 1);
    }

    /// The speed a ball leaves the right paddle at after `returns` returns in `mode`.
    fn speed_after(mode: Mode, returns: u8) -> f32 {
        let mut app = playing(mode, 5, None);
        record::<BallHitPaddle>(&mut app);
        for _ in 0..returns {
            place_ball(&mut app, Vec2::new(72.0, 2.0), Vec2::new(200.0, 0.0));
            frame(&mut app);
        }
        assert_eq!(
            take_recorded::<BallHitPaddle>(&mut app).len(),
            usize::from(returns)
        );
        ball_state(&mut app).1.length()
    }

    /// M2 spec 2.6.0's done-condition: after the same returns a Chaos ball is
    /// slower than a Classic one. Embedded: 7 a return against 3.
    #[test]
    fn after_the_same_returns_a_chaos_ball_is_slower_than_a_classic_one() {
        let classic = speed_after(Mode::Classic, 3);
        let chaos = speed_after(chaos(3), 3);
        assert_near(classic, 101.0, 1e-3);
        assert_near(chaos, 89.0, 1e-3);
        assert!(chaos < classic);
    }

    /// M2 spec 2.1.1's done-condition: a headless Chaos match reaches
    /// `MatchOver`. Two one-star computers play to five.
    #[test]
    fn a_headless_chaos_match_between_two_computers_reaches_match_over() {
        let mut app = headless_app(3);
        boot(&mut app);
        enter_match(
            &mut app,
            MatchConfig {
                left: Controller::Ai(Difficulty::OneStar),
                right: Controller::Ai(Difficulty::OneStar),
                players: SeatPlayers::GUESTS,
                points_to_win: NonZeroU8::new(5).expect("five is non-zero"),
                mode: chaos(3),
            },
        );
        run_until(
            &mut app,
            |app| match_state(app) == MatchState::MatchOver,
            200_000,
            "the Chaos match never ended",
        );
        let score = *app.world().resource::<Score>();
        assert_eq!(score.side(Side::Left).max(score.side(Side::Right)), 5);
    }
}

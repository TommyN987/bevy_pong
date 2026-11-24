//! The flight: what holds for a ball from a serve or a paddle return until
//! the next return or a goal (M2 spec 2.2.0, ADR 0011).
//!
//! Effects change nothing in mid-air. At a flight's start each active effect
//! is asked once for its [`Contribution`]; the contributions compose into one
//! [`Flight`], which the physics step and the AI's prediction both read. That
//! shared reading extends the fairness guarantee of spec 1.2 to forces.

use std::fmt;

use bevy::math::Vec2;

use super::geometry::Side;
use super::integrate::Motion;
use super::obstacle::{BounceRule, Obstacle, ObstaclesInFlight};
use super::random::RallySeed;

/// What a paddle did in the current fixed step: the displacement it actually
/// made, not the key that was held. A paddle pressed against a wall is `Still`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StepMotion {
    /// It moved toward `+y`.
    Up,
    /// It did not move.
    #[default]
    Still,
    /// It moved toward `-y`.
    Down,
}

impl StepMotion {
    /// The motion that displaced a paddle by `dy` in one step.
    #[must_use]
    pub fn of(dy: f32) -> Self {
        if dy > 0.0 {
            Self::Up
        } else if dy < 0.0 {
            Self::Down
        } else {
            Self::Still
        }
    }
}

/// A lateral force field: one effect's push on the ball along y.
///
/// Implementations are pure: the same motion and time give the same answer,
/// which is what lets the AI predict through a field (design.md, invariants).
/// Parameters are fixed when the flight starts. `Send + Sync` because a
/// flight is stored in a component.
pub trait Field: fmt::Debug + Send + Sync {
    /// Acceleration along y, in units per second squared (the ball has unit
    /// mass), for a ball in `motion`, `seconds_in_flight` after the flight began.
    fn lateral(&self, motion: Motion, seconds_in_flight: f32) -> f32;
}

/// A steady pull across the ball's path, stated as what it does to the ball
/// (M2 spec 2.6.2): `per_crossing` arena units of lateral displacement over
/// one crossing of `crossing_width`, at any horizontal speed. Positive
/// carries the ball toward `+y`.
///
/// A slower ball is in the air longer and is pulled more gently, so the
/// displacement at the far side is the same: the dial is what the player
/// sees at the paddle, not an acceleration. From rest across the path the
/// displacement is `a t² / 2` over the time `crossing_width / |vx|`, which
/// gives `a = 2 · per_crossing · (vx / crossing_width)²`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drift {
    per_crossing: f32,
    crossing_width: f32,
}

impl Drift {
    /// A drift of `per_crossing` units over a crossing of `crossing_width`.
    ///
    /// Precondition (the caller's): `crossing_width` is positive and both
    /// are finite, as geometry from validated tuning is.
    #[must_use]
    pub const fn new(per_crossing: f32, crossing_width: f32) -> Self {
        Self {
            per_crossing,
            crossing_width,
        }
    }
}

impl Field for Drift {
    fn lateral(&self, motion: Motion, _seconds_in_flight: f32) -> f32 {
        let crossings_per_second = motion.velocity.x / self.crossing_width;
        2.0 * self.per_crossing * crossings_per_second * crossings_per_second
    }
}

/// The paddle return that starts a flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    /// The side of the paddle that returned the ball.
    pub side: Side,
    /// What that paddle did in the step of the hit.
    pub paddle_motion: StepMotion,
}

/// The moment a flight begins: what every effect is told when asked for its
/// contribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlightStart {
    /// Paddle returns in the rally so far, this one included. Zero at a serve.
    pub returns: u16,
    /// The return that starts the flight; `None` at a serve.
    pub hit: Option<Hit>,
    /// Whether `returns` is a held count (M2 spec 2.9.1) and not the
    /// rally's own: then every flight of the rally starts at it, the
    /// flights before this one too. An effect whose answer looks back over
    /// earlier flights finds them all at this count.
    pub returns_held: bool,
}

impl FlightStart {
    /// The start of a rally's first flight.
    pub const SERVE: Self = Self {
        returns: 0,
        hit: None,
        returns_held: false,
    };
}

/// One effect's share of one flight: its stance on every channel.
///
/// Deliberately without `Default`: an effect writes every field, so adding a
/// channel stops every effect compiling until it states its stance (ADR 0011).
#[derive(Debug)]
pub struct Contribution {
    /// Added to the ball's launch speed, in units per second. The base adds
    /// the serve speed and applies the cap ([`launch_speed`]).
    ///
    /// [`launch_speed`]: super::reflect::launch_speed
    pub speed_gain: f32,
    /// Multiplies the height of both paddles; `1.0` leaves them alone. The
    /// base applies the product through the paddle's clamped setter.
    pub paddle_height_factor: f32,
    /// A lateral force acting for the whole flight, if any.
    pub field: Option<Box<dyn Field>>,
    /// Solid obstacles standing on the floor for the whole flight, if any
    /// (M2 spec 2.2.6). The base bounds how the ball bounces off them
    /// ([`BounceRule`]).
    pub obstacles: Vec<Obstacle>,
}

/// The composed state of one flight.
///
/// Invariant: nothing but [`tick`](Self::tick) and
/// [`bounced`](Self::bounced) changes a flight once [`start`](Self::start),
/// [`in_rally`](Self::in_rally), and [`bouncing_by`](Self::bouncing_by)
/// have built it: its clock and its count of bounces off obstacles move,
/// and what the effects contributed does not.
/// [`believed_as`](Self::believed_as) builds the one other kind: a belief
/// about a flight, with that flight's start, rally, and clock, made to be
/// predicted under and never to be flown. A belief is about what the
/// effects contribute; the bounce rule is the base's, and a believed flight
/// keeps the true one.
/// A flight is of one rally and keeps that rally's seed: the seed of the
/// next rally is drawn while the last flight of this one is still on the
/// ball, and whatever is derived from a flight is derived from its own.
/// Returns, steps, and bounces stop at `u16::MAX` rather than wrap; returns
/// and steps are `u16` because every `u16` is exact in `f32`.
#[derive(Debug)]
pub struct Flight {
    began_at: FlightStart,
    rally: RallySeed,
    steps: u16,
    speed_gain: f32,
    paddle_height_factor: f32,
    fields: Vec<Box<dyn Field>>,
    obstacles: Vec<Obstacle>,
    bounce: BounceRule,
    obstacle_bounces: u16,
}

/// The empty flight: no returns, no gain, paddles at their own height, no
/// forces, no obstacles. What a ball is in between rallies.
impl Default for Flight {
    fn default() -> Self {
        Self {
            began_at: FlightStart::SERVE,
            rally: RallySeed::default(),
            steps: 0,
            speed_gain: 0.0,
            paddle_height_factor: 1.0,
            fields: Vec::new(),
            obstacles: Vec::new(),
            bounce: BounceRule::default(),
            obstacle_bounces: 0,
        }
    }
}

impl Flight {
    /// The flight that begins at `start`, composed from each active effect's
    /// contribution. Gains and fields compose by sum, height factors by
    /// product, and obstacles by union, so the order of `contributions`
    /// does not matter.
    #[must_use]
    pub fn start(
        start: FlightStart,
        contributions: impl IntoIterator<Item = Contribution>,
    ) -> Self {
        let mut flight = Self {
            began_at: start,
            ..Self::default()
        };
        for contribution in contributions {
            flight.speed_gain += contribution.speed_gain;
            flight.paddle_height_factor *= contribution.paddle_height_factor;
            flight.fields.extend(contribution.field);
            flight.obstacles.extend(contribution.obstacles);
        }
        flight
    }

    /// This flight as a flight of the rally of `rally`. Without it a flight
    /// is of no rally and has the default seed, as the empty flight does.
    #[must_use]
    pub fn in_rally(self, rally: RallySeed) -> Self {
        Self { rally, ..self }
    }

    /// This flight bouncing off its obstacles by `rule`: the base's bounds
    /// on the obstacle channel (M2 spec 2.2.6). Without it a flight bounces
    /// by [`BounceRule::default`].
    #[must_use]
    pub fn bouncing_by(self, rule: BounceRule) -> Self {
        Self {
            bounce: rule,
            ..self
        }
    }

    /// This flight as an observer believes it (M2 spec 2.3.12): the same
    /// start, rally, and clock, composed from `believed` contributions in
    /// place of the true ones, with the bounces off obstacles the flight
    /// has made, which an observer has seen. Prediction under the result is
    /// what an AI with that belief expects the ball to do.
    ///
    /// Precondition (the caller's): `believed` holds what each effect of
    /// this flight is believed to contribute. Believing every effect's true
    /// contribution gives a flight that pushes exactly as this one.
    #[must_use]
    pub fn believed_as(&self, believed: impl IntoIterator<Item = Contribution>) -> Self {
        Self {
            steps: self.steps,
            rally: self.rally,
            bounce: self.bounce,
            obstacle_bounces: self.obstacle_bounces,
            ..Self::start(self.began_at, believed)
        }
    }

    /// The seed of the rally this flight began in: what its effects chose
    /// from, whatever seed has been drawn since.
    #[must_use]
    pub const fn rally(&self) -> RallySeed {
        self.rally
    }

    /// The moment this flight began: what every effect was told when asked
    /// for its contribution. [`FlightStart::SERVE`] for the empty flight.
    #[must_use]
    pub const fn began_at(&self) -> FlightStart {
        self.began_at
    }

    /// Paddle returns in the rally so far. Zero from the serve to the first return.
    #[must_use]
    pub const fn returns(&self) -> u16 {
        self.began_at.returns
    }

    /// The summed speed gain of the effects, before the serve speed and the cap.
    #[must_use]
    pub const fn speed_gain(&self) -> f32 {
        self.speed_gain
    }

    /// The product of the effects' paddle height factors; `1.0` with none.
    #[must_use]
    pub const fn paddle_height_factor(&self) -> f32 {
        self.paddle_height_factor
    }

    /// The obstacles standing on the floor during this flight, in no
    /// meaningful order.
    #[must_use]
    pub fn obstacles(&self) -> &[Obstacle] {
        &self.obstacles
    }

    /// The rule the ball bounces off those obstacles by.
    #[must_use]
    pub const fn bounce_rule(&self) -> BounceRule {
        self.bounce
    }

    /// Bounces off obstacles since this flight began.
    #[must_use]
    pub const fn obstacle_bounces(&self) -> u16 {
        self.obstacle_bounces
    }

    /// This flight's obstacles as the ball meets them now: with the rule it
    /// bounces by and the bounces made so far, which is what one step among
    /// them asks for
    /// ([`advance_among_obstacles`](super::obstacle::advance_among_obstacles)).
    #[must_use]
    pub fn obstacles_in_flight(&self) -> ObstaclesInFlight<'_> {
        ObstaclesInFlight {
            obstacles: &self.obstacles,
            rule: self.bounce,
            bounced: self.obstacle_bounces,
        }
    }

    /// Fixed steps since this flight began.
    #[must_use]
    pub const fn steps(&self) -> u16 {
        self.steps
    }

    /// Force on a ball in `motion` at this flight's present step.
    #[must_use]
    pub fn force(&self, motion: Motion, dt: f32) -> Vec2 {
        self.force_ahead(motion, 0, dt)
    }

    /// Force on a ball in `motion`, `steps_ahead` fixed steps of `dt` seconds
    /// after this flight's present step: what [`force`](Self::force) will
    /// return once [`tick`](Self::tick) has run that many times. Prediction
    /// looks ahead with this instead of mutating the flight.
    #[must_use]
    pub fn force_ahead(&self, motion: Motion, steps_ahead: u32, dt: f32) -> Vec2 {
        let ahead = u16::try_from(steps_ahead).unwrap_or(u16::MAX);
        let seconds = f32::from(self.steps.saturating_add(ahead)) * dt;
        let lateral: f32 = self
            .fields
            .iter()
            .map(|field| field.lateral(motion, seconds))
            .sum();
        Vec2::new(0.0, lateral)
    }

    /// Advances the flight's clock by one fixed step.
    pub const fn tick(&mut self) {
        self.steps = self.steps.saturating_add(1);
    }

    /// Counts `bounces` more bounces off obstacles: those of the step just
    /// taken. The bounce budget is spent from this count (M2 spec 2.2.6.1).
    pub fn bounced(&mut self, bounces: u8) {
        self.obstacle_bounces = self.obstacle_bounces.saturating_add(u16::from(bounces));
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec2;

    use std::num::NonZeroU8;

    use super::{Contribution, Drift, Field, Flight, FlightStart, Hit, StepMotion};
    use crate::kernel::geometry::Side;
    use crate::kernel::integrate::{Motion, integrate};
    use crate::kernel::obstacle::{BounceRule, Obstacle};
    use crate::kernel::random::RallySeed;
    use crate::kernel::reflect::MaxDeflection;
    use crate::kernel::test_support::{assert_near, assert_vec_near};

    const DT: f32 = 1.0 / 60.0;

    /// A constant push.
    #[derive(Debug)]
    struct Steady(f32);

    impl Field for Steady {
        fn lateral(&self, _motion: Motion, _seconds_in_flight: f32) -> f32 {
            self.0
        }
    }

    /// A push that grows with time in flight and with the ball's height.
    #[derive(Debug)]
    struct Growing;

    impl Field for Growing {
        fn lateral(&self, motion: Motion, seconds_in_flight: f32) -> f32 {
            seconds_in_flight + motion.position.y
        }
    }

    fn at(y: f32) -> Motion {
        Motion {
            position: Vec2::new(0.0, y),
            velocity: Vec2::new(60.0, 0.0),
        }
    }

    fn pushing(speed_gain: f32, field: impl Field + 'static) -> Contribution {
        Contribution {
            speed_gain,
            paddle_height_factor: 1.0,
            field: Some(Box::new(field)),
            obstacles: Vec::new(),
        }
    }

    /// Where a level ball ends after `steps` steps at `velocity_x` under
    /// `drift`, from the origin.
    fn carried(drift: Drift, velocity_x: f32, steps: u16) -> Vec2 {
        let mut flight = Flight::start(FlightStart::SERVE, [pushing(0.0, drift)]);
        let mut motion = Motion {
            position: Vec2::ZERO,
            velocity: Vec2::new(velocity_x, 0.0),
        };
        for _ in 0..steps {
            motion = integrate(motion, flight.force(motion, DT), DT);
            flight.tick();
        }
        motion.position
    }

    /// M2 spec 2.6.2: the dial is in play terms. A drift moves the ball by
    /// its amount over one crossing whatever the ball's speed and whichever
    /// way it flies, apart from the integrator's own budget: semi-implicit
    /// Euler lands a further `amount / steps` along the pull.
    #[test]
    fn a_drift_carries_the_ball_its_amount_over_one_crossing_at_any_speed() {
        let width = 150.0;
        // 150 units take 36 steps at 250 units/s and 120 at 75.
        for (velocity_x, steps) in [(250.0, 36_u16), (-250.0, 36), (75.0, 120)] {
            let end = carried(Drift::new(24.0, width), velocity_x, steps);
            assert_near(end.x.abs(), width, 1e-2);
            let budget = 24.0 / f32::from(steps);
            assert!(
                (24.0 - 1e-3..=24.0 + budget + 1e-3).contains(&end.y),
                "at {velocity_x} the ball drifted {}, not 24 to {}",
                end.y,
                24.0 + budget
            );
        }
    }

    #[test]
    fn a_negative_drift_carries_the_ball_down_and_a_drift_of_nothing_nowhere() {
        let down = carried(Drift::new(-24.0, 150.0), 250.0, 36);
        let up = carried(Drift::new(24.0, 150.0), 250.0, 36);
        assert_near(down.y, -up.y, 1e-4);
        let none = carried(Drift::new(0.0, 150.0), 250.0, 36);
        assert_near(none.y, 0.0, 1e-6);
    }

    #[test]
    fn step_motion_is_the_sign_of_the_displacement() {
        assert_eq!(StepMotion::of(1.5), StepMotion::Up);
        assert_eq!(StepMotion::of(-0.01), StepMotion::Down);
        assert_eq!(StepMotion::of(0.0), StepMotion::Still);
        assert_eq!(StepMotion::default(), StepMotion::Still);
    }

    #[test]
    fn an_empty_flight_pushes_nothing_and_gains_nothing() {
        let flight = Flight::default();
        assert_eq!(flight.force(at(3.0), DT), Vec2::ZERO);
        assert_eq!(flight.returns(), 0);
        assert_eq!(flight.steps(), 0);
        assert_near(flight.speed_gain(), 0.0, 1e-6);
        assert_near(flight.paddle_height_factor(), 1.0, 1e-6);
    }

    #[test]
    fn height_factors_compose_by_product_in_either_order() {
        let shrinking = |factor: f32| Contribution {
            speed_gain: 0.0,
            paddle_height_factor: factor,
            field: None,
            obstacles: Vec::new(),
        };
        let one = Flight::start(FlightStart::SERVE, [shrinking(0.5), shrinking(0.8)]);
        let other = Flight::start(FlightStart::SERVE, [shrinking(0.8), shrinking(0.5)]);
        assert_near(one.paddle_height_factor(), 0.4, 1e-6);
        assert_near(other.paddle_height_factor(), 0.4, 1e-6);
    }

    #[test]
    fn the_clock_counts_the_steps_ticked() {
        let mut flight = Flight::default();
        flight.tick();
        flight.tick();
        assert_eq!(flight.steps(), 2);
    }

    /// A serve belongs to a rally that counts: no count is held at it.
    #[test]
    fn the_serve_holds_no_count() {
        assert_eq!(
            FlightStart::SERVE,
            FlightStart {
                returns: 0,
                hit: None,
                returns_held: false,
            }
        );
    }

    #[test]
    fn a_flight_keeps_the_return_count_it_started_with() {
        let start = FlightStart {
            returns: 4,
            hit: Some(Hit {
                side: Side::Left,
                paddle_motion: StepMotion::Up,
            }),
            returns_held: false,
        };
        assert_eq!(Flight::start(start, []).returns(), 4);
        assert_eq!(Flight::start(FlightStart::SERVE, []).returns(), 0);
    }

    #[test]
    fn a_flight_remembers_the_start_it_began_at() {
        let start = FlightStart {
            returns: 4,
            hit: Some(Hit {
                side: Side::Left,
                paddle_motion: StepMotion::Up,
            }),
            returns_held: false,
        };
        assert_eq!(Flight::start(start, []).began_at(), start);
        assert_eq!(Flight::default().began_at(), FlightStart::SERVE);
    }

    /// A flight belongs to one rally and keeps that rally's seed, whatever
    /// seed is drawn after it began; one built for no rally has the seed of
    /// none.
    #[test]
    fn a_flight_remembers_the_rally_it_began_in() {
        let rally = RallySeed::new(41);
        let mut flight = Flight::start(FlightStart::SERVE, []).in_rally(rally);
        flight.tick();
        assert_eq!(flight.rally(), rally);
        assert_eq!(
            Flight::start(FlightStart::SERVE, []).rally(),
            RallySeed::default()
        );
        assert_eq!(Flight::default().rally(), RallySeed::default());
    }

    /// A belief about a flight is about that flight: the same rally.
    #[test]
    fn a_believed_flight_is_of_the_same_rally() {
        let rally = RallySeed::new(41);
        let flown = Flight::start(FlightStart::SERVE, [pushing(0.0, Steady(2.0))]).in_rally(rally);
        assert_eq!(flown.believed_as([]).rally(), rally);
    }

    /// M2 spec 2.3.12: an AI's degraded belief is the same flight composed
    /// from what the AI believes each effect contributes.
    #[test]
    fn a_believed_flight_keeps_the_start_and_the_clock_and_pushes_as_believed() {
        let start = FlightStart {
            returns: 4,
            hit: None,
            returns_held: false,
        };
        let mut flown = Flight::start(start, [pushing(7.0, Steady(2.0)), pushing(0.0, Growing)]);
        for _ in 0..30 {
            flown.tick();
        }
        // The AI sees the growing push and misses the steady one.
        let believed = flown.believed_as([pushing(0.0, Growing)]);
        assert_eq!(believed.began_at(), start);
        assert_eq!(believed.steps(), 30);
        // Growing at height 5, half a second in: 5.5, without Steady's 2.
        assert_vec_near(believed.force(at(5.0), DT), Vec2::new(0.0, 5.5), 1e-6);
        assert_vec_near(flown.force(at(5.0), DT), Vec2::new(0.0, 7.5), 1e-6);
    }

    #[test]
    fn contributions_compose_by_sum_in_either_order() {
        let one = Flight::start(
            FlightStart::SERVE,
            [pushing(7.0, Steady(2.0)), pushing(3.0, Growing)],
        );
        let other = Flight::start(
            FlightStart::SERVE,
            [pushing(3.0, Growing), pushing(7.0, Steady(2.0))],
        );
        assert_near(one.speed_gain(), 10.0, 1e-6);
        assert_near(other.speed_gain(), 10.0, 1e-6);
        // Steady 2, plus Growing at height 5 and time zero: 5.
        assert_vec_near(one.force(at(5.0), DT), Vec2::new(0.0, 7.0), 1e-6);
        assert_vec_near(other.force(at(5.0), DT), Vec2::new(0.0, 7.0), 1e-6);
    }

    #[test]
    fn a_contribution_without_a_field_adds_no_force() {
        let flight = Flight::start(
            FlightStart::SERVE,
            [Contribution {
                speed_gain: 7.0,
                paddle_height_factor: 1.0,
                field: None,
                obstacles: Vec::new(),
            }],
        );
        assert_eq!(flight.force(at(5.0), DT), Vec2::ZERO);
        assert_near(flight.speed_gain(), 7.0, 1e-6);
    }

    #[test]
    fn looking_ahead_equals_ticking_that_many_steps() {
        let mut flight = Flight::start(FlightStart::SERVE, [pushing(0.0, Growing)]);
        let ahead = flight.force_ahead(at(0.0), 30, DT);
        for _ in 0..30 {
            flight.tick();
        }
        assert_eq!(flight.force(at(0.0), DT), ahead);
        assert_vec_near(ahead, Vec2::new(0.0, 0.5), 1e-6);
    }

    #[test]
    fn the_clock_stops_at_its_limit_instead_of_wrapping() {
        let flight = Flight::start(FlightStart::SERVE, [pushing(0.0, Growing)]);
        let limit = f32::from(u16::MAX) * DT;
        assert_vec_near(
            flight.force_ahead(at(0.0), 1_000_000, DT),
            Vec2::new(0.0, limit),
            1e-3,
        );
    }

    fn standing(obstacles: Vec<Obstacle>) -> Contribution {
        Contribution {
            speed_gain: 0.0,
            paddle_height_factor: 1.0,
            field: None,
            obstacles,
        }
    }

    fn block_at(x: f32) -> Obstacle {
        Obstacle::new(Vec2::new(x, 0.0), Vec2::new(1.0, 7.0), 0.2)
    }

    /// M2 spec 2.2.6: the obstacle channel composes by union.
    #[test]
    fn a_flight_carries_the_obstacles_of_every_contribution_in_either_order() {
        let one = || standing(vec![block_at(-30.0), block_at(30.0)]);
        let other = || standing(vec![block_at(10.0)]);
        for flight in [
            Flight::start(FlightStart::SERVE, [one(), other()]),
            Flight::start(FlightStart::SERVE, [other(), one()]),
        ] {
            assert_eq!(flight.obstacles().len(), 3);
            for x in [-30.0, 10.0, 30.0] {
                assert!(flight.obstacles().contains(&block_at(x)), "no block at {x}");
            }
        }
        assert_eq!(Flight::default().obstacles(), []);
    }

    fn four_at_sixty_degrees() -> BounceRule {
        BounceRule::new(
            NonZeroU8::new(4).expect("four is not zero"),
            MaxDeflection::new(60.0_f32.to_radians()).expect("sixty degrees is valid"),
        )
    }

    /// M2 spec 2.2.6: the bounds are the channel's. A flight bounces by
    /// the rule the base gives it, and by one contact and no floor until then.
    #[test]
    fn a_flight_bounces_by_the_rule_it_is_given() {
        let flight = Flight::start(FlightStart::SERVE, [standing(vec![block_at(0.0)])]);
        assert_eq!(flight.bounce_rule(), BounceRule::default());
        let ruled = flight.bouncing_by(four_at_sixty_degrees());
        assert_eq!(ruled.bounce_rule(), four_at_sixty_degrees());
        assert_eq!(ruled.obstacles(), [block_at(0.0)]);
    }

    /// M2 spec 2.3.12: an AI may believe in other obstacles than lie there,
    /// and not in another rule: the rule is the base's, not an effect's.
    #[test]
    fn a_believed_flight_bounces_by_the_true_rule_off_the_believed_obstacles() {
        let flight = Flight::start(FlightStart::SERVE, [standing(vec![block_at(0.0)])])
            .bouncing_by(four_at_sixty_degrees());
        let believed = flight.believed_as([standing(vec![block_at(5.0)])]);
        assert_eq!(believed.obstacles(), [block_at(5.0)]);
        assert_eq!(believed.bounce_rule(), four_at_sixty_degrees());
    }

    /// M2 spec 2.2.6.1: the budget is a flight's. A flight counts its
    /// bounces off obstacles, hands the step its obstacles with that count,
    /// and a new flight starts at none.
    #[test]
    fn a_flight_counts_its_bounces_off_obstacles_and_a_new_flight_starts_at_none() {
        let mut flight = Flight::start(FlightStart::SERVE, [standing(vec![block_at(0.0)])])
            .bouncing_by(four_at_sixty_degrees());
        assert_eq!(flight.obstacle_bounces(), 0);
        flight.bounced(2);
        flight.bounced(0);
        flight.bounced(1);
        assert_eq!(flight.obstacle_bounces(), 3);
        let in_flight = flight.obstacles_in_flight();
        assert_eq!(in_flight.obstacles, [block_at(0.0)]);
        assert_eq!(in_flight.rule, four_at_sixty_degrees());
        assert_eq!(in_flight.bounced, 3);

        let next = Flight::start(FlightStart::SERVE, [standing(vec![block_at(0.0)])]);
        assert_eq!(next.obstacle_bounces(), 0);
    }

    /// An AI watching the ball has seen it bounce: a belief about a flight
    /// keeps the flight's count, as it keeps its clock.
    #[test]
    fn a_believed_flight_keeps_the_bounces_made() {
        let mut flight = Flight::start(FlightStart::SERVE, [standing(vec![block_at(0.0)])]);
        flight.bounced(2);
        let believed = flight.believed_as([standing(Vec::new())]);
        assert_eq!(believed.obstacle_bounces(), 2);
    }

    #[test]
    fn the_bounce_count_stops_at_its_limit_instead_of_wrapping() {
        let mut flight = Flight::default();
        for _ in 0..300 {
            flight.bounced(u8::MAX);
        }
        assert_eq!(flight.obstacle_bounces(), u16::MAX);
    }
}

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
}

impl FlightStart {
    /// The start of a rally's first flight.
    pub const SERVE: Self = Self {
        returns: 0,
        hit: None,
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
}

/// The composed state of one flight.
///
/// Invariant: nothing but [`tick`](Self::tick) changes a flight after
/// [`start`](Self::start) built it. Returns and steps stop at `u16::MAX`
/// rather than wrap; both are `u16` because every `u16` is exact in `f32`.
#[derive(Debug)]
pub struct Flight {
    returns: u16,
    steps: u16,
    speed_gain: f32,
    paddle_height_factor: f32,
    fields: Vec<Box<dyn Field>>,
}

/// The empty flight: no returns, no gain, paddles at their own height, no
/// forces. What a ball is in between rallies.
impl Default for Flight {
    fn default() -> Self {
        Self {
            returns: 0,
            steps: 0,
            speed_gain: 0.0,
            paddle_height_factor: 1.0,
            fields: Vec::new(),
        }
    }
}

impl Flight {
    /// The flight that begins at `start`, composed from each active effect's
    /// contribution. Gains and fields compose by sum and height factors by
    /// product, so the order of `contributions` does not matter.
    #[must_use]
    pub fn start(
        start: FlightStart,
        contributions: impl IntoIterator<Item = Contribution>,
    ) -> Self {
        let mut flight = Self {
            returns: start.returns,
            ..Self::default()
        };
        for contribution in contributions {
            flight.speed_gain += contribution.speed_gain;
            flight.paddle_height_factor *= contribution.paddle_height_factor;
            flight.fields.extend(contribution.field);
        }
        flight
    }

    /// Paddle returns in the rally so far. Zero from the serve to the first return.
    #[must_use]
    pub const fn returns(&self) -> u16 {
        self.returns
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
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec2;

    use super::{Contribution, Field, Flight, FlightStart, Hit, StepMotion};
    use crate::kernel::geometry::Side;
    use crate::kernel::integrate::Motion;
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
        }
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

    #[test]
    fn a_flight_keeps_the_return_count_it_started_with() {
        let start = FlightStart {
            returns: 4,
            hit: Some(Hit {
                side: Side::Left,
                paddle_motion: StepMotion::Up,
            }),
        };
        assert_eq!(Flight::start(start, []).returns(), 4);
        assert_eq!(Flight::start(FlightStart::SERVE, []).returns(), 0);
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
}

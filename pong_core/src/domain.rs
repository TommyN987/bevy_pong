//! Core vocabulary (spec 1.3): the types two or more features share, and the
//! events the core emits for the shell to observe. No systems live here.
//!
//! `Side` is also vocabulary; it is defined in [`crate::kernel::geometry`]
//! because geometry needs it first. Count (ADR 0002): six types of twelve
//! (`Side`, `Seat`, `Difficulty`, `Controller`, `MatchConfig`, `Random`) and
//! seven messages, each with one emitting feature, `Notice` excepted.

use std::num::NonZeroU8;

use bevy::ecs::message::Message;
use bevy::ecs::resource::Resource;
use bevy::math::Vec2;

use crate::kernel::geometry::Side;
use crate::kernel::random::Pcg32;

/// A player's chair: which key set they hold and which profile slot they fill.
///
/// Distinct from [`Side`](crate::kernel::geometry::Side): a seat is who plays,
/// a side is where a paddle stands. Against the computer the one human seat
/// drives one side; in the menus it picks the profiles for both sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Seat {
    /// The left chair: W/A/S/D by default (tuning bindings).
    Left,
    /// The right chair: the arrow keys by default (tuning bindings).
    Right,
}

/// AI strength, shown to the kids as stars. Each variant has one tuning entry
/// (spec 1.4.6), so adding a level makes the compiler ask for its numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Difficulty {
    /// Easiest.
    OneStar,
    /// Middle.
    TwoStars,
    /// Hardest.
    ThreeStars,
}

impl Difficulty {
    /// How many stars the menu shows for this level.
    #[must_use]
    pub const fn stars(self) -> u8 {
        match self {
            Self::OneStar => 1,
            Self::TwoStars => 2,
            Self::ThreeStars => 3,
        }
    }
}

/// What drives a paddle: a seated human or the mirror AI at a difficulty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Controller {
    /// Intents come from this seat's keys.
    Human(Seat),
    /// Intents come from the AI feature at this strength.
    Ai(Difficulty),
}

/// Who plays each side and how long the match runs. Built by the menu at match
/// start; read by seats, rules, and the AI. Points-to-win comes from the tuning
/// options, so zero is unrepresentable rather than checked.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchConfig {
    /// Controller of the left paddle.
    pub left: Controller,
    /// Controller of the right paddle.
    pub right: Controller,
    /// First to this many points wins.
    pub points_to_win: NonZeroU8,
}

impl MatchConfig {
    /// The controller holding `side`.
    #[must_use]
    pub const fn controller(self, side: Side) -> Controller {
        match side {
            Side::Left => self.left,
            Side::Right => self.right,
        }
    }
}

/// The only source of randomness in the core. The shell seeds it from entropy
/// at startup; tests seed it with a constant and assert exact outcomes.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct Random(Pcg32);

impl Random {
    /// A generator whose whole future follows from `seed`.
    #[must_use]
    pub fn seeded(seed: u64) -> Self {
        Self(Pcg32::seed(seed))
    }

    /// Uniform in `lo..hi`. Precondition: `lo <= hi`, both finite.
    pub fn next_f32_in(&mut self, lo: f32, hi: f32) -> f32 {
        self.0.next_f32_in(lo, hi)
    }
}

/// Launch the ball toward `toward`. Emitted by rules when a countdown ends;
/// consumed by the ball feature, which owns the serve geometry.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Serve {
    /// The side the ball flies at: the side that conceded the last point, or
    /// a seeded random side for the first serve of a match.
    pub toward: Side,
}

/// The ball bounced off the paddle standing on `side`. Emitted by physics.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BallHitPaddle {
    /// Which paddle.
    pub side: Side,
}

/// The ball bounced off the top or bottom wall. Emitted by physics.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BallHitWall;

/// The ball crossed a goal line. Emitted by rules.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointScored {
    /// The side that won the point, opposite the goal line crossed.
    pub scorer: Side,
}

/// A side reached the points-to-win. Emitted by rules.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchEnded {
    /// The side that won the match.
    pub winner: Side,
}

/// The sanity phase found the simulation broken (spec 1.4.7). Emitted by
/// sanity; rules answers it by restarting the rally, so the serve keeps one
/// emitter (ADR 0002). Each variant names what broke and the values seen.
#[derive(Message, Debug, Clone, PartialEq, thiserror::Error)]
pub enum SimFault {
    /// M1 plays with exactly one ball.
    #[error("{count} balls in the match; M1 plays with exactly one")]
    BallCount {
        /// How many balls the phase found.
        count: usize,
    },
    /// A component of the ball's velocity is NaN or infinite.
    #[error("ball velocity {velocity} is not finite")]
    BallVelocityNotFinite {
        /// The velocity seen.
        velocity: Vec2,
    },
    /// The ball is through a wall, farther past a goal line than one step at
    /// the speed cap, or at a position that is not finite.
    #[error("ball at {position} is outside the arena")]
    BallOutsideArena {
        /// The position seen.
        position: Vec2,
    },
    /// Part of the paddle is through a wall, or its centre is not finite.
    #[error("{side:?} paddle centred at y = {center_y}, {height} tall, is outside the arena")]
    PaddleOutsideArena {
        /// Which paddle.
        side: Side,
        /// Its centre seen.
        center_y: f32,
        /// Its height seen.
        height: f32,
    },
}

/// One plain sentence the player can act on, shown by the shell's notice line
/// (error policy, docs/architecture.md). Emitted by tuning, later persistence:
/// the one message with several emitters (ADR 0002).
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// The sentence, already in the player's words.
    pub text: String,
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;

    use bevy::app::{App, Update};
    use bevy::ecs::message::{Message, MessageReader, MessageWriter};
    use bevy::ecs::resource::Resource;
    use bevy::ecs::schedule::IntoScheduleConfigs;
    use bevy::ecs::system::ResMut;

    use super::{
        BallHitPaddle, BallHitWall, Controller, Difficulty, MatchConfig, MatchEnded, Notice,
        PointScored, Random, Seat, Serve, SimFault,
    };
    use crate::kernel::geometry::Side;

    #[derive(Resource)]
    struct Received<M>(Vec<M>);

    impl<M> Default for Received<M> {
        fn default() -> Self {
            Self(Vec::new())
        }
    }

    /// Writes `message` in one system and collects it in the next, headless.
    fn round_trip<M: Message + Clone>(message: M) -> Vec<M> {
        let mut app = App::new();
        app.add_message::<M>().init_resource::<Received<M>>();
        app.add_systems(
            Update,
            (
                move |mut writer: MessageWriter<M>| {
                    writer.write(message.clone());
                },
                |mut reader: MessageReader<M>, mut received: ResMut<Received<M>>| {
                    received.0.extend(reader.read().cloned());
                },
            )
                .chain(),
        );
        app.update();
        std::mem::take(&mut app.world_mut().resource_mut::<Received<M>>().0)
    }

    #[test]
    fn every_core_event_survives_a_write_and_read_in_a_headless_app() {
        assert_eq!(
            round_trip(BallHitPaddle { side: Side::Right }),
            vec![BallHitPaddle { side: Side::Right }]
        );
        assert_eq!(round_trip(BallHitWall), vec![BallHitWall]);
        assert_eq!(
            round_trip(PointScored { scorer: Side::Left }),
            vec![PointScored { scorer: Side::Left }]
        );
        assert_eq!(
            round_trip(MatchEnded { winner: Side::Left }),
            vec![MatchEnded { winner: Side::Left }]
        );
        assert_eq!(
            round_trip(Notice {
                text: "Saved games could not be read. Starting fresh.".to_owned(),
            }),
            vec![Notice {
                text: "Saved games could not be read. Starting fresh.".to_owned(),
            }]
        );
    }

    #[test]
    fn difficulty_shows_as_one_two_or_three_stars() {
        assert_eq!(Difficulty::OneStar.stars(), 1);
        assert_eq!(Difficulty::TwoStars.stars(), 2);
        assert_eq!(Difficulty::ThreeStars.stars(), 3);
    }

    #[test]
    fn random_resources_seeded_alike_draw_alike() {
        let mut a = Random::seeded(99);
        let mut b = Random::seeded(99);
        assert_eq!(a.next_f32_in(-1.0, 1.0), b.next_f32_in(-1.0, 1.0));
        assert_ne!(
            a.next_f32_in(-1.0, 1.0),
            Random::seeded(100).next_f32_in(-1.0, 1.0)
        );
    }

    #[test]
    fn serve_round_trips_as_a_message() {
        assert_eq!(
            round_trip(Serve {
                toward: Side::Right
            }),
            vec![Serve {
                toward: Side::Right
            }]
        );
    }

    #[test]
    fn a_sim_fault_round_trips_as_a_message_and_reads_as_a_sentence() {
        let fault = SimFault::BallCount { count: 2 };
        assert_eq!(round_trip(fault.clone()), vec![fault.clone()]);
        assert_eq!(
            fault.to_string(),
            "2 balls in the match; M1 plays with exactly one"
        );
    }

    #[test]
    fn match_config_answers_which_controller_holds_each_side() {
        let config = MatchConfig {
            left: Controller::Human(Seat::Right),
            right: Controller::Ai(Difficulty::OneStar),
            points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
        };
        assert_eq!(
            config.controller(Side::Left),
            Controller::Human(Seat::Right)
        );
        assert_eq!(
            config.controller(Side::Right),
            Controller::Ai(Difficulty::OneStar)
        );
    }

    #[test]
    fn a_controller_is_either_a_seated_human_or_an_ai_at_a_difficulty() {
        let human = Controller::Human(Seat::Right);
        let ai = Controller::Ai(Difficulty::TwoStars);
        assert_ne!(human, ai);
        assert!(matches!(human, Controller::Human(Seat::Right)));
        assert!(matches!(ai, Controller::Ai(d) if d.stars() == 2));
    }
}

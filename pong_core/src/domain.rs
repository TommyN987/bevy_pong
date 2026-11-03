//! Core vocabulary (spec 1.3): the types two or more features share, and the
//! events the core emits for the shell to observe. No systems live here.
//!
//! `Side` is also vocabulary; it is defined in [`crate::kernel::geometry`]
//! because geometry needs it first. Count at milestone close: under twelve.

/// A player's chair: which key set they hold and which profile slot they fill.
///
/// Distinct from [`Side`](crate::kernel::geometry::Side): a seat is who plays,
/// a side is where a paddle stands. One-human modes have one seat drive both sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Seat {
    /// The left chair: W/A/S/D.
    Left,
    /// The right chair: the arrow keys.
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

use std::num::NonZeroU8;

use bevy::ecs::message::Message;
use bevy::ecs::resource::Resource;

use crate::kernel::geometry::Side;

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

/// One plain sentence the player can act on, shown by the shell's notice line
/// (error policy, docs/architecture.md). Emitted by persistence and tuning.
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
        PointScored, Seat,
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

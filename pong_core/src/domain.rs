//! Core vocabulary (spec 1.3): the types two or more features share, and the
//! events the core emits for the shell to observe. No systems live here.
//!
//! `Side` is also vocabulary; it is defined in [`crate::kernel::geometry`]
//! because geometry needs it first. Count (ADR 0002, ADR 0008): nine capped
//! types of twelve (`Side`, `Seat`, `Difficulty`, `Controller`, `MatchConfig`,
//! `Random`, `Player`, `SeatPlayers`, `Mode`), two identifier types outside the
//! cap (`ProfileId`, `EffectId`), and nine messages, each with one emitting feature,
//! `Notice` excepted.

use std::num::NonZeroU8;

use bevy::ecs::message::Message;
use bevy::ecs::resource::Resource;
use bevy::math::Vec2;
use serde::{Deserialize, Serialize};

use crate::kernel::geometry::Side;
use crate::kernel::random::{Pcg32, RallySeed};

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

/// A profile's identity, stable across renames (spec 1.4.9). An id newtype,
/// so outside the vocabulary cap (ADR 0002, amended 2026-11-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProfileId(pub u32);

/// An effect's identity (ADR 0011): the name a mode lists, and later a room
/// composes and an item carries. An identifier, so outside the vocabulary cap
/// (ADR 0008).
///
/// The enum is closed and its serialized names are stable: a name written to
/// a file keeps meaning the same effect. Behaviour is not matched on this
/// type; each crate matches it once, to register the effect's half, and
/// reaches the behaviour through the effect's trait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectId {
    /// The ball leaves each paddle return faster (M2 spec 2.6.0).
    #[serde(rename = "speed")]
    Speed,
    /// Both paddles shorten as the rally grows (M2 spec 2.6.1).
    #[serde(rename = "shrink")]
    Shrink,
    /// A steady pull up or down, its direction chosen per rally (M2 spec 2.6.2).
    #[serde(rename = "wind")]
    Wind,
    /// A return from a moving paddle curves the way the paddle moved (M2
    /// spec 2.6.4).
    #[serde(rename = "spin")]
    Spin,
}

impl EffectId {
    /// Every effect, in declaration order.
    pub const ALL: [Self; 4] = [Self::Speed, Self::Shrink, Self::Wind, Self::Spin];
}

/// Which game a match plays (M2 spec 2.1.1). A mode is data: tuning holds
/// each mode's effects and schedule, and nothing in the simulation branches
/// on this type except to look that data up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Pong as M1 shipped it: the speed ramp alone.
    Classic,
    /// Effects join and rotate as the match goes on.
    Chaos {
        /// The most effects in play at once, the player's choice. Zero is
        /// unrepresentable; a cap above what the mode's schedule can reach
        /// simply never binds.
        cap: NonZeroU8,
    },
}

/// Who a seated human plays as (ADR 0008): the built-in Guest, never saved,
/// or a child's profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Player {
    /// Anyone; records nothing (spec 1.4.9).
    Guest,
    /// The profile with this id.
    Profile(ProfileId),
}

/// Who each seat plays as in a match (ADR 0008). Keyed by seat, not side:
/// the menu picks a player per seat (spec 1.6.3), a computer has no seat and
/// so no player, and seats outlast the side and paddle layouts later
/// milestones bring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeatPlayers {
    /// Who sits at the left seat's keys.
    pub left_seat: Player,
    /// Who sits at the right seat's keys.
    pub right_seat: Player,
}

impl SeatPlayers {
    /// Who sits at `seat`.
    #[must_use]
    pub const fn seat(self, seat: Seat) -> Player {
        match seat {
            Seat::Left => self.left_seat,
            Seat::Right => self.right_seat,
        }
    }

    /// Both seats as Guest: a match that records nothing.
    pub const GUESTS: Self = Self {
        left_seat: Player::Guest,
        right_seat: Player::Guest,
    };
}

/// Who plays each side, as whom, and how long the match runs. Built by the
/// menu at match start; read by seats, rules, the AI, and persistence.
/// Points-to-win comes from the tuning options, so zero is unrepresentable
/// rather than checked.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchConfig {
    /// Controller of the left paddle.
    pub left: Controller,
    /// Controller of the right paddle.
    pub right: Controller,
    /// Who each seat plays as.
    pub players: SeatPlayers,
    /// First to this many points wins.
    pub points_to_win: NonZeroU8,
    /// Which game is played.
    pub mode: Mode,
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

    /// Who plays on `side`: the player of the seat that holds it, or `None`
    /// for a side the computer holds.
    #[must_use]
    pub const fn player_on(self, side: Side) -> Option<Player> {
        match self.controller(side) {
            Controller::Human(seat) => Some(self.players.seat(seat)),
            Controller::Ai(_) => None,
        }
    }
}

/// The only source of randomness in the core. The shell seeds it from entropy
/// at startup; tests seed it with a constant and assert exact outcomes.
///
/// Two streams follow from the one seed. Rally seeds are drawn from the
/// second, so drawing one moves no other draw: a match whose effects use no
/// rally seed plays exactly as it would without them.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct Random {
    draws: Pcg32,
    rally_seeds: Pcg32,
}

impl Random {
    /// Sets the rally seeds' stream apart from the other draws of the same
    /// seed: the 64-bit golden ratio, an arbitrary odd constant.
    const RALLY_SEEDS: u64 = 0x9E37_79B9_7F4A_7C15;

    /// A generator whose whole future follows from `seed`.
    #[must_use]
    pub fn seeded(seed: u64) -> Self {
        Self {
            draws: Pcg32::seed(seed),
            rally_seeds: Pcg32::seed(seed ^ Self::RALLY_SEEDS),
        }
    }

    /// The next rally's seed, from the rally seeds' own stream.
    pub fn next_rally_seed(&mut self) -> RallySeed {
        RallySeed::new(self.rally_seeds.next_u32())
    }

    /// Uniform in `lo..hi`. Precondition: `lo <= hi`, both finite.
    pub fn next_f32_in(&mut self, lo: f32, hi: f32) -> f32 {
        self.draws.next_f32_in(lo, hi)
    }

    /// A whole number in `0..below`, for picking one of `below` things.
    /// Precondition: `below > 0`. The modulo's bias is far below anything a
    /// shuffle of a handful of effects can show.
    pub fn next_below(&mut self, below: u32) -> u32 {
        self.draws.next_u32() % below
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

/// An effect joined the effects in play, between two points (M2 spec 2.3.7).
/// Emitted by escalation, for a mode's scheduled effects only: an effect that
/// is always on never starts.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectStarted {
    /// The effect that joined.
    pub effect: EffectId,
}

/// An effect left the effects in play, between two points. Emitted by
/// escalation. Leaving a match ends nothing: what a match made goes with it.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectEnded {
    /// The effect that left.
    pub effect: EffectId,
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
    /// The most paddle hits in any rally of the match (plan 7.4), taken when
    /// the match ended, so a reader never depends on when it reads.
    pub longest_rally: u32,
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
    /// The ball has flown longer than any flight can without a paddle return
    /// or a goal (M2 spec 2.5.6): it is stalled or trapped.
    #[error("the ball has flown {seconds} s without a return or a goal; the limit is {limit} s")]
    FlightTooLong {
        /// How long the flight has lasted.
        seconds: f32,
        /// The tuned limit.
        limit: f32,
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
        BallHitPaddle, BallHitWall, Controller, Difficulty, EffectId, MatchConfig, MatchEnded,
        Mode, Notice, Player, PointScored, ProfileId, Random, Seat, SeatPlayers, Serve, SimFault,
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

    /// Rally seeds have a stream of their own, of the same seed: drawing
    /// them moves no other draw, so a mode whose effects use no seed plays
    /// as it did before the seeds existed (M2 plan 5.2).
    #[test]
    fn rally_seeds_come_from_their_own_stream_and_leave_every_other_draw_as_it_was() {
        let mut drawing = Random::seeded(7);
        let mut plain = Random::seeded(7);
        let seeds: Vec<_> = (0..8).map(|_| drawing.next_rally_seed()).collect();
        for _ in 0..8 {
            assert_eq!(drawing.next_below(1_000), plain.next_below(1_000));
        }

        let mut again = Random::seeded(7);
        let seeds_again: Vec<_> = (0..8).map(|_| again.next_rally_seed()).collect();
        assert_eq!(seeds, seeds_again, "the same seed, the same rally seeds");
        assert!(
            seeds.windows(2).all(|pair| pair[0] != pair[1]),
            "a rally's seed repeated the one before: {seeds:?}"
        );
        assert_ne!(Random::seeded(8).next_rally_seed(), seeds[0]);
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
            round_trip(MatchEnded {
                winner: Side::Left,
                longest_rally: 7
            }),
            vec![MatchEnded {
                winner: Side::Left,
                longest_rally: 7
            }]
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
            players: SeatPlayers::GUESTS,
            points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
            mode: Mode::Classic,
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

    #[test]
    fn each_seat_has_its_own_player() {
        let players = SeatPlayers {
            left_seat: Player::Profile(ProfileId(1)),
            right_seat: Player::Profile(ProfileId(2)),
        };
        assert_eq!(players.seat(Seat::Left), Player::Profile(ProfileId(1)));
        assert_eq!(players.seat(Seat::Right), Player::Profile(ProfileId(2)));
    }

    /// ADR 0008: a side reaches its player through the seat holding it; a
    /// computer side has none.
    #[test]
    fn a_side_is_played_by_its_seats_player_and_a_computer_side_by_no_one() {
        let config = MatchConfig {
            left: Controller::Human(Seat::Right),
            right: Controller::Ai(Difficulty::OneStar),
            players: SeatPlayers {
                left_seat: Player::Profile(ProfileId(1)),
                right_seat: Player::Profile(ProfileId(2)),
            },
            points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
            mode: Mode::Classic,
        };
        assert_eq!(
            config.player_on(Side::Left),
            Some(Player::Profile(ProfileId(2)))
        );
        assert_eq!(config.player_on(Side::Right), None);
    }

    /// M2 spec 2.3.1: every effect's name survives a round trip, and the
    /// names are the ones files already hold.
    #[test]
    fn every_effect_id_round_trips_through_its_stable_name() {
        for id in EffectId::ALL {
            let text = ron::to_string(&id).expect("an effect id serializes");
            let back: EffectId = ron::from_str(&text).expect("its own name parses");
            assert_eq!(back, id, "{text}");
        }
        assert_eq!(
            ron::to_string(&EffectId::Speed).expect("an effect id serializes"),
            "speed"
        );
        assert_eq!(
            ron::to_string(&EffectId::Shrink).expect("an effect id serializes"),
            "shrink"
        );
        assert_eq!(
            ron::to_string(&EffectId::Wind).expect("an effect id serializes"),
            "wind"
        );
        assert_eq!(
            ron::to_string(&EffectId::Spin).expect("an effect id serializes"),
            "spin"
        );
    }

    /// `ALL` is written by hand; this match stops compiling when a variant is
    /// added, pointing at the list to extend.
    #[test]
    fn all_lists_every_effect_once() {
        for (index, id) in EffectId::ALL.into_iter().enumerate() {
            let declared = match id {
                EffectId::Speed => 0,
                EffectId::Shrink => 1,
                EffectId::Wind => 2,
                EffectId::Spin => 3,
            };
            assert_eq!(index, declared);
        }
    }
}

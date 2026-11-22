//! Fog (M2 spec 2.6.3, 2.2.5): a band centred on midfield that hides the
//! ball, wider as the rally grows. It is view-only: the ball flies through
//! it as through clear air, and this half carries the identity, the dial,
//! and the AI's flaw.

use std::time::Duration;

use super::Effect;
use super::Flaw;
use crate::domain::{Difficulty, EffectId};
use crate::features::tuning::{FogTuning, Tuning};
use crate::kernel::flight::{Contribution, FlightStart};

/// Hides the ball inside a band centred on midfield, by a width that rides
/// the rally's return count.
///
/// The dial is mild at zero (R9): a serve crosses in plain sight. Fog
/// contributes to no channel, so it changes nothing the ball or a paddle
/// does (R2, R4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fog {
    tuning: FogTuning,
}

impl Fog {
    /// Fog at `tuning`'s dial.
    #[must_use]
    pub const fn new(tuning: &Tuning) -> Self {
        Self {
            tuning: tuning.effects().fog(),
        }
    }

    /// How wide the band is for the flight that began at `start`, in arena
    /// units: the dial's amount, whichever paddle returned the ball and
    /// however it moved.
    ///
    /// Postcondition: from zero to the tuned maximum. A pure function of
    /// `start`, so the shell hides the ball in the band the flaw is sized
    /// by.
    #[must_use]
    pub fn width(&self, start: &FlightStart) -> f32 {
        self.tuning.width().at(start.returns)
    }
}

impl Effect for Fog {
    fn id(&self) -> EffectId {
        EffectId::Fog
    }

    /// Nothing, on every channel (M2 spec 2.2.5).
    fn contribute(&self, _start: &FlightStart) -> Contribution {
        Contribution {
            speed_gain: 0.0,
            paddle_height_factor: 1.0,
            field: None,
        }
    }

    /// The first form alone (M2 spec 2.3.12), and synthetic (M2 spec
    /// 2.6.3): an AI sees through fog, so the fog widens the spread of its
    /// aim instead, by its star's added error at full width and by that
    /// width's share of it in a narrower band. It believes the flight as it
    /// is, since Fog changes none of it.
    fn flaw(&self, start: &FlightStart, difficulty: Difficulty) -> Flaw {
        let max_width = self.tuning.max_width();
        let share = if max_width > 0.0 {
            self.width(start) / max_width
        } else {
            0.0
        };
        Flaw {
            added_error: share * self.tuning.added_error(difficulty),
            added_delay: Duration::ZERO,
            believed: self.contribute(start),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;
    use std::time::Duration;

    use bevy::math::Vec2;

    use super::Fog;
    use crate::domain::{Controller, Difficulty, EffectId, MatchConfig, Mode, Seat, SeatPlayers};
    use crate::features::effects::{Effect, Sandbox};
    use crate::features::paddle::Paddle;
    use crate::features::seats::PaddleIntent;
    use crate::features::tuning::{EMBEDDED_RON, Tuning};
    use crate::kernel::flight::{FlightStart, Hit, StepMotion};
    use crate::kernel::geometry::Side;
    use crate::kernel::test_support::assert_near;
    use crate::testing::{ball_state, boot, enter_match, frame, headless_app, report_intent};

    const STARS: [Difficulty; 3] = [
        Difficulty::OneStar,
        Difficulty::TwoStars,
        Difficulty::ThreeStars,
    ];

    fn tuning() -> Tuning {
        Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid")
    }

    fn fog() -> Fog {
        Fog::new(&tuning())
    }

    fn after(returns: u16) -> FlightStart {
        FlightStart {
            returns,
            hit: None,
            returns_held: false,
        }
    }

    #[test]
    fn it_names_itself() {
        assert_eq!(fog().id(), EffectId::Fog);
    }

    /// R9: mild at zero. The embedded dial starts at nothing, so a serve
    /// crosses in plain sight.
    #[test]
    fn a_rally_starts_with_no_fog() {
        assert_near(fog().width(&FlightStart::SERVE), 0.0, 1e-6);
    }

    /// M2 spec 2.6.3's dial, R9's grades, and R3's shape: the band never
    /// narrows as the rally grows, and is as wide from the return the dial
    /// is full at. The embedded fog is 64 units wide from sixteen returns on.
    #[test]
    fn the_band_never_narrows_as_the_rally_grows() {
        let fog = fog();
        let widths: Vec<f32> = (0..30).map(|returns| fog.width(&after(returns))).collect();
        for pair in widths.windows(2) {
            assert!(pair[1] >= pair[0], "the fog narrowed: {pair:?}");
        }
        assert_near(widths[1], 4.0, 1e-4);
        assert_near(widths[4], 16.0, 1e-4);
        assert_near(widths[8], 32.0, 1e-4);
        assert_near(widths[16], 64.0, 1e-4);
        assert_near(widths[29], 64.0, 1e-4);
    }

    /// The band is the rally's, not the hitter's: as wide off either paddle,
    /// however it moved.
    #[test]
    fn the_band_is_as_wide_whichever_paddle_returned_the_ball_and_however_it_moved() {
        let fog = fog();
        for side in [Side::Left, Side::Right] {
            for paddle_motion in [StepMotion::Up, StepMotion::Still, StepMotion::Down] {
                let returned = FlightStart {
                    returns: 4,
                    hit: Some(Hit {
                        side,
                        paddle_motion,
                    }),
                    returns_held: false,
                };
                assert_near(fog.width(&returned), 16.0, 1e-4);
            }
        }
    }

    /// M2 spec 2.2.5 and R4: view-only. At every dial Fog adds no speed,
    /// leaves the paddles whole, and pulls nothing.
    #[test]
    fn it_contributes_nothing_on_any_channel() {
        for returns in [0, 4, 8, 40] {
            let contribution = fog().contribute(&after(returns));
            assert_near(contribution.speed_gain, 0.0, 1e-6);
            assert_near(contribution.paddle_height_factor, 1.0, 1e-6);
            assert!(contribution.field.is_none(), "after {returns} returns");
        }
    }

    /// M2 spec 2.6.3 and 2.3.12, first form: the error added to a star's
    /// aim scales with the width, from nothing at the serve to the star's
    /// tuned error at full width. Nothing is added to its delay, and it
    /// believes the flight as it is.
    #[test]
    fn each_stars_added_error_scales_with_the_width_and_it_is_charged_nothing_else() {
        let tuning = tuning();
        let fog = fog();
        for difficulty in STARS {
            let full = tuning.effects().fog().added_error(difficulty);
            for (returns, share) in [(0, 0.0), (4, 0.25), (8, 0.5), (16, 1.0), (40, 1.0)] {
                let flaw = fog.flaw(&after(returns), difficulty);
                assert_near(flaw.added_error, share * full, 1e-4);
                assert_eq!(flaw.added_delay, Duration::ZERO);
                assert_near(flaw.believed.speed_gain, 0.0, 1e-6);
                assert_near(flaw.believed.paddle_height_factor, 1.0, 1e-6);
                assert!(flaw.believed.field.is_none());
            }
        }
    }

    /// More stars, a steadier aim in the fog: the embedded errors fall as
    /// the stars rise, and none is nothing.
    #[test]
    fn the_embedded_errors_fall_as_the_stars_rise_and_none_is_nothing() {
        let fog = tuning().effects().fog();
        let errors = STARS.map(|difficulty| fog.added_error(difficulty));
        assert!(errors[0] > errors[1] && errors[1] > errors[2], "{errors:?}");
        assert!(errors[2] > 0.0, "{errors:?}");
    }

    /// Where the ball is and how it moves at every frame of half a minute of
    /// Chaos with exactly `effects` in play, both seats chasing the ball's
    /// height: rallies long enough to turn every dial up.
    fn half_a_minute_with(effects: &[EffectId]) -> Vec<(Vec2, Vec2)> {
        let mut app = headless_app(7);
        boot(&mut app);
        app.world_mut()
            .resource_mut::<Sandbox>()
            .force_effects(effects);
        enter_match(
            &mut app,
            MatchConfig {
                left: Controller::Human(Seat::Left),
                right: Controller::Human(Seat::Right),
                players: SeatPlayers::GUESTS,
                points_to_win: NonZeroU8::new(21).expect("twenty-one is non-zero"),
                mode: Mode::Chaos {
                    cap: NonZeroU8::MIN,
                },
            },
        );
        (0..1800)
            .map(|_| {
                let (ball, _) = ball_state(&mut app);
                let paddles: Vec<Paddle> = app
                    .world_mut()
                    .query::<&Paddle>()
                    .iter(app.world())
                    .copied()
                    .collect();
                for paddle in paddles {
                    let seat = match paddle.side() {
                        Side::Left => Seat::Left,
                        Side::Right => Seat::Right,
                    };
                    let intent = if ball.y > paddle.center_y() + 1.0 {
                        PaddleIntent::Up
                    } else if ball.y < paddle.center_y() - 1.0 {
                        PaddleIntent::Down
                    } else {
                        PaddleIntent::Still
                    };
                    report_intent(&mut app, seat, intent);
                }
                frame(&mut app);
                ball_state(&mut app)
            })
            .collect()
    }

    /// M2 spec 2.2.5 and R2: Fog has no simulation contribution. From the
    /// same seed, under the same hands, the ball is in the same place at
    /// every frame with Fog in play as without it, so in fog skill decides
    /// what it decides in clear air.
    #[test]
    fn the_ball_flies_the_same_frame_for_frame_with_fog_in_play_and_without() {
        let clear = half_a_minute_with(&[EffectId::Speed]);
        let foggy = half_a_minute_with(&[EffectId::Speed, EffectId::Fog]);
        assert_eq!(clear, foggy);
        let turned = clear
            .windows(2)
            .filter(|pair| pair[0].1.x * pair[1].1.x < 0.0)
            .count();
        assert!(turned >= 8, "only {turned} returns: the fog never grew");
    }
}

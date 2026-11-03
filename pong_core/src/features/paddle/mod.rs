//! Paddle (spec 1.4.3): a component that owns its size and position invariants,
//! spawned per side for a match, moved by its side's intent in the integrate phase.

use bevy::app::{App, FixedUpdate, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::math::{Rect, Vec2};
use bevy::state::state::OnEnter;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::time::{Fixed, Time};

use crate::app::phases::FixedStepPhase;
use crate::app::states::AppState;
use crate::features::seats::{PaddleIntent, PaddleIntents};
use crate::features::tuning::Tuning;
use crate::kernel::geometry::{Arena, LengthBounds, Side};

/// Spawns one paddle per side for the length of a match and moves each by its
/// side's intent in the integrate phase.
pub struct PaddlePlugin;

impl Plugin for PaddlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InMatch), spawn_paddles)
            .add_systems(FixedUpdate, move_paddles.in_set(FixedStepPhase::Integrate));
    }
}

fn spawn_paddles(mut commands: Commands, tuning: Res<Tuning>) {
    let paddle = tuning.paddle();
    for side in [Side::Left, Side::Right] {
        commands.spawn((
            Paddle::new(
                side,
                paddle.height_default(),
                paddle.thickness(),
                paddle.inset(),
            ),
            DespawnOnExit(AppState::InMatch),
        ));
    }
}

fn move_paddles(
    intents: Res<PaddleIntents>,
    tuning: Res<Tuning>,
    time: Res<Time<Fixed>>,
    mut paddles: Query<&mut Paddle>,
) {
    let travel = tuning.paddle().speed() * time.delta_secs();
    for mut paddle in &mut paddles {
        let dy = match intents.side(paddle.side()) {
            PaddleIntent::Up => travel,
            PaddleIntent::Down => -travel,
            PaddleIntent::Still => 0.0,
        };
        paddle.move_by(dy, tuning.arena());
    }
}

/// One side's paddle: an axis-aligned rectangle that slides along y near its
/// goal line.
///
/// Invariants, kept by the setters: height within the tuning bounds given to
/// [`set_height`](Self::set_height); the whole paddle inside the walls after
/// every [`move_by`](Self::move_by). The x position is fixed by the side, the
/// inset, and the thickness.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Paddle {
    side: Side,
    center_y: f32,
    height: f32,
    thickness: f32,
    inset: f32,
}

impl Paddle {
    /// A paddle on `side`, centered vertically, its back face `inset` units in
    /// from the goal line. Values come from tuning, already validated.
    #[must_use]
    pub const fn new(side: Side, height: f32, thickness: f32, inset: f32) -> Self {
        Self {
            side,
            center_y: 0.0,
            height,
            thickness,
            inset,
        }
    }

    /// Which goal line this paddle guards.
    #[must_use]
    pub const fn side(self) -> Side {
        self.side
    }

    /// Vertical center.
    #[must_use]
    pub const fn center_y(self) -> f32 {
        self.center_y
    }

    /// Full height.
    #[must_use]
    pub const fn height(self) -> f32 {
        self.height
    }

    /// Sets the height, pulled into `bounds`. The center stays put; call
    /// [`move_by`](Self::move_by) with zero afterwards to re-clamp against the walls.
    pub fn set_height(&mut self, height: f32, bounds: LengthBounds) {
        self.height = bounds.clamp(height);
    }

    /// Slides the paddle by `dy`, then keeps the whole paddle inside the walls.
    pub fn move_by(&mut self, dy: f32, arena: Arena) {
        let limit = arena.half_extents().y - self.height / 2.0;
        self.center_y = (self.center_y + dy).clamp(-limit, limit);
    }

    /// The rectangle the ball collides with, in arena units.
    #[must_use]
    pub fn rect(self, arena: Arena) -> Rect {
        let center_x =
            arena.goal_x(self.side) - self.side.sign() * (self.inset + self.thickness / 2.0);
        Rect::from_center_half_size(
            Vec2::new(center_x, self.center_y),
            Vec2::new(self.thickness / 2.0, self.height / 2.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;

    use bevy::app::App;
    use bevy::state::state::NextState;

    use super::Paddle;
    use crate::app::states::AppState;
    use crate::domain::{Controller, MatchConfig, Seat};
    use crate::features::seats::{PaddleIntent, SeatIntents};
    use crate::features::tuning::Tuning;
    use crate::kernel::geometry::{LengthBounds, Side};
    use crate::kernel::test_support::{assert_near, standard_arena};
    use crate::testing::{boot, headless_app, start_match, step};

    fn two_humans() -> MatchConfig {
        MatchConfig {
            left: Controller::Human(Seat::Left),
            right: Controller::Human(Seat::Right),
            points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
        }
    }

    fn paddles(app: &mut App) -> Vec<Paddle> {
        let mut paddles: Vec<Paddle> = app
            .world_mut()
            .query::<&Paddle>()
            .iter(app.world())
            .copied()
            .collect();
        paddles.sort_by_key(|p| p.side() == Side::Right);
        paddles
    }

    #[test]
    fn entering_a_match_spawns_one_centered_paddle_per_side_at_the_tuned_height() {
        let mut app = headless_app(1);
        boot(&mut app);
        start_match(&mut app, two_humans());
        let default_height = app.world().resource::<Tuning>().paddle().height_default();
        let paddles = paddles(&mut app);
        assert_eq!(paddles.len(), 2);
        assert_eq!(paddles[0].side(), Side::Left);
        assert_eq!(paddles[1].side(), Side::Right);
        for paddle in &paddles {
            assert_near(paddle.center_y(), 0.0, 1e-6);
            assert_near(paddle.height(), default_height, 1e-6);
        }
    }

    #[test]
    fn a_paddle_moves_at_the_tuned_speed_in_its_seats_direction_each_step() {
        let mut app = headless_app(1);
        boot(&mut app);
        start_match(&mut app, two_humans());
        let speed = app.world().resource::<Tuning>().paddle().speed();
        app.world_mut()
            .resource_mut::<SeatIntents>()
            .report(Seat::Left, PaddleIntent::Up);
        step(&mut app);
        let paddles = paddles(&mut app);
        assert_near(paddles[0].center_y(), speed / 60.0, 1e-4);
        assert_near(paddles[1].center_y(), 0.0, 1e-6);
    }

    #[test]
    fn leaving_the_match_despawns_the_paddles() {
        let mut app = headless_app(1);
        boot(&mut app);
        start_match(&mut app, two_humans());
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Menu);
        step(&mut app);
        assert_eq!(paddles(&mut app), Vec::<Paddle>::new());
    }

    fn bounds() -> LengthBounds {
        LengthBounds::new(8.0, 30.0).expect("8 to 30 are valid bounds")
    }

    /// Right paddle, 2 thick, 14 tall, back face 4 units in from the goal line.
    fn paddle() -> Paddle {
        Paddle::new(Side::Right, 14.0, 2.0, 4.0)
    }

    #[test]
    fn a_new_paddle_is_centered_on_its_side_inset_from_the_goal_line() {
        let rect = paddle().rect(standard_arena());
        // Goal line at x = 80; back face at 76; two thick, so the center is at 75.
        assert_near(rect.center().x, 75.0, 1e-6);
        assert_near(rect.center().y, 0.0, 1e-6);
        assert_near(rect.half_size().x, 1.0, 1e-6);
        assert_near(rect.half_size().y, 7.0, 1e-6);
        assert_eq!(paddle().side(), Side::Right);
        let left = Paddle::new(Side::Left, 14.0, 2.0, 4.0).rect(standard_arena());
        assert_near(left.center().x, -75.0, 1e-6);
    }

    #[test]
    fn height_setter_clamps_to_both_ends_of_the_bounds() {
        let mut paddle = paddle();
        paddle.set_height(100.0, bounds());
        assert_near(paddle.height(), 30.0, 1e-6);
        paddle.set_height(1.0, bounds());
        assert_near(paddle.height(), 8.0, 1e-6);
        paddle.set_height(20.0, bounds());
        assert_near(paddle.height(), 20.0, 1e-6);
    }

    #[test]
    fn moving_clamps_the_paddle_inside_both_walls() {
        let arena = standard_arena();
        let mut paddle = paddle();
        paddle.move_by(1_000.0, arena);
        // Half-height 7 against a wall at 45.
        assert_near(paddle.center_y(), 38.0, 1e-6);
        paddle.move_by(-1_000.0, arena);
        assert_near(paddle.center_y(), -38.0, 1e-6);
        paddle.move_by(10.0, arena);
        assert_near(paddle.center_y(), -28.0, 1e-6);
    }

    #[test]
    fn growing_against_a_wall_pushes_the_paddle_back_inside() {
        let arena = standard_arena();
        let mut paddle = paddle();
        paddle.move_by(1_000.0, arena);
        paddle.set_height(30.0, bounds());
        paddle.move_by(0.0, arena);
        assert_near(paddle.center_y(), 30.0, 1e-6);
        assert!(paddle.rect(arena).max.y <= 45.0 + 1e-6);
    }
}

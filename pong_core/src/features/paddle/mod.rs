//! Paddle (spec 1.4.3): a component that owns its size and position invariants,
//! spawned per side for a match, moved by its side's intent in the integrate phase.

use bevy::app::{App, FixedUpdate, Plugin};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::math::{Rect, Vec2};
use bevy::state::state::OnEnter;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::time::{Fixed, Time};

use crate::features::seats::{PaddleIntent, PaddleIntents};
use crate::features::tuning::{PaddleTuning, Tuning};
use crate::flow::phases::FixedStepPhase;
use crate::flow::states::AppState;
use crate::kernel::flight::StepMotion;
use crate::kernel::geometry::{Arena, LengthBounds, Side};

/// Spawns one paddle per side for the length of a match and moves each by its
/// side's intent in the integrate phase, first refitting the paddles to a
/// reloaded tuning file.
pub struct PaddlePlugin;

impl Plugin for PaddlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InMatch), spawn_paddles)
            .add_systems(
                FixedUpdate,
                (fit_paddles_to_changed_tuning, move_paddles)
                    .chain()
                    .in_set(FixedStepPhase::Integrate),
            );
    }
}

fn spawn_paddles(mut commands: Commands, tuning: Res<Tuning>) {
    for side in [Side::Left, Side::Right] {
        commands.spawn((
            Paddle::new(side, tuning.paddle()),
            DespawnOnExit(AppState::InMatch),
        ));
    }
}

/// A reloaded tuning file (dev override) may narrow the height bounds or the
/// arena under paddles already on the field. Pulling each height back into
/// the new bounds restores both invariants, since the new file guarantees
/// `height_max ≤ arena.height`; it runs before the paddles next move.
fn fit_paddles_to_changed_tuning(tuning: Res<Tuning>, mut paddles: Query<&mut Paddle>) {
    if !tuning.is_changed() {
        return;
    }
    for mut paddle in &mut paddles {
        let height = paddle.height();
        paddle.set_height(height, tuning.paddle().height_bounds(), tuning.arena());
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
/// The component holds only what changes during a match: where the paddle is,
/// how tall it is, and what it did in the current step. Thickness and inset
/// are tuning constants read at use, so a reloaded tuning file applies to
/// paddles already on the field.
///
/// Invariants, kept by the constructor and the setters: height within the
/// tuning bounds; the whole paddle inside the walls; the step motion is the
/// displacement the last [`move_by`](Self::move_by) actually made. Both hold because tuning
/// guarantees `height_max ≤ arena.height` (plan 3.17), and across a reload
/// because the plugin refits every paddle to the new file before it moves.
/// Precondition of [`move_by`](Self::move_by): `height ≤ arena.height`;
/// otherwise its wall clamp has no room and panics.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Paddle {
    side: Side,
    center_y: f32,
    height: f32,
    step_motion: StepMotion,
}

impl Paddle {
    /// A paddle on `side`, centered vertically, at the tuned default height,
    /// which validation keeps within the height bounds.
    #[must_use]
    pub const fn new(side: Side, tuning: PaddleTuning) -> Self {
        Self {
            side,
            center_y: 0.0,
            height: tuning.height_default(),
            step_motion: StepMotion::Still,
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

    /// What the paddle did in the current fixed step: the displacement the
    /// last [`move_by`](Self::move_by) made after the wall clamp, so a paddle
    /// pressed against a wall is still (M2 spec 2.0.3). The paddle moves in
    /// the integrate phase; read this in the collide phase or later.
    #[must_use]
    pub const fn step_motion(self) -> StepMotion {
        self.step_motion
    }

    /// Sets the height, pulled into `bounds`, then moves the paddle back
    /// inside the walls if growing pushed it through one. The center stays
    /// put otherwise. A resize is not the player's motion: the step motion
    /// is left as it was.
    pub fn set_height(&mut self, height: f32, bounds: LengthBounds, arena: Arena) {
        self.height = bounds.clamp(height);
        self.center_y = self.inside_the_walls(self.center_y, arena);
    }

    /// Slides the paddle by `dy`, then keeps the whole paddle inside the
    /// walls, and records the displacement made as this step's motion.
    pub fn move_by(&mut self, dy: f32, arena: Arena) {
        let before = self.center_y;
        self.center_y = self.inside_the_walls(before + dy, arena);
        self.step_motion = StepMotion::of(self.center_y - before);
    }

    /// `center_y` pulled back so the whole paddle is inside the walls.
    fn inside_the_walls(self, center_y: f32, arena: Arena) -> f32 {
        let limit = arena.half_extents().y - self.height / 2.0;
        center_y.clamp(-limit, limit)
    }

    /// The rectangle the ball collides with, in arena units: `tuning`'s
    /// thickness, its back face `inset` units in from this side's goal line.
    #[must_use]
    pub fn rect(self, tuning: &Tuning) -> Rect {
        let shape = tuning.paddle();
        let center_x = tuning.arena().goal_x(self.side)
            - self.side.sign() * (shape.inset() + shape.thickness() / 2.0);
        Rect::from_center_half_size(
            Vec2::new(center_x, self.center_y),
            Vec2::new(shape.thickness() / 2.0, self.height / 2.0),
        )
    }

    /// The x at which an incoming ball's centre touches this paddle's front
    /// face, the face toward the centre line: one ball radius in front of it.
    /// The AI aims its prediction at this line.
    #[must_use]
    pub fn contact_x(self, tuning: &Tuning) -> f32 {
        self.side.sign() * tuning.crossing_width() / 2.0
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;

    use bevy::app::App;
    use bevy::state::state::NextState;

    use super::Paddle;
    use crate::domain::{Controller, MatchConfig, Mode, Seat, SeatPlayers};
    use crate::features::seats::PaddleIntent;
    use crate::features::tuning::{EMBEDDED_RON, Tuning};
    use crate::flow::states::{AppState, MatchState};
    use crate::kernel::flight::StepMotion;
    use crate::kernel::geometry::{LengthBounds, Side};
    use crate::kernel::test_support::{assert_near, standard_arena};
    use crate::testing::{
        FrameLength, ball_state, boot, fixed_timestep, frame, headless_app, match_state,
        report_intent, set_frame_length, start_match,
    };

    /// How far one fixed step of `Up` moves a paddle.
    fn one_step_of_travel(app: &App) -> f32 {
        app.world().resource::<Tuning>().paddle().speed() * fixed_timestep(app).as_secs_f32()
    }

    fn two_humans() -> MatchConfig {
        MatchConfig {
            left: Controller::Human(Seat::Left),
            right: Controller::Human(Seat::Right),
            players: SeatPlayers::GUESTS,
            points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
            mode: Mode::Classic,
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
        report_intent(&mut app, Seat::Left, PaddleIntent::Up);
        frame(&mut app);
        let travel = one_step_of_travel(&app);
        let paddles = paddles(&mut app);
        assert_near(paddles[0].center_y(), travel, 1e-4);
        assert_near(paddles[1].center_y(), 0.0, 1e-6);
    }

    /// A match in `state`, entered from play, with one frame run in it.
    fn match_in(state: MatchState) -> App {
        let mut app = headless_app(1);
        boot(&mut app);
        start_match(&mut app, two_humans());
        app.world_mut()
            .resource_mut::<NextState<MatchState>>()
            .set(state);
        frame(&mut app);
        assert_eq!(match_state(&app), state);
        app
    }

    /// ADR 0004: players line up during the countdown and after a point,
    /// while the ball waits.
    #[test]
    fn between_rallies_a_paddle_moves_while_the_ball_waits() {
        for state in [MatchState::Countdown, MatchState::PointScored] {
            let mut app = match_in(state);
            let before = paddles(&mut app)[0].center_y();
            let ball = ball_state(&mut app).0;
            report_intent(&mut app, Seat::Left, PaddleIntent::Up);
            frame(&mut app);
            let travel = one_step_of_travel(&app);
            assert_near(paddles(&mut app)[0].center_y() - before, travel, 1e-4);
            assert_eq!(ball_state(&mut app).0, ball, "{state:?}");
        }
    }

    #[test]
    fn a_paddle_stays_still_while_paused_or_after_the_match() {
        for state in [MatchState::Paused, MatchState::MatchOver] {
            let mut app = match_in(state);
            let before = paddles(&mut app)[0].center_y();
            report_intent(&mut app, Seat::Left, PaddleIntent::Up);
            frame(&mut app);
            assert_near(paddles(&mut app)[0].center_y(), before, 1e-6);
        }
    }

    /// Spec 1.4.1 as amended: a hold moves on every fixed step, however many
    /// a frame runs.
    #[test]
    fn a_hold_moves_on_every_fixed_step_of_a_two_step_frame() {
        let mut app = headless_app(1);
        boot(&mut app);
        start_match(&mut app, two_humans());
        set_frame_length(&mut app, FrameLength::Steps(2));
        report_intent(&mut app, Seat::Left, PaddleIntent::Up);
        frame(&mut app);
        let travel = one_step_of_travel(&app);
        assert_near(paddles(&mut app)[0].center_y(), 2.0 * travel, 1e-4);
    }

    /// Spec 1.4.1 as amended: on frames shorter than a fixed step, a press
    /// reported for one frame and released the next moves exactly once,
    /// whether or not its own frame ran a step.
    #[test]
    fn a_press_shorter_than_a_frame_moves_exactly_once_and_is_never_missed() {
        for phase_shift in [false, true] {
            let mut app = headless_app(1);
            boot(&mut app);
            start_match(&mut app, two_humans());
            set_frame_length(&mut app, FrameLength::HalfStep);
            if phase_shift {
                frame(&mut app);
            }
            report_intent(&mut app, Seat::Left, PaddleIntent::Up);
            frame(&mut app);
            report_intent(&mut app, Seat::Left, PaddleIntent::Still);
            for _ in 0..6 {
                frame(&mut app);
            }
            let travel = one_step_of_travel(&app);
            assert_near(paddles(&mut app)[0].center_y(), travel, 1e-4);
        }
    }

    #[test]
    fn leaving_the_match_despawns_the_paddles() {
        let mut app = headless_app(1);
        boot(&mut app);
        start_match(&mut app, two_humans());
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Menu);
        frame(&mut app);
        assert_eq!(paddles(&mut app), Vec::<Paddle>::new());
    }

    /// The embedded tuning: a 160 by 90 arena; paddles 2 thick, 14 tall by
    /// default within 8 to 30, back face 4 units in from the goal line.
    fn tuning() -> Tuning {
        Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid")
    }

    fn bounds() -> LengthBounds {
        tuning().paddle().height_bounds()
    }

    fn paddle() -> Paddle {
        Paddle::new(Side::Right, tuning().paddle())
    }

    #[test]
    fn a_new_paddle_is_centered_on_its_side_inset_from_the_goal_line() {
        let rect = paddle().rect(&tuning());
        // Goal line at x = 80; back face at 76; two thick, so the center is at 75.
        assert_near(rect.center().x, 75.0, 1e-6);
        assert_near(rect.center().y, 0.0, 1e-6);
        assert_near(rect.half_size().x, 1.0, 1e-6);
        assert_near(rect.half_size().y, 7.0, 1e-6);
        assert_eq!(paddle().side(), Side::Right);
        let left = Paddle::new(Side::Left, tuning().paddle()).rect(&tuning());
        assert_near(left.center().x, -75.0, 1e-6);
    }

    #[test]
    fn a_ball_touches_the_face_where_its_centre_is_one_radius_in_front_of_it() {
        // Right face at x = 74, left face at -74; the ball's radius is 1.
        assert_near(paddle().contact_x(&tuning()), 73.0, 1e-6);
        let left = Paddle::new(Side::Left, tuning().paddle());
        assert_near(left.contact_x(&tuning()), -73.0, 1e-6);
    }

    #[test]
    fn height_setter_clamps_to_both_ends_of_the_bounds() {
        let arena = standard_arena();
        let mut paddle = paddle();
        paddle.set_height(100.0, bounds(), arena);
        assert_near(paddle.height(), 30.0, 1e-6);
        paddle.set_height(1.0, bounds(), arena);
        assert_near(paddle.height(), 8.0, 1e-6);
        paddle.set_height(20.0, bounds(), arena);
        assert_near(paddle.height(), 20.0, 1e-6);
    }

    /// A reload may shrink the arena below the height of a paddle already on
    /// the field. The paddle fits itself to the new tuning before it next
    /// moves, so its wall clamp never sees a negative limit.
    #[test]
    fn a_reload_that_shrinks_the_arena_refits_paddles_before_they_next_move() {
        let mut app = headless_app(1);
        boot(&mut app);
        start_match(&mut app, two_humans());
        let cramped = EMBEDDED_RON
            .replace("height: 90.0", "height: 12.0")
            .replace("height_max: 30.0", "height_max: 10.0")
            .replace("height_default: 14.0", "height_default: 9.0")
            // Wind's drift and Spin's must each stay under half the playable
            // height, now 5.
            .replace("max_drift: 24.0", "max_drift: 4.0")
            .replace("max_drift: 16.0", "max_drift: 4.0");
        app.insert_resource(Tuning::from_ron(&cramped).expect("a cramped arena is valid"));
        report_intent(&mut app, Seat::Left, PaddleIntent::Up);
        frame(&mut app);
        let tuning = app.world().resource::<Tuning>().clone();
        for paddle in paddles(&mut app) {
            assert_near(paddle.height(), 10.0, 1e-6);
            let rect = paddle.rect(&tuning);
            assert!(
                rect.min.y >= -6.0 - 1e-6 && rect.max.y <= 6.0 + 1e-6,
                "{rect:?}"
            );
        }
    }

    /// Plan 3.16: thickness and inset come from tuning at use, so a reload
    /// reaches paddles already on the field. Adopting a reload replaces the
    /// `Tuning` resource, as here.
    #[test]
    fn a_reloaded_thickness_reshapes_a_paddle_already_on_the_field_within_one_frame() {
        let mut app = headless_app(1);
        boot(&mut app);
        start_match(&mut app, two_humans());
        let thicker = EMBEDDED_RON.replace("thickness: 2.0", "thickness: 3.0");
        app.insert_resource(Tuning::from_ron(&thicker).expect("thicker paddles are valid"));
        frame(&mut app);
        let tuning = app.world().resource::<Tuning>().clone();
        let rect = paddles(&mut app)[1].rect(&tuning);
        assert_near(rect.half_size().x, 1.5, 1e-6);
        assert_near(rect.max.x, 76.0, 1e-6);
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
    fn growing_against_a_wall_keeps_the_paddle_inside_by_itself() {
        let arena = standard_arena();
        let mut paddle = paddle();
        paddle.move_by(1_000.0, arena);
        paddle.set_height(30.0, bounds(), arena);
        assert_near(paddle.center_y(), 30.0, 1e-6);
        assert!(paddle.rect(&tuning()).max.y <= 45.0 + 1e-6);
    }

    /// M2 spec 2.0.3: the motion is what the paddle did, not what was asked.
    #[test]
    fn the_step_motion_is_the_displacement_made_so_a_paddle_at_a_wall_is_still() {
        let arena = standard_arena();
        let mut paddle = paddle();
        assert_eq!(paddle.step_motion(), StepMotion::Still);
        paddle.move_by(1.5, arena);
        assert_eq!(paddle.step_motion(), StepMotion::Up);
        paddle.move_by(-1.5, arena);
        assert_eq!(paddle.step_motion(), StepMotion::Down);
        paddle.move_by(0.0, arena);
        assert_eq!(paddle.step_motion(), StepMotion::Still);
        paddle.move_by(1_000.0, arena);
        assert_eq!(
            paddle.step_motion(),
            StepMotion::Up,
            "the step that reaches the wall moves"
        );
        paddle.move_by(1.5, arena);
        assert_eq!(
            paddle.step_motion(),
            StepMotion::Still,
            "pressed against the wall"
        );
    }

    #[test]
    fn a_resize_leaves_the_step_motion_as_it_was() {
        let arena = standard_arena();
        let mut paddle = paddle();
        paddle.move_by(1_000.0, arena);
        paddle.set_height(30.0, bounds(), arena);
        assert_eq!(paddle.step_motion(), StepMotion::Up);
    }

    /// M2 spec 2.0.3's done-condition: up, still, and down read from a paddle
    /// driven by each intent.
    #[test]
    fn a_paddle_driven_by_each_intent_reports_that_steps_motion() {
        let mut app = headless_app(1);
        boot(&mut app);
        start_match(&mut app, two_humans());
        for (intent, expected) in [
            (PaddleIntent::Up, StepMotion::Up),
            (PaddleIntent::Still, StepMotion::Still),
            (PaddleIntent::Down, StepMotion::Down),
        ] {
            report_intent(&mut app, Seat::Left, intent);
            frame(&mut app);
            assert_eq!(paddles(&mut app)[0].step_motion(), expected, "{intent:?}");
            assert_eq!(paddles(&mut app)[1].step_motion(), StepMotion::Still);
        }
    }
}

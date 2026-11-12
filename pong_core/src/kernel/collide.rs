//! Swept circle-versus-rectangle collision (spec 1.2.3).

use bevy::math::{Rect, Vec2};

use super::geometry::Radius;

/// A face of an axis-aligned rectangle, named from the rectangle's point of view.
///
/// A ball flying `+x` into a paddle strikes the paddle's `Left` face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    /// The `-x` face.
    Left,
    /// The `+x` face.
    Right,
    /// The `+y` face.
    Top,
    /// The `-y` face.
    Bottom,
}

impl Face {
    /// Unit normal pointing out of the rectangle through this face.
    #[must_use]
    pub const fn normal(self) -> Vec2 {
        match self {
            Self::Left => Vec2::NEG_X,
            Self::Right => Vec2::X,
            Self::Top => Vec2::Y,
            Self::Bottom => Vec2::NEG_Y,
        }
    }
}

/// Where and how a sweep first touches a rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Impact {
    /// Fraction of the displacement travelled at contact, in `0.0..=1.0`.
    /// Zero when the circle already overlaps the rectangle at the start.
    pub toi: f32,
    /// The face struck.
    pub face: Face,
}

/// Sweeps a circle of `radius` at `center` along `displacement` and reports the
/// first contact with the axis-aligned `rect`, or `None` when the sweep misses.
///
/// The sweep is a ray against `rect` grown by `radius`, so a corner is treated
/// as square rather than rounded: a ball clipping a corner is reported a hair
/// early. Pong paddles are struck on their faces, and this keeps every contact
/// on one of the four [`Face`]s.
///
/// A circle that starts overlapping gets `toi` zero and the normal of the face
/// it crossed most recently, so a paddle that moved onto the ball still deflects it.
///
/// A circle that starts touching the rectangle and moves away from it misses:
/// it is leaving. A contact that ends a step exactly on a face starts the
/// next sweep there, and that sweep is not a second contact.
#[must_use]
pub fn swept_circle_rect(
    center: Vec2,
    radius: Radius,
    displacement: Vec2,
    rect: Rect,
) -> Option<Impact> {
    let min = rect.min - Vec2::splat(radius.value());
    let max = rect.max + Vec2::splat(radius.value());

    let x = slab(center.x, displacement.x, min.x, max.x)?;
    let y = slab(center.y, displacement.y, min.y, max.y)?;

    // The later entry is the face actually struck; a face is entered against its normal.
    let (t_enter, face) = if x.enter >= y.enter {
        let face = if displacement.x > 0.0 {
            Face::Left
        } else {
            Face::Right
        };
        (x.enter, face)
    } else {
        let face = if displacement.y > 0.0 {
            Face::Bottom
        } else {
            Face::Top
        };
        (y.enter, face)
    };
    let t_exit = x.exit.min(y.exit);

    // Entering after leaving means the ray passes beside the box. Leaving at
    // or before the start means the box is behind the sweep, or touched only
    // where the sweep begins; `<=` also refuses the `-0.0` a start on the
    // edge divides out to. A non-finite entry means no axis is moving: a
    // stationary overlap is not an impact.
    if t_enter > t_exit || t_exit <= 0.0 || t_enter > 1.0 || !t_enter.is_finite() {
        return None;
    }
    Some(Impact {
        toi: t_enter.max(0.0),
        face,
    })
}

/// Parametric interval during which a 1-D ray `origin + t * delta` is within `lo..=hi`.
#[derive(Debug, Clone, Copy)]
struct Interval {
    enter: f32,
    exit: f32,
}

/// `None` when the ray never enters the slab; the unbounded interval when it does
/// not move along this axis but already lies inside.
fn slab(origin: f32, delta: f32, lo: f32, hi: f32) -> Option<Interval> {
    if delta == 0.0 {
        return (lo..=hi).contains(&origin).then_some(Interval {
            enter: f32::NEG_INFINITY,
            exit: f32::INFINITY,
        });
    }
    let t_lo = (lo - origin) / delta;
    let t_hi = (hi - origin) / delta;
    Some(Interval {
        enter: t_lo.min(t_hi),
        exit: t_lo.max(t_hi),
    })
}

#[cfg(test)]
mod tests {
    use bevy::math::{Rect, Vec2};

    use super::{Face, swept_circle_rect};
    use crate::kernel::test_support::{assert_near, assert_vec_near, unit_radius};

    /// A right-side paddle two units thick, twelve tall, face at x = 69.
    fn paddle() -> Rect {
        Rect::from_center_half_size(Vec2::new(70.0, 0.0), Vec2::new(1.0, 6.0))
    }

    #[test]
    fn ball_moving_ten_times_paddle_thickness_per_step_still_hits_the_face() {
        let Some(impact) = swept_circle_rect(
            Vec2::new(60.0, 0.0),
            unit_radius(),
            Vec2::new(20.0, 0.0),
            paddle(),
        ) else {
            panic!("fast ball tunneled through the paddle");
        };
        // Expanded face at x = 68; the ball starts at 60 and travels 20.
        assert_near(impact.toi, 0.4, 1e-6);
        assert_eq!(impact.face, Face::Left);
    }

    #[test]
    fn each_face_has_the_outward_unit_normal() {
        assert_vec_near(Face::Left.normal(), Vec2::NEG_X, 1e-6);
        assert_vec_near(Face::Right.normal(), Vec2::X, 1e-6);
        assert_vec_near(Face::Top.normal(), Vec2::Y, 1e-6);
        assert_vec_near(Face::Bottom.normal(), Vec2::NEG_Y, 1e-6);
    }

    #[test]
    fn ball_passing_beside_the_paddle_misses() {
        // Expanded paddle reaches y = 7; the ball flies level at y = 8.
        assert!(
            swept_circle_rect(
                Vec2::new(60.0, 8.0),
                unit_radius(),
                Vec2::new(20.0, 0.0),
                paddle()
            )
            .is_none()
        );
    }

    #[test]
    fn ball_stopping_short_of_the_paddle_misses() {
        assert!(
            swept_circle_rect(
                Vec2::new(60.0, 0.0),
                unit_radius(),
                Vec2::new(5.0, 0.0),
                paddle()
            )
            .is_none()
        );
    }

    #[test]
    fn ball_moving_away_from_the_paddle_misses() {
        assert!(
            swept_circle_rect(
                Vec2::new(60.0, 0.0),
                unit_radius(),
                Vec2::new(-20.0, 0.0),
                paddle()
            )
            .is_none()
        );
    }

    /// A ball that touches the paddle and moves away is leaving, not
    /// arriving. A return whose step ends exactly on the face starts the next
    /// step's sweep here, and that sweep must not be a second return.
    #[test]
    fn ball_touching_the_face_and_moving_away_misses() {
        // Expanded face at x = 68: the ball's centre starts on it.
        assert!(
            swept_circle_rect(
                Vec2::new(68.0, 0.0),
                unit_radius(),
                Vec2::new(-1.0, 0.0),
                paddle()
            )
            .is_none()
        );
    }

    /// The other half of the boundary: touching and moving in is a hit now.
    #[test]
    fn ball_touching_the_face_and_moving_in_hits_at_once() {
        let Some(impact) = swept_circle_rect(
            Vec2::new(68.0, 0.0),
            unit_radius(),
            Vec2::new(1.0, 0.0),
            paddle(),
        ) else {
            panic!("a ball on the face moving in reported no impact");
        };
        assert_near(impact.toi, 0.0, 1e-6);
        assert_eq!(impact.face, Face::Left);
    }

    #[test]
    fn ball_dropping_onto_the_top_edge_gets_the_upward_normal() {
        let Some(impact) = swept_circle_rect(
            Vec2::new(70.0, 20.0),
            unit_radius(),
            Vec2::new(0.0, -26.0),
            paddle(),
        ) else {
            panic!("ball passed through the paddle top");
        };
        // Expanded top at y = 7; the ball starts at 20 and travels 26 down.
        assert_near(impact.toi, 0.5, 1e-6);
        assert_eq!(impact.face, Face::Top);
    }

    #[test]
    fn ball_already_overlapping_reports_toi_zero_and_the_face_it_crossed() {
        let Some(impact) = swept_circle_rect(
            Vec2::new(68.5, 0.0),
            unit_radius(),
            Vec2::new(5.0, 0.0),
            paddle(),
        ) else {
            panic!("overlapping ball reported no impact");
        };
        assert_near(impact.toi, 0.0, 1e-6);
        assert_eq!(impact.face, Face::Left);
    }

    #[test]
    fn stationary_overlap_is_not_an_impact() {
        assert!(
            swept_circle_rect(Vec2::new(68.5, 0.0), unit_radius(), Vec2::ZERO, paddle()).is_none()
        );
    }
}

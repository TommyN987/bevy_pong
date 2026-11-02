//! Arena geometry in arena units (spec 1.2.1).

use bevy::math::Vec2;

use super::{OutOfRange, Range};

/// One end of the arena: which goal line, which paddle, which player's half.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// The `-x` end.
    Left,
    /// The `+x` end.
    Right,
}

impl Side {
    /// Direction along x toward this side's goal line: `-1.0` or `1.0`.
    #[must_use]
    pub const fn sign(self) -> f32 {
        match self {
            Self::Left => -1.0,
            Self::Right => 1.0,
        }
    }

    /// The other end.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

/// Radius of a round body in arena units. Invariant: positive and finite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radius(f32);

impl Radius {
    /// # Errors
    /// `radius` is not positive and finite.
    pub fn new(radius: f32) -> Result<Self, OutOfRange> {
        Range::PositiveFinite.check(radius).map(Self)
    }

    /// The radius in arena units.
    #[must_use]
    pub const fn value(self) -> f32 {
        self.0
    }
}

/// A closed interval of lengths a size may take, in arena units.
/// Invariant: both ends positive and finite, `min <= max`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LengthBounds {
    min: f32,
    max: f32,
}

/// Why [`LengthBounds::new`] refused its ends.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum InvalidLengthBounds {
    /// The lower end was not positive and finite.
    #[error("minimum {0}")]
    Min(OutOfRange),
    /// The upper end was not positive and finite.
    #[error("maximum {0}")]
    Max(OutOfRange),
    /// The ends were valid but reversed.
    #[error("maximum {max} is below minimum {min}")]
    MaxBelowMin {
        /// The lower end given.
        min: f32,
        /// The upper end given.
        max: f32,
    },
}

impl LengthBounds {
    /// # Errors
    /// An end that is not positive and finite, or a maximum below the minimum.
    pub fn new(min: f32, max: f32) -> Result<Self, InvalidLengthBounds> {
        let min = Range::PositiveFinite
            .check(min)
            .map_err(InvalidLengthBounds::Min)?;
        let max = Range::PositiveFinite
            .check(max)
            .map_err(InvalidLengthBounds::Max)?;
        if max < min {
            return Err(InvalidLengthBounds::MaxBelowMin { min, max });
        }
        Ok(Self { min, max })
    }

    /// The lower end.
    #[must_use]
    pub const fn min(self) -> f32 {
        self.min
    }

    /// The upper end.
    #[must_use]
    pub const fn max(self) -> f32 {
        self.max
    }

    /// Whether `length` lies within the bounds, ends included.
    #[must_use]
    pub fn contains(self, length: f32) -> bool {
        (self.min..=self.max).contains(&length)
    }

    /// `length` pulled to the nearest end if it lies outside.
    #[must_use]
    pub fn clamp(self, length: f32) -> f32 {
        length.clamp(self.min, self.max)
    }
}

/// The playfield, centered on the origin with y up.
///
/// Goal lines are the vertical edges at `±half_extents.x`; walls are the
/// horizontal edges at `±half_extents.y`. The boundary counts as inside: a point
/// is outside only once it has crossed past an edge.
///
/// Invariant: both dimensions are positive and finite, enforced by [`Arena::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arena {
    half_extents: Vec2,
}

/// Why [`Arena::new`] refused its dimensions.
///
/// The inner [`OutOfRange`] is part of the message, not a `source`, so a
/// report prints one sentence rather than the same text twice.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum InvalidArena {
    /// The width was not positive and finite.
    #[error("arena width {0}")]
    Width(OutOfRange),
    /// The height was not positive and finite.
    #[error("arena height {0}")]
    Height(OutOfRange),
}

impl Arena {
    /// Arena of the given `width` and `height` in arena units.
    ///
    /// # Errors
    /// A dimension that is not positive and finite, naming which one.
    pub fn new(width: f32, height: f32) -> Result<Self, InvalidArena> {
        let width = Range::PositiveFinite
            .check(width)
            .map_err(InvalidArena::Width)?;
        let height = Range::PositiveFinite
            .check(height)
            .map_err(InvalidArena::Height)?;
        Ok(Self {
            half_extents: Vec2::new(width / 2.0, height / 2.0),
        })
    }

    /// Half the width and half the height.
    #[must_use]
    pub const fn half_extents(self) -> Vec2 {
        self.half_extents
    }

    /// X coordinate of the goal line at `side`.
    #[must_use]
    pub const fn goal_x(self, side: Side) -> f32 {
        side.sign() * self.half_extents.x
    }

    /// Whether `point` lies inside the arena or on its boundary.
    #[must_use]
    pub fn contains(self, point: Vec2) -> bool {
        let p = point.abs();
        p.x <= self.half_extents.x && p.y <= self.half_extents.y
    }

    /// Highest `|y|` the center of a ball of `radius` can reach before its edge
    /// touches a wall.
    #[must_use]
    pub fn center_limit_y(self, radius: Radius) -> f32 {
        self.half_extents.y - radius.value()
    }

    /// Signed distance from `point` to the nearer wall: positive inside, zero on
    /// the wall, negative once past it.
    #[must_use]
    pub fn wall_distance(self, point: Vec2) -> f32 {
        self.half_extents.y - point.y.abs()
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec2;

    use super::{Arena, InvalidArena, InvalidLengthBounds, LengthBounds, Radius, Side};
    use crate::kernel::test_support::{assert_near, assert_vec_near, standard_arena};
    use crate::kernel::{OutOfRange, Range};

    #[test]
    fn non_positive_or_non_finite_dimensions_are_rejected_naming_the_axis() {
        assert_eq!(
            Arena::new(0.0, 90.0),
            Err(InvalidArena::Width(OutOfRange {
                value: 0.0,
                expected: Range::PositiveFinite,
            }))
        );
        assert_eq!(
            Arena::new(160.0, -90.0),
            Err(InvalidArena::Height(OutOfRange {
                value: -90.0,
                expected: Range::PositiveFinite,
            }))
        );
        assert!(matches!(
            Arena::new(f32::INFINITY, 90.0),
            Err(InvalidArena::Width(_))
        ));
        assert!(matches!(
            Arena::new(160.0, f32::NAN),
            Err(InvalidArena::Height(_))
        ));
    }

    #[test]
    fn radius_must_be_positive_and_finite_and_exposes_its_value() {
        assert_eq!(
            Radius::new(0.0),
            Err(OutOfRange {
                value: 0.0,
                expected: Range::PositiveFinite,
            })
        );
        assert!(Radius::new(f32::NAN).is_err());
        assert!(Radius::new(-1.0).is_err());
        let Ok(radius) = Radius::new(1.5) else {
            panic!("valid radius rejected");
        };
        assert_near(radius.value(), 1.5, 1e-6);
    }

    #[test]
    fn length_bounds_clamp_and_contain_between_valid_ends() {
        let Ok(bounds) = LengthBounds::new(8.0, 30.0) else {
            panic!("valid bounds rejected");
        };
        assert_near(bounds.clamp(100.0), 30.0, 1e-6);
        assert_near(bounds.clamp(1.0), 8.0, 1e-6);
        assert_near(bounds.clamp(14.0), 14.0, 1e-6);
        assert!(bounds.contains(8.0) && bounds.contains(30.0) && !bounds.contains(30.5));
        assert!(LengthBounds::new(8.0, 8.0).is_ok());
    }

    #[test]
    fn length_bounds_reject_bad_ends_and_reversed_ends() {
        assert!(matches!(
            LengthBounds::new(0.0, 30.0),
            Err(InvalidLengthBounds::Min(_))
        ));
        assert!(matches!(
            LengthBounds::new(8.0, f32::NAN),
            Err(InvalidLengthBounds::Max(_))
        ));
        assert_eq!(
            LengthBounds::new(8.0, 4.0),
            Err(InvalidLengthBounds::MaxBelowMin { min: 8.0, max: 4.0 })
        );
    }

    #[test]
    fn out_of_range_error_reads_as_one_sentence() {
        let Err(err) = Arena::new(0.0, 90.0) else {
            panic!("zero width accepted");
        };
        assert_eq!(
            err.to_string(),
            "arena width 0 is out of range: expected a positive finite number"
        );
    }

    #[test]
    fn center_is_inside_and_point_past_goal_line_is_outside() {
        assert!(standard_arena().contains(Vec2::ZERO));
        assert!(!standard_arena().contains(Vec2::new(80.5, 0.0)));
    }

    #[test]
    fn point_past_wall_is_outside_and_point_on_wall_is_inside() {
        assert!(!standard_arena().contains(Vec2::new(0.0, -45.5)));
        assert!(standard_arena().contains(Vec2::new(0.0, 45.0)));
    }

    #[test]
    fn wall_distance_is_to_nearest_wall_and_negative_past_it() {
        assert_near(standard_arena().wall_distance(Vec2::ZERO), 45.0, 1e-6);
        assert_near(
            standard_arena().wall_distance(Vec2::new(30.0, 40.0)),
            5.0,
            1e-6,
        );
        assert_near(
            standard_arena().wall_distance(Vec2::new(30.0, -40.0)),
            5.0,
            1e-6,
        );
        assert_near(
            standard_arena().wall_distance(Vec2::new(0.0, 47.0)),
            -2.0,
            1e-6,
        );
    }

    #[test]
    fn goal_lines_sit_at_half_width_on_each_side() {
        assert_near(standard_arena().goal_x(Side::Left), -80.0, 1e-6);
        assert_near(standard_arena().goal_x(Side::Right), 80.0, 1e-6);
        assert_vec_near(standard_arena().half_extents(), Vec2::new(80.0, 45.0), 1e-6);
    }

    #[test]
    fn side_sign_points_along_x_toward_that_goal_and_opposite_flips_it() {
        assert_near(Side::Left.sign(), -1.0, 1e-6);
        assert_near(Side::Right.sign(), 1.0, 1e-6);
        assert_eq!(Side::Left.opposite(), Side::Right);
        assert_eq!(Side::Right.opposite(), Side::Left);
    }
}

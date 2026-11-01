//! Float assertions for kernel unit tests. Exact float equality is a lint error
//! (`clippy::float_cmp`) and a test smell; every kernel assertion states its tolerance.

use bevy::math::Vec2;

use super::geometry::{Arena, Radius};

/// The M1 arena, 160 by 90 arena units (spec decisions).
pub(crate) fn standard_arena() -> Arena {
    Arena::new(160.0, 90.0).unwrap_or_else(|err| panic!("standard arena rejected: {err}"))
}

/// Panics unless `actual` is within `tolerance` of `expected`.
#[track_caller]
pub(crate) fn assert_near(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected} ± {tolerance}, got {actual}"
    );
}

/// Panics unless every component of `actual` is within `tolerance` of `expected`.
#[track_caller]
pub(crate) fn assert_vec_near(actual: Vec2, expected: Vec2, tolerance: f32) {
    assert!(
        (actual - expected).abs().max_element() <= tolerance,
        "expected {expected} ± {tolerance}, got {actual}"
    );
}

/// A ball one arena unit across its radius.
pub(crate) fn unit_radius() -> Radius {
    Radius::new(1.0).unwrap_or_else(|err| panic!("unit radius rejected: {err}"))
}

//! Render-free simulation core for `bevy_pong`.
//!
//! Boundaries (docs/architecture.md): this crate emits events and reads port
//! resources; it never depends on render, window, audio, or UI crates.
//! Feature modules arrive with the milestones in docs/specs/.

// The error policy (docs/architecture.md) forbids bare panics in core.
// Test code may `expect` (see clippy.toml); `unwrap` is denied everywhere.
#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod app;
pub mod domain;
pub mod features;
pub mod kernel;

#[cfg(test)]
pub(crate) mod testing;

//! Render-free simulation core for `bevy_pong`.
//!
//! Boundaries (docs/architecture.md): this crate emits events and reads port
//! resources; it never depends on render, window, audio, or UI crates.
//! Feature modules arrive with the milestones in docs/specs/.

// The error policy (docs/architecture.md) forbids bare panics in core.
#![deny(clippy::unwrap_used, clippy::expect_used)]

//! Feature modules: one directory per feature with a plugin at its root
//! (docs/architecture.md, intra-crate rules 1 and 7).
//!
//! The feature order (ADR 0002), earliest first:
//! `tuning, seats, ball, paddle, physics, rules, ai, sanity, persistence`.
//! A feature may import the public components, resources, and messages of a
//! feature earlier in the order, never its systems or plugin internals, and
//! never the app roof. A new feature takes a place in this list.

pub mod tuning;

pub mod seats;

pub mod ball;

pub mod paddle;

pub mod physics;

pub mod rules;

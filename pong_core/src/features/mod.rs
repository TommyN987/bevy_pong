//! Feature modules: one directory per feature with a plugin at its root
//! (docs/architecture.md, intra-crate rules 1 and 7). Features meet through
//! vocabulary types and events, never by importing each other.

pub mod ball;
pub mod paddle;
pub mod physics;
pub mod seats;
pub mod tuning;

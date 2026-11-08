//! Persistence (spec 1.4.9, docs/architecture.md "Persistence"): the profiles
//! and settings the game keeps between runs, their on-disk documents, and the
//! file IO the core performs through the shell's [`io::SavePaths`] port.
//!
//! - [`model`]: what the game holds in memory; every value valid by construction.
//! - [`document`]: what is on disk; each released version's shape is frozen.
//! - [`io`]: where files live and how they are written without tearing.

pub mod document;
pub mod io;
pub mod model;

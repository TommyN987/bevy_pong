//! `bevy_pong` shell: window, rendering, audio, and UI around `pong_core`.
//!
//! A library so its plugins are an API that `main.rs` assembles and tests import
//! (docs/architecture.md, "Intra-crate structure": the shell mirrors core).

pub mod ports;
pub mod views;
pub mod xdg;

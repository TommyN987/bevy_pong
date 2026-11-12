//! `bevy_pong` shell: window, rendering, audio, and UI around `pong_core`.
//!
//! A library so its plugins are an API that `main.rs` assembles and tests import
//! (docs/architecture.md, "Intra-crate structure": the shell mirrors core).

pub mod audio;
pub mod crash;
pub mod effects;
pub mod overlay;
pub mod ports;
pub mod seed;
pub mod ui;
pub mod views;
pub mod window;
pub mod xdg;

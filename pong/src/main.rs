//! `bevy_pong` shell: window, rendering, audio, and UI around `pong_core`.

#[cfg(all(feature = "dev", not(debug_assertions)))]
compile_error!(
    "the `dev` feature (dynamic linking) is for debug builds only; build release without it"
);

fn main() {}

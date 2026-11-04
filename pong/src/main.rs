//! `bevy_pong` binary: assembles the core plugin group and the shell's plugins
//! from the `pong` library. The app is built here from plan 4.5 on.

#[cfg(all(feature = "dev", not(debug_assertions)))]
compile_error!(
    "the `dev` feature (dynamic linking) is for debug builds only; build release without it"
);

fn main() {}

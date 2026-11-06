//! `bevy_pong` binary: assembles Bevy's default plugins, the core plugin group,
//! and the shell's plugins from the `pong` library.

use bevy::DefaultPlugins;
use bevy::app::{App, AppExit};
use bevy::log::info;
use pong::dev_entry::DevEntryPlugin;
use pong::ports::PortsPlugin;
use pong::views::ViewsPlugin;
use pong_core::app::CorePlugins;

#[cfg(all(feature = "dev", not(debug_assertions)))]
compile_error!(
    "the `dev` feature (dynamic linking) is for debug builds only; build release without it"
);

fn main() -> AppExit {
    let seed = pong::seed::from_entropy();
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    // Logged so a launch can be replayed from its seed; the log exists once
    // `DefaultPlugins` is added.
    info!("core seed {seed}");
    app.add_plugins((CorePlugins { seed }, PortsPlugin, ViewsPlugin));
    // Compiled in every build, so lint and tests see it; added only in dev
    // builds until the menus replace it (plan 8.3).
    if cfg!(feature = "dev") {
        app.add_plugins(DevEntryPlugin);
    }
    app.run()
}

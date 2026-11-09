//! `bevy_pong` binary: assembles Bevy's default plugins, the core plugin group,
//! and the shell's plugins from the `pong` library.

use bevy::DefaultPlugins;
use bevy::app::{App, AppExit, PluginGroup};
use bevy::log::{info, warn};
use bevy::window::WindowPlugin;
use pong::audio::SoundsPlugin;
use pong::audio::synth::Synth;
use pong::crash::{self, CrashContext};
use pong::overlay::OverlayPlugin;
use pong::ports::{PortsPlugin, save_paths};
use pong::ui::ScreensPlugin;
use pong::views::ViewsPlugin;
use pong::window::{WindowModePlugin, opening_mode, primary_cursor_options, primary_window};
use pong::xdg::AppDirs;
use pong_core::app::CorePlugins;

#[cfg(all(feature = "dev", not(debug_assertions)))]
compile_error!(
    "the `dev` feature (dynamic linking) is for debug builds only; build release without it"
);

fn main() -> AppExit {
    let seed = pong::seed::from_entropy();
    let dirs = AppDirs::from_env();
    let paths = dirs.as_ref().ok().map(save_paths::from_app_dirs);
    // The window opens in the saved mode, read before `Boot` loads the
    // settings: switching it in the first frames can be lost (`pong::window`).
    let window_mode = opening_mode(paths.as_ref());
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(primary_window(window_mode)),
        primary_cursor_options: Some(primary_cursor_options()),
        ..WindowPlugin::default()
    }));
    // Without `SavePaths` nothing is saved; plan 7.2's load policy decides
    // what the game shows then.
    if let Some(paths) = paths {
        app.insert_resource(paths);
    }
    // After `DefaultPlugins`, whose panic handler would otherwise wrap this
    // hook and skip it for some panics; the log exists from here on too.
    match dirs {
        Ok(dirs) => crash::install(dirs.state().to_owned(), CrashContext { seed }),
        Err(error) => warn!("no crash files and no saves: {error}"),
    }
    // Logged so a launch can be replayed from its seed.
    info!("core seed {seed}");
    // `SoundsPlugin` after `DefaultPlugins`: it plays only if it finds Bevy's
    // `AudioPlugin` already added.
    app.add_plugins((
        CorePlugins { seed },
        SoundsPlugin(Synth),
        PortsPlugin,
        ViewsPlugin,
        ScreensPlugin,
        WindowModePlugin,
        OverlayPlugin,
    ));
    app.run()
}

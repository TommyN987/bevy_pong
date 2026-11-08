//! `bevy_pong` binary: assembles Bevy's default plugins, the core plugin group,
//! and the shell's plugins from the `pong` library.

use bevy::DefaultPlugins;
use bevy::app::{App, AppExit, PluginGroup};
use bevy::log::{info, warn};
use bevy::window::WindowPlugin;
use pong::crash::{self, CrashContext};
use pong::dev_entry::{self, DevEntryPlugin};
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
    // Read before anything is built: building `DefaultPlugins` already opens
    // the display and the GPU, and the log does not exist yet.
    let dev_entry = if cfg!(feature = "dev") {
        let args: Vec<String> = std::env::args_os()
            .skip(1)
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        match dev_entry::dev_match(&args) {
            Ok(config) => Some(DevEntryPlugin { config }),
            Err(error) => {
                eprintln!("bevy_pong: {error}");
                return AppExit::error();
            }
        }
    } else {
        None
    };
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
    app.add_plugins((
        CorePlugins { seed },
        PortsPlugin,
        ViewsPlugin,
        ScreensPlugin,
        WindowModePlugin,
    ));
    // Compiled in every build, so lint and tests see it; added only in dev
    // builds until the match screens replace it (plan 8.3).
    if let Some(plugin) = dev_entry {
        app.add_plugins(plugin);
    }
    app.run()
}

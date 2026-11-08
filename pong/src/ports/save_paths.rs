//! The save paths port (spec 1.4.9, plan 7.1): the core's `SavePaths` from
//! the XDG directories, settings in the config directory and profiles in the
//! data directory (docs/architecture.md "Persistence").

use pong_core::features::persistence::io::SavePaths;

use crate::xdg::AppDirs;

/// Where the core keeps the save files on this machine.
#[must_use]
pub fn from_app_dirs(dirs: &AppDirs) -> SavePaths {
    SavePaths::new(dirs.config(), dirs.data())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::from_app_dirs;
    use crate::xdg::AppDirs;

    #[test]
    fn settings_go_to_the_config_directory_and_profiles_to_the_data_directory() {
        let dirs = AppDirs::resolve(|name| (name == "HOME").then(|| OsString::from("/home/kid")))
            .expect("HOME is absolute");
        let paths = from_app_dirs(&dirs);
        assert_eq!(
            paths.settings_file(),
            PathBuf::from("/home/kid/.config/bevy_pong/settings.ron")
        );
        assert_eq!(
            paths.profiles_file(),
            PathBuf::from("/home/kid/.local/share/bevy_pong/profiles.ron")
        );
    }
}

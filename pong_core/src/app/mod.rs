//! The app roof (docs/architecture.md): the core plugin group, app states,
//! fixed-step phase sets, and the error policy. Depends on every feature;
//! nothing depends on it.

use bevy::app::{PluginGroup, PluginGroupBuilder};

use crate::features::seats::SeatsPlugin;
use crate::features::tuning::TuningPlugin;

pub mod error_policy;
pub mod phases;
pub mod states;

/// Every core plugin, in build order. The shell adds this after `DefaultPlugins`;
/// the testing module adds it after `MinimalPlugins` and `AssetPlugin`.
/// Contract: `AssetPlugin` precedes the group (the tuning asset needs it);
/// `StatesPlugin` is added by the group itself if absent.
pub struct CorePlugins;

impl PluginGroup for CorePlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(error_policy::ErrorPolicyPlugin)
            .add(states::AppStatePlugin)
            .add(phases::PhasesPlugin)
            .add(TuningPlugin)
            .add(SeatsPlugin)
    }
}

#[cfg(test)]
mod tests {
    use bevy::MinimalPlugins;
    use bevy::app::App;
    use bevy::asset::AssetPlugin;
    use bevy::state::state::State;

    use super::CorePlugins;
    use super::error_policy::ErrorPolicyPlugin;
    use super::phases::{PhasesPlugin, unphased_fixed_step_systems};
    use super::states::{AppState, AppStatePlugin};
    use crate::features::seats::SeatsPlugin;
    use crate::features::tuning::TuningPlugin;

    fn headless_core() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), CorePlugins));
        app
    }

    #[test]
    fn the_group_adds_every_core_plugin_once() {
        let app = headless_core();
        assert!(app.is_plugin_added::<ErrorPolicyPlugin>());
        assert!(app.is_plugin_added::<AppStatePlugin>());
        assert!(app.is_plugin_added::<PhasesPlugin>());
        assert!(app.is_plugin_added::<TuningPlugin>());
        assert!(app.is_plugin_added::<SeatsPlugin>());
    }

    #[test]
    fn a_headless_app_boots_through_the_tuning_load_into_the_menu() {
        let mut app = headless_core();
        let reached_menu = (0..1_000).any(|_| {
            app.update();
            *app.world().resource::<State<AppState>>().get() == AppState::Menu
        });
        assert!(reached_menu, "still in Boot after the step budget");
    }

    #[test]
    fn every_core_fixed_step_system_declares_a_phase() {
        let mut app = headless_core();
        assert_eq!(unphased_fixed_step_systems(&mut app), Vec::<String>::new());
    }
}

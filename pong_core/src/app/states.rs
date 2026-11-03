//! App states (spec 1.4.10, docs/architecture.md "States"): `Boot` waits for
//! everything the menu needs, then `Menu`, then `InMatch`.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::schedule::common_conditions::resource_exists;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemCondition};
use bevy::ecs::system::ResMut;
use bevy::state::app::{AppExtStates, StatesPlugin};
use bevy::state::condition::in_state;
use bevy::state::state::{NextState, States};

use crate::features::tuning::Tuning;

/// Top-level flow. Match substates arrive with the rules feature (3.7).
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    /// Waiting for tuning, settings, and profiles. Nothing is shown yet.
    #[default]
    Boot,
    /// Menus: mode, profiles, settings.
    Menu,
    /// A match is in progress.
    InMatch,
}

/// Declares [`AppState`] and the gate out of `Boot`.
///
/// Adds Bevy's `StatesPlugin` if the app lacks it: `MinimalPlugins` does not
/// include it and state transitions silently never run without it (ADR 0001).
/// The gate currently waits on the [`Tuning`] resource; persistence (plan
/// phase 7) extends the condition with settings and profiles.
pub struct AppStatePlugin;

impl Plugin for AppStatePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        app.init_state::<AppState>().add_systems(
            Update,
            leave_boot.run_if(in_state(AppState::Boot).and_then(resource_exists::<Tuning>)),
        );
    }
}

fn leave_boot(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Menu);
}

#[cfg(test)]
mod tests {
    use bevy::MinimalPlugins;
    use bevy::app::App;
    use bevy::state::state::State;

    use super::{AppState, AppStatePlugin};
    use crate::features::tuning::{EMBEDDED_RON, Tuning};

    fn app() -> App {
        let mut app = App::new();
        // No `StatesPlugin` here: the plugin must supply it itself.
        app.add_plugins((MinimalPlugins, AppStatePlugin));
        app
    }

    fn state(app: &App) -> AppState {
        *app.world().resource::<State<AppState>>().get()
    }

    #[test]
    fn a_held_tuning_load_produces_no_transition_out_of_boot() {
        let mut app = app();
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(state(&app), AppState::Boot);
    }

    #[test]
    fn boot_leaves_for_menu_once_tuning_is_resolved() {
        let mut app = app();
        app.update();
        app.insert_resource(Tuning::from_ron(EMBEDDED_RON).expect("embedded tuning is valid"));
        app.update();
        app.update();
        assert_eq!(state(&app), AppState::Menu);
    }
}

//! The gate out of `Boot` (spec 1.4.10): the app leaves `Boot` once everything
//! the menu needs is resolved. It lives in the roof because it reads features.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::schedule::common_conditions::resource_exists;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemCondition};
use bevy::ecs::system::ResMut;
use bevy::state::condition::in_state;
use bevy::state::state::NextState;

use crate::features::persistence::model::{Profiles, Settings};
use crate::features::tuning::Tuning;
use crate::flow::states::AppState;

/// Moves `Boot` to `Menu` once tuning, settings, and profiles are resolved
/// (spec 1.4.10): the [`Tuning`], [`Settings`], and [`Profiles`] resources
/// exist.
///
/// Precondition: `FlowPlugin` is added first, so `AppState` exists.
pub struct BootPlugin;

impl Plugin for BootPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            leave_boot.run_if(
                in_state(AppState::Boot)
                    .and_then(resource_exists::<Tuning>)
                    .and_then(resource_exists::<Settings>)
                    .and_then(resource_exists::<Profiles>),
            ),
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

    use super::BootPlugin;
    use crate::features::persistence::model::{Profiles, Settings};
    use crate::features::tuning::{EMBEDDED_RON, Tuning};
    use crate::flow::FlowPlugin;
    use crate::flow::states::AppState;

    fn app() -> App {
        let mut app = App::new();
        // No `StatesPlugin` here: `FlowPlugin` must supply it itself.
        app.add_plugins((MinimalPlugins, FlowPlugin, BootPlugin));
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

    fn insert_tuning(app: &mut App) {
        app.insert_resource(Tuning::from_ron(EMBEDDED_RON).expect("embedded tuning is valid"));
    }

    /// Spec 1.4.10: settings and profiles are resolved before the menu.
    #[test]
    fn a_held_save_load_produces_no_transition_out_of_boot() {
        let mut app = app();
        insert_tuning(&mut app);
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(state(&app), AppState::Boot);
    }

    #[test]
    fn boot_leaves_for_menu_once_tuning_settings_and_profiles_are_resolved() {
        let mut app = app();
        app.update();
        insert_tuning(&mut app);
        app.insert_resource(Profiles::default());
        app.insert_resource(Settings::default());
        app.update();
        app.update();
        assert_eq!(state(&app), AppState::Menu);
    }
}

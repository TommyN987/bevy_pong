//! The gate out of `Boot` (spec 1.4.10): the app leaves `Boot` once everything
//! the menu needs is resolved. It lives in the roof because it reads features.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::schedule::common_conditions::resource_exists;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemCondition};
use bevy::ecs::system::ResMut;
use bevy::state::condition::in_state;
use bevy::state::state::NextState;

use crate::features::tuning::Tuning;
use crate::flow::states::AppState;

/// Moves `Boot` to `Menu` once the [`Tuning`] resource exists. Persistence
/// (plan phase 7) extends the condition with settings and profiles.
///
/// Precondition: `FlowPlugin` is added first, so `AppState` exists.
pub struct BootPlugin;

impl Plugin for BootPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
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

    use super::BootPlugin;
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

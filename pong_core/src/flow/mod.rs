//! Flow (ADR 0002): when core systems run. The app and match states and the
//! fixed-step phases, declared once for every feature to gate on.
//!
//! Imports only kernel, domain, and Bevy. Features import flow; flow imports
//! no feature and not the app roof.

use bevy::app::{App, Plugin};
use bevy::state::app::{AppExtStates, StatesPlugin};
use bevy::time::{Fixed, Time};

pub mod phases;
pub mod states;

/// Fixed steps per second, in the game and in every test (ADR 0003). Tuning
/// values such as paddle speed are reasoned at this rate.
pub const FIXED_HZ: f64 = 60.0;

/// Registers [`states::AppState`] and [`states::MatchState`], configures the
/// [`phases::FixedStepPhase`] sets, and sets the fixed clock to [`FIXED_HZ`].
///
/// Adds Bevy's `StatesPlugin` if the app lacks it: `MinimalPlugins` does not
/// include it, and without it state transitions silently never run (ADR 0001).
/// Order against Bevy's `TimePlugin` does not matter: that plugin only
/// initializes the fixed clock if absent.
pub struct FlowPlugin;

impl Plugin for FlowPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        app.insert_resource(Time::<Fixed>::from_hz(FIXED_HZ))
            .init_state::<states::AppState>()
            .add_sub_state::<states::MatchState>();
        phases::configure(app);
    }
}

#[cfg(test)]
mod tests {
    use bevy::MinimalPlugins;
    use bevy::app::App;
    use bevy::time::{Fixed, Time};

    use super::{FIXED_HZ, FlowPlugin};

    #[test]
    fn the_fixed_clock_runs_at_the_one_core_rate_whatever_the_plugin_order() {
        let expected = Time::<Fixed>::from_hz(FIXED_HZ).timestep();
        for flow_first in [true, false] {
            let mut app = App::new();
            if flow_first {
                app.add_plugins((FlowPlugin, MinimalPlugins));
            } else {
                app.add_plugins((MinimalPlugins, FlowPlugin));
            }
            assert_eq!(
                app.world().resource::<Time<Fixed>>().timestep(),
                expected,
                "flow first: {flow_first}"
            );
        }
    }
}

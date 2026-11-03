//! Fixed-step phases (spec 1.4.10, docs/architecture.md rule 4): declared once,
//! in order. Every core system in `FixedUpdate` names its phase and never orders
//! itself against another feature's system.

use bevy::app::{App, FixedUpdate, Plugin};
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::state::condition::in_state;

use super::states::AppState;

/// The named sets one fixed step runs, in this order.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedStepPhase {
    /// Port resources become paddle intents (seats, AI).
    Intent,
    /// Active effects adjust state and accumulate forces.
    Effects,
    /// Paddles and ball move.
    Integrate,
    /// Swept collisions, reflections, hit events.
    Collide,
    /// Scoring, serve, match flow.
    Rules,
    /// Invariant checks; faults heal or panic per profile.
    Sanity,
}

impl FixedStepPhase {
    /// Every phase, in execution order. The single declaration of the order.
    pub const IN_ORDER: [Self; 6] = [
        Self::Intent,
        Self::Effects,
        Self::Integrate,
        Self::Collide,
        Self::Rules,
        Self::Sanity,
    ];
}

/// Configures the phases in `FixedUpdate`. Features add systems with
/// `.in_set(FixedStepPhase::X)` and nothing else about ordering or gating.
///
/// Every phase runs only in [`AppState::InMatch`]: the simulation's resources
/// and entities exist only then, so no feature system guards for them.
/// Contract: [`AppStatePlugin`](super::states::AppStatePlugin) precedes this plugin.
pub struct PhasesPlugin;

impl Plugin for PhasesPlugin {
    fn build(&self, app: &mut App) {
        for phase in FixedStepPhase::IN_ORDER {
            app.configure_sets(FixedUpdate, phase.run_if(in_state(AppState::InMatch)));
        }
        for pair in FixedStepPhase::IN_ORDER.windows(2) {
            app.configure_sets(FixedUpdate, pair[1].after(pair[0]));
        }
    }
}

/// Names of `FixedUpdate` systems that belong to no [`FixedStepPhase`].
/// Builds the schedule first, so it works on an app that has not run.
#[cfg(test)]
pub(crate) fn unphased_fixed_step_systems(app: &mut App) -> Vec<String> {
    use std::collections::HashSet;

    app.world_mut()
        .schedule_scope(FixedUpdate, |world, schedule| {
            schedule
                .initialize(world)
                .expect("the FixedUpdate schedule builds");
            let graph = schedule.graph();
            let phased: HashSet<_> = FixedStepPhase::IN_ORDER
                .iter()
                .filter_map(|phase| graph.systems_in_set(phase.intern()).ok())
                .flat_map(|keys| keys.iter().copied())
                .collect();
            // Once built, the systems live in the executable, not the graph; only
            // their keys and set membership remain on the graph side.
            schedule
                .systems()
                .expect("an initialized schedule lists its systems")
                .filter(|(key, _)| !phased.contains(key))
                .map(|(_, system)| system.name().to_string())
                // Bevy inserts its own sync points between phases when a system
                // uses commands; they are not core systems and have no phase.
                .filter(|name| name != "bevy_ecs::apply_deferred")
                .collect()
        })
}

#[cfg(test)]
mod tests {
    use bevy::MinimalPlugins;
    use bevy::app::{App, FixedUpdate};
    use bevy::ecs::resource::Resource;
    use bevy::ecs::schedule::IntoScheduleConfigs;
    use bevy::ecs::system::ResMut;

    use bevy::state::state::{NextState, State};

    use super::{FixedStepPhase, PhasesPlugin, unphased_fixed_step_systems};
    use crate::app::states::{AppState, AppStatePlugin};

    #[derive(Resource, Default)]
    struct Ran(Vec<FixedStepPhase>);

    /// States and phases, already in a match so the phases run.
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AppStatePlugin, PhasesPlugin))
            .init_resource::<Ran>();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InMatch);
        app.update();
        let state = *app.world().resource::<State<AppState>>().get();
        assert_eq!(state, AppState::InMatch, "test app did not enter the match");
        app
    }

    #[test]
    fn outside_a_match_no_phase_runs() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AppStatePlugin, PhasesPlugin))
            .init_resource::<Ran>();
        app.add_systems(
            FixedUpdate,
            record(FixedStepPhase::Intent).in_set(FixedStepPhase::Intent),
        );
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(
            app.world().resource::<Ran>().0,
            Vec::<FixedStepPhase>::new()
        );
    }

    fn record(phase: FixedStepPhase) -> impl FnMut(ResMut<Ran>) {
        move |mut ran: ResMut<Ran>| ran.0.push(phase)
    }

    #[test]
    fn phases_run_in_declared_order_regardless_of_registration_order() {
        let mut app = app();
        app.add_systems(
            FixedUpdate,
            record(FixedStepPhase::Sanity).in_set(FixedStepPhase::Sanity),
        )
        .add_systems(
            FixedUpdate,
            record(FixedStepPhase::Rules).in_set(FixedStepPhase::Rules),
        )
        .add_systems(
            FixedUpdate,
            record(FixedStepPhase::Collide).in_set(FixedStepPhase::Collide),
        )
        .add_systems(
            FixedUpdate,
            record(FixedStepPhase::Integrate).in_set(FixedStepPhase::Integrate),
        )
        .add_systems(
            FixedUpdate,
            record(FixedStepPhase::Effects).in_set(FixedStepPhase::Effects),
        )
        .add_systems(
            FixedUpdate,
            record(FixedStepPhase::Intent).in_set(FixedStepPhase::Intent),
        );
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(
            app.world().resource::<Ran>().0,
            vec![
                FixedStepPhase::Intent,
                FixedStepPhase::Effects,
                FixedStepPhase::Integrate,
                FixedStepPhase::Collide,
                FixedStepPhase::Rules,
                FixedStepPhase::Sanity,
            ]
        );
    }

    fn unphased_system() {}
    fn phased_system() {}

    #[test]
    fn the_walker_names_a_fixed_step_system_that_declares_no_phase() {
        let mut app = app();
        app.add_systems(
            FixedUpdate,
            (unphased_system, phased_system.in_set(FixedStepPhase::Rules)),
        );
        let unphased = unphased_fixed_step_systems(&mut app);
        assert_eq!(unphased.len(), 1, "{unphased:?}");
        assert!(unphased[0].contains("unphased_system"), "{unphased:?}");
    }

    #[test]
    fn the_walker_accepts_an_app_whose_fixed_step_systems_all_declare_a_phase() {
        let mut app = app();
        app.add_systems(FixedUpdate, phased_system.in_set(FixedStepPhase::Integrate));
        assert_eq!(unphased_fixed_step_systems(&mut app), Vec::<String>::new());
    }
}

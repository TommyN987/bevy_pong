//! The app roof (docs/architecture.md, ADR 0002): the core plugin group, the
//! error policy, and the Boot gate. Depends on every feature and on flow;
//! nothing depends on it.

use bevy::app::{App, Plugin, PluginGroup, PluginGroupBuilder};

use crate::domain::Random;
use crate::features::ai::AiPlugin;
use crate::features::ball::BallPlugin;
use crate::features::effects::EffectsPlugin;
use crate::features::escalation::EscalationPlugin;
use crate::features::paddle::PaddlePlugin;
use crate::features::persistence::PersistencePlugin;
use crate::features::physics::PhysicsPlugin;
use crate::features::rules::RulesPlugin;
use crate::features::sanity::SanityPlugin;
use crate::features::seats::SeatsPlugin;
use crate::features::tuning::TuningPlugin;
use crate::flow::FlowPlugin;

pub mod boot;
pub mod error_policy;

/// Every core plugin, in build order: the error policy, the seeded random
/// source, flow, the Boot gate, then the features in their declared order
/// (`features/mod.rs`). The shell adds this after `DefaultPlugins`; the testing
/// module adds it after `MinimalPlugins` and `AssetPlugin`.
///
/// `seed` determines every random draw in the core: the shell passes entropy
/// (std's `RandomState`, no extra crate); tests pass a constant.
///
/// Preconditions, the caller's to meet:
/// - `AssetPlugin` is added first; the tuning asset needs it.
/// - Bevy's `TimePlugin` is present (in both `MinimalPlugins` and
///   `DefaultPlugins`), or the fixed clock never advances.
/// - Nothing else sets the app's error handler; [`error_policy`] sets it once.
/// - A [`MatchConfig`](crate::domain::MatchConfig) is inserted before entering
///   `AppState::InMatch`; the menu builds it.
///
/// `StatesPlugin` is not a precondition: `FlowPlugin` adds it if absent.
pub struct CorePlugins {
    /// Seed of the core's only random source.
    pub seed: u64,
}

impl PluginGroup for CorePlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(error_policy::ErrorPolicyPlugin::default())
            .add(SeededRandomPlugin { seed: self.seed })
            .add(FlowPlugin)
            .add(boot::BootPlugin)
            .add(TuningPlugin)
            .add(SeatsPlugin)
            .add(BallPlugin)
            .add(PaddlePlugin)
            .add(EffectsPlugin)
            .add(PhysicsPlugin)
            .add(RulesPlugin)
            .add(EscalationPlugin)
            .add(AiPlugin)
            .add(SanityPlugin)
            .add(PersistencePlugin)
    }
}

/// Inserts the core's one [`Random`] source, seeded by [`CorePlugins::seed`].
struct SeededRandomPlugin {
    seed: u64,
}

impl Plugin for SeededRandomPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Random::seeded(self.seed));
    }
}

#[cfg(test)]
mod tests {
    use bevy::MinimalPlugins;
    use bevy::app::App;
    use bevy::asset::AssetPlugin;
    use bevy::state::state::State;

    use super::CorePlugins;
    use super::boot::BootPlugin;
    use super::error_policy::ErrorPolicyPlugin;
    use crate::domain::Random;
    use crate::features::ai::AiPlugin;
    use crate::features::ball::BallPlugin;
    use crate::features::effects::EffectsPlugin;
    use crate::features::escalation::EscalationPlugin;
    use crate::features::paddle::PaddlePlugin;
    use crate::features::persistence::PersistencePlugin;
    use crate::features::physics::PhysicsPlugin;
    use crate::features::rules::RulesPlugin;
    use crate::features::sanity::SanityPlugin;
    use crate::features::seats::SeatsPlugin;
    use crate::features::tuning::TuningPlugin;
    use crate::flow::FlowPlugin;
    use crate::flow::states::AppState;
    use crate::testing::unphased_fixed_step_systems;

    fn headless_core() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            CorePlugins { seed: 7 },
        ));
        app
    }

    /// Plan 3.18: the group cannot be built without a seed, and installs the
    /// random source from it.
    #[test]
    fn the_group_installs_the_random_source_from_its_seed() {
        let app = headless_core();
        assert_eq!(*app.world().resource::<Random>(), Random::seeded(7));
    }

    #[test]
    fn the_group_adds_every_core_plugin_once() {
        let app = headless_core();
        assert!(app.is_plugin_added::<ErrorPolicyPlugin>());
        assert!(app.is_plugin_added::<FlowPlugin>());
        assert!(app.is_plugin_added::<BootPlugin>());
        assert!(app.is_plugin_added::<TuningPlugin>());
        assert!(app.is_plugin_added::<SeatsPlugin>());
        assert!(app.is_plugin_added::<BallPlugin>());
        assert!(app.is_plugin_added::<PaddlePlugin>());
        assert!(app.is_plugin_added::<EffectsPlugin>());
        assert!(app.is_plugin_added::<PhysicsPlugin>());
        assert!(app.is_plugin_added::<RulesPlugin>());
        assert!(app.is_plugin_added::<EscalationPlugin>());
        assert!(app.is_plugin_added::<AiPlugin>());
        assert!(app.is_plugin_added::<SanityPlugin>());
        assert!(app.is_plugin_added::<PersistencePlugin>());
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

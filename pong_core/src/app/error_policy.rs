//! Error handler per build profile (spec 1.7.1, docs/architecture.md error policy):
//! a developer sees every bug the first time, a child never sees a crash.

use bevy::app::{App, Plugin};
use bevy::ecs::error::ErrorHandler;

/// What happens when a system returns an error. One code path in both builds;
/// only the handler differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorPolicy {
    /// Panic at the first error so the bug is seen the first time it happens.
    Dev,
    /// Log at error level and skip the failing system this step; the game goes on.
    Release,
}

impl ErrorPolicy {
    /// `Dev` when debug assertions are on, `Release` otherwise.
    #[must_use]
    pub const fn for_this_build() -> Self {
        if cfg!(debug_assertions) {
            Self::Dev
        } else {
            Self::Release
        }
    }

    /// The app-level handler that enacts this policy.
    #[must_use]
    pub fn handler(self) -> ErrorHandler {
        match self {
            Self::Dev => bevy::ecs::error::panic,
            Self::Release => bevy::ecs::error::error,
        }
    }
}

/// Installs the handler for the given policy; the default is this build's.
/// Contract: an app sets its error handler once, so the shell must not set
/// its own. The test harness overrides the group's instance with `Dev`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorPolicyPlugin(pub ErrorPolicy);

impl Default for ErrorPolicyPlugin {
    fn default() -> Self {
        Self(ErrorPolicy::for_this_build())
    }
}

impl Plugin for ErrorPolicyPlugin {
    fn build(&self, app: &mut App) {
        app.set_error_handler(self.0.handler());
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::{App, Update};
    use bevy::ecs::error::BevyError;
    use bevy::ecs::resource::Resource;
    use bevy::ecs::schedule::IntoScheduleConfigs;
    use bevy::ecs::system::ResMut;

    use super::{ErrorPolicy, ErrorPolicyPlugin};

    #[derive(Resource, Default)]
    struct StepsAfterFailure(u32);

    fn failing_system() -> Result<(), BevyError> {
        Err(BevyError::from("deliberate test failure"))
    }

    fn app_with_failing_system() -> App {
        let mut app = App::new();
        app.init_resource::<StepsAfterFailure>();
        app.add_systems(
            Update,
            (failing_system, |mut steps: ResMut<StepsAfterFailure>| {
                steps.0 += 1;
            })
                .chain(),
        );
        app
    }

    #[test]
    #[should_panic(expected = "deliberate test failure")]
    fn under_the_dev_policy_a_failing_system_panics_the_app() {
        let mut app = app_with_failing_system();
        app.set_error_handler(ErrorPolicy::Dev.handler());
        app.update();
    }

    #[test]
    fn under_the_release_policy_a_failing_system_is_logged_and_the_app_goes_on() {
        let mut app = app_with_failing_system();
        app.set_error_handler(ErrorPolicy::Release.handler());
        app.update();
        app.update();
        assert_eq!(app.world().resource::<StepsAfterFailure>().0, 2);
    }

    #[test]
    fn this_build_picks_the_policy_matching_debug_assertions() {
        let expected = if cfg!(debug_assertions) {
            ErrorPolicy::Dev
        } else {
            ErrorPolicy::Release
        };
        assert_eq!(ErrorPolicy::for_this_build(), expected);
    }

    #[test]
    fn the_plugin_installs_the_policy_it_is_given() {
        let mut app = app_with_failing_system();
        app.add_plugins(ErrorPolicyPlugin(ErrorPolicy::Release));
        app.update();
        assert_eq!(app.world().resource::<StepsAfterFailure>().0, 1);
    }

    #[test]
    fn the_plugin_installs_a_handler() {
        let mut app = App::new();
        assert!(app.get_error_handler().is_none());
        app.add_plugins(ErrorPolicyPlugin::default());
        assert!(app.get_error_handler().is_some());
    }
}

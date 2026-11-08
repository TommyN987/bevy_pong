//! The boot screen (spec 1.6.3): shown while `Boot` waits, gone after.

use bevy::app::App;
use bevy::ecs::query::With;
use pong::ui::boot_screen::{BootScreen, BootScreenPlugin};
use pong_core::flow::states::AppState;
use pong_core::testing::{app_state, boot, frame, headless_app};

fn boot_screens(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<(), With<BootScreen>>()
        .iter(app.world())
        .count()
}

#[test]
fn the_boot_screen_shows_while_booting_and_goes_with_boot() {
    let mut app = headless_app(1);
    app.add_plugins(BootScreenPlugin);
    frame(&mut app);
    assert_eq!(app_state(&app), AppState::Boot);
    assert_eq!(boot_screens(&mut app), 1);
    boot(&mut app);
    frame(&mut app);
    assert_eq!(boot_screens(&mut app), 0);
}

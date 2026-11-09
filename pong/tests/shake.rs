//! Screen shake (spec 1.6.2 MAY, plan 10.2): a point shakes the camera, which
//! comes to rest at the centre. Headless: the core harness plus the shake
//! plugin, on a plain entity in the camera's place, since a camera needs the
//! renderer.

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::Messages;
use bevy::math::Vec2;
use bevy::transform::components::Transform;
use pong::views::shake::{SHAKE_FOR, Shake, ShakePlugin};
use pong_core::domain::PointScored;
use pong_core::kernel::geometry::Side;
use pong_core::testing::{boot, frame, headless_app};

/// The app and the stand-in camera, at z 10, which a shake must keep.
fn shake_app() -> (App, Entity) {
    let mut app = headless_app(1);
    app.add_plugins(ShakePlugin);
    boot(&mut app);
    frame(&mut app);
    let camera = app
        .world_mut()
        .spawn((Shake::default(), Transform::from_xyz(0.0, 0.0, 10.0)))
        .id();
    (app, camera)
}

fn camera_at(app: &App, camera: Entity) -> Transform {
    *app.world().get::<Transform>(camera).expect("the camera")
}

fn point(app: &mut App) {
    app.world_mut()
        .resource_mut::<Messages<PointScored>>()
        .write(PointScored { scorer: Side::Left });
}

/// Frames to outlast a shake at one fixed step (1/60 s) per frame.
fn frames_past_a_shake() -> u32 {
    u32::try_from(SHAKE_FOR.as_millis() * 60 / 1000).expect("a short shake") + 2
}

#[test]
fn a_point_shakes_the_camera_which_comes_to_rest_at_the_centre() {
    let (mut app, camera) = shake_app();
    frame(&mut app);
    assert_eq!(camera_at(&app, camera).translation.truncate(), Vec2::ZERO);
    point(&mut app);
    let mut moved = false;
    for _ in 0..frames_past_a_shake() {
        frame(&mut app);
        moved |= camera_at(&app, camera).translation.truncate() != Vec2::ZERO;
        assert_eq!(camera_at(&app, camera).translation.z, 10.0, "z is kept");
    }
    assert!(moved, "the point never shook the camera");
    assert_eq!(camera_at(&app, camera).translation.truncate(), Vec2::ZERO);
}

#[test]
fn without_a_point_the_camera_stays_at_the_centre() {
    let (mut app, camera) = shake_app();
    for _ in 0..frames_past_a_shake() {
        frame(&mut app);
        assert_eq!(camera_at(&app, camera).translation.truncate(), Vec2::ZERO);
    }
}

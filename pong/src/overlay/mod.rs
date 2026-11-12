//! The developer overlay (spec 1.6.5): F3 shows the measured fixed-step and
//! frame rates, the ball's speed, the effects in play and the return count
//! (M2 spec 2.9.2), each computer player's aim, and the state and sub-state.
//! It ships in release builds, hidden by default (`docs/architecture.md`):
//! the rates mean something only in an optimized build, and playtests run one.
//!
//! While it shows during a match it is also the sandbox (M2 spec 2.9.1): the
//! number keys toggle effects, the brackets hold the return count, and 0 lets
//! go. The keys write the core's [`Sandbox`] port and nothing else; the core
//! decides when a forced value takes hold, and that the match records nothing.

use std::time::Duration;

use bevy::app::{App, FixedPostUpdate, Plugin, Startup, Update};
use bevy::camera::visibility::Visibility;
use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::state::state::State;
use bevy::text::TextColor;
use bevy::time::Time;
use bevy::ui::widget::Text;
use bevy::ui::{GlobalZIndex, Node, PositionType, Val};
use pong_core::domain::EffectId;
use pong_core::features::ai::{Aim, MirrorController};
use pong_core::features::ball::{Ball, Velocity};
use pong_core::features::effects::{ActiveEffects, BallFlight, Sandbox};
use pong_core::flow::states::{AppState, MatchState};
use pong_core::kernel::geometry::Side;

use crate::ui::menu::MenuScreen;
use crate::ui::style::{DIM, font};

/// Shows and hides the overlay; no seat uses it.
const TOGGLE: KeyCode = KeyCode::F3;

/// Marks the overlay's text.
#[derive(Component, Debug, Default)]
pub struct OverlayText;

/// Shows the overlay while F3 has toggled it on; counts the rates always.
///
/// Precondition: the input plugin; the core plugin group.
pub struct OverlayPlugin;

impl Plugin for OverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Rates>()
            .add_systems(Startup, spawn_overlay)
            .add_systems(FixedPostUpdate, count_tick)
            .add_systems(
                Update,
                (count_frame, toggle_overlay, drive_sandbox, show_readings).chain(),
            );
    }
}

/// Top left, over every screen, for the life of the app.
fn spawn_overlay(mut commands: Commands) {
    commands.spawn((
        OverlayText,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Vw(1.0),
            top: Val::Vh(1.0),
            ..Node::default()
        },
        Text::default(),
        font(2.5),
        TextColor(DIM),
        GlobalZIndex(1),
        Visibility::Hidden,
    ));
}

fn count_tick(mut rates: ResMut<Rates>) {
    rates.tick();
}

fn count_frame(time: Res<Time>, mut rates: ResMut<Rates>) {
    rates.frame(time.elapsed());
}

fn toggle_overlay(
    keys: Res<ButtonInput<KeyCode>>,
    mut overlays: Query<&mut Visibility, With<OverlayText>>,
) {
    if !keys.just_pressed(TOGGLE) {
        return;
    }
    for mut visibility in &mut overlays {
        *visibility = if *visibility == Visibility::Hidden {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// The keys that toggle an effect, in [`EffectId::ALL`]'s order: key 1 the
/// first effect. A key past the last effect does nothing.
const EFFECT_KEYS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];
/// Hold the return count one higher, or one lower.
const RAISE_RETURNS: KeyCode = KeyCode::BracketRight;
const LOWER_RETURNS: KeyCode = KeyCode::BracketLeft;
/// Force nothing any more.
const RELEASE: KeyCode = KeyCode::Digit0;

/// While the overlay shows in a match, turns the sandbox keys into writes
/// to the [`Sandbox`] port. A count is first held from the rally's own.
fn drive_sandbox(
    keys: Res<ButtonInput<KeyCode>>,
    overlays: Query<&Visibility, With<OverlayText>>,
    active: Res<ActiveEffects>,
    flights: Query<&BallFlight, With<Ball>>,
    mut sandbox: ResMut<Sandbox>,
) {
    let shown = overlays
        .iter()
        .any(|visibility| *visibility != Visibility::Hidden);
    // A ball exists exactly while a match does.
    let Some(flight) = flights.iter().next().filter(|_| shown) else {
        return;
    };
    for (&key, effect) in EFFECT_KEYS.iter().zip(EffectId::ALL) {
        if keys.just_pressed(key) {
            sandbox.toggle(effect, active.ids());
        }
    }
    let held = sandbox
        .held_returns()
        .unwrap_or_else(|| flight.flight().returns());
    if keys.just_pressed(RAISE_RETURNS) {
        sandbox.hold_returns(Some(held.saturating_add(1)));
    }
    if keys.just_pressed(LOWER_RETURNS) {
        sandbox.hold_returns(Some(held.saturating_sub(1)));
    }
    if keys.just_pressed(RELEASE) {
        sandbox.release();
    }
}

/// What the overlay reads from a match.
#[derive(SystemParam)]
struct MatchReadings<'w, 's> {
    balls: Query<'w, 's, (&'static Velocity, &'static BallFlight), With<Ball>>,
    active: Res<'w, ActiveEffects>,
    sandbox: Res<'w, Sandbox>,
}

/// While shown, writes the readings when they change.
fn show_readings(
    rates: Res<Rates>,
    in_match: MatchReadings,
    controllers: Query<&MirrorController>,
    app_state: Option<Res<State<AppState>>>,
    match_state: Option<Res<State<MatchState>>>,
    menu_screen: Option<Res<State<MenuScreen>>>,
    mut overlays: Query<(&Visibility, &mut Text), With<OverlayText>>,
) {
    for (visibility, mut text) in &mut overlays {
        if *visibility == Visibility::Hidden {
            continue;
        }
        let mut aims: Vec<_> = controllers.iter().map(|c| (c.side(), c.aim())).collect();
        aims.sort_by_key(|&(side, _)| side == Side::Right);
        let states = [
            app_state.as_ref().map(|state| format!("{:?}", state.get())),
            match_state
                .as_ref()
                .map(|state| format!("{:?}", state.get())),
            menu_screen
                .as_ref()
                .map(|state| format!("{:?}", state.get())),
        ];
        // A ball exists exactly while a match does.
        let ball = in_match.balls.iter().next();
        let sandbox = &*in_match.sandbox;
        let forcing = sandbox.forced_effects().is_some() || sandbox.held_returns().is_some();
        let readings = Readings {
            rates: *rates,
            ball_speed: ball.map(|(velocity, _)| velocity.0.length()),
            effects: ball.map(|_| in_match.active.ids().to_vec()),
            returns: ball.map(|(_, flight)| flight.flight().returns()),
            sandbox: ball
                .filter(|_| forcing || sandbox.touched_match())
                .map(|_| sandbox.clone()),
            aims,
            states: states.into_iter().flatten().collect(),
        };
        let shown = overlay_text(&readings);
        if text.0 != shown {
            text.0 = shown;
        }
    }
}

/// Frames and fixed ticks counted over the last whole second or more of game
/// time, then as rates per second.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub struct Rates {
    /// The frame the count began at, itself not counted: `n` frames span
    /// `n - 1` steps. `None` before the first frame.
    since: Option<Duration>,
    frames: u32,
    ticks: u32,
    frames_per_second: f64,
    ticks_per_second: f64,
}

impl Rates {
    /// One fixed tick ran.
    pub const fn tick(&mut self) {
        self.ticks = self.ticks.saturating_add(1);
    }

    /// One frame ran, at `now` on the game clock. The first frame begins the
    /// count. Once a second or more has passed since, the rates become the
    /// counts over that exact span, and this frame begins the next count.
    pub fn frame(&mut self, now: Duration) {
        let Some(since) = self.since else {
            self.restart(now);
            return;
        };
        self.frames = self.frames.saturating_add(1);
        let span = now.saturating_sub(since);
        if span >= Duration::from_secs(1) {
            let seconds = span.as_secs_f64();
            self.frames_per_second = f64::from(self.frames) / seconds;
            self.ticks_per_second = f64::from(self.ticks) / seconds;
            self.restart(now);
        }
    }

    const fn restart(&mut self, now: Duration) {
        self.since = Some(now);
        self.frames = 0;
        self.ticks = 0;
    }

    /// Frames per second over the last closed span.
    #[must_use]
    pub const fn frames_per_second(&self) -> f64 {
        self.frames_per_second
    }

    /// Fixed ticks per second over the last closed span.
    #[must_use]
    pub const fn ticks_per_second(&self) -> f64 {
        self.ticks_per_second
    }
}

/// What the overlay shows this frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Readings {
    /// The measured rates.
    pub rates: Rates,
    /// The ball's speed in arena units per second, during a match.
    pub ball_speed: Option<f32>,
    /// The effects in play, during a match (M2 spec 2.9.2).
    pub effects: Option<Vec<EffectId>>,
    /// The ball's return count, during a match (M2 spec 2.9.2).
    pub returns: Option<u16>,
    /// What the sandbox forces or has touched, during a match in which it
    /// did either (M2 spec 2.9.1).
    pub sandbox: Option<Sandbox>,
    /// Each computer player's side and aim, left first.
    pub aims: Vec<(Side, Option<Aim>)>,
    /// The state, then the sub-state if there is one.
    pub states: Vec<String>,
}

/// The overlay's lines, one reading each; a line with nothing to read is
/// left out.
#[must_use]
pub fn overlay_text(readings: &Readings) -> String {
    let rates = readings.rates;
    let mut lines = vec![format!(
        "fixed {:.1} Hz   frame {:.0} fps",
        rates.ticks_per_second(),
        rates.frames_per_second()
    )];
    if let Some(speed) = readings.ball_speed {
        lines.push(format!("ball {speed:.1} u/s"));
    }
    if let Some(effects) = &readings.effects {
        lines.push(format!("effects {}", names(effects)));
    }
    if let Some(returns) = readings.returns {
        lines.push(format!("returns {returns}"));
    }
    for &(side, aim) in &readings.aims {
        let side = match side {
            Side::Left => "left",
            Side::Right => "right",
        };
        lines.push(match aim {
            None => format!("AI {side}: centre"),
            Some(aim) => format!(
                "AI {side}: target {:.1}, error {:+.1}",
                aim.target_y, aim.error
            ),
        });
    }
    if let Some(sandbox) = &readings.sandbox {
        let forced = [
            sandbox
                .forced_effects()
                .map(|effects| format!("effects {}", names(effects))),
            sandbox
                .held_returns()
                .map(|returns| format!("returns {returns}")),
        ];
        let forced: Vec<String> = forced.into_iter().flatten().collect();
        let what = if forced.is_empty() {
            "released".to_owned()
        } else {
            forced.join(", ")
        };
        lines.push(format!("sandbox: {what}; this match records nothing"));
    }
    if readings.effects.is_some() {
        lines.push(format!(
            "sandbox keys: 1-{} effect, [ ] returns, 0 release",
            EffectId::ALL.len()
        ));
    }
    lines.push(readings.states.join(" / "));
    lines.join("\n")
}

/// Effects by name, as the enum spells them; `none` for none.
fn names(effects: &[EffectId]) -> String {
    if effects.is_empty() {
        return "none".to_owned();
    }
    let names: Vec<String> = effects.iter().map(|effect| format!("{effect:?}")).collect();
    names.join(", ")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use pong_core::domain::EffectId;
    use pong_core::features::ai::Aim;
    use pong_core::features::effects::Sandbox;
    use pong_core::kernel::geometry::Side;

    use super::{Rates, Readings, overlay_text};

    /// `frames` frames `frame` apart, the first at zero as Bevy's is, with
    /// `ticks` fixed ticks before each frame after the first.
    fn run(frame: Duration, frames: u32, ticks: u32) -> Rates {
        let mut rates = Rates::default();
        for n in 0..frames {
            if n > 0 {
                for _ in 0..ticks {
                    rates.tick();
                }
            }
            rates.frame(frame * n);
        }
        rates
    }

    #[track_caller]
    fn assert_near(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.01,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn rates_are_counts_over_the_last_closed_second() {
        // Sixty frames of 1/60 s fall 40 ns short of a second in whole
        // nanoseconds, so the span closes on the 62nd frame, 61 steps on.
        let rates = run(Duration::from_secs(1) / 60, 62, 1);
        assert_near(rates.frames_per_second(), 60.0);
        assert_near(rates.ticks_per_second(), 60.0);
        let slow = run(Duration::from_secs(1) / 30, 32, 2);
        assert_near(slow.frames_per_second(), 30.0);
        assert_near(slow.ticks_per_second(), 60.0);
    }

    /// Each span counts afresh: after a fast second, a slow one reads slow.
    #[test]
    fn each_closed_span_counts_only_its_own_frames_and_ticks() {
        let mut rates = run(Duration::from_secs(1) / 60, 62, 1);
        let start = Duration::from_secs(1) / 60 * 61;
        for n in 1..=31 {
            rates.tick();
            rates.tick();
            rates.frame(start + Duration::from_secs(1) / 30 * n);
        }
        assert_near(rates.frames_per_second(), 30.0);
        assert_near(rates.ticks_per_second(), 60.0);
    }

    #[test]
    fn rates_hold_until_a_second_has_passed() {
        let rates = run(Duration::from_secs(1) / 60, 30, 1);
        assert_near(rates.frames_per_second(), 0.0);
    }

    fn readings() -> Readings {
        Readings {
            rates: run(Duration::from_secs(1) / 60, 62, 1),
            ball_speed: Some(72.04),
            effects: None,
            returns: None,
            sandbox: None,
            aims: vec![
                (Side::Left, None),
                (
                    Side::Right,
                    Some(Aim {
                        target_y: 12.31,
                        error: 4.06,
                    }),
                ),
            ],
            states: vec!["InMatch".to_owned(), "Playing".to_owned()],
        }
    }

    #[test]
    fn the_overlay_reads_each_value_on_a_line() {
        assert_eq!(
            overlay_text(&readings()),
            "fixed 60.0 Hz   frame 60 fps\n\
             ball 72.0 u/s\n\
             AI left: centre\n\
             AI right: target 12.3, error +4.1\n\
             InMatch / Playing"
        );
    }

    /// M2 spec 2.9.2: the overlay lists active effects and the hit count,
    /// and in a match how to drive the sandbox.
    #[test]
    fn in_a_match_the_overlay_lists_the_effects_the_return_count_and_the_sandbox_keys() {
        let in_chaos = Readings {
            effects: Some(vec![EffectId::Speed, EffectId::Shrink]),
            returns: Some(7),
            ..readings()
        };
        assert_eq!(
            overlay_text(&in_chaos),
            "fixed 60.0 Hz   frame 60 fps\n\
             ball 72.0 u/s\n\
             effects Speed, Shrink\n\
             returns 7\n\
             AI left: centre\n\
             AI right: target 12.3, error +4.1\n\
             sandbox keys: 1-2 effect, [ ] returns, 0 release\n\
             InMatch / Playing"
        );
        let none = Readings {
            effects: Some(Vec::new()),
            ..in_chaos
        };
        assert!(overlay_text(&none).contains("\neffects none\n"));
    }

    /// M2 spec 2.9.1: what is forced is on screen, and that it costs the
    /// match its records.
    #[test]
    fn a_touched_match_reads_what_is_forced_and_that_nothing_is_recorded() {
        let mut sandbox = Sandbox::default();
        sandbox.toggle(EffectId::Shrink, &[]);
        sandbox.hold_returns(Some(9));
        let forcing = Readings {
            effects: Some(vec![EffectId::Speed]),
            returns: Some(3),
            sandbox: Some(sandbox.clone()),
            ..readings()
        };
        assert!(
            overlay_text(&forcing)
                .contains("\nsandbox: effects Shrink, returns 9; this match records nothing\n"),
            "{}",
            overlay_text(&forcing)
        );
        sandbox.release();
        let released = Readings {
            sandbox: Some(sandbox),
            ..forcing
        };
        assert!(
            overlay_text(&released).contains("\nsandbox: released; this match records nothing\n"),
            "{}",
            overlay_text(&released)
        );
    }

    #[test]
    fn outside_a_match_only_the_rates_and_states_show() {
        let menu = Readings {
            ball_speed: None,
            aims: Vec::new(),
            states: vec!["Menu".to_owned(), "Volume".to_owned()],
            ..readings()
        };
        assert_eq!(
            overlay_text(&menu),
            "fixed 60.0 Hz   frame 60 fps\nMenu / Volume"
        );
    }
}

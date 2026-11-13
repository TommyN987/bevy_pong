//! Effects (M2 spec 2.2, 2.3; ADR 0011): the contract every effect
//! implements, the effects in play, and the flight they compose on the ball.
//!
//! An effect never touches the simulation itself. At each flight start (a
//! serve or a paddle return) it is asked once for its
//! [`Contribution`](crate::kernel::flight::Contribution); the base composes
//! the answers into the ball's [`Flight`], which physics and the AI's
//! prediction read. One module per effect; effects do not import each other.
//!
//! An effect also states its [`Flaw`]: how an AI of each difficulty
//! misjudges it (M2 spec 2.3.12). The AI asks when it plans an approach.

use std::time::Duration;

use bevy::app::{App, FixedUpdate, Plugin};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::message::MessageReader;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Local, Query, Res, ResMut};
use bevy::state::state::{OnEnter, OnExit};

use crate::domain::{Difficulty, EffectId, Mode, Serve};
use crate::features::ball::Ball;
use crate::features::paddle::Paddle;
use crate::features::tuning::Tuning;
use crate::flow::phases::{BallInPlay, FixedStepPhase};
use crate::flow::states::{AppState, MatchState};
use crate::kernel::flight::{Contribution, Flight, FlightStart};

pub mod shrink;
pub mod speed;

/// One effect's core half (ADR 0011).
///
/// No method has a default body: an effect states its identity, its stance
/// on every channel, and its AI flaw, or it does not compile.
/// `Send + Sync + 'static` because the effects in play live in a resource.
pub trait Effect: Send + Sync + 'static {
    /// Which effect this is.
    fn id(&self) -> EffectId;

    /// This effect's share of the flight that begins at `start`. Called once
    /// per flight; the answer holds until the next flight starts.
    ///
    /// Postcondition (the effect's): the answer depends on `start` and on
    /// nothing that changes while the flight lasts. [`flaw`](Self::flaw) is
    /// asked later in the same flight, and an effect that is believed as it
    /// is answers it by contributing again.
    fn contribute(&self, start: &FlightStart) -> Contribution;

    /// How an AI of `difficulty` misjudges this effect over the flight that
    /// began at `start` (M2 spec 2.3.12). Called when an AI plans an
    /// approach, so any number of times per flight.
    ///
    /// Postcondition (the effect's): the same `start` and `difficulty` give
    /// the same flaw for as long as the flight lasts, and
    /// [`Flaw::added_error`] is not negative.
    fn flaw(&self, start: &FlightStart, difficulty: Difficulty) -> Flaw;
}

/// How an AI misjudges one effect over one flight: the flaw's two forms
/// (M2 spec 2.3.12).
///
/// Deliberately without `Default`, as [`Contribution`]: an effect whose flaw
/// is inherent, in a faster ball or a shorter paddle, says so by adding
/// nothing and believing what it contributes.
#[derive(Debug)]
pub struct Flaw {
    /// First form: added to the spread of the AI's aim error, in arena units.
    pub added_error: f32,
    /// First form: added to the AI's reaction delay.
    pub added_delay: Duration,
    /// Second form: what the AI believes this effect contributes to the
    /// flight, in place of [`Effect::contribute`]'s answer.
    pub believed: Contribution,
}

/// What an AI of one difficulty makes of one flight: the [`Flaw`]s of the
/// effects in play, composed. Errors and delays compose by sum; the believed
/// contributions compose into a flight as the true ones do.
#[derive(Debug)]
pub struct FlightFlaw {
    /// Added to the spread of the AI's aim error, in arena units.
    pub added_error: f32,
    /// Added to the AI's reaction delay.
    pub added_delay: Duration,
    /// The flight the AI believes the ball is in: it predicts under this
    /// one, and the ball flies under the true one.
    pub believed: Flight,
}

/// The core's registration of every effect: the one place in this crate that
/// matches on [`EffectId`]. A new variant does not compile until it is built here.
fn build(id: EffectId, tuning: &Tuning, mode: Mode) -> Box<dyn Effect> {
    match id {
        EffectId::Speed => Box::new(speed::Speed::new(tuning.mode(mode).speed_rule())),
        EffectId::Shrink => Box::new(shrink::Shrink::new(tuning)),
    }
}

/// Wires every effect into the schedule (M2 spec 2.3.11), so an effect
/// writes systems only for state the base lacks.
///
/// - Makes [`BallFlight`] a required component of [`Ball`]. Precondition
///   (Bevy's): no `Ball` exists before this plugin builds.
/// - Which effects are in play is escalation's to decide, through
///   [`ActiveEffects::set`]; this plugin starts with none.
/// - Entering a countdown clears the ball's flight; the serve starts the
///   rally's first flight in the effects phase. Physics starts every later
///   flight at a paddle return, through [`ActiveEffects::start_flight`].
/// - The flight's paddle height factor reaches the paddles in the effects
///   phase (M2 spec 2.2.3).
/// - The [`Sandbox`] port starts empty; its held count reaches every flight
///   start, leaving a match releases it, and entering a match clears its
///   touched mark unless something was forced for that match beforehand.
pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveEffects>()
            .init_resource::<Sandbox>()
            .register_required_components::<Ball, BallFlight>()
            .add_systems(OnEnter(AppState::InMatch), begin_the_match_untouched)
            .add_systems(
                OnExit(AppState::InMatch),
                release_the_sandbox_with_the_match,
            )
            .add_systems(OnEnter(MatchState::Countdown), clear_flights)
            .add_systems(
                FixedUpdate,
                (
                    rebuild_effects_from_changed_tuning.in_set(FixedStepPhase::Effects),
                    hold_the_sandbox_count
                        .in_set(FixedStepPhase::Effects)
                        .before(start_flight_on_serve),
                    fit_paddles_to_the_flight.in_set(FixedStepPhase::Effects),
                    start_flight_on_serve
                        .in_set(FixedStepPhase::Effects)
                        .in_set(BallInPlay),
                ),
            );
    }
}

/// The effects in play, in no meaningful order: every channel composes
/// commutatively.
///
/// Invariant: `effects` holds exactly the effects `ids` names, each built
/// from the tuning last seen, for `mode`.
#[derive(Resource)]
pub struct ActiveEffects {
    mode: Mode,
    ids: Vec<EffectId>,
    effects: Vec<Box<dyn Effect>>,
    /// The [`Sandbox`]'s held count, copied here each step so that every
    /// flight start, the serve's and physics', passes through one place.
    held_returns: Option<u16>,
}

/// No effects in play: what holds outside a match.
impl Default for ActiveEffects {
    fn default() -> Self {
        Self {
            mode: Mode::Classic,
            ids: Vec::new(),
            effects: Vec::new(),
            held_returns: None,
        }
    }
}

impl ActiveEffects {
    /// Puts exactly `ids` in play, each built from `tuning` for `mode`.
    /// Escalation calls this; it takes hold at the next flight start.
    pub fn set(&mut self, ids: &[EffectId], tuning: &Tuning, mode: Mode) {
        self.mode = mode;
        self.ids = ids.to_vec();
        self.effects = ids.iter().map(|id| build(*id, tuning, mode)).collect();
    }

    /// Puts one more effect in play, as given: a test's probe, which no
    /// tuning builds.
    #[cfg(test)]
    pub(crate) fn add(&mut self, effect: Box<dyn Effect>) {
        self.ids.push(effect.id());
        self.effects.push(effect);
    }

    /// The effects in play.
    #[must_use]
    pub fn ids(&self) -> &[EffectId] {
        &self.ids
    }

    /// The flight that begins at `start`: every effect in play asked once
    /// for its contribution (M2 spec 2.2.4). While the [`Sandbox`] holds a
    /// count, the flight starts at that count instead of `start`'s.
    #[must_use]
    pub fn start_flight(&self, start: FlightStart) -> Flight {
        let start = FlightStart {
            returns: self.held_returns.unwrap_or(start.returns),
            ..start
        };
        Flight::start(
            start,
            self.effects.iter().map(|effect| effect.contribute(&start)),
        )
    }

    /// What an AI of `difficulty` makes of `flight`: every effect in play
    /// asked for its [`Flaw`] over the start `flight` began at.
    ///
    /// Precondition (the caller's): `flight` came from
    /// [`start_flight`](Self::start_flight) with these effects in play, as
    /// the ball's flight does within a rally. With no effect in play the
    /// answer adds nothing and believes an empty flight.
    ///
    /// Known gap, dev builds only: a tuning file reloaded in mid-flight
    /// rebuilds the effects at once, and the ball's flight keeps the
    /// contributions of the old ones until its next start. An AI that plans
    /// in between is told the new numbers about the old flight.
    #[must_use]
    pub fn flaw_over(&self, flight: &Flight, difficulty: Difficulty) -> FlightFlaw {
        let start = flight.began_at();
        let mut added_error = 0.0;
        let mut added_delay = Duration::ZERO;
        let mut believed = Vec::with_capacity(self.effects.len());
        for effect in &self.effects {
            let flaw = effect.flaw(&start, difficulty);
            added_error += flaw.added_error;
            added_delay = added_delay.saturating_add(flaw.added_delay);
            believed.push(flaw.believed);
        }
        FlightFlaw {
            added_error,
            added_delay,
            believed: flight.believed_as(believed),
        }
    }
}

/// Port: the developer sandbox (M2 spec 2.9.1). The shell's overlay writes
/// it; escalation, this feature, and persistence read it. It forces which
/// effects are in play and holds the return count every flight starts at, so
/// one effect can be looked at alone, at one dial.
///
/// Forcing keeps the simulation's invariants (M2 spec 2.4.3): a forced set
/// takes hold at the next countdown, a held count at the next flight start.
/// A match in which anything was forced is [touched](Self::touched_match)
/// and records nothing, since the overlay ships in release builds.
///
/// Forcing does not outlive its match: this feature releases the sandbox
/// when a match is left, so the next one plays and records as the game
/// decides. What is forced before a match is entered, as a test harness
/// does, holds for that match.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct Sandbox {
    /// The whole set in play while forced, in [`EffectId::ALL`]'s order.
    effects: Option<Vec<EffectId>>,
    returns: Option<u16>,
    touched_match: bool,
}

impl Sandbox {
    /// Forces exactly `effects` into play, the always-on effects included:
    /// what is not named is out. Kept in [`EffectId::ALL`]'s order, each
    /// effect once, whatever the order and repeats of `effects`.
    pub fn force_effects(&mut self, effects: &[EffectId]) {
        let forced = EffectId::ALL
            .into_iter()
            .filter(|id| effects.contains(id))
            .collect();
        self.effects = Some(forced);
        self.touched_match = true;
    }

    /// Puts `effect` in the forced set, or takes it out. The first toggle
    /// starts from `in_play`, so it changes one effect and keeps the rest.
    pub fn toggle(&mut self, effect: EffectId, in_play: &[EffectId]) {
        let base = self.effects.as_deref().unwrap_or(in_play);
        let toggled: Vec<EffectId> = EffectId::ALL
            .into_iter()
            .filter(|id| base.contains(id) != (*id == effect))
            .collect();
        self.force_effects(&toggled);
    }

    /// Holds every flight's return count at `returns`; `None` lets the rally
    /// count again, on from the held count.
    pub const fn hold_returns(&mut self, returns: Option<u16>) {
        self.returns = returns;
        self.touched_match = true;
    }

    /// Forces nothing any more. The match stays touched.
    pub fn release(&mut self) {
        self.effects = None;
        self.returns = None;
    }

    /// The effects forced into play; `None` while the schedule decides.
    #[must_use]
    pub fn forced_effects(&self) -> Option<&[EffectId]> {
        self.effects.as_deref()
    }

    /// The count every flight starts at; `None` while the rally counts.
    #[must_use]
    pub const fn held_returns(&self) -> Option<u16> {
        self.returns
    }

    /// Whether anything is forced now: a set of effects, a held count, or both.
    #[must_use]
    pub const fn is_forcing(&self) -> bool {
        self.effects.is_some() || self.returns.is_some()
    }

    /// Whether anything was forced at any time in the current match.
    #[must_use]
    pub const fn touched_match(&self) -> bool {
        self.touched_match
    }
}

/// The flight a ball is in: the rally's return count, the flight's clock, and
/// the composed contributions of the effects in play.
///
/// Invariant: the flight is replaced only at a flight start, by
/// [`begin`](Self::begin), and cleared only between rallies, by
/// [`clear`](Self::clear); in between, only its clock advances. While
/// cleared, the flight is the empty one.
#[derive(Component, Debug, Default)]
pub struct BallFlight {
    flight: Flight,
    /// Whether a serve or a return began `flight`. A begun flight can be as
    /// empty as the cleared one, with no effect in play, so the flight's
    /// content cannot tell.
    begun: bool,
}

impl BallFlight {
    /// The flight, for physics and the view to read: the empty flight while
    /// the ball is in none.
    #[must_use]
    pub const fn flight(&self) -> &Flight {
        &self.flight
    }

    /// The flight the ball is in, or `None` between rallies: from a
    /// countdown's start until the serve's flight begins, which is one
    /// phase after the serve launches the ball.
    #[must_use]
    pub const fn in_flight(&self) -> Option<&Flight> {
        if self.begun { Some(&self.flight) } else { None }
    }

    /// Starts a new flight in place of the old.
    pub fn begin(&mut self, flight: Flight) {
        self.flight = flight;
        self.begun = true;
    }

    /// Ends the rally's last flight: the ball is in none until the next
    /// [`begin`](Self::begin).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Advances the flight's clock by one fixed step.
    pub const fn tick(&mut self) {
        self.flight.tick();
    }
}

/// A new match is untouched by the sandbox, unless something was forced for
/// it before it was entered: then the match is touched from its first step.
fn begin_the_match_untouched(mut sandbox: ResMut<Sandbox>) {
    let forcing = sandbox.is_forcing();
    if sandbox.touched_match != forcing {
        sandbox.touched_match = forcing;
    }
}

/// Forcing ends with its match: the next match plays, and records, as the
/// game decides, with no key pressed to make it so.
///
/// Runs in `OnExit(InMatch)`: in the transition that leaves the match,
/// before the next state is entered, and not when the app is dropped in a
/// match. Bevy 0.20.1 also runs it for a transition from `InMatch` to
/// `InMatch` requested with `NextState::set`; nothing requests one today,
/// and a rematch that did would start unforced.
fn release_the_sandbox_with_the_match(mut sandbox: ResMut<Sandbox>) {
    if sandbox.is_forcing() {
        sandbox.release();
    }
}

/// Copies the sandbox's held count to where flights start. Writes only on a
/// change, so the effects in play are not marked changed every step.
fn hold_the_sandbox_count(sandbox: Res<Sandbox>, mut active: ResMut<ActiveEffects>) {
    if active.held_returns != sandbox.returns {
        active.held_returns = sandbox.returns;
    }
}

/// A reloaded tuning file (dev override) reaches the effects in play: each is
/// rebuilt from the new values before the next flight starts.
fn rebuild_effects_from_changed_tuning(mut active: ResMut<ActiveEffects>, tuning: Res<Tuning>) {
    if !tuning.is_changed() {
        return;
    }
    let ids = active.ids().to_vec();
    let mode = active.mode;
    active.set(&ids, &tuning, mode);
}

/// Sets both paddles to the default height times the flight's factor
/// (M2 spec 2.2.3), through the paddle's clamped setter.
///
/// Runs every step and writes only when the factor has changed since it last
/// wrote, so a paddle's height is otherwise its own: a tuning reload still
/// refits paddles as the paddle feature decides. A flight starts in the
/// collide phase, so the paddles follow one step later, with the ball a
/// flight away. The cleared flight of a countdown has factor one: the paddles
/// are whole for every serve.
fn fit_paddles_to_the_flight(
    tuning: Res<Tuning>,
    flights: Query<&BallFlight, With<Ball>>,
    mut paddles: Query<&mut Paddle>,
    mut applied: Local<Option<u32>>,
) {
    let Some(flight) = flights.iter().next() else {
        return;
    };
    let factor = flight.flight().paddle_height_factor();
    // Bit equality: "the very factor last applied", not a numeric comparison.
    if *applied == Some(factor.to_bits()) {
        return;
    }
    *applied = Some(factor.to_bits());
    let shape = tuning.paddle();
    for mut paddle in &mut paddles {
        paddle.set_height(
            shape.height_default() * factor,
            shape.height_bounds(),
            tuning.arena(),
        );
    }
}

/// Every countdown starts a new rally with the ball at rest (ADR 0003): no
/// returns yet, no forces.
fn clear_flights(mut flights: Query<&mut BallFlight, With<Ball>>) {
    for mut flight in &mut flights {
        flight.clear();
    }
}

/// The serve starts the rally's first flight. The ball feature launches the
/// ball in the intent phase; this runs in the effects phase of the same step,
/// before the ball first moves.
fn start_flight_on_serve(
    mut serves: MessageReader<Serve>,
    active: Res<ActiveEffects>,
    mut flights: Query<&mut BallFlight, With<Ball>>,
) {
    if serves.read().last().is_none() {
        return;
    }
    for mut flight in &mut flights {
        flight.begin(active.start_flight(FlightStart::SERVE));
    }
}

#[cfg(test)]
pub(crate) mod probe;

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use bevy::app::App;
    use bevy::ecs::query::With;
    use bevy::math::Vec2;
    use bevy::state::state::NextState;

    use super::probe::Misjudged;
    use super::{ActiveEffects, BallFlight, Effect, Flaw, Sandbox};
    use crate::domain::{
        BallHitPaddle, Controller, Difficulty, EffectId, MatchConfig, Mode, Seat, SeatPlayers,
    };
    use crate::features::ball::{Ball, Position, Velocity};
    use crate::features::paddle::Paddle;
    use crate::features::seats::PaddleIntent;
    use crate::features::tuning::{EMBEDDED_RON, Tuning};
    use crate::flow::states::MatchState;
    use crate::kernel::flight::{Contribution, Field, FlightStart, Hit, StepMotion};
    use crate::kernel::geometry::Side;
    use crate::kernel::integrate::Motion;
    use crate::kernel::predict::crossing_y;
    use crate::kernel::test_support::assert_near;
    use crate::testing::{
        ball_state, boot, enter_match, fixed_timestep, frame, headless_app, leave_match,
        match_state, place_ball, record, report_intent, run_until, start_match, take_recorded,
    };

    fn two_humans() -> MatchConfig {
        MatchConfig {
            left: Controller::Human(Seat::Left),
            right: Controller::Human(Seat::Right),
            players: SeatPlayers::GUESTS,
            points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
            mode: Mode::Classic,
        }
    }

    fn playing() -> App {
        let mut app = headless_app(1);
        record::<BallHitPaddle>(&mut app);
        boot(&mut app);
        start_match(&mut app, two_humans());
        app
    }

    /// Into the right paddle's face within one step, whatever the speed named.
    fn into_the_right_paddle(app: &mut App, velocity: Vec2) {
        place_ball(app, Vec2::new(72.0, 2.0), velocity);
        frame(app);
    }

    fn returns(app: &mut App) -> u16 {
        app.world_mut()
            .query_filtered::<&BallFlight, With<Ball>>()
            .single(app.world())
            .expect("exactly one ball")
            .flight()
            .returns()
    }

    /// The speed a ball leaves its `returns`-th return at, by the tuning in `app`.
    fn launch_speed(app: &App, returns: u16) -> f32 {
        let tuning = app.world().resource::<Tuning>();
        let rule = tuning.mode(Mode::Classic).speed_rule();
        rule.launch(
            tuning.ball().serve_speed(),
            rule.per_hit_increase() * f32::from(returns),
        )
    }

    /// M2 spec 2.9.1: the sandbox forces the hit count. A held count is the
    /// count every flight starts at, the serve's too, so an effect's dial
    /// stays where the developer put it.
    #[test]
    fn a_held_return_count_is_the_count_every_flight_starts_at() {
        let mut app = playing();
        app.world_mut()
            .resource_mut::<Sandbox>()
            .hold_returns(Some(9));
        assert_eq!(returns(&mut app), 0, "a flight keeps its count");
        into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        assert_eq!(returns(&mut app), 9);
        into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        assert_eq!(returns(&mut app), 9, "the next return counted on");

        app.world_mut()
            .resource_mut::<NextState<MatchState>>()
            .set(MatchState::Countdown);
        // The request applies in the next frame; only then wait for play.
        frame(&mut app);
        assert_eq!(match_state(&app), MatchState::Countdown);
        assert_eq!(returns(&mut app), 0, "a countdown has no flight to hold");
        run_until(
            &mut app,
            |app| match_state(app) == MatchState::Playing,
            1_000,
            "play never began again",
        );
        for _ in 0..3 {
            frame(&mut app);
        }
        assert_eq!(returns(&mut app), 9, "the serve started at zero");
    }

    /// Letting go of the count: the rally counts on from where it was held.
    #[test]
    fn a_released_return_count_counts_on_from_the_held_one() {
        let mut app = playing();
        app.world_mut()
            .resource_mut::<Sandbox>()
            .hold_returns(Some(9));
        into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        app.world_mut().resource_mut::<Sandbox>().hold_returns(None);
        into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        assert_eq!(returns(&mut app), 10);
    }

    /// The first toggle starts from what is in play, so one key press
    /// changes one effect; the forced list keeps the enum's order.
    #[test]
    fn toggling_starts_from_the_effects_in_play_and_changes_one() {
        let mut sandbox = Sandbox::default();
        assert_eq!(sandbox.forced_effects(), None);
        sandbox.toggle(EffectId::Shrink, &[EffectId::Speed]);
        assert_eq!(
            sandbox.forced_effects(),
            Some([EffectId::Speed, EffectId::Shrink].as_slice())
        );
        // What is in play no longer matters: the forced list is the base.
        sandbox.toggle(EffectId::Speed, &[]);
        assert_eq!(
            sandbox.forced_effects(),
            Some([EffectId::Shrink].as_slice())
        );
        sandbox.toggle(EffectId::Shrink, &[]);
        assert_eq!(
            sandbox.forced_effects(),
            Some([].as_slice()),
            "no effect at all is a forced set too"
        );
    }

    /// The whole set at once, for a harness that knows the set it wants:
    /// the list is kept in the enum's order, each effect once.
    #[test]
    fn forcing_a_set_puts_exactly_that_set_in_play_and_touches_the_match() {
        let mut sandbox = Sandbox::default();
        sandbox.force_effects(&[EffectId::Shrink, EffectId::Speed, EffectId::Shrink]);
        assert_eq!(
            sandbox.forced_effects(),
            Some([EffectId::Speed, EffectId::Shrink].as_slice())
        );
        assert!(sandbox.touched_match());
        sandbox.force_effects(&[]);
        assert_eq!(
            sandbox.forced_effects(),
            Some([].as_slice()),
            "no effect at all is a forced set too"
        );
    }

    #[test]
    fn releasing_the_sandbox_forces_nothing_but_the_match_stays_touched() {
        let mut sandbox = Sandbox::default();
        assert!(!sandbox.touched_match());
        sandbox.toggle(EffectId::Shrink, &[]);
        sandbox.hold_returns(Some(4));
        sandbox.release();
        assert_eq!(
            (sandbox.forced_effects(), sandbox.held_returns()),
            (None, None)
        );
        assert!(sandbox.touched_match());
    }

    /// A new match is untouched unless something was forced for it before
    /// it was entered: a held count, or a set of effects, each on its own.
    #[test]
    fn a_new_match_is_touched_only_if_something_was_forced_for_it_beforehand() {
        let touched = |app: &App| app.world().resource::<Sandbox>().touched_match();
        let forcings: [fn(&mut Sandbox); 2] = [
            |sandbox| sandbox.hold_returns(Some(3)),
            |sandbox| sandbox.force_effects(&[EffectId::Speed]),
        ];
        for force in forcings {
            let mut app = headless_app(1);
            boot(&mut app);
            force(&mut app.world_mut().resource_mut::<Sandbox>());
            start_match(&mut app, two_humans());
            assert!(touched(&app), "forced before the match was entered");

            leave_match(&mut app);
            start_match(&mut app, two_humans());
            assert!(!touched(&app), "nothing is forced any more");
        }
    }

    /// The user's ruling after the phase 3 and 4 review: forcing does not
    /// outlive its match, so no one has to remember to let go of it.
    #[test]
    fn leaving_a_match_releases_what_the_sandbox_forces() {
        let mut app = playing();
        let mut sandbox = app.world_mut().resource_mut::<Sandbox>();
        sandbox.force_effects(&[EffectId::Shrink]);
        sandbox.hold_returns(Some(9));
        leave_match(&mut app);
        let sandbox = app.world().resource::<Sandbox>();
        assert_eq!(
            (sandbox.forced_effects(), sandbox.held_returns()),
            (None, None)
        );
        assert!(!sandbox.is_forcing());
    }

    /// It borrows Speed's name: the identity enum is closed, and a probe is
    /// no effect of the game.
    struct Probe(Arc<Mutex<Vec<FlightStart>>>);

    impl Effect for Probe {
        fn id(&self) -> EffectId {
            EffectId::Speed
        }

        fn contribute(&self, start: &FlightStart) -> Contribution {
            self.0
                .lock()
                .expect("the probe's log is not poisoned")
                .push(*start);
            Contribution {
                speed_gain: 0.0,
                paddle_height_factor: 1.0,
                field: None,
            }
        }

        fn flaw(&self, _start: &FlightStart, _difficulty: Difficulty) -> Flaw {
            Flaw {
                added_error: 0.0,
                added_delay: Duration::ZERO,
                believed: Contribution {
                    speed_gain: 0.0,
                    paddle_height_factor: 1.0,
                    field: None,
                },
            }
        }
    }

    #[test]
    fn a_match_puts_speed_alone_in_play() {
        let app = playing();
        assert_eq!(
            app.world().resource::<ActiveEffects>().ids(),
            [EffectId::Speed]
        );
    }

    /// M2 spec 2.2.4's done-condition: asked exactly once per flight, at the
    /// serve and at each return, and told the paddle's motion at a hit.
    #[test]
    fn an_effect_is_asked_once_per_flight_and_sees_the_paddles_motion_at_a_hit() {
        let mut app = headless_app(1);
        boot(&mut app);
        enter_match(&mut app, two_humans());
        let asked = Arc::new(Mutex::new(Vec::new()));
        app.world_mut()
            .resource_mut::<ActiveEffects>()
            .add(Box::new(Probe(Arc::clone(&asked))));
        run_until(
            &mut app,
            |app| match_state(app) == MatchState::Playing,
            1_000,
            "play never began",
        );
        for _ in 0..5 {
            frame(&mut app);
        }
        let seen = |asked: &Arc<Mutex<Vec<FlightStart>>>| {
            asked
                .lock()
                .expect("the probe's log is not poisoned")
                .clone()
        };
        assert_eq!(
            seen(&asked),
            [FlightStart::SERVE],
            "the serve starts one flight"
        );

        report_intent(&mut app, Seat::Right, PaddleIntent::Up);
        into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        let first_return = FlightStart {
            returns: 1,
            hit: Some(Hit {
                side: Side::Right,
                paddle_motion: StepMotion::Up,
            }),
        };
        assert_eq!(seen(&asked), [FlightStart::SERVE, first_return]);

        report_intent(&mut app, Seat::Right, PaddleIntent::Still);
        for _ in 0..5 {
            frame(&mut app);
        }
        assert_eq!(seen(&asked).len(), 2, "nothing is asked in mid-flight");
    }

    fn in_flight(app: &mut App) -> bool {
        app.world_mut()
            .query_filtered::<&BallFlight, With<Ball>>()
            .single(app.world())
            .expect("exactly one ball")
            .in_flight()
            .is_some()
    }

    /// The AI plans an approach under the ball's flight, so it must tell a
    /// ball waiting for its flight from one flying in an empty flight.
    #[test]
    fn the_ball_is_in_no_flight_until_the_serve_begins_one_and_in_none_after_the_rally() {
        let mut app = headless_app(1);
        boot(&mut app);
        enter_match(&mut app, two_humans());
        assert!(!in_flight(&mut app), "the countdown before the first serve");
        run_until(
            &mut app,
            |app| match_state(app) == MatchState::Playing,
            1_000,
            "play never began",
        );
        for _ in 0..3 {
            frame(&mut app);
        }
        assert!(in_flight(&mut app), "the serve began a flight");
        into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        assert!(in_flight(&mut app), "a return begins the next");

        app.world_mut()
            .resource_mut::<NextState<MatchState>>()
            .set(MatchState::Countdown);
        frame(&mut app);
        assert_eq!(match_state(&app), MatchState::Countdown);
        assert!(!in_flight(&mut app), "the rally is over");
    }

    #[test]
    fn each_return_counts_once_and_a_countdown_starts_the_count_again() {
        let mut app = playing();
        assert_eq!(returns(&mut app), 0);
        into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        assert_eq!(returns(&mut app), 1);
        frame(&mut app);
        assert_eq!(returns(&mut app), 1, "a flight keeps its count");

        app.world_mut()
            .resource_mut::<NextState<MatchState>>()
            .set(MatchState::Countdown);
        frame(&mut app);
        assert_eq!(match_state(&app), MatchState::Countdown);
        assert_eq!(returns(&mut app), 0);
    }

    /// M2 spec 2.0.4's done-condition: the launch speed follows the return
    /// count, not the speed the ball arrived at.
    #[test]
    fn a_ball_given_extra_speed_in_flight_leaves_its_next_hit_at_the_counts_speed() {
        let mut leaving = Vec::new();
        for arriving in [Vec2::new(80.0, 0.0), Vec2::new(150.0, 90.0)] {
            let mut app = playing();
            into_the_right_paddle(&mut app, arriving);
            assert_eq!(
                take_recorded::<BallHitPaddle>(&mut app).len(),
                1,
                "{arriving:?}"
            );
            assert_near(ball_state(&mut app).1.length(), launch_speed(&app, 1), 1e-3);
            leaving.push(ball_state(&mut app).1.length());
        }
        assert_near(leaving[0], leaving[1], 1e-3);
        assert_near(leaving[0], 87.0, 1e-3);
    }

    /// A ball that clips a paddle's end is touched, not returned: it flies on
    /// in the same flight at the same speed. M1 raised the speed on a face
    /// hit only; the return count keeps that.
    #[test]
    fn a_clip_on_a_paddles_end_is_a_touch_but_starts_no_flight() {
        let mut app = playing();
        // The right paddle spans x 74..76 and y -7..7: straight down onto its top end.
        place_ball(&mut app, Vec2::new(75.0, 9.0), Vec2::new(0.0, -200.0));
        frame(&mut app);
        assert_eq!(
            take_recorded::<BallHitPaddle>(&mut app),
            vec![BallHitPaddle { side: Side::Right }]
        );
        assert_eq!(returns(&mut app), 0);
        let (_, velocity) = ball_state(&mut app);
        assert!(
            velocity.y > 0.0,
            "the end mirrors the ball back up: {velocity:?}"
        );
        assert_near(velocity.length(), 200.0, 1e-3);
    }

    /// A reloaded tuning file (dev override) reaches the effects in play.
    #[test]
    fn a_reloaded_increase_reaches_speed_before_the_next_return() {
        let mut app = playing();
        let steeper = EMBEDDED_RON.replace("per_hit_increase: 7.0", "per_hit_increase: 20.0");
        assert_ne!(
            steeper, EMBEDDED_RON,
            "the increase was not found in the RON"
        );
        app.insert_resource(Tuning::from_ron(&steeper).expect("a steeper ramp is valid"));
        into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        assert_near(ball_state(&mut app).1.length(), 100.0, 1e-3);
    }

    /// A push that swings with time in flight: wrong unless the clock runs.
    #[derive(Debug)]
    struct Swing;

    impl Field for Swing {
        fn lateral(&self, _motion: Motion, seconds_in_flight: f32) -> f32 {
            60.0 * (4.0 * seconds_in_flight).cos()
        }
    }

    /// Contributes [`Swing`] to every flight, under Speed's borrowed name.
    struct Swinger;

    impl Effect for Swinger {
        fn id(&self) -> EffectId {
            EffectId::Speed
        }

        fn contribute(&self, _start: &FlightStart) -> Contribution {
            Contribution {
                speed_gain: 0.0,
                paddle_height_factor: 1.0,
                field: Some(Box::new(Swing)),
            }
        }

        fn flaw(&self, start: &FlightStart, _difficulty: Difficulty) -> Flaw {
            Flaw {
                added_error: 0.0,
                added_delay: Duration::ZERO,
                believed: self.contribute(start),
            }
        }
    }

    /// M2 spec 2.2.1 through the systems: the physics step applies the
    /// flight's force and runs its clock, so the ball goes where the kernel
    /// predicts from the same flight.
    #[test]
    fn under_a_field_the_ball_crosses_where_the_kernel_predicts_from_its_flight() {
        let mut app = playing();
        app.world_mut()
            .resource_mut::<ActiveEffects>()
            .add(Box::new(Swinger));
        // The return starts a flight that carries the field; the ball leaves leftward.
        into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        for _ in 0..10 {
            frame(&mut app);
        }
        let tuning = app.world().resource::<Tuning>().clone();
        let dt = fixed_timestep(&app).as_secs_f32();
        let target_x = -60.0;
        let predicted = {
            let mut query = app
                .world_mut()
                .query_filtered::<(&Position, &Velocity, &BallFlight), With<Ball>>();
            let (position, velocity, flight) = query.single(app.world()).expect("exactly one ball");
            let now = Motion {
                position: position.0,
                velocity: velocity.0,
            };
            crossing_y(
                tuning.arena(),
                tuning.ball().radius(),
                now,
                flight.flight(),
                target_x,
                dt,
            )
            .expect("the ball flies toward the target")
        };
        let unforced = {
            let (position, velocity) = ball_state(&mut app);
            position.y + velocity.y * (target_x - position.x) / velocity.x
        };
        assert!(
            (predicted - unforced).abs() > 3.0,
            "the field bends the course: {predicted} against {unforced}"
        );
        let mut before = ball_state(&mut app).0;
        for _ in 0..600 {
            frame(&mut app);
            let after = ball_state(&mut app).0;
            if after.x <= target_x {
                let fraction = (target_x - before.x) / (after.x - before.x);
                assert_near(before.y + fraction * (after.y - before.y), predicted, 1e-2);
                return;
            }
            before = after;
        }
        panic!("the ball never reached x = {target_x}");
    }

    fn paddle_heights(app: &mut App) -> Vec<f32> {
        app.world_mut()
            .query::<&Paddle>()
            .iter(app.world())
            .map(|paddle| paddle.height())
            .collect()
    }

    /// A match in play with Shrink beside Speed, as escalation would set it.
    fn playing_with_shrink() -> App {
        let mut app = playing();
        let tuning = app.world().resource::<Tuning>().clone();
        app.world_mut().resource_mut::<ActiveEffects>().set(
            &[EffectId::Speed, EffectId::Shrink],
            &tuning,
            Mode::Classic,
        );
        app
    }

    /// M2 spec 2.6.1's done-condition: both paddles reach the tuned minimum
    /// at full dial and are whole again for the next rally. The embedded
    /// dial starts to move after return 3 and is full at return 15.
    #[test]
    fn both_paddles_shrink_to_the_minimum_at_full_dial_and_restore_for_the_next_rally() {
        let mut app = playing_with_shrink();
        for _ in 0..3 {
            into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        }
        frame(&mut app);
        assert_eq!(
            paddle_heights(&mut app),
            [14.0, 14.0],
            "the dial has not started"
        );

        for _ in 3..9 {
            into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        }
        frame(&mut app);
        // Half way from return 3 to return 15: half way from 14 to 8.
        for height in paddle_heights(&mut app) {
            assert_near(height, 11.0, 1e-4);
        }

        for _ in 9..15 {
            into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        }
        frame(&mut app);
        for height in paddle_heights(&mut app) {
            assert_near(height, 8.0, 1e-4);
        }
        into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        frame(&mut app);
        for height in paddle_heights(&mut app) {
            assert_near(height, 8.0, 1e-4);
        }

        app.world_mut()
            .resource_mut::<NextState<MatchState>>()
            .set(MatchState::Countdown);
        frame(&mut app);
        frame(&mut app);
        assert_eq!(match_state(&app), MatchState::Countdown);
        assert_eq!(paddle_heights(&mut app), [14.0, 14.0]);
    }

    /// The paddles follow the flight one step after the return that starts it.
    #[test]
    fn the_paddles_take_a_flights_height_one_step_after_it_starts() {
        let mut app = playing_with_shrink();
        for _ in 0..9 {
            into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        }
        let before = paddle_heights(&mut app);
        frame(&mut app);
        let after = paddle_heights(&mut app);
        assert!(
            after[0] < before[0] && after[1] < before[1],
            "{before:?} then {after:?}"
        );
    }

    /// Without Shrink in play the paddles keep their height however long the rally.
    #[test]
    fn with_speed_alone_the_paddles_keep_their_height() {
        let mut app = playing();
        for _ in 0..20 {
            into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        }
        frame(&mut app);
        assert_eq!(paddle_heights(&mut app), [14.0, 14.0]);
    }

    const DT: f32 = 1.0 / 60.0;

    fn level_ball() -> Motion {
        Motion {
            position: Vec2::ZERO,
            velocity: Vec2::new(60.0, 0.0),
        }
    }

    /// Classic's AI: nothing in play adds to the star's own handicap.
    #[test]
    fn with_no_effect_in_play_a_flight_is_read_without_flaw() {
        let active = ActiveEffects::default();
        let flight = active.start_flight(FlightStart::SERVE);
        let flaw = active.flaw_over(&flight, Difficulty::OneStar);
        assert_near(flaw.added_error, 0.0, 1e-6);
        assert_eq!(flaw.added_delay, Duration::ZERO);
        assert_eq!(flaw.believed.force(level_ball(), DT), Vec2::ZERO);
    }

    /// M2 spec 2.3.12, first form: added error and delay, composed by sum.
    #[test]
    fn the_flaws_of_the_effects_in_play_add_their_errors_and_their_delays() {
        let mut active = ActiveEffects::default();
        active.add(Box::new(Misjudged::costing(
            2.0,
            Duration::from_millis(100),
        )));
        active.add(Box::new(Misjudged::costing(
            3.0,
            Duration::from_millis(200),
        )));
        let flight = active.start_flight(FlightStart::SERVE);
        let flaw = active.flaw_over(&flight, Difficulty::TwoStars);
        assert_near(flaw.added_error, 5.0, 1e-6);
        assert_eq!(flaw.added_delay, Duration::from_millis(300));
    }

    /// M2 spec 2.3.12, second form: the AI predicts under the flight it
    /// believes in, which starts and ticks as the true one does.
    #[test]
    fn the_believed_flight_pushes_as_each_effect_is_believed_to_and_keeps_the_clock() {
        let mut active = ActiveEffects::default();
        active.add(Box::new(Misjudged::pushing(8.0, Some(6.0))));
        active.add(Box::new(Misjudged::pushing(5.0, None)));
        let start = FlightStart {
            returns: 4,
            hit: None,
        };
        let mut flight = active.start_flight(start);
        for _ in 0..7 {
            flight.tick();
        }
        let flaw = active.flaw_over(&flight, Difficulty::TwoStars);
        assert_eq!(
            flight.force(level_ball(), DT),
            Vec2::new(0.0, 13.0),
            "the ball feels both pushes"
        );
        assert_eq!(
            flaw.believed.force(level_ball(), DT),
            Vec2::new(0.0, 6.0),
            "the AI underrates one and misses the other"
        );
        assert_eq!(flaw.believed.began_at(), start);
        assert_eq!(flaw.believed.steps(), 7);
    }

    /// A flaw may depend on the dial and on the stars: each effect is asked
    /// with the start its flight began at and the difficulty of the AI.
    #[test]
    fn an_effect_is_asked_for_its_flaw_with_the_flights_start_and_the_ais_difficulty() {
        let probe = Misjudged::costing(0.0, Duration::ZERO);
        let asked = Arc::clone(&probe.asked);
        let mut active = ActiveEffects::default();
        active.add(Box::new(probe));
        let start = FlightStart {
            returns: 9,
            hit: Some(Hit {
                side: Side::Left,
                paddle_motion: StepMotion::Down,
            }),
        };
        let flight = active.start_flight(start);
        assert!(
            asked
                .lock()
                .expect("the probe's log is not poisoned")
                .is_empty(),
            "starting a flight asks for no flaw"
        );
        let _ = active.flaw_over(&flight, Difficulty::ThreeStars);
        assert_eq!(
            *asked.lock().expect("the probe's log is not poisoned"),
            [(start, Difficulty::ThreeStars)]
        );
    }
}

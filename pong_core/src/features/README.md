# features

One directory per feature, each with a plugin at its root.

## Order and imports

The order is declared in [`mod.rs`](mod.rs) (ADR 0002):
`tuning, seats, ball, paddle, effects, physics, rules, escalation, ai, sanity, persistence`.
A feature may import the public components, resources, and messages of an earlier feature;
never its systems, never a later feature, never the app roof.
`just arch-check` enforces the order by grep,
and that an effect's module imports the effects base and never another effect (ADR 0011).

Every feature has one shape:
its plugin registers its resources and messages,
its systems join a `FixedStepPhase`, an `OnEnter`/`OnTransition` schedule, or `Update`
(tuning, rules, and persistence run there; tuning's load also starts in `Startup`),
and its public types are what later features may use.

## tuning

One embedded RON file of every number, validated once, so no system re-checks a value.

| Stage | Where | What |
|---|---|---|
| Raw shape | `file.rs` | Plain numbers. `#[serde(deny_unknown_fields)]`: a misspelt key is an error, not a silent default. |
| Field checks | `Tuning::from_ron` | Each section through kernel constructors; a refusal names its path, e.g. `ball.radius`. |
| Cross-field checks | `check_everything_fits_the_arena` | Paddle not taller than the arena; ball fits; paddles short of the centre line; speed cap under one playable height per step; Wind's `max_drift` and Spin's each under half the playable height. Bindings reject duplicates and `Escape`. |

`Tuning` has private fields and one constructor: holding one means it passed.
`crossing_width()` is the distance between the two contact lines, the one formula `Paddle::contact_x`, Wind's drift, and Spin's all read.

Loading goes through Bevy's asset system:
`embedded_asset!` compiles `tuning.ron` into the binary under the `embedded://` source;
`begin_loading` (`Startup`) calls `AssetServer::load`, which returns a `Handle` at once and parses on the IO task pool through `TuningLoader`;
Bevy inserts the result into `Assets<Tuning>` in `PreUpdate`;
`promote_when_loaded` copies it into the `Tuning` resource, running only while the resource is absent.
A failed embedded load goes to the error handler.

`Tuning` is both an asset and a resource.
The asset side gives asynchronous loading and hot reload;
the resource side gives systems a `Res<Tuning>` that, after `Boot`, always exists and is whole.
`EMBEDDED_RON` keeps the same bytes as a constant, so CI validates the file synchronously.

Dev hot reload (`dev` feature, `embedded_watcher`):
an accepted file raises `AssetEvent::Modified`, and `adopt_reloaded_tuning` re-inserts the resource, which counts as a change for `is_changed()`;
a refused file never reaches `Assets`, so the last good values stay by construction,
and `notice_refused_override` writes one `Notice`.
`ShownRefusal` exists because Bevy 0.20.1 reports one refused reload twice.

The watcher watches `BEVY_ASSET_ROOT`, else `CARGO_MANIFEST_DIR`, which under `cargo run -p pong` is `pong/` and misses `tuning.ron`.
So `just run` sets `BEVY_ASSET_ROOT` to `pong_core/`; plan 3.9's manual check passed with it (2026-11-07).

## seats

The input port: the shell writes, the core reads.
The core has no window or input plugin, so a resource stands between them.
An AI or a test writes the same output with no keyboard.

`SeatIntents` (the port) keeps two things per seat,
because frames run at the display rate and fixed steps at 60 Hz:

| Field | Set by `report` | Read by `consume` | Covers |
|---|---|---|---|
| `held` | Every report | Every step, while not `Still` | A held key moves the paddle on every step, however many steps a frame runs. |
| `tapped` | Any non-still report | Once, then cleared | A press shorter than a frame that ran no step still moves the paddle once. |

`resolve_human_intents` (Intent) maps each human seat onto its side in `PaddleIntents`;
the AI's `steer_ai_paddles` (Intent) writes the AI sides there.
`forget_reported_intents` (`OnEnter(Countdown)`) clears the port,
so a menu key press never becomes a serve's first move.

Shell contract: report once per frame while `Playing`, before the fixed steps;
report a key pressed and released within one frame as held for that frame;
call `SeatIntents::clear` on focus loss, after that frame's report.
The shell's adapter is `pong::ports`.

## ball

The ball's components, its lifetime over a match, and the serve.
Movement belongs to physics.

| Component | Meaning |
|---|---|
| `Ball` | Marker for queries. |
| `Position`, `Velocity` | Centre in arena units; units per second. Not `Transform`: the shell derives that, interpolating between steps. |
| `Snap` | The body jumped; the view must not interpolate. Removed by the view. |

| System | Runs | Does |
|---|---|---|
| `spawn_ball` | `OnEnter(InMatch)` | Spawns the ball at rest with `Snap` and `DespawnOnExit(InMatch)`; Bevy despawns it in the transition leaving the match. |
| `place_ball_on_centre_spot` | `OnEnter(Countdown)` | Centre, zero velocity, `Snap`. |
| `serve_on_request` | Intent | Reads `Serve` (`read().last()` consumes all, acts on the newest) and sets velocity at a seeded angle within the serve cone. |

## paddle

`Paddle` is a component that keeps its own invariants:
height within the tuning bounds, the whole paddle inside the walls,
and a step motion that is the displacement its last move made.
Its fields are private; `new`, `set_height`, and `move_by` maintain them.
`step_motion()` reports what the paddle did, not what was asked:
a paddle pressed against a wall is `Still` (M2 spec 2.0.3).
The paddle moves in Integrate, so the motion is read in Collide or later.
Thickness and inset are read from tuning at use by `rect(&Tuning)`, so a reload applies at once.

| System | Runs | Does |
|---|---|---|
| `spawn_paddles` | `OnEnter(InMatch)` | One per side, `DespawnOnExit(InMatch)`. |
| `fit_paddles_to_changed_tuning` | Integrate, before `move_paddles` (`.chain()`) | When `Tuning` changed since this system last ran, re-clamps every paddle. Idempotent, so its first run, which always sees a change, is harmless. |
| `move_paddles` | Integrate | Moves by intent × speed × fixed delta. |

Writing through `Query<&mut Paddle>` marks the component changed,
which the view can use to know a paddle moved.

## effects

The contract every effect implements, the effects in play, and the flight they compose on the ball
(ADR 0011, M2 spec 2.2 and 2.3).

An effect never touches the simulation.
At each flight start, a serve or a paddle return, it is asked once for a `Contribution`:
its stance on every channel.
The base composes the answers into a `Flight` (`kernel::flight`), which sits on the ball.
Physics integrates under it and the AI predicts through the same math, so both see the same forces.
Nothing changes in mid-flight but the flight's clock.

An effect also states its `Flaw`: how an AI of each difficulty misjudges it (M2 spec 2.3.12).
The flaw has two forms. The first adds to the star's own handicap: aim error and reaction delay.
The second is a belief: the `Contribution` the AI thinks the effect makes, in place of the true one.
The AI predicts under the flight composed from the beliefs, and the ball flies under the true one.
An effect whose flaw is inherent, Speed's faster ball or Shrink's shorter paddle, adds nothing and is believed as it is.
Wind's is the second form alone: the AI under-leads by the share of the drift it does not believe in.
Spin's is the same form: the AI commits its aim at the turn, having read only its star's share of the spin.

A rally has one seed (`RallySeed`, kernel), drawn from `Random`'s own stream of rally seeds before the rally's countdown.
An effect that chooses something once per rally derives it from the seed and draws nothing itself:
an effect value holds no state of its own, since the effects in play are built again at every countdown and on a tuning reload.
The seed is drawn on entering a match and on the transition from play to a point; a pause, a re-entered countdown, and a fault restart draw none.

| Piece | Shape and reason |
|---|---|
| `Effect` | A trait with no default methods: `id`, `contribute`, and `flaw`. An effect that skips a part does not compile. |
| `Flaw` | A struct without `Default`: `added_error`, `added_delay`, and `believed`, a `Contribution`. One effect's share of an AI's misjudgement of one flight. |
| `FlightFlaw` | The flaws of the effects in play, composed: errors and delays by sum, beliefs into a `Flight` with the true flight's start and clock (`Flight::believed_as`). |
| `Contribution` (kernel) | A struct without `Default`. Adding a channel adds a field, and every effect stops compiling until it states its stance. |
| `build` | The crate's one match on `EffectId`: the registration. A new variant does not compile until it is built there. An effect is built for one rally, from tuning, the mode, and the rally's seed. |
| `Sandbox` | Port resource (M2 spec 2.9.1): the shell's overlay writes it. `force_effects(effects)` forces the whole set in play, read by escalation at the next countdown, and `toggle(effect, in_play)` changes one effect of it; `hold_returns` holds the count every flight starts at, copied into `ActiveEffects` each step; `hold_hit_motion` holds the paddle motion every return is told, copied the same way, for a harness and with no overlay key; `release` forces nothing. Any forcing marks the match touched, which persistence reads. Leaving a match releases the sandbox, so forcing does not carry into the next one; entering a match clears the mark unless something was forced for it beforehand, as a test harness does. |
| `ActiveEffects` | Resource: the effects in play, set by escalation through `set(ids, tuning, mode)`. `start_flight(start)` asks each once, at the sandbox's held count if one is held, and with its held hit motion in place of the paddle's at a return. Every channel composes commutatively, so order does not matter. `flaw_over(flight, difficulty)` asks each for its flaw over the start the flight began at; the AI calls it when it plans an approach, so the per-star numbers cost nothing in a match without an AI. It also holds the rally's `RallySeed`, and every effect in play is built for that rally: `rally_seed()` is what an effect that chooses once per rally chose from, so the shell can show the choice before it acts. |
| `BallFlight` | Component, required by `Ball`: the ball's flight. Replaced only at a flight start (`begin`), cleared only between rallies (`clear`). `flight()` is the empty flight while cleared; `in_flight()` is `None` then, since a begun flight with no effect in play is just as empty. |
| `probe.rs` | Test only: `Misjudged`, an effect no tuning builds, with a push, a believed push, and an added error and delay, for the tests of the base and of the AI. |
| `speed.rs` | Speed: a launch-speed gain per return of the rally, at the per-hit increase of the mode played. One module per effect. |
| `shrink.rs` | Shrink: a factor on both paddles' height, riding a `Ramp` over the rally's returns from one to `height_min / height_default`. |
| `wind.rs` | Wind: a `Drift` field (kernel) whose drift over one crossing rides a `Ramp` from nothing to `max_drift`, up or down as `WindDirection::of` reads the rally's seed. Its flaw is the second form: each star believes its tuned share of the true drift. In Chaos's bag since M2 plan phase 5. |
| `spin.rs` | Spin: the same `Drift` field, begun at a return whose paddle moved in the step of the hit, toward the paddle's motion; a still paddle's return and a serve get none. `Spin::drift(start)` is the signed drift of a flight, for the field and for the shell alike. Its flaw is the second form: each star has read its tuned share of the true spin. In no bag yet (M2 plan phase 6, step 4). |

Channels, each composing so that effects cannot see each other:

| Channel | Composes by | Bound, kept by the base |
|---|---|---|
| Launch speed gain | Sum | `launch_speed` adds the serve speed and applies `ball.speed_cap`. |
| Paddle height factor | Product | `Paddle::set_height` clamps to the tuned bounds. One factor for both paddles. |
| Lateral field | Sum | The wall step cuts vertical speed to the playable height per step, what its one fold undoes (`advance_between_walls`, M2 spec 2.2.2). Wind contributes the first field, Spin the second. |

Two counts that look alike and are not:
the flight counts returns, face hits that turn the ball back and start a flight;
rules counts touches, every `BallHitPaddle`, an end clip included, for the rally record.
M1 raised the speed on a face hit only, and the return count keeps that.

Speed's gain depends on the return count alone, never on the ball's current speed (M2 spec 2.0.4),
so a force that speeds the ball up in mid-air does not feed the ramp.
The base adds the serve speed and applies the cap (`SpeedRule::launch`): bounds sit on the channel.

| System | Runs | Does |
|---|---|---|
| `rebuild_effects_from_changed_tuning` | Effects | Rebuilds the effects in play when `Tuning` changed, so a dev reload reaches them. |
| `fit_paddles_to_the_flight` | Effects | Sets both paddles to the default height times the flight's factor, only when the factor changed since it last wrote. One step after the return that started the flight. |
| `clear_flights` | `OnEnter(Countdown)` | The ball is in no flight: no returns, no forces, factor one. So paddles are whole for every serve, and stay short through the point pause before it. |
| `start_flight_on_serve` | Effects, `BallInPlay` | On `Serve`, starts the rally's first flight, after the ball feature launched the ball in Intent. |
| `hold_what_the_sandbox_holds` | Effects, before `start_flight_on_serve` | Copies the sandbox's held count and held hit motion to `ActiveEffects`, each only when it changed, so every flight start passes through one place. |
| `begin_the_match_untouched` | `OnEnter(InMatch)` | Sets the sandbox's touched mark to whether anything was forced for the match before it was entered. |
| `release_the_sandbox_with_the_match` | `OnExit(InMatch)` | Releases whatever the sandbox forces: forcing does not outlive its match. |
| `draw_the_next_rallys_seed` | `OnEnter(InMatch)`; `OnTransition` from `Playing` to `PointScored` | Draws the seed of the rally about to begin and builds the effects in play for it. |

Later flights start in physics, at a paddle return:
the collide system needs the new flight's speed in the middle of its own run, so it calls `start_flight` directly.

## physics

Thin systems over the kernel.

`register_required_components::<Ball, StepStart>()`
makes Bevy add it whenever a `Ball` is created; effects does the same for `BallFlight`.
A later feature extends an earlier feature's entity without the earlier one importing it.
Bevy panics if a `Ball` archetype already exists at registration,
so it happens at plugin build, before any system spawns a ball.

| Component | Meaning |
|---|---|
| `StepStart` | Centre at the start of the step: the sweep's start, and the view's earlier interpolation end (plan 4.8). |

| System | Phase | Does |
|---|---|---|
| `integrate_ball` | Integrate | Saves `StepStart`, asks the flight for this step's force, calls `advance_between_walls`, ticks the flight's clock, writes `BallHitWall` on a bounce. |
| `collide_ball_with_paddles` | Collide | Sweeps `StepStart → Position` against every paddle; takes the earliest contact; a face hit is a return: it starts the next flight, aims by offset, and leaves at the flight's launch speed; an end hit mirrors and starts no flight; the rest of the step continues along the new velocity. Writes `BallHitPaddle`. |

Integration and collision are separate phases so `Effects` sits before one and `Sanity` after the other.
The cost, documented on the system: a wall bounce and a paddle-end hit in the same step are swept along the folded line.

## rules

Owns every `MatchState` transition (ADR 0003): scoring, serve timing, pause, match end.

| System | Runs | Does |
|---|---|---|
| `begin_match` | `OnEnter(InMatch)` | Resets `Score`; draws the first serve's side. |
| `begin_countdown`, `begin_point_pause` | `OnEnter(Countdown)`, `OnEnter(PointScored)` | Insert a `StateTimer` from tuning; it is public read-only (`time_left`), so the shell can show the countdown (plan 8.4). |
| `advance_countdown`, `advance_point_pause` | `Update`, in their state | Tick on virtual time; request `Playing` or `Countdown`. |
| `request_serve` | `OnTransition { Countdown → Playing }` | Writes `Serve`. |
| `restart_rally_after_fault` | `Update`, after the timers, while the paddles move | On any `SimFault`, requests `Countdown` unless the match is already ending. |
| `apply_pause_requests` | `Update`, after the timers and the heal | Acts on `MatchControl`. |
| `count_rally_hits` → `score_goal_crossings` → `end_match_at_target` | Rules, chained | Count this step's `BallHitPaddle` into `Rally`; award the point, write `PointScored`, request `PointScored`; then, if `Score` changed and a side reached the target, write `MatchEnded` with the match's longest rally and request `MatchOver`. The later `set` wins. Counting first puts a hit in the winning step into the reported rally. |

Timers run in `Update` because the phases run only while `Playing`;
a countdown must advance while not playing.

The serve hangs on the transition, not `OnEnter(Playing)`:
resuming from `Paused` also enters `Playing` and must not serve.
`Serve` is written in `StateTransition`, before the fixed loop,
so the first live step reads it in the same frame,
or the next frame's if this one runs no step.

`MatchControl` is the pause port.
The shell calls `request_pause` or `request_resume`; rules decides.
Pause is honoured in `Countdown`, `Playing`, `PointScored`;
`PausedFrom` records which, and resume returns there.
A resumed `Countdown` or `PointScored` restarts its timer, because `set` re-runs its `OnEnter`.
While a transition is pending, the request waits a frame.
`Paused` with no `PausedFrom` is an error to the error handler: something else entered `Paused`.

`Score` is public for the shell's display.

`Rally` (plan 7.4) counts paddle hits from a serve to the point that ends it, and keeps the most in any rally of the match.
Entering `Countdown` starts a new rally: every serve follows a countdown, the heal's restart included; a pause and resume does not.
Entering a match clears both. Records read the snapshot in `MatchEnded`. The Chaos HUD's count is not this one: it shows the flight's returns (M2 spec 2.7.2).

## escalation

Which effects are in play, decided between points (M2 spec 2.4).
The match scale sets breadth here; the rally scale sets intensity in each effect, from the flight's return count.
This feature knows no effect: it reads names from the mode's tuning and hands them to `ActiveEffects`.
It is the one writer of the effects in play; the effects feature itself writes only the sandbox's held count and held hit motion there.
While the `Sandbox` forces a set, that set is in play instead, the always-on effects included, and nothing is announced; the schedule turns underneath, so a release finds it where the score has brought it.

A mode is data (`tuning.mode(mode)`): effects that are `always` on, a `bag` to schedule,
`thresholds_percent` of the leader's progress at which one more joins, and `rotate_every_points`.
Classic is the mode with an empty bag, so nothing here branches on the mode.

`schedule.rs` is the arithmetic, free of the ECS and generic over the item, tested with plain numbers:

| Piece | Does |
|---|---|
| `count` | One, plus one per threshold reached, capped by the match's cap and by the bag. Compared in whole numbers, so 3 of 10 against 30 percent is exact. |
| `shuffle` | Seeded order for the bag. A bag of one or none draws nothing, so Classic's random stream is M1's. |
| `Rotation<T>` | Items in play, oldest first, and a bench. A rising count only adds. A rotation swaps the oldest for the next; the one that leaves goes to the back, so none returns before every other had its turn. |

| System | Runs | Does |
|---|---|---|
| `draw_the_bag` | `OnEnter(InMatch)` | Shuffles the mode's bag; resets the schedule. |
| `bring_effects_up_to_date` | `OnEnter(Countdown)` | Recomputes the count from the score; rotates if a rotation interval of points has passed; writes `EffectEnded` then `EffectStarted` for bag effects; sets `ActiveEffects`. |

`OnEnter(Countdown)` also fires after a fault and on resuming a paused countdown.
The update counts points, not countdowns, so those re-entries change nothing.
Nothing changes inside a rally (M2 spec 2.4.3).
Always-on effects announce nothing, and leaving a match ends nothing.

## ai

The mirror controller (spec 1.4.6): the computer's player writes the same `PaddleIntents` a seat does.

| System | Runs | Does |
|---|---|---|
| `spawn_controllers` | `OnEnter(InMatch)` | One `MirrorController` entity per side held by `Controller::Ai`, despawned with the match. |
| `steer_ai_paddles` | Intent | Each controller is handed a `Sighting` (the ball's motion, the flight it is in if any, the effects in play) and its paddle, and writes its side's intent. |

A controller is an entity of its own, not a component on the paddle:
spawning it needs no order against the paddle's spawn, which belongs to another feature.
Seats writes the human sides and the AI the others, so the two writers of `PaddleIntents` need no order.

`MirrorController::steer` holds the decisions and is a plain method, tested without an `App`.
An approach is a ball in a flight, flying toward the controller's side.
On the step an approach begins, the controller asks the effects in play for their flaws at its difficulty (`ActiveEffects::flaw_over`, M2 spec 2.3.12).
It predicts the crossing with the kernel's `crossing_y`, the same stepping the physics uses, under the flight it believes in,
adds an error drawn once from the spread, the star's `prediction_error` plus the effects' added error,
and keeps that target for the approach.
It holds still through the star's `reaction_delay` plus the effects' added delay, then steers to the target.
In Classic, Speed adds nothing and is believed as it is, so the star's handicap is the whole flaw, as in M1.
When the ball is not coming it steers to the centre, where the next serve starts.
`intent_toward` stops within half a step's travel of a target, so the paddle never jitters.
Of its state, only `aim` (the approach's `Aim`: target and error) and `side` are readable, for the shell's overlay (plan 10.1).

Steering is not in `BallInPlay`: the AI lines up between rallies as players do (ADR 0004).
The serve shares the intent phase, unordered, and the serve's flight begins in the effects phase after both.
A ball in no flight is not an approach, so the AI plans a served ball on the step after the serve, under the serve's flight, whichever of the two ran first.

## sanity

The last phase of every step in which the phases run checks that the simulation is sound (spec 1.4.7):
exactly one ball; its velocity finite; its centre inside the walls and no farther past a goal line than one step at the speed cap;
each paddle's centre finite and the whole paddle inside the walls;
the ball's flight no longer than `ball.flight_limit_seconds` (M2 spec 2.5.6).
The last is the one exit for a stalled or trapped ball, whatever effect caused it:
no effect carries a guard of its own for it. The flight's clock runs only in play, so a pause or a countdown does not count.
It runs between rallies too, not in `BallInPlay`: a ball resting past the goal after a point is within bounds (ADR 0004).

One code path in both builds. A broken invariant is written as a `SimFault` and returned as the system's error;
the error handler decides. Dev panics in that step.
Release logs it at error level through Bevy's handler, and rules' `restart_rally_after_fault` restarts the rally from a countdown,
whose end serves as after any point. `SimFault` is registered by rules, its reader and the earlier feature.
`pong_core` cannot log itself (no `bevy_log` feature), so the handler's line is the log.

A fault the heal cannot cure, such as a second ball, faults again every step in release.
In M1 only a code bug can cause one, and dev panics on it first.

## persistence

Profiles and settings kept between runs (spec 1.4.9); the file format is ADR 0007.

| System | Runs | Does |
|---|---|---|
| `load_saves` | `OnEnter(Boot)` | Reads both files and inserts `Profiles`, `Settings`, and the private `SaveState`; the Boot gate waits for them (spec 1.4.10). |
| `record_match_result` | `Update`, on `MatchEnded` | In the records of the match's mode (M2 spec 2.8.3): a win to the winning side's player, a loss to the loser's, and to both the match's longest rally if it beats their record, through `MatchConfig::player_on` (ADR 0008). Guest and computer sides record nothing; one profile on both seats records nothing; a match the `Sandbox` touched records nothing. An unknown profile id is an error. |
| `write_saves_at_match_end` | `Update`, on `MatchEnded`, after `record_match_result` | Writes each writable file whose encoding differs from what it last wrote. |
| `write_saves_after_menu_change` | `Update`, in `Menu`, when `Profiles` or `Settings` changed since it last ran | The same write, so a change on the menu screens survives the game closing before a match ends (plan 8.1). |

Load policy, per file: missing starts fresh; corrupt, newer, or unreadable is renamed to `<name>.<seconds>.bak` and the game starts fresh with one `Notice`;
if it cannot even be renamed, the game starts fresh and never writes that file this session ("never overwrite").
Without `SavePaths` the game starts fresh, writes nothing, and says so once.

Write policy: a failed write keeps the model in memory, and the next match end or menu change retries it, because `last_written` only moves on success.
The first failure of a session sends one `Notice`, whichever trigger failed.
Nothing is written during a match: a change made in one is written at its end, or on returning to the menu if it never ends.

| Module | Holds |
|---|---|
| `model` | `Profiles`, `Profile`, `ProfileName`, `Records`, `RecordsByMode`, `Settings`, `Volume`, `WindowMode`. Constructors check every value, so a model in hand is valid. A profile keeps one `Records` per mode, not per cap: `RecordsByMode::of` matches on `Mode` and ignores the cap, so a new mode does not compile until it has records. `Settings` holds the last mode picked, Classic before the first pick. `Profiles::create` gives a new profile the id after the highest, so ids stay unique (plan 8.1); `rename`, `recolor`, and `remove` hand back what they replace or take (plans 8.2, 8.4). |
| `document` | `encode_*` and `decode_*` per kind; the frozen `v1`, `v2`, and `v3` shapes; the versioned enums and their migration chains, one `From` step per version. Settings are at version 3: version 2 added the last points-to-win picked (plan 10.2), version 3 the last mode picked (M2 spec 2.8.2), and a version 2 file migrates to Classic. Profiles are at version 2: records per mode (M2 spec 2.8.3), and a version 1 file migrates with its records under Classic. |
| `io` | The `SavePaths` port and `write_atomically`. |
| `load` | The per-file load policy: `load`, `Loaded`, `LoadError`, backup names. |

Decoding reads the top-level `version` first, so a file from a newer game is `NewerVersion`, not `Corrupt`.
A value a model refuses is `Corrupt`, as the error table says for invalid save data.
Writes go through a temporary file beside the target and a rename, so a file is never half-written.


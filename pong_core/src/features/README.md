# features

One directory per feature, each with a plugin at its root.

## Order and imports

The order is declared in [`mod.rs`](mod.rs) (ADR 0002):
`tuning, seats, ball, paddle, physics, rules, ai, sanity, persistence`.
A feature may import the public components, resources, and messages of an earlier feature;
never its systems, never a later feature, never the app roof.
`just arch-check` enforces the order by grep.

Every feature has one shape:
its plugin registers its resources and messages,
its systems join a `FixedStepPhase` or an `OnEnter`/`OnTransition` schedule,
and its public types are what later features may use.

## tuning

One embedded RON file of every number, validated once, so no system re-checks a value.

| Stage | Where | What |
|---|---|---|
| Raw shape | `file.rs` | Plain numbers. `#[serde(deny_unknown_fields)]`: a misspelt key is an error, not a silent default. |
| Field checks | `Tuning::from_ron` | Each section through kernel constructors; a refusal names its path, e.g. `ball.radius`. |
| Cross-field checks | `check_everything_fits_the_arena` | Paddle not taller than the arena; ball fits; paddles short of the centre line; speed cap under one playable height per step. Bindings reject duplicates and `Escape`. |

`Tuning` has private fields and one constructor: holding one means it passed.

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

Open, unverified: the watcher watches `BEVY_ASSET_ROOT`, else `CARGO_MANIFEST_DIR`.
Under `cargo run -p pong` that is likely `pong/`, which does not contain `tuning.ron`.
Plan 3.9's manual check settles it.

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
the AI will write AI sides there.
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
height within the tuning bounds, and the whole paddle inside the walls.
Its fields are private; `new`, `set_height`, and `move_by` maintain both.
Thickness and inset are read from tuning at use by `rect(&Tuning)`, so a reload applies at once.

| System | Runs | Does |
|---|---|---|
| `spawn_paddles` | `OnEnter(InMatch)` | One per side, `DespawnOnExit(InMatch)`. |
| `fit_paddles_to_changed_tuning` | Integrate, before `move_paddles` (`.chain()`) | When `Tuning` changed since this system last ran, re-clamps every paddle. Idempotent, so its first run, which always sees a change, is harmless. |
| `move_paddles` | Integrate | Moves by intent × speed × fixed delta. |

Writing through `Query<&mut Paddle>` marks the component changed,
which the view can use to know a paddle moved.

## physics

Thin systems over the kernel.

`register_required_components::<Ball, Force>()` and `::<Ball, StepStart>()`
make Bevy add both whenever a `Ball` is created.
A later feature extends an earlier feature's entity without the earlier one importing it.
Bevy panics if a `Ball` archetype already exists at registration,
so it happens at plugin build, before any system spawns a ball.

| Component | Meaning |
|---|---|
| `Force` | Force this step (unit mass, so also acceleration). Effects add to it; integration spends and zeroes it. Zero in M1. |
| `StepStart` | Centre at the start of the step: the sweep's start, and the view's earlier interpolation end (plan 4.8). |

| System | Phase | Does |
|---|---|---|
| `integrate_ball` | Integrate | Saves `StepStart`, calls `advance_between_walls`, zeroes `Force`, writes `BallHitWall` on a bounce. |
| `collide_ball_with_paddles` | Collide | Sweeps `StepStart → Position` against every paddle; takes the earliest contact; a face hit aims by offset and applies the speed rule, an end hit mirrors; the rest of the step continues along the new velocity. Writes `BallHitPaddle`. |

Integration and collision are separate phases so `Effects` sits before one and `Sanity` after the other.
The cost, documented on the system: a wall bounce and a paddle-end hit in the same step are swept along the folded line.

## rules

Owns every `MatchState` transition (ADR 0003): scoring, serve timing, pause, match end.

| System | Runs | Does |
|---|---|---|
| `begin_match` | `OnEnter(InMatch)` | Resets `Score`; draws the first serve's side. |
| `begin_countdown`, `begin_point_pause` | `OnEnter(Countdown)`, `OnEnter(PointScored)` | Insert a `StateTimer` from tuning. |
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
Entering a match clears both. It is public for C1's meter and a HUD; records read the snapshot in `MatchEnded`.

## ai

The mirror controller (spec 1.4.6): the computer's player writes the same `PaddleIntents` a seat does.

| System | Runs | Does |
|---|---|---|
| `spawn_controllers` | `OnEnter(InMatch)` | One `MirrorController` entity per side held by `Controller::Ai`, despawned with the match. |
| `steer_ai_paddles` | Intent | Each controller reads the ball now and its paddle, and writes its side's intent. |

A controller is an entity of its own, not a component on the paddle:
spawning it needs no order against the paddle's spawn, which belongs to another feature.
Seats writes the human sides and the AI the others, so the two writers of `PaddleIntents` need no order.

`MirrorController::steer` holds the decisions and is a plain method, tested without an `App`.
When the ball turns toward its side, it predicts the crossing with the kernel's `crossing_y`,
the same stepping the physics uses, adds an error drawn once from `±prediction_error`,
and keeps that target for the approach.
It holds still through `reaction_delay`, then steers to the target.
When the ball is not coming it steers to the centre, where the next serve starts.
`intent_toward` stops within half a step's travel of a target, so the paddle never jitters.

Steering is not in `BallInPlay`: the AI lines up between rallies as players do (ADR 0004).
The serve shares the intent phase, unordered, so a reaction to a serve may start one step late.

## sanity

The last phase of every step in which the phases run checks that the simulation is sound (spec 1.4.7):
exactly one ball; its velocity finite; its centre inside the walls and no farther past a goal line than one step at the speed cap;
each paddle's centre finite and the whole paddle inside the walls.
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
| `record_match_result` | `Update`, on `MatchEnded` | A win to the winning side's player, a loss to the loser's, and to both the match's longest rally if it beats their record, through `MatchConfig::player_on` (ADR 0008). Guest and computer sides record nothing; one profile on both seats records nothing. An unknown profile id is an error. |
| `write_saves_at_match_end` | `Update`, on `MatchEnded`, after `record_match_result` | Writes each writable file whose encoding differs from what it last wrote. |

Load policy, per file: missing starts fresh; corrupt, newer, or unreadable is renamed to `<name>.<seconds>.bak` and the game starts fresh with one `Notice`;
if it cannot even be renamed, the game starts fresh and never writes that file this session ("never overwrite").
Without `SavePaths` the game starts fresh, writes nothing, and says so once.

Write policy: a failed write keeps the model in memory, and the next match end retries it, because `last_written` only moves on success.
The first failure of a session sends one `Notice`.
Writes happen only at match end; plan 8.1's settings and profile screens will need their own trigger.

| Module | Holds |
|---|---|
| `model` | `Profiles`, `Profile`, `ProfileName`, `Records`, `Settings`, `Volume`, `WindowMode`. Constructors check every value, so a model in hand is valid. |
| `document` | `encode_*` and `decode_*` per kind; the frozen `v1` shapes; the versioned enums and their migration chains. |
| `io` | The `SavePaths` port and `write_atomically`. |
| `load` | The per-file load policy: `load`, `Loaded`, `LoadError`, backup names. |

Decoding reads the top-level `version` first, so a file from a newer game is `NewerVersion`, not `Corrupt`.
A value a model refuses is `Corrupt`, as the error table says for invalid save data.
Writes go through a temporary file beside the target and a rename, so a file is never half-written.


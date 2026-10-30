# M1: Basic Pong

Specified 2026-10-30. First playable milestone, per [design.md](../design.md) and [architecture.md](../architecture.md).
Obligation levels: MUST, SHOULD, MAY, WONT. Every item carries a done-condition.

## Facts relied on

- Bevy 0.20.1; `2d`, `ui`, `audio` feature collections; `embedded_watcher`, `dynamic_linking`, `bevy_ci_testing` opt-in. [confirmed]
- `MinimalPlugins` is headless. [confirmed]
- `App::set_error_handler` exists; fallible systems panic by default. [confirmed, 0.17 onward]
- Render-free core compiles without `wgpu`. [probable; M0 verifies]
- 2D bloom needs an HDR camera and colors above 1.0; TonyMcMapface tonemapping and deband dither recommended. [confirmed]
- A window focus event exists. [probable]
- Bevy 0.19 ships an `EditableText` component for text entry. [confirmed]
- Bevy's audio accepts an in-memory source built from bytes or a custom decodable source. [probable; verify in the first audio task]
- cargo-nextest does not run doctests. [confirmed]
- nextest supports per-test timeouts that fail the run. [probable]
- GitHub Actions needs Ubuntu build dependencies for Bevy: pkg-config, libx11-dev, libasound2-dev, libudev-dev, libxkbcommon-x11-0, libwayland-dev. [confirmed, research]

## Decisions taken during specification

- Points to win: default 10, chosen per match from a tuning list of 5, 10, 15, 21.
- Arena: 160 by 90 arena units.
- Keys, mirrored per seat: left W/A/S/D, right Up/Left/Down/Right. In play: W/Up and S/Down move; D/Right is the action key, unused until C2; A/Left reserved. In menus: up/down navigate, D/Right confirms, A/Left backs out. Escape pauses or backs out from anywhere.
- AI difficulties: three, shown as one, two, three stars.
- Profiles: typed name up to twelve characters, color from a palette of eight defined in tuning; profiles store the color value, not a palette index.
- Sounds: synthesized in code at startup behind a swappable bank builder.
- Window: borderless fullscreen by default; windowed available in settings.
- Tooling: `cargo-nextest` throughout; `just` as the single source of commands; `cargo-deny` as SHOULD.
- CI: GitHub Actions on Ubuntu.
- Soak test time budget: under thirty seconds.

## 1.1 Repository foundation

- **1.1.1 MUST** Workspace per M0's ADR, Bevy pinned to an exact 0.20 patch, `rust-toolchain.toml` on stable. Done: `cargo build` succeeds on a clean clone.
- **1.1.2 MUST** `dynamic_linking` behind a dev-only cargo feature. Done: `ldd` on the release binary shows no `libbevy_dylib`.
- **1.1.3 MUST** GitHub Actions on Ubuntu: Linux deps, `just`, nextest, cargo cache; runs `just ci-check` then a release build. Done: green on the default branch. Depends on the user creating the GitHub remote.
- **1.1.4 MUST** `.gitignore`, license file, `docs/adr/` with a template, `docs/specs/` holding this spec. Done: files present.
- **1.1.5 SHOULD** Clippy beyond default: selected pedantic lints; `unwrap_used` denied in core. Done: lint config in workspace `Cargo.toml`, CI green.
- **1.1.6 WONT** Release packaging or installers.
- **1.1.7 MUST** `justfile` recipes: `fmt`, `lint`, `test`, `test-core`, `dev-check`, `ci-check`, `run`, `run-release`. Done: every command referenced by CLAUDE.md, CI, and hooks is a `just` recipe, verified by grep.
- **1.1.8 MUST** `just dev-check` formats in place, then clippy with warnings denied, then the full test suite. `just ci-check` is identical with fmt in check mode. Done: `dev-check` exits non-zero on a planted clippy warning and a planted failing test; `ci-check` exits non-zero on an unformatted file.
- **1.1.9 MUST** Tests run via `cargo nextest run`; doctests via `cargo test --doc` in the same recipe. Done: a planted failing doctest fails `ci-check`.
- **1.1.10 SHOULD** `cargo-deny` with license allowlist and advisories, inside `ci-check`. Done: `deny.toml` present, passes.

## 1.2 Core kernel

ECS-free pure functions over plain data.

- **1.2.1 MUST** Arena geometry type in arena units with width, height, goal lines. Done: unit tests for containment and wall distance.
- **1.2.2 MUST** Integrate: position, velocity, forces, step to new position. Done: unit test against an analytic trajectory.
- **1.2.3 MUST** Swept circle-versus-rectangle collision returning time of impact and normal. Done: a ball faster than paddle thickness per step still collides in the test.
- **1.2.4 MUST** Paddle reflection: outgoing angle from hit offset, within a configured maximum. Done: center hit straight, edge hit at maximum, symmetric for both sides.
- **1.2.5 MUST** Speed cap after every reflection. Done: a thousand reflections never exceed the cap.
- **1.2.6 MUST** Predict: where the ball crosses a given x, using 1.2.2 and wall bounces. Done: equals simulation for a scripted trajectory within one unit.
- **1.2.7 SHOULD** Per-hit speed increase, tunable, zero allowed. Done: zero shows no change; positive is monotone until cap.
- **1.2.8 WONT** Field forces; the accumulator exists and receives zero.

Coupling: 1.2.6 and 1.2.2 share one integrate function. This is the AI fairness guarantee.

## 1.3 Core vocabulary

- **1.3.1 MUST** `Seat { Left, Right }`, `Side`, `Controller { Human(Seat), Ai(Difficulty) }`. Done: each used by two or more features.
- **1.3.2 MUST** Events: `BallHitPaddle { seat }`, `BallHitWall`, `PointScored { side }`, `MatchEnded { winner }`, `Notice { text }`. Done: each emitted by exactly one feature, observed by the shell.
- **1.3.3 MUST** Vocabulary under a dozen types. Done: counted at milestone close.
- **1.3.4 WONT** `EffectId`, `BossId`, `ItemId`.

## 1.4 Core features

### 1.4.1 Seats and intents
- **MUST** `PaddleIntent { Up, Down, None }` per seat; latched in `Update`, consumed and cleared in `FixedUpdate`. Done: a one-frame press spanning two fixed steps moves the paddle exactly once.
- **MUST** Key bindings per seat in tuning. Done: changing the RON changes keys without recompiling in dev.
- **MUST** Action and reserved keys exist in the binding schema. Done: present in the RON.

### 1.4.2 Ball
- **MUST** `Position`, `Velocity`, `Ball`, `Snap` marker. Done: snap set on serve, cleared by the view after one frame.
- **MUST** Serve: countdown, then launch toward the side that conceded at a random angle within a tunable cone from the seeded random resource. Done: fixed seed reproduces the angle.

### 1.4.3 Paddle
- **MUST** `Paddle` with a size setter clamped to tuning bounds; speed from tuning; clamped to the arena. Done: clamp tests at both edges.

### 1.4.4 Physics
- **MUST** Systems in phases `integrate` and `collide` wrapping 1.2. Done: a headless rally produces `BallHitPaddle` and `BallHitWall` in the expected sequence.

### 1.4.5 Rules
- **MUST** `MatchConfig { left, right, points_to_win }`. Done: built by the menu, read by rules.
- **MUST** Substates `Countdown`, `Playing`, `PointScored`, `Paused`, `MatchOver` under `InMatch`. Done: a headless test walks the cycle to `MatchOver`.
- **MUST** Score per side; `MatchEnded` at target. Done: test.
- **MUST** Points-to-win options list and default in tuning. Done: menu offers the list, default preselected, match ends at the chosen value by test.
- **SHOULD** Countdown length in tuning. Done: changeable.
- **MAY** Persist last chosen points-to-win in settings. Done: restored on next start.
- **WONT** Deuce or best-of formats.

### 1.4.6 AI, mirror controller
- **MUST** Reaction delay and prediction error from a difficulty entry; target from 1.2.6, recomputed when the ball turns toward the AI; error resampled per approach. Done: zero delay and zero error returns every ball; large error misses some over a seeded run.
- **MUST** AI reads no future ball position beyond 1.2.6. Done: review item at milestone close.
- **MUST** Three difficulty entries in tuning. Done: present.
- **WONT** Adaptive difficulty.

### 1.4.7 Sanity and heal
- **MUST** `sanity` phase checks finite velocity, ball within arena plus margin, paddles within arena. Done: injected NaN fires the fault.
- **MUST** Dev panics; release emits `SimFault`, heal re-serves and logs at error. Done: both paths tested with the handler toggled.

### 1.4.8 Tuning
- **MUST** One embedded RON deserialized at `Boot` into a validated `Tuning` resource. Done: CI parses the embedded file; validation rejects a crafted out-of-range value.
- **MUST** Dev override from the source tree with hot reload; invalid override keeps last good values and logs. Done: manual check plus unit test of keep-last-good.
- **MUST** Schema: arena size, paddle size bounds and speed, ball speed, cap, per-hit increase, maximum reflection angle, serve cone, countdown, points-to-win list and default, key bindings per seat, AI difficulties, color palette. Done: all present.
- **MUST** Palette is a list; profiles store the color value. Done: a profile keeps its color after the palette is reordered, by test.

### 1.4.9 Persistence
- **MUST** `SavePaths` resource injected by the shell; core reads and writes with `std::fs`, atomic via temp file and rename. Done: an injected failing writer leaves the original intact.
- **MUST** Profiles file v1: id, display name, color value, records (wins, losses, longest rally). Done: round-trip test.
- **MUST** Settings file v1: master volume, window mode. Done: round-trip test.
- **MUST** Load policy: corrupt or newer version backs up with timestamp suffix, starts fresh, emits one `Notice`. Done: fixtures for corrupt and for version 99.
- **MUST** Write policy: failure keeps the model in memory, retries at next match end, one `Notice` per session. Done: test with an unwritable path.
- **MUST** Versioned enum migration chain; fixture `v1.ron`; a test asserting fixture count equals current version. Done: tests pass.
- **MUST** Guest built in, never written; a Guest side records nothing, the other side records normally. Done: test.
- **SHOULD** Records written at `MatchEnded` only. Done: no write during play, by test.

### 1.4.10 App assembly
- **MUST** `AppState { Boot, Menu, InMatch }`; `Boot` leaves only when tuning, settings, profiles are resolved. Done: a held load produces no transition.
- **MUST** Fixed-step phase sets in order: intent, effects, integrate, collide, rules, sanity. Done: every core system declares a phase, checked by a schedule-walking test.
- **MUST** One core plugin group used by both shell and testing module. Done: same group in both.

## 1.5 Core testing

- **1.5.1 MUST** `testing` module behind a cargo feature: `MinimalPlugins` plus chosen core plugins, fake intents, temp-dir `SavePaths`, fixed seed. Done: used by every headless test.
- **1.5.2 MUST** Soak: scripted match with random intents for both seats over at least 100,000 fixed steps, panic handler on, no fault; nextest per-test timeout of thirty seconds fails the run. Done: passes in CI.
- **1.5.3 SHOULD** Schedule randomization on in tests. Done: tests pass with it enabled.

## 1.6 Shell

### 1.6.1 Ports
- **MUST** Keyboard adapter maps physical keys from tuning to `PaddleIntent` per seat; `SavePaths` from XDG directories. Done: a held key maps to an intent by test.
- **MUST** Focus loss clears intents and enters `Paused` only from `Playing`. Done: manual check plus a test sending the focus event.

### 1.6.2 Views
- **MUST** Arena, paddles, ball as `Mesh2d` with HDR colors; `Transform` from `Position` with interpolation; snap honored. Done: no visible judder at a frame rate unequal to the fixed rate.
- **MUST** Camera fits the arena with letterboxing; no pixel units in shell logic. Done: resizing keeps the aspect.
- **MUST** Bloom with HDR, TonyMcMapface, deband dither. Done: glow visible.
- **SHOULD** Ball trail. Done: visible.
- **MAY** Screen shake on a point. Done: visible.

### 1.6.3 UI
- **MUST** Screens: boot, main menu (play, settings, quit), mode select (vs human, vs computer, difficulty stars, points-to-win), profile select per seat with seat cue, match HUD with score, pause, match over with winner and records delta, notice line. Done: full flow playable keyboard-only.
- **MUST** During a seat's turn only that seat's keys move the cursor; D/Right confirms; A/Left and Escape back out. Done: the other seat's keys do nothing, by test.
- **MUST** Profile creation: typed name, color from the palette. Done: new profile appears in the file.
- **MUST** Seat key filter suspended during text entry. Done: a name using letters from both key sets is enterable.
- **MUST** Name cap twelve characters; duplicates allowed. Done: thirteenth character rejected; two identical names coexist, by test.
- **SHOULD** Rename and delete a profile. Done: both reachable from profile select.
- **SHOULD** Text limited to single words or icons where a 9-year-old must read. Done: review at milestone close.
- **MUST** Settings screen: volume, window mode only. Done: both change and persist.

### 1.6.4 Audio
- **MUST** Audio plugin observes `BallHitPaddle`, `BallHitWall`, `PointScored`, `MatchEnded` and plays cues; volume from settings. Done: audible; a test asserts one play command per event with audio disabled.
- **MUST** Sounds synthesized at startup into assets by a `SoundBank` builder behind a trait; the cue map depends only on cue names. Done: four distinct sounds; unit test asserts buffers finite, non-silent, in range.
- **SHOULD** Builder swappable for an embedded-file builder without touching the cue map. Done: cue map has no import from the synth module.
- **WONT** Music.

### 1.6.5 Overlay
- **SHOULD** Function-key toggle: fixed-step rate, frame rate, ball speed, AI target and error, state and substate. Done: visible in a release build.

### 1.6.6 Window
- **MUST** Title; default borderless fullscreen; mode from settings. Done: first start fullscreen; after toggling, windowed on next start.
- **MAY** Alt+Enter toggles mode. Done: works.

## 1.7 Error policy wiring

- **1.7.1 MUST** Error handler per build profile: panic in dev, log in release. Done: a failing test system behaves differently under each.
- **1.7.2 MUST** Panic hook writes a crash file to the XDG state directory. Done: a forced panic produces the file.
- **1.7.3 MUST** `Notice` rendered as one line on the menu, cleared at next match start. Done: visible after a corrupt-save fixture is placed.

## 1.8 Playtest

- **1.8.1 MUST** `docs/playtest.md` with the observe-then-interpret template. Done: exists.
- **1.8.2 MUST** Keyboard ghosting check on the kids' machine: both seats hold up, down, and action, six keys. Done: result in the playtest log before C3.
- **1.8.3 SHOULD** One session per child recorded. Done: entries exist.

## 1.9 Milestone close

- **1.9.1 MUST** ADRs: crate structure (M0), error policy, persistence format, audio boundary. Done: four files.
- **1.9.2 MUST** CLAUDE.md, rules, hooks per [claude-code-setup.md](../claude-code-setup.md). Done: hooks fire on an edit and a commit.
- **1.9.3 MUST** Checklist: CI green, docs updated, vocabulary under twelve, no `unwrap` in core, AI review item 1.4.6 signed. Done: checklist marked in this file.

## Deliberately absent

- Effects, meter, rooms, items, beat: later milestones. The force accumulator and `effects` phase exist empty.
- AI versus AI attract mode.
- Gamepads, localization, analytics, network.

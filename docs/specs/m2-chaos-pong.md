# M2: Chaos Pong

Specified 2026-11-11. Second milestone, design candidate C1, per [design.md](../design.md) ("C1 direction") and [architecture.md](../architecture.md).
Chaos Pong is a second mode beside Classic: rallies escalate by effects, not mainly by speed.
Obligation levels: MUST, SHOULD, MAY, WONT. Every item carries a done-condition.

## Facts relied on

- `integrate` takes a force, and `crossing_y` passes zero through the same call. [confirmed, `pong_core/src/kernel/predict.rs`, `integrate.rs`]
- `advance_between_walls` requires a step's vertical travel to stay under the playable height, so one fold lands inside. [confirmed, `pong_core/src/kernel/integrate.rs`]
- M1's `Force` is a per-step accumulator component on the ball, zeroed by integration. [confirmed, `pong_core/src/features/physics/mod.rs`]
- Classic's speed rule: serve 80, plus 7 per hit, cap 250. It adds to the ball's current speed, read as `velocity.length()` at the hit. [confirmed, `tuning.ron`, `features/physics/mod.rs`]
- Speed as serve plus gain times hit count equals M1's repeated addition up to the last float bits. [uncertain; checked by 2.0.1 against M1's suite]
- Without escalation a competent defender returns every ball: at serve speed the ball's vertical speed tops out near 69 units/s, under the paddle's 90. [probable, inferred from `tuning.ron`; verified by 2.5.1]
- Rules counts paddle hits in `Rally` and resets it on entering `Countdown`. [confirmed, `pong_core/src/features/rules/mod.rs`]
- The paddle has no velocity state; it moves by intent times speed, all or nothing. The collide phase does not know the paddle's motion. [confirmed, `features/paddle/mod.rs`, `features/physics/mod.rs`]
- The collide phase resolves one contact per step and travels the remainder against the walls only. [confirmed, `features/physics/mod.rs`]
- The AI plans once per approach and ignores later course changes. [confirmed, `features/ai/mod.rs`]
- Tests call `crossing_y` in four files, and the speed rule in `sanity`, `ai`, and the soak. [confirmed, grep 2026-11-11]
- Rules comes after physics in the feature order and counts hits in its own phase, after collide. [confirmed, `features/mod.rs`, `features/rules/mod.rs`]
- The soak's running time, and so whether a soak per subset of six effects fits CI. [unknown; measured in M2 plan 4.3]
- Sanity faults unless exactly one ball exists. [confirmed, `features/sanity/mod.rs`]
- `Paddle` has a size setter clamped to tuning bounds: minimum 8, default 14; paddle thickness is 2. [confirmed, spec 1.4.3, `tuning.ron`]
- The kernel has swept circle-versus-rectangle collision for axis-aligned rectangles. [confirmed, spec 1.2.3]
- What `swept_circle_rect` returns for a sweep that starts inside the rectangle. [unknown; read before 2.2.6 is planned]
- A level ball on a 45° face leaves vertical, and a vertical ball between the walls reaches no goal. [confirmed, reflection geometry]
- A ball between a block and its mirror copy leaves in a few bounces unless the pair is within about a degree of upright. [probable, derived]
- Blocks may lengthen rallies by shielding goals. [uncertain; 2.5.2 decides]
- Only the match-over screen shows records. [confirmed, grep of `pong/src`]
- M1's menu runs opponent, points to win, profiles. [confirmed, spec 1.6.3]
- Profiles file is at v1, settings file at v2. [confirmed, spec 1.4.9]
- The kids' rally-length distribution. [unknown; one local profile records a longest rally of 21, owner unknown; observed in the 2.10 sessions]
- `Material2d` and custom 2D shaders exist in Bevy 0.20. [probable, research 2026-10-30; `bevy-api-check` against 0.20.1 before 2.6.3 is planned]
- `bevy_hanabi` supported Bevy 0.19 only on 2026-10-30. [confirmed then, uncertain now; no package below depends on it]

## Decisions taken during specification

Modes:
- Classic Pong stays selectable, unchanged. Chaos Pong is a variant beside it, not a layer on top.
- A mode is data over one simulation: a list of effects with their curves, and a schedule. `MatchConfig` carries the mode played.
- Classic's speed ramp is an effect, Speed. Classic is Speed alone; Chaos carries Speed at a smaller gain plus the scheduled effects.
- The refactor that makes Speed the first effect is phase one of the plan.

Effects:
- The word is "effect"; "perturbation", the first round's working name, is retired.
- Effects are pluggable and independent of each other ([ADR 0011](../adr/0011-effect-identity-and-contract.md)). The touch list is a SHOULD.
- No bake-off of unpolished prototypes. Every effect ships at the quality bar and is playtested as the real thing.
- A field is a pure function of the ball's motion and time in flight. A flight runs from a serve or paddle hit to the next paddle hit or a goal. An effect's parameters change only at a flight start.
- A lens and a block start no new flight: the ball moves through a constant field, and a block bounce is a collision inside a flight.
- Forces are lateral only: along y. A lens stands in for the design's gravity well.
- Fog hides the ball only; arena furniture is drawn above it.
- Roster: Speed, Shrink, Wind, Fog, Spin are MUST; Lens and Blocks are SHOULD; Ice is MAY; Squeeze and Multi-ball are WONT. Multi-ball is parked, potentially for co-op.
- Dials are in play terms: what the player sees at the paddle, not accelerations.
- Fog's dial is width. Lens's and Blocks' dial is count: each item behaves the same, and more of them appear.
- Lens has one kind: it pulls toward its centre line.
- Blocks are the default paddle's length, at drawn angles, not upright only.
- Items are drawn free on one side and copied mirrored on the other.
- Blocks get a horizontal floor. A bounce budget and spacing are MAY, revisited only if play or the soak shows a stall or trap.
- Spin curves the ball the way the paddle moved.
- Wind's direction is drawn per rally and shown in the countdown.

Escalation:
- Two scales. The match sets breadth: how many effects are in play, changing only between points. The rally sets intensity: the rally's hit count, handed to each active effect.
- Breadth is driven by the leader's progress toward the points target: one effect from the first point, two from 30%, three from 70%, capped per match.
- The cap, one to three, is the player's choice on the mode screen. Effect types are not the player's choice.
- Which effects fill the count: a bag shuffled per match, rotating one effect per interval of points, oldest out. The interval is tuning.
- Chaos is in play from the first point; every rally starts mild because every dial is mild at zero.
- No catch-up bias in M2; a playtest hypothesis stands in.
- The career scale is deferred, as a possible campaign (design.md, "C1 direction"). M2 persists no unlock pool.
- The action key stays unused until C2.

Criteria:
- Every effect is held to R1 to R11 below. R3 is a MUST for the stack and a SHOULD per effect. R9 asks for a graded dial, not a continuous one.

AI, display, menu, records:
- AI star entries stay shared between modes; each effect's tuning carries its flaw per star.
- The display is an icon per active effect and the rally count as a number; no meter bar.
- The mode screen comes first: Classic, Chaos ●, Chaos ●●, Chaos ●●●.
- Records are kept per mode, not per cap. Match over shows the played mode's delta.
- An effect is dropped by the user's ruling after its session, citing a logged observation, after one retune.

## Selection criteria

Each effect in 2.6 is held to these. A criterion's "falsified if" is its check.

| # | Level | Criterion | Falsified if |
|---|---|---|---|
| R1 | MUST | The cause is on screen before the consequence | A child cannot point at what changed the ball |
| R2 | MUST | Skill still decides the point | A seeded run shows the outcome independent of paddle input |
| R3 | MUST for the stack, SHOULD per effect | It raises miss probability, monotone in its dial | AI-versus-AI rally length does not shorten as the dial rises |
| R4 | MUST | It terminates: no orbit, no stall | The soak finds a rally without a goal inside a bounded step count |
| R5 | MUST | The AI predicts it through shared math, with a tunable flaw | The zero-flaw AI misses, or the flaw dial changes nothing |
| R6 | SHOULD | It is reusable as a room ingredient and an item payload | It needs escalation state to function |
| R7 | SHOULD | It teaches a Bevy area the others do not | Two effects share the same new API surface |
| R8 | SHOULD | The roster spans at least three links of the player's loop (perceive, predict, reach, execute, attend) | All shipped effects attack one link |
| R9 | MUST | It has a graded dial, absent or mild at zero | Its dial has only two states |
| R10 | SHOULD | It gives the hitter a use, not only the defender a tax | No seeded test shows a hit that scores because of it |
| R11 | MUST | No strobing or full-screen flashing | Any view element flashes faster than 3 Hz |

## 2.0 Refactor: Speed as the first effect

Phase one of the plan. No new behaviour.

- **2.0.1 MUST** The flight (2.2), the contract (2.3), and Speed (2.6.0) exist, and Classic runs on them. Done: M1's suite passes under Classic with no assertion changed; call sites may change for a changed signature, each listed in the phase report; a test that differs only in the last float bits is named there too. Amended 2026-11-11 ([M2 plan](../plans/m2-plan.md)): the first wording, "passes unmodified", could not hold, since tests call `crossing_y` and the speed rule directly.
- **2.0.2 MUST** M1's `Force` accumulator is removed. Done: no `Force` component remains in `pong_core`.
- **2.0.3 MUST** The paddle exposes its motion in the current step. Done: a test reads up, still, and down from a paddle driven by each intent.
- **2.0.4 MUST** Speed is a function of the rally's hit count, not of the ball's current speed. Done: a ball given extra vertical speed in flight leaves its next hit at the count's speed, by test.
- **2.0.5 MUST** The feature order gains two places: `effects` after paddle and before physics, `escalation` after rules. Done: `features/mod.rs` and ADR 0002's list agree; `arch-check` is green. Added 2026-11-11 ([M2 plan](../plans/m2-plan.md)).

## 2.1 Modes

- **2.1.1 MUST** A mode is data: its effects with their curves, and its schedule. `MatchConfig` carries the mode and, for Chaos, the cap. Done: a headless Chaos match reaches `MatchOver`.
- **2.1.2 MUST** Classic behaves as M1 shipped it: Speed alone, at M1's gain and cap. Done: 2.0.1.

## 2.2 Flights and channels

An effect contributes to channels and never touches shared state directly.
Each channel composes commutatively, so effects cannot see each other.

- **2.2.0 MUST** Three terms. A field is a pure function of the ball's motion (position, velocity) and time in flight, giving a lateral acceleration. A flight runs from a serve or paddle hit to the next paddle hit or a goal. A flight start is the only moment an effect's parameters may change. Done: a test stepping a flight sees no parameter change; one at the next hit does.
- **2.2.1 MUST** Lateral force channel: the sum of the active fields, along y only; physics and the AI's prediction evaluate the same sum. Done: two dummy fields compose order-independently, by test; prediction equals simulation under the sum.
- **2.2.2 MUST** Bounds sit on the channel. The base clamps vertical speed so the single-fold precondition holds at every tuned dial, and keeps the speed cap. Done: tuning validation rejects a configuration that could break the precondition; a test at the clamp folds once.
- **2.2.3 MUST** Paddle modifier channel: a product on paddle height per side, clamped by `Paddle`'s API. Done: two dummy modifiers compose order-independently, by test.
- **2.2.4 MUST** Flight-start hook: at a serve or a paddle hit each active effect is asked once for its contribution to the flight, given the hit count and, at a hit, the paddle's side and step motion. The composed flight sits on the ball and owns the rally's hit count, since Speed needs the count inside the hit and rules counts only afterwards; rules keeps the records (amended 2026-11-11, [M2 plan](../plans/m2-plan.md)). Done: a dummy effect is asked exactly once per flight and sees the paddle's motion at a hit, by test.
- **2.2.5 MUST** A view-only effect has no simulation contribution; its core half carries identity, the dial, and the AI flaw. Done: 2.6.3 ships with no kernel change.
- **2.2.6 SHOULD** Obstacle channel: rectangles at any angle, swept by rotating the ball's path into the block's frame; a contact loop per step, bounded by a tuned count, in physics and prediction alike; a horizontal floor, so the ball keeps a minimum horizontal share of its speed after a block bounce. Done: a ball aimed level at a 45° face still reaches a paddle line; a ball in a wedge ends the step inside the contact bound; prediction equals simulation through two bounces.
  - **2.2.6.1 MAY** A bounce budget per flight. Done: only if a stall or trap is observed in play or in the soak.
  - **2.2.6.2 MAY** Spacing between items. Done: as 2.2.6.1.
- **2.2.7 MAY** Paddle motion model. Done: with 2.6.7.
- **2.2.8 WONT** Ball population: more than one ball.
- **2.2.9 MUST** Fog hides the ball only; arena furniture is drawn above it. Done: a lens inside the fog band stays visible, by shell test.

Couplings: 2.2.1 and M1's 1.2.6 share one integrate function, so the AI fairness guarantee of spec 1.2 extends to forces. A field takes velocity as input, so a dial stated per crossing still holds after a block changes the ball's horizontal speed.

## 2.3 Effect contract

Per [ADR 0011](../adr/0011-effect-identity-and-contract.md).

- **2.3.1 MUST** Identity is a closed, serializable enum with stable names. Done: a round-trip test over every variant.
- **2.3.2 MUST** The core half is a trait with no default methods: a stance on every channel, an AI flaw, a tuning section. Done: an effect missing any part does not compile.
- **2.3.3 MUST** The shell half is view, cue, and icon. It may be declared empty; it may not be missing. Done: an effect without a declared shell half does not compile.
- **2.3.4 MUST** An effect receives the rally's hit count and knows nothing of the schedule that put it in play. Done: every effect's tests drive it with a bare number, no escalation plugin.
- **2.3.5 MUST** Effects do not import each other. Done: `arch-check` enforces it.
- **2.3.6 MUST** An effect's clock is fixed-step time in flight. Done: a pause and resume mid-flight leaves prediction equal to simulation, by test.
- **2.3.7 MUST** Core emits a message when an effect enters play and when it leaves, observed by the shell. Done: a headless test sees both in order.
- **2.3.8 SHOULD** Touch list: adding an effect touches its own core module, its own shell module, one enum variant, one registration line per crate, and one tuning section, and nothing in physics, AI, rules, escalation, sanity, or the HUD. Done: measured on 2.6.5's diff.
- **2.3.9 MUST** One shared curve type maps the hit count to a dial: the hit where the dial starts to move, the hit where it is full, and the values at each. It yields a number or a whole count. Done: validation rejects a full hit before the start hit, and an item effect that starts at hit zero.
- **2.3.10 MUST** Registration is one exhaustive match over the identity enum per crate. Done: removing an arm fails the build.
- **2.3.11 MUST** One base-owned plugin wires every effect into the schedule; an effect writes systems only for state the base lacks. Done: Speed ships with no system of its own.
- **2.3.12 MUST** An AI flaw takes two forms, tuned per star in the effect's own section: added error or delay, and a degraded belief about the effect's contribution. Done: per effect, the zero-flaw AI returns every ball and a raised flaw misses some, over a seeded run.

## 2.4 Escalation

- **2.4.1 MUST** Breadth: the count of effects in play follows thresholds on the leader's progress toward the points target, capped by the match's cap and by the number of shipped scheduled effects (the last added 2026-11-11, [M2 plan](../plans/m2-plan.md)). Defaults: one from the first point, two from 30%, three from 70%. Speed is always on and outside the count. Done: by test, a target of 5 gives one at leader scores 0–1, two at 2–3, three at 4; a target of 10 gives 0–2, 3–6, 7–9; a target of 21 gives 0–6, 7–14, 15–20; a cap of one or two holds the count there.
- **2.4.2 MUST** Intensity: the rally's hit count, zero at the serve, reset on a point. Done: a scripted rally shows it rise per paddle hit and return to zero after `PointScored`.
- **2.4.3 MUST** Effect parameters change only at a flight start; which effects are in play changes only between points. Done: 2.2.0's test, and a scripted match in which breadth never changes inside a rally.
- **2.4.4 MUST** Selection: a bag of the shipped effects, shuffled per match from the seeded source. Every tuned interval of points scored, the oldest active effect leaves and the next from the bag joins. When the count rises, one joins and none leaves. Done: by seeded test, no effect repeats before the bag empties; exactly one effect changes per rotation; a fault restart rotates nothing; with an empty bench nothing rotates.
- **2.4.5 MUST** The schedule names no effect: it holds thresholds, a cap, and an interval. Done: removing an effect changes nothing in the escalation feature.
- **2.4.6 WONT** Catch-up bias. A playtest hypothesis stands in ([playtest log](../playtest.md)).

Coupling: 2.4.2 hands 2.3.4 its number.

## 2.5 Termination and safety

- **2.5.1 MUST** Rallies end under Chaos. Done: seeded AI-versus-AI Chaos matches show bounded rally length; the same harness with every dial at zero and no speed ramp does not, confirming the fact relied on.
- **2.5.2 MUST** R3 holds for the stack. Done: AI-versus-AI rally length shortens as breadth and hit count rise together; the same measure per effect is reported in its phase report.
- **2.5.3 MUST** Soak per mode: M1's soak under Classic; under Chaos, every subset of the shipped effects up to the cap, each at full dial, panic handler on. Done: all pass in CI inside the per-test timeout.
- **2.5.4 MUST** Sanity's invariants hold under Chaos. Done: 2.5.3 raises no `SimFault`.
- **2.5.5 MUST** The AI's stars order correctly in Chaos. Done: seeded win rates rise from one star to two to three.
- **2.5.6 MUST** Sanity: a flight ends within a tuned time. It rides the existing fault path: dev panics, release restarts the rally and logs. Done: an injected endless flight raises the fault; under the release policy the heal restarts the rally.

## 2.6 Roster

Each package is held to R1 to R11 by its own tests. Numbers are tuning, found by playtest; the spec fixes each dial's meaning.

- **2.6.0 MUST** Speed. Link: reach. Channel: 2.2.4. Dial: ball speed after the hit, from serve speed to the cap. Chaos's gain is smaller than Classic's. AI flaw: inherent. Shell half: declared empty. Done: after the same number of hits a Chaos ball is slower than a Classic one, by test.
- **2.6.1 MUST** Shrink. Link: reach. Channel: 2.2.3. Dial: height of both paddles, from the default to the minimum. AI flaw: inherent in the smaller paddle. Done: both paddles reach the tuned minimum at full dial and restore on a point, by test. Coupling: the hit offset is relative to paddle height, so the angle mapping scales with it.
- **2.6.2 MUST** Wind. Link: predict. Channel: 2.2.1. Dial: drift over one full crossing, in arena units, from zero to a tuned maximum under half the playable height. Its direction is drawn per rally from the seeded source and shown in the countdown. AI flaw: prediction error scales with strength. Done: prediction equals simulation within one unit at every tested dial; the drift at the paddle matches the dial at two ball speeds.
  - **SHOULD** The wall cue is rate-limited, since wind can pin a slow ball against a wall. Done: a pinned ball produces a bounded number of cues per second, by test.
- **2.6.3 MUST** Fog. Link: perceive. Channel: 2.2.5. Dial: width of one band centred on midfield, from zero to a tuned maximum that leaves a visible run before each paddle. AI flaw: synthetic, prediction error scales with width. Done: the ball, its trail, and its glow are absent inside the band, by a shell test on visibility.
- **2.6.4 MUST** Spin. Link: execute and predict. Channels: 2.2.4 and 2.2.1. Dial: drift over one crossing from a moving-paddle hit, from zero to a tuned maximum. Three outcomes: the paddle moving up, still, or down at the hit; the ball curves the way the paddle moved. AI flaw: reads the spin late. Done: by seeded test, a moving-paddle hit curves the ball that way and a still one does not; at dial zero neither does.
- **2.6.5 SHOULD** Lens. Link: predict. Channel: 2.2.1. Dial: count of lenses, from none to a tuned maximum. Each lens has a fixed size and pull toward its centre line; the largest bend one lens gives any crossing is one tuned angle. A lens appears at a paddle hit, is drawn free on one side and copied mirrored on the other, and stays until the point. AI flaw: its belief lags; it does not see items younger than a tuned number of flights. Done: one lens bends no crossing more than the tuned angle; a drawn lens and its copy are mirror images; its diff touches only the touch list (2.3.8).
- **2.6.6 SHOULD** Blocks. Link: predict. Channel: 2.2.6. Dial: count of blocks, from none to a tuned maximum. Each block has the default paddle's length and the paddle's thickness; its place and angle are drawn free on one side and copied mirrored on the other; the angle range is tuning; blocks stay clear of both paddles' contact zones. A block appears at a paddle hit and stays until the point. AI flaw: its belief lags, as Lens. Done: a drawn block and its copy are mirror images, by seeded test; 2.2.6's conditions hold.
- **2.6.7 MAY** Ice. Link: execute. Channel: 2.2.7. Dial: time for a paddle to reach or shed full speed, from zero as today to a tuned maximum. AI flaw: overshoot, inherent. Done: defined only if taken.
- **2.6.8 WONT** Squeeze (paddles advance toward the centre).
- **2.6.9 WONT** Multi-ball. Parked, potentially for co-op.

Coupling: Wind and Spin use the same field with different causes; whether they feel alike is a playtest question (2.10).

## 2.7 Display

- **2.7.1 MUST** An icon per active effect; the one joining enters large during the countdown. Done: a viewer names the active effects during the countdown without reading text.
- **2.7.2 MUST** The rally's hit count is shown as a number. Done: visible in Chaos, rising per hit.
- **2.7.3 WONT** A meter bar.
- **2.7.4 MUST** No presentation element violates R11. Done: review item at milestone close.

## 2.8 Mode choice and persistence

- **2.8.1 MUST** The mode screen comes first, before opponent: Classic, Chaos ●, Chaos ●●, Chaos ●●●. Done: all four playable keyboard-only from the menu.
- **2.8.2 MUST** The last mode and cap picked are persisted in settings. Done: the last row is preselected on next start; settings fixture v3 and a v2 migration test.
- **2.8.3 MUST** Records are kept per mode, not per cap; M1's records become Classic's. Done: profiles fixture v2; a v1 file migrates with its records under Classic; a Chaos match leaves Classic's records untouched, by test.
- **2.8.4 MUST** Match over shows the played mode's records delta. Done: by test after a Chaos match.

## 2.9 Developer tooling

- **2.9.1 SHOULD** A sandbox forces the active effects and the hit count from the overlay. Done: reachable in a release build.
- **2.9.2 SHOULD** The overlay lists active effects and the hit count. Done: visible.

## 2.10 Playtest

- **2.10.1 MUST** One logged session per shipped effect, in the observe-then-interpret template. Done: entries exist in `docs/playtest.md`. Amended 2026-11-11 ([M2 plan](../plans/m2-plan.md) 5.4): the kids first play Chaos after Wind lands, and that session covers Shrink and Wind.
- **2.10.2 MUST** Drop rule: an effect is dropped by the user's ruling after its session, citing a logged observation, after one retune. A dropped effect is removed by its touch list. Done: the ruling is in the playtest log and the removal's diff in the phase report.
- **2.10.3 MUST** The rotation interval (2.4.4) is chosen from play. Done: the value and the observation behind it are in the playtest log.

## 2.11 Milestone close

- **2.11.1 MUST** ADRs: 0011 (effect identity and contract), plus one per deviation a phase report records. Done: files exist.
- **2.11.2 MUST** Checklist: CI green, docs updated, R1 to R11 per shipped effect, touch list measured, R11 review signed. Done: checklist marked in this file.

## Deliberately absent

- Unlock pool, career or campaign progression: deferred; C3b's campaign is where it would land.
- Catch-up bias: no session has shown a lopsided pairing in this game, and side-targeting would force every effect to define a side.
- A job for the action key: C2.
- The player choosing effect types: the cap is the only choice.
- Multi-ball and the ball population channel: parked, potentially for co-op.
- Radial forces and a true gravity well: lateral-only forces give termination by construction; radial forces need a clamp and come later if wanted.
- A bounce budget and item spacing: MAY, revisited on evidence; blocks stay solid always.
- A pushing lens: one kind keeps the behaviour learnable.
- Redrawing the whole effect set each point: three new icons do not read in a three-second countdown.
- A meter bar: the effects show intensity themselves, and the hit count is the number.
- Decoys: parked.
- Reversed controls: no dial (R9); fits a C2 item.
- Portals: no dial (R9); fits a C3 room.
- Random deflection: rejected, fails R2 and R5.
- Closing walls: rejected, a smaller arena is easier, fails R3.
- Flicker and rotating view: rejected, fail R11.
- A bake-off of prototypes: rejected in favour of pluggability (ADR 0011).
- Rooms, bosses, items, beat: later milestones.

## Open questions

None.

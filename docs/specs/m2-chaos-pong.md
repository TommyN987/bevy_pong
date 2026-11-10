# M2: Chaos Pong

Specified 2026-11-11. Second milestone, design candidate C1, per [design.md](../design.md) ("C1 direction") and [architecture.md](../architecture.md).
Chaos Pong is a second ruleset beside Classic: rallies escalate by perturbations, not mainly by speed.
Obligation levels: MUST, SHOULD, MAY, WONT. Every item carries a done-condition.
The spec is not complete: see "Open questions" at the end.

## Facts relied on

- `integrate` takes a force, and `crossing_y` passes zero through the same call. [confirmed, `pong_core/src/kernel/predict.rs`, `integrate.rs`]
- `advance_between_walls` requires a step's vertical travel to stay under the playable height, so one fold lands inside. [confirmed, `pong_core/src/kernel/integrate.rs`]
- Classic's speed rule: serve 80, plus 7 per hit, cap 250. [confirmed, `tuning.ron`]
- Without escalation a competent defender returns every ball: at serve speed the ball's vertical speed tops out near 69 units/s, under the paddle's 90. [probable, inferred from `tuning.ron`; verified by 2.5.1]
- Rules counts paddle hits in `Rally` and resets it on entering `Countdown`. [confirmed, `pong_core/src/features/rules/mod.rs`]
- Sanity faults unless exactly one ball exists. [confirmed, `pong_core/src/features/sanity/mod.rs`]
- `Paddle` has a size setter clamped to tuning bounds: minimum 8, default 14. [confirmed, spec 1.4.3, `tuning.ron`]
- The kernel has swept circle-versus-rectangle collision. [confirmed, spec 1.2.3]
- `BallHitPaddle` carries the paddle's side only. [confirmed, spec 1.3.2]
- Whether the paddle's motion at the moment of a hit is available to the collide phase. [unknown; read `features/paddle` and `features/physics` before 2.6.4 is planned]
- Whether paddle motion is instantaneous, with no velocity state. [uncertain, not read; matters only to 2.6.7]
- Profiles file is at v1, settings file at v2. [confirmed, spec 1.4.9]
- The kids' rally-length distribution. [unknown; one local profile records a longest rally of 21, owner unknown; observed in the 2.10 sessions]
- `Material2d` and custom 2D shaders exist in Bevy 0.20. [probable, research 2026-10-30; `bevy-api-check` against 0.20.1 before 2.6.3 is planned]
- `bevy_hanabi` supported Bevy 0.19 only on 2026-10-30. [confirmed then, uncertain now; no package below depends on it]

## Decisions taken during specification

- Classic Pong stays selectable, unchanged. Chaos Pong is a variant beside it, not a layer on top.
- A mode is a ruleset: data over one simulation. `MatchConfig` carries which one is played.
- Chaos keeps a small speed ramp, smaller than Classic's.
- Chaos escalates on two scales. The match sets breadth: which perturbations are in play, changing only between points. The rally sets intensity: one number handed to each active perturbation, rising on paddle hits and resetting on a point.
- The match scale is driven by the leader's progress toward the points target.
- The career scale is deferred, as a possible campaign (design.md, "C1 direction"). M2 persists no unlock pool.
- "Perturbation" is the working name for what the architecture calls an effect.
- Perturbations are pluggable and independent of each other ([ADR 0011](../adr/0011-perturbation-identity-and-contract.md)): identity is a closed enum, behaviour sits behind a contract with a core half and a shell half. The touch list is a SHOULD.
- No bake-off of unpolished prototypes. Every perturbation ships at the quality bar and is playtested as the real thing.
- The action key stays unused until C2.
- Records are kept per ruleset.
- Forces in M2 are lateral only: along y. Horizontal speed changes only at a paddle. A lens stands in for the design's gravity well.
- Roster: Shrink, Wind, Fog, Spin are MUST; Lens and Blocks are SHOULD; Ice is MAY; Squeeze and Multi-ball are WONT. Multi-ball is parked, potentially for co-op.
- Spin is accepted: a paddle moving at the hit curves the ball. It amends design.md's last invariant: a force is a pure function of ball state, position, and time.
- Every perturbation is held to the selection criteria R1 to R11 below.

## Selection criteria

Each perturbation in 2.6 passes these. A criterion's "falsified if" is its check.

| # | Level | Criterion | Falsified if |
|---|---|---|---|
| R1 | MUST | The cause is on screen before the consequence | A child cannot point at what changed the ball |
| R2 | MUST | Skill still decides the point | A seeded run shows the outcome independent of paddle input |
| R3 | MUST | It raises miss probability, monotone in its dial | AI-versus-AI rally length does not shorten as the dial rises |
| R4 | MUST | It terminates: no orbit, no stall | The soak finds a rally without a goal inside a bounded step count |
| R5 | MUST | The AI predicts it through shared math, with a tunable flaw | The zero-flaw AI misses, or the flaw dial changes nothing |
| R6 | SHOULD | It is reusable as a room ingredient and an item payload | It needs escalation state to function |
| R7 | SHOULD | It teaches a Bevy area the others do not | Two perturbations share the same new API surface |
| R8 | SHOULD | The roster spans at least three links of the player's loop (perceive, predict, reach, execute, attend) | All shipped perturbations attack one link |
| R9 | MUST | It has a continuous dial that is mild at zero | It is on or off |
| R10 | SHOULD | It gives the hitter a use, not only the defender a tax | No seeded test shows a hit that scores because of it |
| R11 | MUST | No strobing or full-screen flashing | Any view element flashes faster than 3 Hz |

## 2.1 Ruleset

- **2.1.1 MUST** A ruleset is data naming the speed rule and the escalation; Classic and Chaos are its two values; `MatchConfig` carries the one played. Done: a headless Chaos match reaches `MatchOver`.
- **2.1.2 MUST** Classic behaves as M1 shipped it. Done: M1's tests pass unmodified under Classic.
- **2.1.3 MUST** Chaos has its own per-hit speed increase in tuning, smaller than Classic's. Done: after the same number of hits a Chaos rally's ball is slower than a Classic one's, by test.

## 2.2 Composition channels

A perturbation contributes to channels and never touches shared state directly.
Each channel composes commutatively, so perturbations cannot see each other.
The channel model is pending open question 2.

- **2.2.1 MUST** Lateral force channel: the sum of pure functions of ball state, position, and time, along y only; physics and the AI's prediction take the same sum. Done: two dummy contributions compose order-independently, by test; prediction equals simulation under the sum.
- **2.2.2 MUST** The force channel clamps vertical speed so the single-fold precondition holds at every tuned intensity. Done: tuning validation rejects a configuration that could break it; a test at the clamp folds once.
- **2.2.3 MUST** Paddle modifier channel: a product on paddle height per side, clamped by `Paddle`'s API. Done: two dummy modifiers compose order-independently, by test.
- **2.2.4 MUST** Hit hook: a perturbation may set ball state at a paddle hit, given the paddle's motion. Done: a dummy hook observes the paddle's motion at a hit, by test.
- **2.2.5 MUST** View-only perturbations have no simulation contribution; their core half carries identity, intensity, and the AI flaw. Done: 2.6.3 ships with no kernel change.
- **2.2.6 SHOULD** Obstacle set: a union of axis-aligned shapes stepped by collision and prediction alike. Done: with 2.6.6.
- **2.2.7 MAY** Paddle motion model. Done: with 2.6.7.
- **2.2.8 WONT** Ball population: more than one ball.

Coupling: 2.2.1 and M1's 1.2.6 share one integrate function; the AI fairness guarantee of spec 1.2 extends to forces.

## 2.3 Perturbation contract

Per [ADR 0011](../adr/0011-perturbation-identity-and-contract.md). The contract's form is open question 1.

- **2.3.1 MUST** Identity is a closed, serializable enum with stable names. Done: a round-trip test over every variant.
- **2.3.2 MUST** A core half: simulation through 2.2's channels, an AI flaw, a tuning section mapping intensity to its own parameters. Done: a perturbation missing any part does not compile or does not register.
- **2.3.3 MUST** A shell half: view and cue. Done: a perturbation without a shell half fails a test or does not compile.
- **2.3.4 MUST** A perturbation receives one intensity number and knows nothing of the escalation that produced it. Done: every perturbation's tests drive it with a bare number, no escalation plugin.
- **2.3.5 MUST** Perturbations do not import each other. Done: `arch-check` enforces it.
- **2.3.6 MUST** A perturbation's clock is fixed-step time. Done: a pause and resume mid-flight leaves prediction equal to simulation, by test.
- **2.3.7 MUST** Core emits a start and an end message per perturbation, observed by the shell. Done: a headless test sees both in order.
- **2.3.8 SHOULD** Touch list: adding a perturbation touches its own core module, its own shell module, one enum variant, one registration line per crate, and one tuning section, and nothing in physics, AI, rules, escalation, sanity, or the HUD. Done: measured on 2.6.5's diff.

## 2.4 Escalation

- **2.4.1 MUST** Match scale sets breadth: which perturbations are in play, driven by the leader's progress toward the points target, changing only between points. Done: a scripted match shows breadth rising with the leader's score and never changing inside a rally.
- **2.4.2 MUST** Rally scale sets intensity: one number rising on paddle hits and resetting on a point. Done: a scripted rally shows it rise per hit and return to its start after `PointScored`.
- **2.4.3 MUST** The simulation's perturbation state changes only at a paddle hit or between points. Done: a test stepping a flight between two hits sees no change.

Coupling: 2.4 hands 2.3.4 its number. The schedules are open questions 3 and 4.

## 2.5 Termination and safety

- **2.5.1 MUST** Rallies end under Chaos. Done: seeded AI-versus-AI Chaos matches show bounded rally length; the same harness with every dial at zero and no speed ramp does not, confirming the fact relied on.
- **2.5.2 MUST** R3 holds for the stack. Done: AI-versus-AI rally length shortens as breadth and intensity rise together.
- **2.5.3 MUST** Soak per ruleset: M1's soak under Classic, and under Chaos with every shipped perturbation at full intensity together, panic handler on. Done: both pass in CI inside the per-test timeout.
- **2.5.4 MUST** Sanity's invariants hold under Chaos. Done: 2.5.3 raises no `SimFault`.

## 2.6 Roster

Each package passes R1 to R11 by its own tests. AI flaws are tunable per perturbation (R5).

- **2.6.1 MUST** Shrink: both paddles shorten with intensity. Link: reach. Channel: 2.2.3. AI flaw: inherent in the smaller paddle. Done: both paddles reach the tuned minimum at full intensity and restore on a point, by test. Coupling: the hit offset is relative to paddle height, so the angle mapping scales with it.
- **2.6.2 MUST** Wind: a steady vertical push, its direction shown in the countdown. Link: predict. Channel: 2.2.1. AI flaw: prediction error scales with strength. Done: prediction equals simulation within one unit at every tested strength.
  - **SHOULD** The wall cue is rate-limited, since wind can pin a slow ball against a wall. Done: a pinned ball produces a bounded number of cues per second, by test.
- **2.6.3 MUST** Fog: a midfield band hides the ball. Link: perceive. Channel: 2.2.5. AI flaw: synthetic, prediction error scales with band width. Done: the ball, its trail, and its glow are absent inside the band, by a shell test on visibility.
- **2.6.4 MUST** Spin: a paddle moving at the hit curves the ball; a still paddle does not. Link: execute and predict. Channels: 2.2.4 and 2.2.1. AI flaw: reads the spin late. Done: a moving-paddle hit curves the ball and a still one does not, by seeded test; at intensity zero neither does.
- **2.6.5 SHOULD** Lens: a band bends paths toward or from a line. Link: predict. Channel: 2.2.1. AI flaw: as Wind. Done: its diff touches only the touch list (2.3.8).
- **2.6.6 SHOULD** Blocks: axis-aligned midfield blocks that grow with intensity. Link: predict. Channel: 2.2.6. AI flaw: ignores blocks beyond a count. Done: prediction equals simulation through one bank shot.
- **2.6.7 MAY** Ice: paddles gain inertia. Link: execute. Channel: 2.2.7. AI flaw: overshoot, inherent. Done: defined only if taken.
- **2.6.8 WONT** Squeeze (paddles advance toward the centre).
- **2.6.9 WONT** Multi-ball. Parked, potentially for co-op.

## 2.7 Escalation state on screen

- **2.7.1 MUST** The perturbations joining the next rally are shown during the countdown. Done: a viewer names the active perturbations during the countdown without reading text.
- **2.7.2 MUST** No presentation element violates R11. Done: review item at milestone close.

The form is open question 6.

## 2.8 Mode choice and persistence

- **2.8.1 MUST** The menu offers Classic and Chaos. Done: both playable keyboard-only from the menu.
- **2.8.2 MUST** The last mode picked is persisted in settings. Done: preselected on next start; settings fixture v3 and a v2 migration test.
- **2.8.3 MUST** Records are kept per ruleset; M1's records become Classic's. Done: profiles fixture v2; a v1 file migrates with its records under Classic; a Chaos match leaves Classic's records untouched, by test.

## 2.9 Developer tooling

- **2.9.1 SHOULD** A sandbox forces breadth and intensity from the overlay. Done: reachable in a release build.
- **2.9.2 SHOULD** The overlay lists active perturbations and the intensity. Done: visible.

## 2.10 Playtest

- **2.10.1 MUST** One logged session per shipped perturbation, in the observe-then-interpret template. Done: entries exist in `docs/playtest.md`.
- **2.10.2 MUST** A dropped perturbation is removed by its touch list. Done: the removal's diff is recorded in the phase report.

The drop rule is open question 9.

## 2.11 Milestone close

- **2.11.1 MUST** ADRs: 0011 (perturbation identity and contract), plus one per decision the open questions turn into structure. Done: files exist.
- **2.11.2 MUST** Checklist: CI green, docs updated, R1 to R11 per shipped perturbation, touch list measured, R11 review signed. Done: checklist marked in this file.

## Deliberately absent

- Unlock pool, career or campaign progression: deferred; C3b's campaign is where it would land.
- A job for the action key: C2.
- Multi-ball and the ball population channel: parked, potentially for co-op.
- Radial forces and a true gravity well: lateral-only forces give termination by construction; radial forces need a clamp and come later if wanted.
- Decoys: parked.
- Reversed controls: no dial (R9); fits a C2 item.
- Portals: no dial (R9); fits a C3 room.
- Random deflection: rejected, fails R2 and R5.
- Closing walls: rejected, a smaller arena is easier, fails R3.
- Flicker and rotating view: rejected, fail R11.
- A bake-off of prototypes: rejected in favour of pluggability (ADR 0011).
- Rooms, bosses, items, beat: later milestones.

## Open questions

The spec is complete when this section is empty.

1. **Contract form.** A Rust trait, or a plugin plus a descriptor in ECS idiom (ADR 0011 leaves it to this spec).
2. **Channel model.** Section 2.2 was proposed and built on, never ruled on as a whole; ADR 0011 still records it as proposed.
3. **Breadth schedule.** How the leader's progress maps to a count; the order perturbations join in, fixed or seeded; whether the first point of a match has any.
4. **Intensity schedule.** The step per hit, the ceiling, and the starting value of a rally.
5. **Catch-up bias.** Design's invariant is advisory with its form open: in M2 or not, and by what mechanism.
6. **Form of the escalation display** (2.7): a meter, icons, the arena itself.
7. **AI difficulty per ruleset.** Whether Chaos has its own three star entries in tuning.
8. **Mode choice in the menu** (2.8.1): where it sits in the flow and how it reads without text.
9. **Drop rule** (2.10.2): what observation drops a perturbation.
10. **Names.** Whether code and docs say "perturbation" or keep "effect" and `EffectId`.
11. **Wind's direction.** Who picks it and when: seeded per point, alternating, or fixed per match.
12. **Records on screen.** Which ruleset's records match over and the profile screens show.

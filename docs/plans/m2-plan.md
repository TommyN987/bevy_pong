# M2 Implementation Plan

Decided 2026-11-11. Sequences [m2-chaos-pong.md](../specs/m2-chaos-pong.md) into phases.
Spec item numbers (2.x.y) refer to the M2 spec; plan items are cited as "M2 plan N.M". Each phase is one branch and one PR.

## Ordering rationale

1. The refactor comes first: M1's suite is the oracle before any behaviour changes.
2. A playable Chaos comes before most effects: mode, schedule, menu, and display exist by the second effect, so each later effect is played as it lands.
3. Persistence lands with the first playable Chaos: otherwise Chaos results are written into Classic's records.
4. The AI harness precedes the force effects: it is the instrument for R3, termination, and star order, and every later phase reports through it.
5. One effect per phase and per PR: the PR's diff is then the touch list, measured for free.
6. Wind before Spin before Fog: Spin reuses Wind's field, and the Wind-versus-Spin hypothesis needs both. Fog's shader is the riskier fact, and it is isolated: its failure blocks nothing else.
7. The kids first play Chaos after Wind lands: shrinking paddles alone are not chaos.

## Working agreement

As in the [M1 plan](m1-plan.md): git is the user's domain, and pair-programming roles vary per phase or per item.
Who drives each phase is decided when its implementation starts (P.1).

## Phase protocol

P.1 to P.6 of the [M1 plan](m1-plan.md) carry over unchanged.

## Effect protocol

Every effect phase (3 for Shrink's shell half, then 5 to 10) follows it.

- **E.1 MUST** `bevy-api-check` on every shell API the effect's view needs, before code. Done: the report is in the session.
- **E.2 MUST** Core half, tuning section, and tests for R2, R4, R5, R9. Done: green.
- **E.3 MUST** Shell half: view, cue, icon, held to R1 and R11. Done: visible in `just run`.
- **E.4 MUST** The soak's subsets include the effect; its R3 measure goes in the phase report. Done: CI green; number reported.
- **E.5 SHOULD** The PR diff is compared with the touch list (spec 2.3.8). Done: files outside it are named in the report.
- **E.6 MUST** One playtest session, then the user's ruling: keep, retune once, or drop (spec 2.10.2). Done: logged. Shrink's session is held with Wind's (5.4).
- **E.7 SHOULD** A seeded test shows a hit that scores because of the effect (R10). Done: green, or its absence is reported.

## Phases

### 1. Refactor: flight, contract, Speed
Realizes spec 2.0, 2.1.2, 2.2.0, 2.2.4, 2.3.1 through 2.3.6, 2.3.9 through 2.3.11, 2.6.0 for Classic. Depends on nothing. No new behaviour.

- **1.1 MUST** Kernel: the field trait and the composed flight; prediction and the wall step take a flight. Done: prediction equals simulation under two dummy fields, in either order.
- **1.2 MUST** The flight sits on the ball and owns the rally's hit count; `Force` is removed; rules keeps the records only. Done: no `Force` in `pong_core`; M1's rally-record tests pass. Amended 2026-11-11 ([phase 1 report](m2-phase1-report.md)): the flight counts returns, not touches (spec 2.2.4, amended).
- **1.3 MUST** The paddle exposes its step motion. Done: spec 2.0.3's test.
- **1.4 MUST** `EffectId`, the effect trait, the curve type, one registration match per crate, the base plugin. Done: removing a match arm fails the build. Amended 2026-11-11 ([phase 1 report](m2-phase1-report.md)): the curve type moved to 2.3, with Shrink as its first user; the shell's registration match moved to 3.3, where the icons first consume an effect. Phase 1 built the core's match.
- **1.5 MUST** Speed as the first effect; Classic's tuning carries its gain. Done: spec 2.0.4's test.
- **1.6 MUST** The feature order gains `effects`, after paddle and before physics (spec 2.0.5); `arch-check` learns that effects do not import each other. Done: a planted cross-import fails `arch-check`; ADR 0002's list matches `features/mod.rs`.
- **1.7 MUST** M1's suite passes with no assertion changed. Done: the phase report lists every call site changed for a changed signature, and any test that differs in the last float bits.
- **1.8 MUST** Classic feels the same by hand. Done: the user's check in `just run`.

### 2. Chaos, headless
Realizes spec 2.1.1, 2.2.3, 2.3.7, 2.4, 2.5.6, 2.6.0 for Chaos, 2.6.1's core half. Depends on 1.

- **2.1 MUST** Mode in `MatchConfig`, with the cap. Done: compiles; vocabulary count reported. Amended 2026-11-11 ([phase 2 report](m2-phase2-report.md)): `Mode::Chaos { cap: NonZeroU8 }`, no new cap type; a cap above the schedule's reach never binds. Vocabulary: 9 capped types of 12.
- **2.2 MUST** Escalation, placed after rules in the feature order: count from thresholds, the cap, and the shipped effects; selection as a pure function over any list, tested without real effects. Done: spec 2.4.1's table and 2.4.4's conditions, by test.
- **2.3 MUST** Paddle modifier channel and Shrink's core half. Done: spec 2.6.1's test. Amended 2026-11-11 ([phase 2 report](m2-phase2-report.md)): the shared curve type (spec 2.3.9) was built here, as `kernel/ramp.rs`, yielding a number only.
- **2.4 MUST** Enter and leave messages. Done: spec 2.3.7's test.
- **2.5 MUST** Flight-time invariant in sanity. Done: spec 2.5.6's tests.
- **2.6 MUST** A headless Chaos match reaches `MatchOver`. Done: green.

### 3. Chaos on screen
Realizes spec 2.7, 2.8, 2.9, 2.6.1's shell half. Depends on 2.

- **3.1 MUST** Profiles v2 and settings v3, with fixtures and migrations. Done: spec 2.8.2 and 2.8.3.
- **3.2 MUST** Mode screen, first in the flow. Done: spec 2.8.1.
- **3.3 MUST** Icons, the joining icon in the countdown, the hit count. Done: spec 2.7.1 and 2.7.2. Amended 2026-11-11 ([phase 1 report](m2-phase1-report.md)): the shell's registration match (spec 2.3.3, 2.3.10) is built here, moved from 1.4.
- **3.4 MUST** Shrink's view, cue, and icon (E.1, E.3); match over shows the played mode's delta. Done: spec 2.8.4; visible.
- **3.5 SHOULD** Overlay lines and the sandbox. Done: spec 2.9.
- **3.6 MUST** A Chaos match with Shrink plays through by hand. Done: the user's check in `just run`. The kids' session waits for 5.4.

### 4. AI under effects, and the harness
Realizes spec 2.3.12, 2.5.1 through 2.5.5. Depends on 2.

- **4.1 MUST** The flaw's two forms in the AI, read from each effect's tuning. Done: spec 2.3.12 for Shrink. Amended 2026-11-13 ([phase 4 report](m2-phase4-report.md)): the two forms are built in the AI and proved with probe effects. "Read from each effect's tuning" moved to 5.2: Shrink's flaw is inherent and has no numbers, and Wind is the first effect that has.
- **4.2 MUST** AI-versus-AI harness reporting rally-length distribution and win rates. Done: spec 2.5.1 and 2.5.5. Amended 2026-11-13 ([phase 4 report](m2-phase4-report.md)): the harness showed no rally between two or three stars ending at M1's handicaps; the user ruled a retune in this phase, and Classic is held to spec 2.5.1 and 2.5.5 beside Chaos.
- **4.3 MUST** Soak over subsets; its running time measured. Done: CI green; the time is in the report, with a spec amendment if the subsets of six effects cannot fit. Amended 2026-11-13 ([phase 4 report](m2-phase4-report.md)): one test per subset size, each dividing a hundred thousand live steps among its subsets, and one soak of Chaos as scheduled. The time is in the report; the subsets of six effects fit by construction, so the spec needed no amendment for fit, only for the form (spec 2.5.3).
- **4.4 MUST** E.4 for Shrink. Done: its R3 measure is in the report.

### 5. Wind
Realizes spec 2.2.1, 2.2.2, 2.6.2. Depends on 3 and 4. Effect protocol.

- **5.1 MUST** The vertical speed clamp and its tuning validation. Done: spec 2.2.2. Amended 2026-11-13 ([phase 5 report](m2-phase5-report.md)): the bound is derived in the wall step and validation gained no rule for it (spec 2.2.2, amended).
- **5.2 MUST** Wind per E.1 through E.5 and E.7; direction drawn per rally. Done: spec 2.6.2. Amended 2026-11-13 ([phase 4 report](m2-phase4-report.md)): Wind's tuning section carries the first per-star flaw numbers (moved from 4.1). Amended 2026-11-13 ([phase 5 report](m2-phase5-report.md)): built with one base capability the proposal added, the rally's seed (spec 2.2.4, amended). R3 against three stars, owed since phase 4, is shown here through the stack.
- **5.3 SHOULD** Wall cue rate limit. Done: spec 2.6.2's sub-item.
- **5.4 MUST** The kids' first Chaos session, covering Shrink and Wind (E.6 for both). Done: logged, with a ruling per effect.

### 6. Spin
Realizes spec 2.6.4. Depends on 5. Effect protocol.

- **6.1 MUST** Spin per E.1 through E.7. Done: spec 2.6.4. Amended 2026-11-13 ([phase 6 report](m2-phase6-report.md)): built with one base capability the proposal added, the sandbox's held hit motion (spec 2.9.1, amended), and with the flaw as a believed share, not a delay (spec 2.6.4, amended).
- **6.2 MUST** The Wind-versus-Spin hypothesis observed in its session. Done: outcome in the playtest log.

### 7. Fog
Realizes spec 2.2.5, 2.2.9, 2.6.3. Depends on 4. Effect protocol.

- **7.1 MUST** Fog per E.1 through E.7; fog hides the ball only. Done: spec 2.6.3 and 2.2.9.

### 8. Lens
Realizes spec 2.6.5, 2.3.8. Depends on 5. Effect protocol.

- **8.1 SHOULD** Lens per E.1 through E.7, with mirrored items and belief lag. Done: spec 2.6.5. Amended 2026-11-11 ([phase 2 report](m2-phase2-report.md)): the curve's whole-count form and the item rule (spec 2.3.9) are built here, with the first item effect.
- **8.2 SHOULD** Touch list measured on this PR. Done: the result is in the report.

### 9. Blocks
Realizes spec 2.2.6, 2.6.6. Depends on 4. Effect protocol.

- **9.1 SHOULD** Read `swept_circle_rect` for a sweep that starts inside the rectangle. Done: the fact is in the spec.
- **9.2 SHOULD** Obstacle channel: tilted sweep, bounded contact loop, horizontal floor. Done: spec 2.2.6.
- **9.3 SHOULD** Blocks per E.1 through E.7. Done: spec 2.6.6.

### 10. Ice
Realizes spec 2.2.7, 2.6.7.

- **10.1 MAY** Decided after phase 9, by the user's ruling. Done: taken with its own items, or recorded as not taken.

### 11. Tune and close
Realizes spec 2.10.3, 2.11. Depends on every shipped phase.

- **11.1 MUST** Rotation interval chosen from play. Done: spec 2.10.3.
- **11.2 MUST** The `milestone-close` skill against spec 2.11. Done: checklist marked.
- **11.3 MAY** The computer spins on purpose (spec 2.6.4's sub-item), decided by the user's ruling before 11.2, with every effect built and playtested. Done: taken with its own items, or recorded as not taken. Added 2026-11-13 ([phase 6 report](m2-phase6-report.md)).

## User actions outside the plan

- A branch per phase, and its commits, push, PR, and merge (P.1, P.5).
- The sessions with the kids (5.4, then one per effect phase) and the ruling after each.
- The hand checks in 1.8 and 3.6.

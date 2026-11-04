# ADR 0002: Intra-crate layering: a flow layer, a feature order, split vocabulary counting

Date: 2026-11-04
Status: accepted

## Context

The phase 3 review (2026-11-03) found the code and the intra-crate rules of `docs/architecture.md` in disagreement.

- Rule 1 forbids feature-to-feature imports.
  Physics imports ball and paddle; rules imports ball and seats; paddle imports seats; every feature but tuning imports tuning.
- `app/` is documented as the roof nothing depends on.
  Every feature imports `app::phases`; ball, paddle, and rules import `app::states`.
- `app::phases` imports `features::rules::MatchState`, which imports `app::phases`: a cycle.
- Rules called `SeatIntents::clear()` directly, against rule 5.
- The architecture conflicts with itself.
  Rule 4 makes every system name a phase set that the directory sketch places in `app/`.
  Rule 5 lets any feature mutate another feature's component, which requires importing its type.
- Vocabulary held twelve types against a cap of "under a dozen"; the phase report counted eleven.
  The events the architecture already plans (`EffectStarted`, `EffectEnded`) guarantee overflow at C1.

## Decision

Add a `flow` layer between vocabulary and features, let features import data types of earlier features in a declared order, and cap vocabulary types separately from messages.

- `pong_core/src/flow/` holds `AppState`, `MatchState` and their registration, `FixedStepPhase` and the phase-set configuration, the step gate, and `FIXED_HZ` (ADR 0003).
  It imports only kernel, domain, and Bevy.
- `app/` keeps `CorePlugins`, the error policy, and the Boot gate, which reads tuning (later settings and profiles).
- Rule 1: a feature imports kernel, domain, flow, and the public components, resources, and messages of features earlier in the order declared in `features/mod.rs`:
  `tuning, seats, ball, paddle, physics, rules, ai, sanity, persistence`.
  Never another feature's systems or plugin internals; never `app/`.
- Seats clears its own intents on entering `Countdown`; rules no longer imports seats.
- Rule 3: the cap of twelve counts non-message vocabulary types.
  Messages have no cap; each has exactly one emitting feature.
  `Notice` is the exception: any feature with a player-actionable failure emits it.
- Count on 2026-11-04: types 6 of 12 (`Side`, `Seat`, `Difficulty`, `Controller`, `MatchConfig`, `Random`); messages 6 (`Serve`, `BallHitPaddle`, `BallHitWall`, `PointScored`, `MatchEnded`, `Notice`).

## Alternatives considered

- Amend rule 1's text only, keep states and phases in `app/`: the cycle stays, and the roof stops meaning anything.
- Move shared components into domain: breaks rule 3; domain becomes the god module the architecture warns about.
- Events owned by the feature that emits them: scatters the core-to-shell contract that architecture constraint 8 and the audio cue map rely on. Considered as the fallback; not chosen.
- Raise the cap to sixteen: arbitrary, and would be raised again.

## Consequences

- The feature order is a contract.
  A new feature takes a place in the list.
  A needed import from a later feature means the order or the design is wrong.
- Plan 3.11 makes the move; 3.11.1 (SHOULD) adds `just arch-check`, a grep check that relies on test modules sitting at the end of each file.
- Known additions bring types to 10 of 12 through C2: `ProfileId` (M1 persistence), `EffectId` (C1), `BossId` (C3), `ItemId` (C2).
- Heal (plan 6.1) re-serves through rules, so `Serve` keeps one emitter.
- Amends `docs/architecture.md` rules 1, 3, 4 and the directory sketch; spec 1.3.2 and 1.3.3.

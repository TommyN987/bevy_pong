# ADR 0011: Effect identity is a closed enum; behaviour sits behind a contract

Date: 2026-11-10, amended 2026-11-11
Status: accepted

## Context

The C1 spec round reopened the design's effect list: any effect may be dropped, changed, or added (`docs/design.md`, "C1 direction").
Architecture constraint 6 says fun is found by playtesting, so the roster is expected to change after the kids play.
Architecture rule 6 makes every extension point an exhaustive enum: a new variant makes the compiler enumerate every spawn table, cue map, and AI branch.
That guarantees no effect lacks a view or AI handling, and it makes every addition or removal an edit across physics, AI, views, and audio.
Later milestones name effects as data: C3 rooms compose them, C2 items carry them (architecture constraint 2).
Render types are unreachable from `pong_core` (ADR 0001), so no single type can hold an effect's simulation and its view.
The kernel is ECS-free and the AI calls the ball's math directly (architecture rule 2, constraint 4), so an effect's force cannot live in a system.
M1's `Force` component is a per-step accumulator, which design's force invariant says cannot be predicted.

## Decision

Identity stays a closed enum; behaviour moves behind a per-effect contract.

- The identity enum (`EffectId`) stays closed and serializable, since rooms and items will name its variants.
- Behaviour is a Rust trait with no default methods: an effect states its stance on every channel, its AI flaw, and its tuning.
- Registration is one exhaustive match over the enum per crate: completeness is checked by the compiler at two sites, not at every use.
- ECS wiring is one base-owned plugin; an effect writes systems only for state the base lacks.
- Each effect has a core half (simulation, AI flaw, tuning) and a shell half (view, cue, icon). A shell half may be declared empty; a missing one does not compile.
- Effects do not depend on each other. They contribute to composition channels, each of which composes commutatively, and never touch shared state directly.
- The unit of constancy is the flight: serve or paddle hit, until the next paddle hit or a goal. An effect's parameters change only at a flight start; which effects are in play changes only between points.
- A field is a pure function of the ball's motion (position, velocity) and time in flight. Physics and the AI's prediction evaluate the same composed flight. This replaces M1's `Force` accumulator.
- Bounds sit on the channel, not in each effect: the speed cap and the vertical speed clamp belong to the base.
- Classic's speed ramp is the first effect on the contract (Speed), built in a refactor before any new behaviour.
- Touch list, SHOULD: adding an effect touches its own core module, its own shell module, one enum variant, one registration line per crate, and one tuning section, and nothing in physics, AI, rules, escalation, sanity, or the HUD.
- The touch list is a SHOULD, not a MUST: an effect may need physics the base does not have yet, and that alone does not rule it out.
- Rule 6 still binds boss and item identities as written.

Amended 2026-11-11: "perturbation", the first round's working name, is retired for "effect"; the contract's form and the channel model, left open on 2026-11-10, are decided as above.

## Alternatives considered

- Rule 6 unchanged, exhaustive matches at every site: rejected; dropping or adding an effect edits every spawn table, cue map, and AI branch, the system-wide friction the roster's expected churn cannot afford.
- A bake-off phase of unpolished prototypes, survivors rebuilt at the quality bar: rejected by the user; a prototype invites sloppiness, and polish is part of the fun the bake-off would try to measure.
- A trait with default methods: rejected; a new channel would leave existing effects silently without a stance.
- Effects as systems adding to the `Force` component: rejected; the AI cannot predict a per-step accumulator.
- Speed kept as a per-mode rule outside the contract: rejected; it has the shape of an effect (a value rising on paddle hits, reset on a point), and reading the ball's current speed at each hit would let lateral forces leak into the ramp.

## Consequences

- An effect that proves unfun is removed by deleting its touch list.
- Every effect ships at the quality bar and is playtested as the real thing.
- A mode is data: a list of effects with their curves and a schedule. Classic is Speed alone.
- The refactor is checked by M1's whole test suite, before any new behaviour exists.
- Adding a channel touches every effect by one line, where each states its stance.
- An effect that needs a new base capability pays for it once, outside its touch list.
- The paddle must expose its motion in the current step, for effects that read it at a hit.
- Amends `docs/architecture.md` rule 6 and its "Effects" section, and `.claude/rules/pong_core.md`.

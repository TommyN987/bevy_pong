# ADR 0011: Perturbation identity is a closed enum; behaviour sits behind a contract

Date: 2026-11-10
Status: accepted

## Context

The C1 spec round reopened the design's effect list: any perturbation may be dropped, changed, or added (`docs/design.md`, "C1 direction").
"Perturbation" is the round's working name for what the docs so far call an effect.
Architecture constraint 6 says fun is found by playtesting, so the roster is expected to change after the kids play.
Architecture rule 6 makes every extension point an exhaustive enum: a new variant makes the compiler enumerate every spawn table, cue map, and AI branch.
That guarantees no effect lacks a view or AI handling, and it makes every addition or removal an edit across physics, AI, views, and audio.
Later milestones name perturbations as data: C3 rooms compose them, C2 items carry them (architecture constraint 2).
Render types are unreachable from `pong_core` (ADR 0001), so no single type can hold a perturbation's simulation and its view.

## Decision

Identity stays a closed enum; behaviour moves behind a per-perturbation contract.

- The identity enum (`EffectId` in the architecture) stays closed and serializable, since rooms and items will name its variants.
- Each perturbation implements a contract with a core half (simulation, AI handling, tuning) and a shell half (view, cue).
- The contract's required parts give the completeness rule 6 gave: a perturbation without a view or AI handling does not compile or does not register.
- Perturbations do not depend on each other.
- Touch list, SHOULD: adding a perturbation touches its own core module, its own shell module, one enum variant, one registration line per crate, and one tuning section, and nothing in physics, AI, rules, escalation, sanity, or the HUD.
- The touch list is a SHOULD, not a MUST: a perturbation may need physics the base does not have yet, and that alone does not rule it out.
- Rule 6 still binds boss and item identities as written.

## Alternatives considered

- Rule 6 unchanged, exhaustive matches at every site: rejected; dropping or adding a perturbation edits every spawn table, cue map, and AI branch, the system-wide friction the roster's expected churn cannot afford.
- A bake-off phase of unpolished prototypes, survivors rebuilt at the quality bar: rejected by the user; a prototype invites sloppiness, and polish is part of the fun the bake-off would try to measure.

## Consequences

- A perturbation that proves unfun is removed by deleting its touch list.
- Every perturbation ships at the quality bar and is playtested as the real thing.
- The contract's form (a Rust trait, or a plugin plus a descriptor) is settled in the C1 spec.
- How perturbations compose without seeing each other is settled in the C1 spec; composition channels with bounds on the channel are proposed there, not decided.
- A perturbation that needs a new base capability pays for it once, outside its touch list.
- Amends `docs/architecture.md` rule 6 and its "Effects" section.

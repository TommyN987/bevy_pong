# ADR 0004: Paddles move between rallies; the ball's systems run only in play

Date: 2026-11-07
Status: accepted

## Context

ADR 0003 opened the fixed step only while play is live, so nothing moved during a countdown or a point pause.
The first two-human runs (plan 4.6, 2026-11-07) showed players waiting for the serve unable to line up their paddles.
Amended 2026-11-09 at milestone close: the runs are recorded in the [phase 4 report](../specs/m1-phase4-report.md); plan 4.6 is the ghosting check.
The user decided paddles should move in every wait between rallies.

- The fixed step is one set of phases; every system names its phase (architecture rule 4).
- The ball's systems (serve, integrate, collide, score) must keep ADR 0003's gating: a ball past the goal line scores once.
- The shell's keyboard adapter reported only in `Playing` (plan 4.1), so a tap during a countdown could not latch and fire at the serve.

## Decision

The step gate opens the phases whenever the paddles move: `Countdown`, `Playing`, `PointScored`, with no transition pending.
A new `flow` system set, `BallInPlay`, narrows its members to `Playing` with no transition pending.

- `MatchState::paddles_move` is the one rule; the gate and the shell's adapter both read it (`flow::states::paddles_move`).
- Ball systems join `BallInPlay` beside their phase: `serve_on_request`, `integrate_ball`, `collide_ball_with_paddles`, `score_goal_crossings`, `end_match_at_target`.
  Amended 2026-11-09 at milestone close (`pong_core/src/features/rules/mod.rs`, plan 7.4): `count_rally_hits` joins them, chained before `score_goal_crossings`.
- Their gating is unchanged from ADR 0003, including the step that stops play finishing and later steps of its frame shutting.
- `Paused` and `MatchOver` run no phase.
- The keyboard adapter reports whenever the paddles move; a countdown tap now moves the paddle in the countdown.
- Focus loss still clears intents in every state and pauses only from `Playing` (spec 1.6.1).
  Amended 2026-11-07 after the plan 3.9 check: focus lost in a countdown let it run out and serve unseen, so focus loss pauses wherever the paddles move.

## Alternatives considered

- Paddles move in the countdown only: rejected by the user; the pause after a point is a wait too.
- Keep ADR 0003's gate: rejected by the user; players cannot line up for the serve.

## Consequences

- A new fixed-step system that serves, moves, collides, or judges the ball must also join `BallInPlay`; nothing enforces it, so review does.
- The soak test counts live steps in `BallInPlay`, not in a phase.
- Sanity (plan 6.1) runs between rallies too; a ball resting past the goal line during `PointScored` is legal there.
- AI (phase 5) positions between rallies as players do.
- Amends ADR 0003 (step gate, focus-loss pause), spec 1.4.5 and 1.6.1, and `docs/architecture.md` rule 4, Simulation model, and Input (recorded 2026-11-09 at milestone close).

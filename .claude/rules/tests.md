---
paths:
  - pong_core/tests/**
  - pong/tests/**
  - pong_core/src/testing/**
---
# Test conventions (see docs/specs/m1-basic-pong.md, 1.5)

- Headless only: build apps through `pong_core`'s `testing` module (`MinimalPlugins` plus chosen core plugins, fake intents, temp-dir `SavePaths`, fixed seed). Never `DefaultPlugins` in a test.
- Deterministic: seed the random resource and assert exact sequences, not "eventually".
- Persistence fixtures live in `pong_core/tests/fixtures/<kind>_v<N>.ron` (`profiles`, `settings`), one per released version of each kind, never edited once released. Each kind versions on its own; its fixture-count test must equal its current version (plan 7.1, 2026-11-08).
- Soak test: at least 100,000 fixed steps counted in `Playing`, both seats mixing random intents with prediction-aimed tracking, frame lengths drawn from {½, 1, 2, 4} steps, the speed cap reached, panic handler on, finishing under the nextest timeout of thirty seconds (plan 3.19).
- Frame timing: `testing::frame` runs one frame; `set_frame_length` makes frames of several fixed steps (at most 14 at 60 Hz) or half a step. Test anything a state transition stops with multi-step frames, since a transition requested in a fixed step applies only next frame.
- Name a test after the spec item's done-condition, for example `latch_moves_paddle_once_across_two_fixed_steps`.
- Run tests through `just test` (nextest). Bare `cargo test` is only for `--doc`.

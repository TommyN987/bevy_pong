---
paths:
  - pong_core/tests/**
  - pong/tests/**
  - pong_core/src/testing/**
---
# Test conventions (see docs/specs/m1-basic-pong.md, 1.5)

- Headless only: build apps through `pong_core`'s `testing` module (`MinimalPlugins` plus chosen core plugins, fake intents, temp-dir `SavePaths`, fixed seed). Never `DefaultPlugins` in a test.
- Deterministic: seed the random resource and assert exact sequences, not "eventually".
- Persistence fixtures live in `pong_core/tests/fixtures/save_v<N>.ron`, one per released save version. The fixture-count test must equal the current version.
- Soak test: at least 100,000 fixed steps with random intents for both seats, panic handler on, finishing under the nextest timeout of thirty seconds.
- Name a test after the spec item's done-condition, for example `latch_moves_paddle_once_across_two_fixed_steps`.
- Run tests through `just test` (nextest). Bare `cargo test` is only for `--doc`.

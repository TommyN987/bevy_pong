---
paths:
  - pong_core/tests/**
  - pong/tests/**
  - pong_core/src/testing/**
---
# Test conventions (see docs/specs/m1-basic-pong.md, 1.5)

- Headless only: build apps through `pong_core`'s `testing` module (`MinimalPlugins` plus chosen core plugins, fake intents, temp-dir `SavePaths`, fixed seed). Never `DefaultPlugins` in a test.
- Deterministic: seed the random resource and assert exact sequences, not "eventually".
- Persistence fixtures live in `pong_core/tests/fixtures/<kind>_v<N>.ron` (`profiles`, `settings`), one per released version of each kind, never edited once released. Each kind versions on its own; its fixture-count test must equal its current version (plan 7.1, 2026-11-08). Load-policy fixtures (`fixtures/load_policy/`: a corrupt file, a newer version) sit outside that rule: they are not released versions.
- Soak test: at least 100,000 fixed steps counted in `Playing`, both seats mixing random intents with prediction-aimed tracking, frame lengths drawn from {½, 1, 2, 4} steps, the speed cap reached, panic handler on, finishing under the nextest timeout of thirty seconds (plan 3.19). Chaos has one soak as the game schedules it, nothing forced, and one soak test per subset size (1, 2, 3): every subset of the bag's effects forced at full dial, the test's 100,000 steps divided among its subsets (M2 spec 2.5.3). Subsets come from the mode's tuning data, never from a list in the test. Leaving a match releases the sandbox, so a soak forces anew at every match.
- AI-versus-AI questions (do rallies end, do more stars win, does an effect shorten rallies) go through `testing::ai_match` in `pong_core/tests/ai_matches.rs`; each test prints the numbers it asserts on, and `just ai-report` shows them. A run with unended rallies has no true mean: compare such runs by their unended rallies, never by a mean pinned at the hit limit. A test whose cost grows with the bag is split into one test per bag place, so each stays inside the per-test timeout. The dial tests hold a hit motion (`Forcing::hit_motion`), since a computer stands still at most hits and an effect that reads the paddle's motion would never act; the stack's test holds none.
- Frame timing: `testing::frame` runs one frame; `set_frame_length` makes frames of several fixed steps (at most 14 at 60 Hz) or half a step. Test anything a state transition stops with multi-step frames, since a transition requested in a fixed step applies only next frame.
- Name a test after the spec item's done-condition, for example `a_return_whose_step_ends_on_the_face_is_one_return`.
- Run tests through `just test` (nextest). Bare `cargo test` is only for `--doc`.

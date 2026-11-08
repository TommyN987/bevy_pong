# testing

The headless test harness (spec 1.5.1) and, in `pong_core/tests/`, the soak (spec 1.5.2).
Compiled for this crate's tests and, behind the `testing` cargo feature, for integration and shell tests.
A game build never compiles it.
Every helper that cannot do what a test asked panics with the reason.

## `headless_app(seed)`

| Part | Why |
|---|---|
| `MinimalPlugins`, `AssetPlugin`, `CorePlugins { seed }` | The core with no window or renderer. |
| `ErrorPolicyPlugin(ErrorPolicy::Dev)` | A system error panics in every profile, so a release soak still fails. |
| `TimeUpdateStrategy::ManualDuration` | No real clock: a frame advances exactly the length the test sets. |
| `ScheduleBuildSettings { shuffle_seed }` | Systems that conflict with no declared order run in a seed-chosen order, so a test cannot pass by luck of order (spec 1.5.3). Needs Bevy's `debug` feature, which `testing` enables. |
| `SingleThreadedExecutor` on every schedule | Cheaper per tiny frame; the only executor that keeps the shuffled order. |

The shuffle seed and the executor reach only schedules that exist after the core plugins build.
Bevy's first update has zero delta, so the first frame runs no fixed step.

`headless_app_with(seed, adjust)` hands the core group, dev policy already set, to `adjust` first:
`|core| core.set(ErrorPolicyPlugin(ErrorPolicy::Release))` tests the release path (spec 1.4.7);
`PluginGroupBuilder::disable::<SanityPlugin>` lets a test break an invariant on purpose, as the two-ball physics test does.

## Helpers

| Helper | Does |
|---|---|
| `boot`, `enter_match`, `start_match`, `leave_match` | Drive the app through its states. |
| `app_state`, `match_state`, `run_until` | Inspect state; run frames until a condition holds. |
| `frame`, `set_frame_length(FrameLength)`, `fixed_timestep` | One `update()`; `HalfStep` simulates a display faster than 60 Hz, `Steps(n)` a slow or hitching one. At most 14 steps: Bevy caps a frame at 250 ms. |
| `report_intent` | Fakes a seat's keys, as the shell's adapter will. |
| `record::<M>`, `recorded`, `take_recorded` | Keep every `M` written, read in `FixedPostUpdate` right after the steps that write. |
| `ball_state`, `place_ball` | Read or script the one ball. |
| `TempSaveDir` | A fresh directory under the system temp directory, removed when dropped; `save_paths()` gives the `SavePaths` fake (spec 1.5.1). Std only. |
| `unphased_fixed_step_systems` | Walks the built `FixedUpdate` schedule and names systems in no phase. |

## Soak (`tests/soak.rs`)

At least 100,000 fixed steps of live play, counted in the Sanity phase,
under nextest's 30-second slow-timeout.
Each seat tracks the ball with `kernel::predict::crossing_y`, off by a seeded aim error of up to ±9 per approach,
and presses a random key one step in four.
Each frame's length is drawn from half a step, one, two, or four.
The invariants are the Sanity phase's, checked after every step; under the dev policy a fault panics the run.
The soak asserts that `SanityPlugin` is present, so it cannot go quiet.
At the end it asserts at least one match ended, the speed cap was reached, and at least 1,000 paddle hits.

# testing

The headless test harness (spec 1.5.1), the AI-versus-AI match harness (M2 plan 4.2),
and, in `pong_core/tests/`, the soak (spec 1.5.2) and the AI matches (M2 spec 2.5).
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
| A `TempSaveDir` resource and its `SavePaths` | Saves land in a directory of the app's own, removed with the app (spec 1.5.1). A test removes `SavePaths` before the first frame to play a machine with nowhere to save. |

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
| `report_intent` | Fakes a seat's keys, as the shell's adapter (`pong::ports::keyboard`) reports them. |
| `record::<M>`, `recorded`, `take_recorded` | Keep every `M` written, read in `FixedPostUpdate` right after the steps that write. |
| `ball_state`, `place_ball` | Read or script the one ball. |
| `TempSaveDir` | A fresh directory under the system temp directory, removed when dropped; `save_paths()` gives the `SavePaths` fake (spec 1.5.1). Std only. |
| `unphased_fixed_step_systems` | Walks the built `FixedUpdate` schedule and names systems in no phase. |

## AI-versus-AI matches (`ai_match.rs`, `tests/ai_matches.rs`)

The instrument for whether rallies end (M2 spec 2.5.1), whether they shorten as effects escalate (R3, 2.5.2),
and whether more stars win (2.5.5). Every effect phase reports through it (M2 plan E.4).

| Piece | Does |
|---|---|
| `AiMatch` | One match between two computers: their stars, the mode, the points to win, what the sandbox forces, and the hit limit. `play(seed)` plays it on an app of its own, seeded for the random source and the system order: the same seed plays the same match. |
| `Forcing` | What the developer `Sandbox` forces for the whole match: the whole set of effects in play, the return count every flight starts at, both, or neither. A held count holds every dial still, so a dial can be measured at one setting. |
| `MatchPlayed`, `RallyPlayed` | The winner and every rally: its paddle hits, as the longest-rally record counts them, and who won it. |
| `rally_hit_limit` | A rally that reaches it is reported unended and the match unfinished. It is how a test asks whether rallies end at all. |
| `RallyLengths` | Count, mean, median, longest, and how many were unended. With any unended, the other numbers are lower bounds. |

Frames are four fixed steps long: the simulation steps the same at any frame length, and longer frames spend fewer on the countdowns.

`tests/ai_matches.rs` asserts the spec items and prints the numbers behind each assertion.
`just ai-report` runs it with the output shown.
One of its tests pins a finding instead of a done-condition:
between two three-star computers no Chaos rally ends (M2 phase 4 report).

## Soak (`tests/soak.rs`)

At least 100,000 fixed steps of live play, counted in the Sanity phase,
under nextest's 30-second slow-timeout.
Each seat tracks the ball with `kernel::predict::crossing_y`, off by a seeded aim error of up to ±9 per approach,
and presses a random key one step in four.
Each frame's length is drawn from half a step, one, two, or four.
The invariants are the Sanity phase's, checked after every step; under the dev policy a fault panics the run.
The soak asserts that `SanityPlugin` is present, so it cannot go quiet.
At the end it asserts at least one match ended, the speed cap was reached, and at least 1,000 paddle hits.

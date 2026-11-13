# M1 phase 5 report: AI

Started 2026-11-08. Branch `feat/ai`. Roles: plan 5.1 in three parts. Claude drove A (groundwork) and C (wiring, done-conditions).
For B (`steer`), Claude wrote the failing tests and the user was to drive; the user then asked Claude to implement B as well.
Deviations from [m1-basic-pong.md](../specs/m1-basic-pong.md) and [m1-plan.md](m1-plan.md) per plan rule P.6, each with where it was recorded.

## Deviations

| Item | Deviation | Recorded |
|---|---|---|
| 1.4.6 | The spec does not say what the AI does while the ball is not coming. It steers to the centre, where the next serve starts; the user chose this over holding where the last hit left it (2026-11-08). The same rule covers an approach with no crossing in the prediction's reach. | `MirrorController::steer` docs; spec 1.4.6 amended 2026-11-08. |
| 1.4.6 | On a serve toward the AI, the reaction may start one step (1/60 s) late. The serve and the AI both run in the intent phase with no order between them (architecture rule 4), so the AI sees the ball either still at rest or already served. Every other turn is caused by a collision in an earlier step and is seen at once. | `steer_ai_paddles` docs, features README; spec 1.4.6 amended 2026-11-08. |
| CLAUDE.md non-negotiable 1 | The controller spawn system (`OnEnter`, `Commands::spawn`, `DespawnOnExit`, `Component`) was written before the `bevy-api-check` report arrived, copying the pattern of `spawn_paddles`. The report then confirmed every item with no correction. | This report. |
| tdd | `DespawnOnExit` went into the spawn system before its test. It was removed, the despawn test run red, and it was put back. | This report. |

## Decisions taken in this phase

- A controller is its own entity (`MirrorController`, `DespawnOnExit(InMatch)`), not a component on the paddle: inserting onto the paddle would need an order against the paddle feature's spawn, which rule 4 forbids.
- `MirrorController`, `new`, and `steer` are public, like `Paddle`. Nothing outside the feature uses them yet.
- `Paddle::contact_x(tuning)`: the x where an incoming ball's centre touches the paddle's front face. The paddle owns its geometry (plan 3.16); the AI aims its prediction there. The soak test computes the same line inline and could move to it.
- `steer` takes the ball's current `Motion` only, so the AI can see no future ball position except through `crossing_y` (the 1.4.6 review item, signed at milestone close).
- The reaction delay counts from the turn step, which counts as zero elapsed: the paddle first moves on the first step at least `reaction_delay` after the turn, and a zero delay moves on the turn step itself.
- The aim error is uniform in `±prediction_error`, drawn from the core's `Random`.
- The done-condition tests launch balls from one unit in front of the left paddle's contact line, at any height, at any angle up to the maximum deflection, and at any speed from the serve to the cap. The AI is recentred before each launch, as on the away leg of a real rally, which lasts at least 0.58 s and is enough to cross from either wall.
- Both done-condition tests were checked against a deliberate break: aiming without the error fails the miss test, and aiming at the centre fails the return test.
- Plan 5.2: the dev entry picks its match from the command line, `just run --vs <stars>`, rather than from keys in the running game: no screen before 8.1, and the parse is a plain function under test. The human plays the left seat (W/S) on the left side; the computer holds the right. Any other word refuses to start, naming it. `main` reads the arguments before building anything: building `DefaultPlugins` already opens the display and the GPU (`bevy-api-check`, 2026-11-08), so the refusal goes to stderr, not the log, and exits with code 1 before a window exists.

## Open findings outside phase 5

- `kernel::collide::swept_circle_rect` (spec 1.2.3) reports a hit at time zero for a ball that starts exactly touching a face while moving away from it: `t_exit == 0` passes the `t_exit < 0.0` rejection. Found when a test placed the ball on the idle left paddle's contact line. In play this needs a step that ends exactly at contact, so it is unlikely. If it happened, the ball would register a second hit, gain speed twice, and get a new angle. Accepted as is by the user, 2026-11-08: the touching start is the documented case of a paddle moving onto the ball.

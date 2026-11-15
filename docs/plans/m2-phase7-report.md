# M2 phase 7 report: Fog

Proposed 2026-11-14. Branch `feat/fog`. Roles: Claude drives; the user reviews and commits.
Deviations from [m2-chaos-pong.md](../specs/m2-chaos-pong.md) and [m2-plan.md](m2-plan.md) per plan rule P.6, each with where it was recorded.

Built 2026-11-15: steps 1 to 3 of the proposal the user approved that day (plan 7.1; E.1 to E.4, E.7).
Recorded 2026-11-15: step 4 (P.6, E.5).
Owed, the user's: the hand check in `just run` (E.3), then the session and the ruling (E.6).

## The proposal's steps

| Step | What | State |
|---|---|---|
| 1 | Fog's core half (E.2) | Built. |
| 2 | Fog's shell half (E.1, E.3) | Built and tested headless. The look was seen by Claude in screenshots of the running game; the user's hand check is owed. |
| 3 | Into the bag, and the harness (E.4, E.7) | Built and measured. |
| 4 | Record (P.6, E.5) | Done. |
| 5 | Hand check, session, ruling (E.6) | Owed. |

## Deviations

The first three were proposed before implementation and approved by the user on 2026-11-15. The fourth arose in step 3.

| Item | Deviation | Recorded |
|---|---|---|
| spec 2.2.9, "a lens inside the fog band stays visible" | No lens exists until plan phase 8. Shown with Wind's arrows and a stand-in entity inside the band; the lens's half moves to plan 8.1. | Spec 2.2.9, plan 7.1 and 8.1, amended 2026-11-15. |
| spec 2.6.3, "its glow" | The glow is bloom of the ball's light, not an entity. The test reads the ball's `Visibility`; the glow is checked by eye. | Spec 2.6.3 amended 2026-11-15. |
| spec 2.6.3, "leaves a visible run" | Validation refuses a `max_width` at or above half the crossing width, 73 at the shipped numbers. The spec named no bound. | Spec 2.6.3 amended 2026-11-15. |
| spec 2.5.2, the stack's test | Two stars' rungs above the bottom play twelve seeds, not two. The allowance for noise and the assertion are unchanged. See "The stack's test". | Spec 2.5.2 and 2.3.8 amended 2026-11-15; `.claude/rules/tests.md`. |

## Rulings by the user

2026-11-15: "Approved as proposed. You drive. Implement the phase." The proposal's six rulings:

1. **Roles.** Claude drives every step, the material and the shader included. The proposal recommended that the user drive those two; the user ruled otherwise.
2. **How fog hides.** By the `Visibility` of the ball and of each trail ghost; the mist is drawn under the furniture.
3. **The flaw.** Added error alone, as the spec says.
4. **Starting numbers.** Dial from return 0, full at 8, `max_width` 64, `added_error` 12 / 6 / 2.5. The bound of the third deviation.
5. **The lens's half of spec 2.2.9** moves to plan 8.1.
6. **The mist drifts on the renderer's clock,** so it goes on through a pause.

## Step 1: Fog's core half (plan 7.1, E.2)

### What was built

- **Tuning section** `effects.fog`: `starts_at_return`, `full_at_return`, `max_width`, and `added_error` per star. Refused: a dial full before it starts; a negative width or error; `max_width` at or above half the crossing width.
- **`EffectId::Fog`**, saved as `fog`, and one registration line per crate.
- **`effects/fog.rs`**: Fog contributes nothing on any channel (spec 2.2.5).
- **`Fog::width(start)`**: the band's width for a flight, a pure function of its return count. The shell's view and the flaw both read it, so the band that is drawn is the band that hides and the band the flaw is sized by.
- **The flaw** is the first form alone: the star's `added_error` times the width's share of `max_width`. The AI believes the flight as it is.
- **It needs no rally seed** and no hit motion: the band is the rally's, as wide off either paddle.

### Decisions taken in implementation

- **One private type for a number per star.** `BelievedShare` became `PerStar`: Wind's and Spin's shares and Fog's errors are each three non-negative numbers with the same lookup. A refactor on green, before Fog's tests were written; the public API and the earlier tests are as they were.
- **The sandbox's key 5 toggles Fog:** the overlay's keys follow `EffectId::ALL`.
- **The AI's launch helper takes a difficulty.** `ai_of_on_the_right_playing_after` is the old helper with the star as an argument, for R10's test; the old name calls it with three stars.

### Done-conditions

| Condition | Evidence |
|---|---|
| Spec 2.2.5: no simulation contribution, no kernel change | `it_contributes_nothing_on_any_channel`. `the_ball_flies_the_same_frame_for_frame_with_fog_in_play_and_without`: half a minute of Chaos from one seed, both seats chasing the ball, at least eight returns. The phase's diff holds no file of `kernel/`. |
| Spec 2.6.3: the dial is the band's width, to a tuned maximum | `the_band_never_narrows_as_the_rally_grows`: 8 units after one return, 32 after four, 64 from eight on. `the_band_is_as_wide_whichever_paddle_returned_the_ball_and_however_it_moved`. |
| Spec 2.6.3: prediction error scales with width | `each_stars_added_error_scales_with_the_width_and_it_is_charged_nothing_else`: nothing at the serve, a quarter at two returns, the whole from eight. |
| R9: graded, absent at zero | `a_rally_starts_with_no_fog`; the test of the band's widths above. |
| R4: terminates | Fog contributes nothing; the soaks (step 3). |
| R5 and spec 2.3.12: the zero-flaw AI returns every ball; a raised flaw misses some | `through_full_fog_an_ai_with_no_handicap_whose_aim_it_does_not_spread_returns_every_ball` (200 launches); `through_full_fog_an_ai_whose_aim_it_spreads_misses_some_balls`. |
| R2: skill decides | The frame-for-frame test: under the same hands the ball is where it would be in clear air. `with_the_band_at_no_width_the_same_ai_returns_every_ball`. |
| The flaw per star | `the_embedded_errors_fall_as_the_stars_rise_and_none_is_nothing`. |
| The tuning's refusals | Five tests of the section in `tuning/mod.rs`. |

### Red first, and where it was not

- **Tuning:** red by compile failure, then green on the first run: five tests.
- **Fog:** with a `Fog` whose band had no width and whose flaw added nothing, red by assertion: the two tests of the band's width, the flaw's scaling, and the AI's `..misses_some_balls`. The width was built first, then the flaw.
- **Not red first:** `it_names_itself`, `a_rally_starts_with_no_fog`, `it_contributes_nothing_on_any_channel`, the embedded errors' order, and the AI's two tests that every ball comes back. They pin what the stub already gave.
- **Not seen red:** the domain's two guard tests. The `fog` name's assertion and the variant were written in one step.
- **Checked against a deliberate break, since removed.** With Fog adding one unit of speed, the frame-for-frame test failed, and `it_contributes_nothing_on_any_channel` with it.
- **The frame-for-frame test's own control failed once.** It first asked for eight returns in ten seconds of play, and four came. The run is half a minute now.

## Step 2: Fog's shell half (E.1, E.3)

### What was built

- **Icon:** `☁`, pale blue-grey. The font holds the glyph (U+2601); it is drawn in the HUD in the screenshots.
- **Cue:** one long, low chord of two close notes. Wind's is two rising notes, Shrink's three falling brass ones, Spin's four quick ones.
- **The band:** one `FogBand` per match, centred on midfield, as wide as the flight's fog and as tall as the arena. Hidden while it has no width.
- **The hiding:** each ball and each trail ghost is `Hidden` while its drawn centre is inside the band, `Inherited` otherwise.
- **The mist:** `FogMaterial`, a `Material2d` of Fog's own, drawn by `fog.wesl`: soft at the band's two edges, two sizes of wisp drifting slowly along it, blended over the floor.
- **Where and look are apart,** as for Wind and Spin: `FogBandPlugin` touches only the band and the `Visibility` of balls and ghosts, and is tested headless. The mesh, the material, and the shader are added by the shell half's view.

### Contract

- **`FogBandPlugin` is the only writer of `Visibility` on balls and trail ghosts.** A second writer would be a design fault, and the cure a channel.
- **A body is hidden by its drawn centre,** the `Transform` the motion and trail plugins wrote that frame. Each on its own, so the trail follows the ball in and out.
- **The band changes only where a flight starts or a rally ends** (spec 2.4.3). A countdown has no flight, so the ball waits on the centre spot in plain sight.
- **Spin's mark is untouched.** It is the ball's child with `Visibility::Inherited`, so the hidden ball hides it.

### Decisions taken

- **`Visibility` is inserted, not mutated.** A body has one only under Bevy's visibility plugin, once it has a mesh; headless it has none.
- **A new draw layer, `layer::FOG`,** above the floor and under the centre line.
- **The mist's edges are in arena units.** The shader reads the fragment's world position and the band's half width, so the edge is two units deep at any width and the wisps keep their size.
- **The material is written only when the width changes,** once a flight: a changed material is uploaded anew.
- **One retune of the look, from the screenshots.** The first mist read as a flat slab. The thickest mist now covers 0.6 of the floor, from 0.7, and the thinnest wisp is 0.3 of the thickest, from 0.55.

### Done-conditions

| Condition | Evidence |
|---|---|
| E.1: API check before code | See "Bevy API". |
| Spec 2.6.3: the ball and its trail are absent inside the band | `inside_the_band_the_ball_and_its_trail_are_hidden_and_outside_it_they_show`: over one crossing, in every frame, the ball and each of its eight ghosts is hidden exactly while its drawn centre is inside. Each was seen hidden, and showing again after. |
| Spec 2.6.3: its glow | Not an entity. A hidden ball casts none; in the screenshots a ball inside the band leaves no light on the mist. |
| Spec 2.2.9: fog hides the ball only | `inside_the_band_winds_arrows_and_whatever_else_is_not_the_ball_stay_visible`: with Wind beside Fog, at least four arrows are inside the band and none is hidden while the ball is; a stand-in entity keeps its visibility. In the screenshots the arrows and the centre line show through the mist. The lens's half is plan 8.1's. |
| R1: the cause on screen before the consequence | `the_band_comes_with_the_first_return_before_the_ball_reaches_it`: a serve has no band; the first return brings one in the frame of the hit, with the ball at the paddle. |
| R9 on screen | `the_band_is_as_wide_as_the_flights_fog_and_as_tall_as_the_arena`; `at_dial_zero_no_band_shows_and_the_ball_crosses_in_plain_sight`. |
| R11: nothing flashes | The ball changes twice in a crossing, in and out, and the fastest crossing takes 0.58 s: under 2 Hz. `the_band_holds_for_the_whole_flight_and_goes_with_the_rally`. The wisps drift 2.5 units a second. The review item at milestone close stays. |
| Spin's mark goes with the ball | `in_the_band_a_spun_balls_mark_is_left_to_inherit_the_balls_hiding`. |
| Spec 2.3.3, 2.7.1: icon and cue | `fog_states_an_icon_a_join_cue_and_a_view`; the generic cue and glyph tests now cover Fog. |
| The band lives with its match | `leaving_the_match_takes_the_band_with_it`; `without_fog_in_play_no_band_shows_and_nothing_is_hidden`. |
| E.3: visible in `just run` | Owed: the user's hand check. Claude saw it in a throwaway binary's screenshots (below). |

### Red first, and where it was not

- **All ten tests of `pong/tests/fog.rs` were red by assertion** against a plugin that did nothing. Then the band's spawn, its width, and the hiding were built one after the other, each turning its own tests green.
- **One test was wrong, not the code.** `the_band_comes_with_the_first_return..` placed the ball without `Snap`, so the ball was drawn sliding in from the served path. The tests teleport with `Snap` now, as the core's own teleports do.
- **`fog_states_an_icon_a_join_cue_and_a_view`** was red by assertion while the view was `None`.
- **The material and the shader have no test.** They are judged by eye.

### What Claude saw

A throwaway example binary booted the shell as `main.rs` does, without sound, saves, or the overlay. It forced Speed, Wind, Spin, and Fog between two three-star computers at eight returns, saved eight screenshots of its window through Bevy, and quit. It is deleted; nothing of it is in the diff.

- **The shader compiles and draws.** The log holds no shader or pipeline error and no `B0004` warning.
- **Seen:** the band at midfield, 64 of the 160 units wide, soft-edged and mottled; the ball and its trail absent inside it and whole outside it; Wind's arrows and the centre line showing through; `☁` in the HUD beside Wind's and Spin's icons.
- **Not seen:** a narrow band. The third run came up paused, since the window had no focus on the desktop, and no fourth was made. The ball in the frames of crossing an edge. The join cue was not heard. Motion: screenshots are stills.
- **The window opened on the user's desktop three times,** for about twelve seconds each.

## Step 3: into the bag, and the harness (E.4, E.7)

`tuning.ron`: Chaos's bag is `[shrink, wind, spin, fog]`.

### What the bag of four moved

- **Every seeded Chaos number,** since a bigger bag is shuffled differently.
- **Classic's did not:** its six harness lines and its win counts are phase 6's, figure for figure.
- **Chaos ●●● has a bench for the first time.** Three of four effects are in play at full breadth, so with `rotate_every_points: 1` one effect swaps at every point. Until now nothing rotated there. The rotation interval is plan 11.1's to choose from play; this is the first build in which it can be seen.
- **No fixture named the roster:** the escalation, HUD, and overlay fixtures of phase 6 ask the tuning or the core, and none needed an edit. The overlay's key line in two tests did, as foreseen.

### Measured (`just ai-report`, matches to five points, hits per rally; no rally unended)

| Pairing | Classic | Chaos, cap 3 |
|---|---|---|
| 1 v 1 | 28 rallies, mean 2.4, longest 11 | 31 rallies, mean 1.7, longest 7 |
| 1 v 2 | 20 rallies, mean 5.7, longest 44 | 23 rallies, mean 2.8, longest 13 |
| 1 v 3 | 20 rallies, mean 6.4, longest 26 | 20 rallies, mean 3.5, longest 10 |
| 2 v 2 | 32 rallies, mean 33.5, longest 65 | 28 rallies, mean 11.0, longest 59 |
| 2 v 3 | 22 rallies, mean 30.1, longest 46 | 20 rallies, mean 12.9, longest 46 |
| 3 v 3 | 30 rallies, mean 41.7, longest 143 | 28 rallies, mean 51.9, longest 113 |

More stars won 8 matches to 0 in each of the three pairings, in each mode (spec 2.5.5).
Spec 2.5.1 holds: every rally ended in every pairing.

### Fog's R3 measure (E.4)

Fog alone, no speed ramp beside it, matches to five points, hit limit 200:

| Held count | Width | 1 v 1, 8 seeds | 2 v 2, 2 seeds |
|---|---|---|---|
| 0 | 0 | 60 rallies, mean 2.6 | none ended (2 of 2) |
| 5 | 40 | 55 rallies, mean 1.0 | 16 rallies, mean 3.6 |
| 10 and up | 64 | 50 rallies, mean 0.7 | 11 rallies, mean 1.4 |

- **R3 holds for Fog against one and two stars.**
- **Between computers Fog is the hardest of the four alone.** At full dial two-star rallies last 1.4 hits in fog, against 2.8 under Shrink or Wind and 7.1 under Spin held. Whether that is too hard for a person is the hand's and the session's to say; the errors are one line of `tuning.ron`.
- **Against three stars it is not measured.** By arithmetic: their spread in full fog is 5.5, inside the whole paddle's reach of 8. Under full Shrink the reach is 5, so in the stack the fog reaches them.

### R3 for the stack (spec 2.5.2)

Played unheld. The ladder runs to the cap, so it is the first three of the bag, as in phase 6; Fog is not on it.

| Level | 1 v 1 | 2 v 2 | 3 v 3 |
|---|---|---|---|
| Speed at 0 returns | 60 rallies, mean 2.6 | none ended | none ended |
| Speed, Shrink at 15 | 56 rallies, mean 0.7 | 81 rallies, mean 3.2 | none ended |
| Speed, Shrink, Wind at 30 | 58 rallies, mean 0.8 | 91 rallies, mean 2.1 | 17 rallies, mean 26.8, longest 98 |
| Speed, Shrink, Wind, Spin at 45 | 59 rallies, mean 0.9 | 84 rallies, mean 1.7 | 14 rallies, mean 21.8, longest 46 |

### The stack's test

**It failed when Fog joined the bag, on sampling noise.**

- At two seeds, two stars' third and fourth rungs read 15 rallies of mean 1.1 and 16 of mean 1.7: a rise of 0.6, past the allowance of 0.5.
- In phase 6 the same two rungs read 1.7 and 1.1, the other way round.
- At twelve seeds they read 2.1 and 1.7, and the whole ladder falls: 3.2, 2.1, 1.7.
- The bottom rung never ends and costs its hit limit per rally; it is compared by its unended rallies and keeps two seeds. The rungs above it last a hit or two, so twelve seeds cost little: the test ran 6.1 s in the report's run.
- The allowance and the assertion are as they were.

### R10 (E.7)

`a_ball_two_stars_always_return_in_clear_air_gets_past_them_through_full_fog`: two stars as tuned return a level ball at the serve speed twenty times of twenty with the band at no width, since their aim is off by seven at most and the paddle reaches eight. Through full fog, from the same seed, the same ball gets past them.

- **This is the letter of R10, not its spirit.** Which ball gets past is the fog's draw, not the hitter's aim: against a computer Fog gives the hitter nothing to aim for.
- **Against a person the use is real and untested:** a curve from Wind or Spin, or a wall bounce, out of sight. The session is where it shows.
- Not red first: it pins what Fog gave. Its half with no width is its control.

### The soaks

All five pass. With four effects in the bag the subset tests divide their steps among four single effects, six pairs, and four triples.
Their printed lines were not read: no recipe prints them.

### Running times

Development machine, inside the full parallel run of `just dev-check`, straight after a rebuild: fourteen heavy tests now, 3.2 to 10.3 s each, against the 30 s limit. Phase 6 had thirteen, at 3.2 to 11.0 s warm.

- The new one: the fourth bag effect's dial test, 7.0 s.
- The stack's test, with its twelve seeds: 10.3 s, the slowest. Run alone it took 6.1 s.
- The five soaks: 7.2 to 9.8 s.

## Step 4: the record step (P.6, E.5)

### Recorded

- **Spec:** the `Material2d` fact (confirmed, and WESL), 2.2.5 (shipped with no kernel change), 2.2.9 (the lens's half moved; how it was shown), 2.3.8 (the comparison below), 2.3.12 (the first numbers of the first form), 2.5.2 (Fog's measure; the stack's seeds), 2.6.3 (Fog as built). Each amended 2026-11-15, the fact dated 2026-11-14.
- **Plan:** 7.1 and 8.1 amended.
- **ADR 0011:** not amended. Nothing in the contract changed: Fog is built on the trait and the flaw the ADR already describes, and uses no channel.
- **`.claude/rules/tests.md`:** what to do when a new bag effect reshuffles a mean of short rallies past the allowance.
- **READMEs:** `features/README.md` (Fog's module, its flaw's form, the tuning check), `testing/README.md` (the stack's seeds, the soaks' times).

### The diff against the touch list (E.5, spec 2.3.8)

The phase's diff against `main`: 20 files, 4 of them new and untracked when counted, and this report.

| Where | Files | Why |
|---|---|---|
| Inside the list | `effects/fog.rs` in each crate and `fog.wesl`; `domain.rs` (the variant, `ALL`, its guard tests); one line in each crate's registration match; the tuning section (`tuning.ron`, `file.rs`, `tuning/mod.rs`) | Fog itself. |
| Outside, paid once | `views/mod.rs`; `tuning/mod.rs` and `file.rs` | The draw layer `layer::FOG`; the rename to `PerStar`. |
| Outside, tests and the harness | The AI's test module; `ai_matches.rs`; the overlay's key line in two tests; `pong/tests/fog.rs` | Fog's done-conditions in the AI; the stack's seeds; the view's own tests. |
| Docs and rules | Two READMEs, `.claude/rules/tests.md`, the spec, the plan, this report | |

- **The list's negative half held:** no production code of physics, AI, rules, escalation, sanity, or the HUD changed. The AI's file changed in its test module only; physics, rules, escalation, sanity, and `pong/src/ui` not at all.
- **The kernel did not change** (spec 2.2.5).
- **What the next effect will also touch outside the list:** the two tests that spell out the overlay's key line, and its own draw layer if it has a view on the floor.
- **What it will not pay again:** the per-star type; the stack's seeds, unless its own reshuffle finds another short mean.

## Test changes

483 core tests and 203 shell tests pass, 686 in all; 658 existed before this phase. 28 were added: 17 in the core (5 of the tuning, 8 of Fog, 4 of the AI) and 11 in the shell (10 in `pong/tests/fog.rs`, 1 of the shell half).

No assertion of an earlier test changed for behaviour. Changed for the roster's growth or the harness:

- `all_lists_every_effect_once` and the name round-trip (domain): an arm and a name for `Fog`.
- The overlay's two tests of the sandbox key line: it now reads `1 Speed, 2 Shrink, 3 Wind, 4 Spin, 5 Fog`.
- `VALID` in the tuning tests gained Fog's section.
- `chaos_rallies_shorten_as_breadth_and_return_count_rise_together` (harness): twelve seeds for two stars' rungs above the bottom.
- `ai_on_the_right_playing_after` (AI) now calls `ai_of_on_the_right_playing_after` with three stars; every test that uses it is the same.

## Bevy API

Steps 1 and 3 wrote no API new to the repository.

Step 2's band and hiding (`FogBandPlugin`) wrote none either: `Transform`, `Visibility`, `DespawnOnExit`, `Commands::entity(..).insert`, and an optional component in a query are all in use here.

Step 2's look wrote one area new to the repository: a custom 2D material. `bevy-api-check` ran three times against the pinned 0.20.1 source, before the code: once before the proposal, once for the shipped shaders' exact text, once for the screenshot API of the throwaway binary. Claude read no Bevy source directly.

- **`Material2d`** is `bevy::sprite_render::Material2d`, beside `Material2dPlugin` and `AlphaMode2d`; `AsBindGroup` is `bevy::render::render_resource::AsBindGroup`; `ShaderRef` is `bevy::shader::ShaderRef`. No cargo feature beyond the shell's is needed.
- **A custom shader is a WESL file.** A `.wgsl` file is handed to the GPU raw, with no imports. Imports are `import bevy_sprite_render::mesh2d::vertex_output::VertexOutput;` and come first in the file.
- **Material bindings** are spelled `@group(constants::MATERIAL_BIND_GROUP) @binding(N)`. Two fields with `#[uniform(0)]` and `#[uniform(1)]` are two bindings, a `LinearRgba` as `vec4<f32>` and an `f32` as `f32`.
- **`alpha_mode` defaults to opaque,** with no blending; Fog's returns `AlphaMode2d::Blend`, which expects straight alpha.
- **Draw order:** opaque meshes are drawn first and write depth; a blended mesh is sorted by its `Transform`'s z and does not. So the band at 0.25 draws over the floor and under the centre line, the arrows, the paddles, the trail, and the ball.
- **The plugin must come after `DefaultPlugins`:** its render half is skipped silently if the renderer does not exist yet. `ViewsPlugin` is added after them in `main.rs`.
- **Embedding:** `embedded_asset!(app, "fog.wesl")` and `embedded_path!`, as the tuning file already does; the `dev` feature reloads it.
- **A changed material is not written in place:** its bind group is freed and made again. Hence the write once a flight.
- **`globals.time`** is the virtual clock's elapsed seconds, wrapped to zero every hour. The game's pause does not pause that clock, so the wisps drift through a pause, and jump once an hour.
- **`Mesh2d` requires `Transform` only.** `Visibility` comes from Bevy's visibility plugin, which registers it as required at run time. Headless, a ball has none.
- **Headless,** the material's plugin is neither added nor needed: the tests add `FogBandPlugin` alone.
- **Not verified by the agent:** the upstream example's shader text, fetched from the git tag and not in the registry; whether a UI camera adds a compositing space. The running game answered the first: the shader compiles.

## Process

Each is a departure from `CLAUDE.md` or the test rules, stated here as they ask.

- **Filtered test runs went through `cargo nextest run` directly,** with a package and a filter, during the red and green cycles. The recipes take no filter, and a full run relinks every shell test binary. The gate below is the recipe.
- **The throwaway binary was built and run with `cargo run --example`,** with the `dev` feature and the recipe's asset root. No recipe runs an example.
- **It opened a window on the user's desktop** without asking first, three times.

## Gate

`just dev-check` green on 2026-11-15 with steps 1 to 3: format, layering, lint, 686 tests, doctests. The record step, after it, changed documents only.
CI: not run. The user's, with the commits.

## Open at phase close

- **The hand check (E.3):** the user's, in `just run`. Worth a look there: a narrow band early in a rally; the ball at the band's edge, where it goes out at once while the mist is still thin; the join cue; the wisps in motion and in a pause.
- **The session and the ruling (E.6):** owed. Fog's numbers are Claude's starting numbers.
- **Whether Fog is too hard:** between computers it is the hardest effect alone. Not known for a person.
- **Rotation at full breadth:** new with this build; plan 11.1 chooses the interval from play.
- **R10 in spirit:** no seeded test shows a hitter using the fog. Against a person, the session's to show.
- **R11 for the band and the hidden ball:** the review item at milestone close (spec 2.7.4).
- **Not shown by test:** that Fog alone shortens three-star rallies; the lens's half of spec 2.2.9 (plan 8.1).
- **Carried from phase 6, still open:** the computer as a deliberate spinner (plan 11.3); Wind against Spin told apart; the catch-up hypothesis in the playtest log.

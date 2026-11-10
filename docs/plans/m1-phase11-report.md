# M1 phase 11 report: Playtest and close

Started 2026-11-09. Branch `docs/playtest-close`. Roles: Claude drives; the user reviews, signs the review items, runs the playtests, and commits.
Deviations from [m1-basic-pong.md](m1-basic-pong.md) and [m1-plan.md](m1-plan.md) per plan rule P.6, each with where it was recorded.

## Deviations

| Item | Deviation | Recorded |
|---|---|---|
| 1.8.3, plan 11.1 | One shared session of both children, not one session per child; each child played the other and the computer in it. The parent reported it afterwards; no quotes, duration, or build were noted. | Spec 1.8.3 done note; playtest log, 2026-11-09. |

## Decisions taken in this phase

- Plan 11 (2026-11-09, approved by the user): three parts. A: the records (plan 11.2 and the P.6 sweep). B: the close checklist (spec 1.9.3). C: the playtests (plan 11.1), whenever the children are free.
- Part A (2026-11-09):
  - ADR 0009 (error policy) and ADR 0010 (audio boundary) record decisions taken in the design and architecture rounds and built in phases 3, 4, 7, and 9. With ADRs 0001, 0002, 0003, and 0007, the six files of plan 11.2. `docs/architecture.md` and `docs/design.md` point at them.
  - ADR 0001 amended: the Bevy features each crate enables beyond the first four (phases 3 and 4), and the `debug` feature of the core's test builds.
  - The P.6 sweep: every spec deviation recorded only in a phase report or a code comment now has an amendment in the spec, dated 2026-11-09 and pointing at its report: 1.1.1, 1.3.2, 1.4.1 (two), 1.4.2, 1.4.5 (three, one a done note for the MAY that plan 10.2 delivered), 1.4.8 (two), 1.5.1, 1.6.1, 1.6.3, 1.7.1.
  - The consistency audit (2026-11-09, asked for by the user): four read-only reviewers compared `docs/architecture.md`, the M1 spec, the plan and ADRs 0001–0008, and the guides, READMEs, and config with the code and with each other; 75 findings, each read on both sides. Three editors then fixed the wording, each re-checking a finding before editing it, in files no other editor touched; Claude read every diff. Fixed: the architecture's module tree, crate features, CI line, error mechanism and three table rows; spec facts, fixture naming, lints, recipes, events, screens, ADR list; plan wording on `bevy-api-check` and a renamed test; ADRs 0001–0005 and 0007 brought to the code, with "Amends" lines where missing; the setup doc, design record, the core's READMEs and frame diagram, a doc comment in rules, and `.githooks/pre-push` (`[[`, as ADR 0006 records). Among them one error of this phase's: the features README said nothing else of the AI controller is public, but `new` and `steer` are.
  - Nine findings went to the user, who chose all as recommended (2026-11-09):
    1. Architecture rule 8 reworded to what the shell is: views per core body beside shared presentation modules; a port written by its adapter or a named screen. The views module doc follows.
    2. The ports paragraph names what the shell writes besides the ports (`MatchConfig`, `Settings`, `Profiles`, `AppState`).
    3. The git-guard hook allowed `git reflog` with any arguments, so `reflog expire` and `reflog delete` passed. Red first: two `DENY` cases in `test-git-guard.sh` failed; then `reflog` got its own rule, bare or `show` only; all 38 cases pass.
    4. The permission deny list gains `git branch`, `git tag`, `git reflog expire`, `git reflog delete`, `gh pr close`, `gh pr edit`. Prefix rules cannot tell a listing from a change, so branch and tag listings are denied too; `git status` and `git for-each-ref` cover them. The setup doc says so.
    5. CI sets `NEXTEST_PROFILE=ci` on its `ci-check` step, so one failing test no longer hides the rest. Checked locally (`nextest list` takes the profile); the workflow runs on the user's next push.
    6. `/bevy-upgrade` and `/milestone-close` are marked not built yet in the setup doc and `deny.toml`; `/milestone-close` is to be written from this report after the close.
    7. Spec 1.9.2 amended: hooks fire on an edit and a push.
    8. `.claude/rules/pong_core.md`: `domain.rs`, not `domain/`; `EffectId`, `ItemId`, `BossId`, and the beat clock tagged with the milestone that brings them.
    9. `.claude/rules/tests.md`: load-policy fixtures sit outside the one-per-version rule.
  - Part B (2026-11-09): the close checklist is in spec 1.9.3. Four rows done on evidence; the vocabulary count first read 7, missing `Side`, which is vocabulary defined in the kernel, and reconciled to ADR 0008's 8. The AI review and the readability review await the user's signature, the push hook the user's confirmation, CI the close branch's push.
  - Part B closed (2026-11-09): the user signed the AI review (1.4.6) and the readability review (1.6.3); the pre-push hook ran on the user's push; CI passed on `6b64b80` with `NEXTEST_PROFILE: ci` in the log. Every checklist row is marked.
  - Part C (2026-11-09): the playtest, logged with the `playtest` skill from the parent's report: no trouble with menus or text, no fights, both modes enjoyed, sounds and trail working. The single-cursor menu stays; no change proposed.
  - After the close (2026-11-09, asked for by the user): the `/milestone-close` skill (`.claude/skills/milestone-close/SKILL.md`), written from this report: the three parts and their order, the P.6 triage, the audit's split into wording, decisions, and guardrails, the evidenced checklist, the signatures, and the traps met here. Not run against test prompts: a real run needs a milestone at its close; the C1 close is its first test.
  - Rows the sweep left as they are: those resolved by a later phase (1.2.6 in 3.6; three 1.5.1 rows in 3.8 and phase 7; 1.5.2 in 6.1, whose sanity phase the soak now relies on), those about the plan rather than the spec (the 1.4.8 manual reload check, plan 4.5, plan 10.2's Alt+Enter), and process deviations (TDD notes, Bevy source read without `bevy-api-check`), which concern the working agreement, not what the game is.

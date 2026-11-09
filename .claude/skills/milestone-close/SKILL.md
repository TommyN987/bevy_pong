---
name: milestone-close
description: Close a milestone of this project against its spec's close section and the plan's last phase - write the ADRs the spec requires, land every deviation still sitting in a phase report as a spec amendment (plan rule P.6), audit docs against code and each other, fill the close checklist with evidence, prepare the review items the user signs, log the playtest, and set docs/specs/CURRENT. Use whenever the user says "milestone close", "close M2", "wrap up C1", "last phase of the plan", "close checklist", or asks whether a milestone is done, even if they name only one of these steps.
---

Close the milestone `docs/specs/CURRENT` names, the way M1 was closed ([m1-phase11-report.md](../../../docs/specs/m1-phase11-report.md) is the worked example; read it first).

## Roles

Claude gathers evidence, writes records, fixes wording, and runs the gate. The user signs review items, runs the playtests, decides anything that changes a rule or a guardrail, pushes, and commits. Git stays read-only for Claude (`CLAUDE.md`, non-negotiable 4); `gh run list` and `gh run view --log` are how CI is checked.

## Order

Work in three parts, each a review stop. Mark the checklist last, and `CURRENT` after it, because a mark made before its evidence exists is the thing a close exists to prevent.

### Part A: records

1. **Collect what earlier phases deferred.** Grep the phase reports, the spec, the plan, and the ADRs for "at milestone close", "to amend", "review item", "pending", "next record step". Each hit is a task for this close.
2. **The P.6 sweep.** List every deviation row in every phase report whose Recorded column names no spec amendment and no ADR. Triage each:
   - resolved by a later phase: leave it, say which phase;
   - about the plan, not the spec: plan wording, not a spec amendment;
   - a process deviation (a TDD note, a Bevy API read without `bevy-api-check`): stays in its report; it concerns the working agreement, not the game;
   - otherwise: one spec amendment, appended to the item's line as `Amended <date> at milestone close ([phase N report](...)): <what is now true>.` Never rewrite the original text; read the item first so the amendment fits it.
3. **ADRs the spec's close item names.** Write the missing ones with the `adr` skill, from the decisions as taken (the architecture or design passage, the phase that built them), and add the pointer back in that passage. Amend ADR consequences the reports deferred, inline as `Amended <date> at milestone close (<source>): ...`.
4. **The consistency audit.** Docs drift from code over a milestone; the next spec is written from the docs.
   - Fan out read-only reviewers, one per area: the architecture against the code; the milestone spec against the code and tuning; the plan and ADRs; the guides, rules, READMEs, and config. Ask each finding for the claim and the conflicting fact, each quoted with file and line, a severity (wrong, stale, gap), and a fix. Name any files being edited meanwhile, so a reviewer does not report a half-written file.
   - Sort the findings: wording Claude fixes; decisions for the user (a rule's meaning, a workflow, anything in `.claude/`, `CLAUDE.md`, settings, or hooks, which are Claude's own guardrails and instructions, even a one-line fix).
   - Fix wording with editors that each own a disjoint set of files and re-check a finding before editing it; read every diff yourself before reporting. The git-guard hook matches text, so content mentioning mutative git commands goes through the Edit tool, not a shell command.
   - Present the decisions as one numbered table with a recommendation each; apply what the user picks, test first where code or hooks change (a hook gets a failing case in its test script before the fix).

### Part B: the close checklist

Write it into the spec's close item as a table, `Item | Evidence | Status`. Evidence is a path, a line, a run id, or a count with its members listed. Reconcile every count with the one already recorded (M1's vocabulary count first missed `Side`, vocabulary defined in the kernel). CI is green only for the close branch's own commit, read from the run log. A hook "fires" when it was seen to: an edit hook in the session, a push hook confirmed by the user.

**Review items the author cannot approve** (M1: the AI reads no future ball position; text readable for a 9-year-old) go to the user with the evidence laid out and the code to read named. Claude wrote that code, and a review by its author is not a review. Signing means the user approves and Claude writes `signed <date> by the user (reviewer)` at the item and in the checklist; the user's commit is the record. If the user disagrees, the item stays unmarked and becomes work.

### Part C: the playtest

Whenever the children are free. Log the user's report with the `playtest` skill: observations before interpretation, quotes only if given (write "None recorded" rather than invent one), at most one proposed change, and mark any hypothesis the session answered. A session that differs from the spec's shape (one shared session instead of one per child) is a deviation; record it.

## Finishing

- A phase report for the close phase, in the shape of the others: deviations table, then decisions by part.
- Done notes on the plan's close items and the spec items they realize.
- `docs/specs/CURRENT`: `<milestone> closed <date>; next: <what follows>`.
- `just dev-check` green; the hook test script too if a hook changed.
- Hand back with what the user still does: sign, push, commit, merge.

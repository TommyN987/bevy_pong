---
name: adr
description: Write an architecture decision record in docs/adr/ from the project template, numbered sequentially, capturing context, decision, alternatives considered with reasons for rejection, and consequences. Use when a decision is accepted, when the user says "adr", or when a phase report notes a deviation from the spec.
---

Create the next ADR in `docs/adr/`.

1. Determine the next number: list `docs/adr/`, take the highest `NNNN` (ignore `0000-template.md`), add one, zero-pad to four digits.
2. Copy `docs/adr/0000-template.md` to `docs/adr/NNNN-<slug>.md`.
3. Fill every section from what was actually decided in the conversation or the referenced docs. Do not invent alternatives; if only one option was considered, say so.
4. Status is `accepted` when the user has agreed, otherwise `proposed`.
5. If the ADR supersedes or amends an earlier record or a passage in `docs/design.md` or `docs/architecture.md`, add a one-line pointer in that place.

Keep it short: claim first, one clause per line. Record what happened, not what could happen.

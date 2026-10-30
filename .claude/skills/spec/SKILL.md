---
name: spec
description: Scaffold a milestone or phase specification in docs/specs/ following this project's spec conventions (facts relied on with confidence labels, decisions taken, numbered work packages with MUST/SHOULD/MAY/WONT and a done-condition each, deliberately-absent list). Use when the user says "spec <milestone>" or asks to start a new spec.
---

Create `docs/specs/<id>-<slug>.md` for the milestone or phase named in the arguments. Read `docs/specs/m1-basic-pong.md` first and mirror its shape exactly.

Required sections, in order:

1. Title and one-line purpose with links to `design.md` and `architecture.md`.
2. **Facts relied on**: every external fact the spec depends on, each labeled `[confirmed]`, `[probable]`, `[uncertain]`, or `[unknown]`, with how it will be verified if not confirmed.
3. **Decisions taken during specification**: answers the user gave, as flat bullets.
4. **Work packages**: numbered `N.M` and `N.M.K`, hierarchy equals dependency. Every item carries an obligation level in bold (`MUST`, `SHOULD`, `MAY`, `WONT`) and ends with `Done:` followed by a condition someone could check. Name couplings between items explicitly.
5. **Deliberately absent**: what is out of scope and why, so omissions are visible.

Rules:
- Check the design invariants in `docs/design.md` ("Invariants carried into specs") and the architecture rules; every item that touches one must honor it.
- Do not invent requirements. Where the user has not decided, add an **Open questions** section at the end and stop; the spec is complete only when it is empty.
- Update `docs/specs/CURRENT` only when the user says the phase has started.

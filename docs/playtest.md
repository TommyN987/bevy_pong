# Playtest Log

Fun is observed, not derived. One short session per child per milestone.
Write observations before interpreting them. Change at most one thing per session.

## Template

```
### YYYY-MM-DD · <milestone or phase> · <who played> · build <commit or branch>
Duration: ~N min
Observations (what happened, in order):
- ...
Quotes (verbatim, by role):
- the 9-year-old: "..."
- the 11-year-old: "..."
Interpretation (may be empty):
- ...
Proposed change (at most one):
- ...
```

## Hypotheses to test

- Menu with a single cursor and sequential seat selection: do the kids fight over the menu? If yes, revisit per-seat cursors (docs/architecture.md, Input). Outcome 2026-11-09: no fights reported (session below); single cursor kept.
- Keyboard ghosting: both seats holding up, down, and action at once (six keys) all register on the kids' keyboard. Result required before C3 (spec 1.8.2). Outcome 2026-11-07: no; at most five of the six register. Every in-play hold of one direction plus action per seat registers (session below).
- Catch-up bias (M2 spec 2.4.6): does one child win Chaos matches against the other so often that the loser stops wanting to play? M2 ships no bias; if yes, revisit one. No outcome yet.
- Wind and Spin (M2 spec 2.6): they share one field with different causes. Do the children tell them apart in play? No outcome yet.

## Sessions

### 2026-11-07 · M1 phase 4 (plan 4.6, spec 1.8.2) · the developer alone · build feat/shell-vertical at 27653fa plus uncommitted `just key-check`
Duration: two short runs; length not recorded.
Machine: the kids' machine, the development laptop's built-in keyboard.
Observations (what happened, in order):
- Run 1: the developer held W, D, A and ↑, ↓, ←. The log reached six: `[KeyW, KeyD, KeyA, ArrowUp, ArrowDown, ArrowLeft]`. A and ← stood in for the spec's S and →.
- Run 2: the developer tried the spec's six, W, S, D and ↑, ↓, →. The log never listed more than five.
- With ↑, ↓, → held, any two of W, S, D registered (W+S, S+D, W+D), never all three.
- The log listed no key outside the six being tried.
Quotes (verbatim, by role):
- the developer: "It's a bit hard to coordinate, but I managed to get the six keys." (run 1)
- the developer: "With those keys, I can't reach 6. It seems like any combination of 5 is fine, but the sixth one doesn't register" (run 2)
Interpretation (may be empty):
- Not a five-key cap: run 1 reached six. Which keys are combined decides it, as in matrix ghost blocking.
- Up and down held together cancel, so no player holds both in play. The heaviest in-play hold is one direction plus action per seat, four keys; each such hold (W or S with D, ↑ or ↓ with →) is inside a five-key set run 2 showed registering.
Proposed change (at most one):
- None now. Repeat this check when C2 gives the action key a job, holding one direction plus action per seat.

### 2026-11-09 · M1 close (plan 11.1, spec 1.8.3) · the 9-year-old and the 11-year-old, together · build not recorded (M1 close branch)
Duration: not recorded.
Reported by the parent after the session; no notes were taken during it.
Observations (what happened, in order):
- Both children played each other, and each played the computer.
- They went through the menus and read the screens without visible trouble.
- Neither fought the other over the menu.
- The sounds played for them, the win fanfare included; the ball trail showed.
Quotes (verbatim, by role):
- None recorded.
Interpretation (may be empty):
- The parent read the session as enjoyed, against each other and against the computer.
- One shared session stood in for one session per child: each child played both modes in it.
Proposed change (at most one):
- None.

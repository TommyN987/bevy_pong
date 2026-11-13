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
- Wind and Spin (M2 spec 2.6): they share one field with different causes. Do the children tell them apart in play? Outcome 2026-11-14: not observed directly. The parent reports that the children like the two best together, "the combination of Wind and Spin in the same rally", which names them as two; no child was reported telling which of the two bent a ball (session below).

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

### 2026-11-12 · M2 phase 4 (the stars' retune, spec 2.1.2) · the developer alone · build feat/ai-effects, commits not recorded
Duration: not recorded.
Recorded on 2026-11-13 from the [phase 4 report](plans/m2-phase4-report.md); no notes were taken during play.
Observations (what happened, in order):
- At M1's handicaps the developer played three stars over several attempts and scored one point in all, in a long rally.
- The stars were retuned by delays alone, to 0.29 s for two stars and 0.24 s for three. The developer played three stars and lost 10 to 0.
- The errors were then raised to 12, 7, and 3, the same evening. That candidate was not played that day.
Quotes (verbatim, by role):
- the developer: "leave it like this for now" (of the second candidate)
Interpretation (may be empty):
- None recorded.
Proposed change (at most one):
- None beyond the retune already made.

### 2026-11-13 · M2 phase 5 (plan 5.4, spec 2.10.1; E.6 for Shrink and Wind) · the 9-year-old, the 11-year-old, and the parent · build feat/wind at 74d9bea, the branch's head; not stated in the report
Duration: not recorded.
Reported by the parent after the session; no notes were taken during it.
Observations (what happened, in order):
- The children played each other, each played the parent, and they played the computer.
- Chaos was played with Shrink and Wind, the two effects in its bag.
- The 9-year-old beat the two-star computer in Chaos.
- Before the session the parent did the hand checks owed: Wind in `just run`, and the stars as retuned in phase 4. No detail of either was reported.
Quotes (verbatim, by role):
- None recorded.
Interpretation (may be empty):
- The parent's reading: both children like both Chaos effects, and like Chaos more than Classic.
- One shared session stood in for one session per effect: it covered Shrink and Wind, as plan 5.4 set.
Proposed change (at most one):
- None. The parent ruled neither a retune nor a drop for Shrink or for Wind (spec 2.10.2), and declared the phase complete.

### 2026-11-13 · M2 phase 6 (the hand check, plan E.3) · the developer alone · build feat/spin with steps 1 to 5 at Spin's starting numbers (committed as 6a87bdd)
Duration: not recorded.
Reported by the developer after playing; no notes were taken during it.
Observations (what happened, in order):
- The developer did the hand check of Spin in `just run`: "Everything seems to be working."
- The developer played "a little with the bots".
- The developer then played the two-star computer in the sandbox with Spin alone, the return count held at 20, where the dial is full.
Quotes (verbatim, by role):
- the developer, of the matches against the computers: "the spin was barely noticeable in the smaller and mid-length rallies."
- the developer, of the sandbox match: "it feels like there should be more spin throughout."
Interpretation (may be empty):
- None recorded.
Proposed change (at most one):
- More spin, for a tryout. Built that day: the dial full at return 6 where it was 12, and a full drift of 30 units where it was 16 ([M2 phase 6 report](plans/m2-phase6-report.md)).

### 2026-11-14 · M2 phase 6 (plan 6.2, spec 2.10.1; E.6 for Spin) · the parent and the children · build feat/spin at c14233c, the tryout's numbers; not stated in the report
Duration: not recorded.
Reported by the parent after the session; no notes were taken during it. Who played whom, and when, was not reported.
Observations (what happened, in order):
- The parent played the retuned Spin.
- The children played it too.
- The parent reports that the children like it, and like best a rally with Wind and Spin both in play.
Quotes (verbatim, by role):
- the parent: "this is way more fun"
- the parent, of the children: "Especially the combination of Wind and Spin in the same rally."
- "Pretty awesome", in the parent's report; the speaker was not stated.
Interpretation (may be empty):
- The parent's reading: the retuned Spin is more fun than Spin at its starting numbers, and the children like it.
- Claude's reading: liking the two effects together treats them as two. That is not the same as a child telling which one bent a ball, which nobody reported watching for. The hypothesis is marked so.
Proposed change (at most one):
- None. The parent ruled to keep Spin as it is: "Let's leave it like this." Neither a retune nor a drop (spec 2.10.2); Spin ships at the tryout's numbers.

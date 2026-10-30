# Design

Decided 2026-10-30, after [the frame](frame.md) and [the research](research.md).
This document records the chosen game concept, its milestone order, and the decisions that bind the first milestone.
Architecture is a separate design round.

## Axes

The design was factored into independent dimensions before candidates were compared.

| Axis | Range | Value fixed by the chosen order |
|---|---|---|
| Chaos source | items / fields / rules | fields (C1), rules (C3), items (C2) |
| Chaos driver | clock / rally length / score gap / level / player choice / beat | rally length (C1), level (C3), player choice (C2), beat (C4) |
| Player agency | zero action keys / one action key per player | zero until C2, then one |
| Opponent model | mirror paddle with error / boss with patterns | mirror (M1), boss (C3) |
| Co-op geometry | side-by-side / keeper and striker / vs boss | open, settled in C3's spec |
| Theme depth | skin only / skin plus rules | open |
| Save payload | records / unlocks / campaign position / per-kid profiles | profiles and records at M1, grown additively |
| Readability aid | none / telegraphing / slow-motion | open, settled per milestone |

Couplings: agency to the six-key keyboard budget; co-op geometry to the opponent model; theme depth to IP exposure; save payload to Bevy upgrade churn.

Surfaced during design and absent from the frame: per-kid profiles, and match structure (points to win, round length).

## Candidates considered

| Candidate | Core idea | Origin | Outcome |
|---|---|---|---|
| C1 Chaos Meter | rally-driven meter unlocks field effects; no action keys | research families C, G; Lethal League | chosen, milestone 2 |
| C2 Loadout | pick-ups stored and fired with one action key | research family A; Mario Kart analogy | chosen, milestone 4 |
| C3 Dungeon | rooms composed from effects, boss AIs, co-op, campaign | research families C, D, E; Shufflepuck; mixed-skill research | chosen, milestone 3 |
| C4 Stage | beat-driven spawns and bonuses with a visible beat bar | research families E, F; Hi-Fi Rush | chosen, milestone 5 |
| C5 Dungeon Master | one player places hazards while the other plays | research family F | rejected as core: not Pong for one player, invites sibling grief; possible later mode |

## Milestone spine

1. **Basic Pong.** Classic rules, mirror AI with reaction delay and prediction error, game states, neon bloom, CI, headless tests, per-kid profiles with records persisted.
2. **C1 Chaos Meter.** Visible meter climbs per paddle hit in a rally; thresholds enable field effects (gravity well, wind, ball split, paddle shrink); unlock pool persisted.
3. **C3 Dungeon.** Rooms composed from the effect pool; boss AIs with personalities; two-player co-op against a boss; campaign position persisted per profile. Split into C3a (one room, boss, co-op) and C3b (campaign and room library).
4. **C2 Loadout.** Pick-ups in the arena; one action key per player fires stored items; AI understands items.
5. **C4 Stage.** Beat clock drives spawns and bonuses; beat bar on screen; original music.

Rationale: the order is dependency-correct, each layer reuses the previous, and it follows the frame's learning path from physics and shaders through states, save and AI, to observers and relationships, to audio clocks.

## Invariants carried into specs

- C1's meter includes a catch-up bias toward the trailing player.
- C3's room and boss definitions carry an "items available" parameter from the start, so C2 does not reopen C3.
- C3 rooms are neon-generic; themes arrive as skins at or after C4.
- The keyboard ghosting test runs before C3 ships.
- The beat never gates paddle or ball movement.

## Decision: persistence lands in Basic Pong

**Chosen:** profiles and records are saved from milestone one, in a versioned plain-data file owned by the game. Later payloads (unlocks, campaign position) are added additively.

**Alternatives:** a dedicated save milestone before C1 (rejected: a milestone with nothing new to play, and records would be retrofitted); no persistence until C3 (rejected: leaves C1's unlock pool meaningless and overloads C3).

**Reasons:** Basic Pong already produces the payload. Two siblings make profile identity data at milestone one. The payload is a document, not a scene, so the game owns its format and versioning rather than serializing entities. Versioning discipline is cheapest when the payload is three fields.

## Decision: built-in audio behind a game-facing boundary

**Chosen:** `bevy_audio` at milestone one, reached only through a boundary that speaks game vocabulary, never engine asset handles. A beat clock is added at C4, and the backend is swapped then if needed.

**Amended 2026-10-30 during the architecture round:** the boundary is core's event vocabulary, not a facade the game calls. Audio is a shell plugin observing core events and app state; music cues derive from state transitions; volume is a setting; no audio type exists in core. See [architecture.md](architecture.md).

**Alternatives:** built-in with no boundary (rejected: systems emitting sound could not run in headless tests); Kira from milestone one, with or without boundary (rejected: third-party dependency on the upgrade treadmill four milestones before its clock is needed, while Bevy's audio direction is unsettled).

**Reasons:** Headless tests on `MinimalPlugins` have no audio, so the boundary is required by the quality bar regardless of backend. With the boundary, deferring the backend choice to C4 costs one module. Bevy is discussing a Firewheel-based replacement for its audio, undecided at time of writing.

## Deferred to specs

Co-op geometry, theme depth, readability aids, match structure, save file location and format details, boss personality design.

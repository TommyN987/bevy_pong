# ADR 0008: Players belong to the match config, per seat

Date: 2026-11-08
Status: accepted

## Context

Plan 7.3 records wins and losses per profile, and a Guest side records nothing (spec 1.4.9).
The match did not know who played as whom: `MatchConfig` named a `Controller` per side, and `Controller::Human(Seat)` named only the keys.
The menu picks a profile per seat (spec 1.6.3, architecture "Input").
Later milestones change who stands where: C3 brings co-op against a boss, with its geometry still open (`docs/design.md`), and campaign position per profile.
Seats do not change: the six-key keyboard gives exactly two (architecture "Input").
ADR 0002 caps the domain vocabulary at twelve non-message types, and expected `ProfileId` among them.

## Decision

The match config carries who each seat plays as; `Controller` stays the input source alone.

- Domain gains `ProfileId` (moved from persistence), `Player { Guest, Profile(ProfileId) }`, and `SeatPlayers { left_seat, right_seat }` with `SeatPlayers::GUESTS`.
- `MatchConfig` gains `players: SeatPlayers` and `player_on(side) -> Option<Player>`: the player of the seat holding the side, or `None` for a computer side.
- `Controller` is unchanged: `Human(Seat)` or `Ai(Difficulty)`.
- Persistence records each `MatchEnded` from `MatchConfig`: a win for the winning side's player, a loss for the loser's, nothing for a Guest or a computer side, and nothing at all when one profile sits on both seats.
- Amends ADR 0002: identifier types, whose only job is to name one thing of a kind (`ProfileId`, and later `EffectId`, `BossId`, `ItemId`), are outside the cap. Count on 2026-11-08: capped types 8 of 12 (`Side`, `Seat`, `Difficulty`, `Controller`, `MatchConfig`, `Random`, `Player`, `SeatPlayers`); identifier types 1 (`ProfileId`); messages 7.

## Alternatives considered

- A separate `SeatPlayers` resource in persistence (proposed first): rejected; the menu would keep two resources in step, a forgotten update would silently record nobody, and it needed a reset-on-leaving-a-match lifecycle.
- `Controller::Human { seat, player }`: rejected; identity would sit in the input type that seats, the AI, and rules match without using it, and would attach to paddles, duplicated when one human plays two paddles in C3. It also meant 35 call sites and a spec 1.3.1 change.
- For the cap: Guest as `Option<ProfileId>`, or `MatchConfig::player(seat)` over an unnamed pair: rejected by the user for the amendment above.

## Consequences

- The menu (plan 8.1) must name a player for both seats to build a `MatchConfig`; the compiler enforces it.
- A computer side cannot hold a profile: it has no seat.
- Seat-keyed players survive C3's reshaping of `MatchConfig` from sides to paddles.
- The approved "reset players to Guest on leaving a match" became unnecessary: there is no separate resource to reset.
- Tests build matches with `SeatPlayers::GUESTS` unless they test records.
- Amends ADR 0002 (vocabulary cap), spec 1.3.3 and 1.4.5, and `docs/architecture.md` rule 3 and "States".

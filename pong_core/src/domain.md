# domain

The shared vocabulary in [`domain.rs`](domain.rs):
types two or more features use, and the messages the core emits.
No systems live here.
It imports only the kernel.

## Why it exists

A type two features share must sit below both.
Otherwise one feature imports the other for the type, and they couple through it.
ADR 0002 caps the vocabulary at twelve non-message types,
so each addition answers "does every feature need this?"
Identifier types, which only name one thing of a kind, are outside the cap (ADR 0008).

## Types

| Type | Kind | Meaning |
|---|---|---|
| `Side` | enum (in `kernel::geometry`, which needs it first) | Where a paddle stands: `Left` (−x) or `Right` (+x). |
| `Seat` | enum | Who plays: a chair and its key set. Against the computer, one seat drives one side; in menus one seat picks for both. |
| `Difficulty` | enum | AI strength in stars; each variant has one tuning entry. |
| `Controller` | enum | What drives a side: `Human(Seat)` or `Ai(Difficulty)`. |
| `MatchConfig` | `Resource` | Controllers per side, `players` per seat, `points_to_win: NonZeroU8`, and the `mode`; zero points is unrepresentable. Inserted by the menu before a match. `player_on(side)` gives who plays a side, `None` for the computer. |
| `Player` | enum | Who a seated human plays as: `Guest` (records nothing, never saved) or `Profile(ProfileId)` (ADR 0008). |
| `SeatPlayers` | struct | A `Player` per seat; `GUESTS` for a match that records nothing. Keyed by seat, so a computer has none. |
| `Mode` | enum | Which game a match plays: `Classic`, or `Chaos { cap }` with the most scheduled effects at once. A mode is data in tuning; nothing branches on it but the lookup (M2 spec 2.1.1). |
| `ProfileId` | identifier, outside the cap | A profile's identity, stable across renames. |
| `EffectId` | identifier, outside the cap | An effect's identity, closed and serialized under a stable name (ADR 0011). Matched once per crate, to register; behaviour is reached through the effect's trait. |
| `Random` | `Resource` | The core's only randomness: two seeded `Pcg32` streams of one seed. Rally seeds are drawn from the second, so drawing one moves no other draw. One seed reproduces a match. |

## Messages

Each message has one emitting feature, `Notice` excepted.
One emitter means one place to look when a message appears.

| Message | Emitted by | Meaning |
|---|---|---|
| `Serve { toward }` | rules | Launch the ball toward a side. Read by ball. |
| `BallHitPaddle { side }` | physics | The ball bounced off that side's paddle. |
| `BallHitWall` | physics | The ball bounced off the top or bottom wall. |
| `EffectStarted { effect }` | escalation | A scheduled effect joined the effects in play, between two points. Always-on effects never start. |
| `EffectEnded { effect }` | escalation | A scheduled effect left the effects in play, between two points. Leaving a match ends nothing. |
| `PointScored { scorer }` | rules | A goal line was crossed. |
| `MatchEnded { winner, longest_rally }` | rules | A side reached the target; `longest_rally` is the match's most paddle hits in one rally, taken then (plan 7.4). |
| `SimFault` | sanity | An invariant broke; rules restarts the rally (spec 1.4.7). Registered by rules, its reader. |
| `Notice { text }` | tuning, persistence | One sentence for the shell's notice line. |

## How Bevy carries a message

`#[derive(Message)]` marks the type;
the emitting feature's plugin calls `add_message`,
which creates `Messages<M>` and lists the type in Bevy's message registry.
One shared system in `First` swaps every listed type's two buffers,
so a message lives about two swaps.
Each `MessageReader` keeps its own cursor in a `Local`,
so readers consume independently.
The swap waits for a fixed step to have run (see [the frame](../README.md#one-frame)),
so fixed-step messages reach `Update` readers even across frames with no fixed step.

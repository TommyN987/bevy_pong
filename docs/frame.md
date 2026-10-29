# Project Frame

Agreed 2026-10-30, before the first line of code.
This document defines the problem. It does not propose solutions.

## Goals

In priority order when goals conflict:

1. Fun for the players.
2. Learn Bevy and the ECS pattern.
3. Exercise creativity through twists on classic Pong.

Non-commercial, but held to a commercial quality bar: code quality, architecture, CI.

## Situation

- Empty repository. Rust and Bevy chosen. Linux desktop only.
- Solo developer, new to Bevy and ECS, hobby time budget.
- Players: two kids (11 and 9) and their parent.
- Play modes required:
  - kid vs kid
  - kid vs parent
  - one player vs computer
  - two players cooperating against the computer
- All local play on one shared keyboard.

## Complication

- Classic Pong is small. The twists carry the project, and the quality bar prices each one.
- The twist register is chosen: power-ups and chaos, amplified by physics distortions, scaled by progression (more progress, more chaos).
- Cooperative play changes Pong's geometry: two paddles on one side is no longer a symmetric duel.
- Player vs computer puts an AI opponent on the critical path of the first playable, not in a later twist.
- "Good-looking" with no artist means visual quality comes from style choices, not asset volume.

## Question

Which twists, in which order, turn Pong into a chaotic, progressively escalating game that two kids on one keyboard find fun, while each milestone teaches a new Bevy area and ships at production quality?

## Constraints

- Rust, Bevy, ECS idioms throughout.
- Linux desktop, shared keyboard, up to two humans plus AI.
- Readable by a 9-year-old: minimal text, visual feedback over menus.
- CI from the start: fmt, clippy, tests, release builds.
- Architecture decision records for significant decisions.
- Visually polished at every milestone, not deferred to the end.
- Baseline visual direction: neon vector.
- Progression is saved across runs.
- Milestone structure: classic Pong first (including the AI opponent), then one milestone per twist, each playable on its own.
- Each twist serves at least two of the three goals.
- Learning path: Bevy basics first, harder or niche areas layered on by later twists.

## Non-goals

- Monetization or store release.
- Online multiplayer or networking.
- Mobile, web, or console builds.
- Gamepad support, unless a twist requires it.
- A hand-drawn asset library.

## Notes for the twist brainstorm

Recorded as input, not as decisions:

- Theming may itself be a twist on top of the neon vector baseline.
- The kids' current interests: The Legend of Zelda (9-year-old), K-pop and Blackpink (11-year-old).

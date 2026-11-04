---
paths:
  - pong_core/**
---
# pong_core rules (see docs/architecture.md, "Intra-crate structure")

- Never depend on or import `bevy_render`, `bevy_window`, `bevy_winit`, `bevy_audio`, `bevy_ui`, `wgpu`, or the `pong` shell crate. Core uses `bevy` with default features off.
- `kernel/` is ECS-free: pure functions and plain data over `bevy_math` types. No `App`, `World`, `Query`, systems, or schedules there. The AI calls these functions directly.
- `domain/` holds only types used by two or more features and no systems. Keep it under a dozen non-message types; each message has exactly one emitting feature, `Notice` excepted (ADR 0002).
- `flow/` holds states, phase sets, the step gate, and `FIXED_HZ`; it imports only kernel, domain, and Bevy. Features import kernel, domain, flow, and the public components, resources, and messages of features earlier in the order in `features/mod.rs`. Never import `app/` from a feature (ADR 0002).
- Every system declares a fixed-step phase set: intent, effects, integrate, collide, rules, sanity. Never order against another feature's system by name.
- Rules owns every `MatchState` transition; the shell asks through ports (ADR 0003).
- Components own their invariants (clamping setters). Any feature may mutate through that API. Cross-feature triggers are events, never direct calls.
- Extension points are exhaustive enums (`EffectId`, `BossId`, `ItemId`). Match them exhaustively; no wildcard arms.
- Core emits events and reads port resources (intents, beat clock, `SavePaths`). It never calls the shell.
- Errors: typed enums only at boundaries (persistence, tuning validation); `Result<(), BevyError>` with context inside systems; invariants through the sanity machinery. No `unwrap` or `expect` in production core (clippy denies both). Test code may `expect` with a message that says what was expected (`clippy.toml`); the `testing` harness carries one module-level `#![expect(clippy::expect_used)]` for the same reason; `unwrap` stays denied everywhere.
- Randomness comes from the seeded random resource only.
- Before writing any Bevy API, verify it against the version pinned in `Cargo.toml` through the `bevy-api-check` agent: pinned crate source in the local cargo registry first, docs.rs as fallback. Behavior questions (what a reload does on failure) go to the agent too; it quotes the source.

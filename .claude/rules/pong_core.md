---
paths:
  - pong_core/**
---
# pong_core rules (see docs/architecture.md, "Intra-crate structure")

- Never depend on or import `bevy_render`, `bevy_window`, `bevy_winit`, `bevy_audio`, `bevy_ui`, `wgpu`, or the `pong` shell crate. Core uses `bevy` with default features off.
- `kernel/` is ECS-free: pure functions and plain data over `bevy_math` types. No `App`, `World`, `Query`, systems, or schedules there. The AI calls these functions directly.
- `domain/` holds only types used by two or more features and no systems. Keep it under a dozen types.
- Every system declares a fixed-step phase set: intent, effects, integrate, collide, rules, sanity. Never order against another feature's system by name.
- Components own their invariants (clamping setters). Any feature may mutate through that API. Cross-feature triggers are events, never direct calls.
- Extension points are exhaustive enums (`EffectId`, `BossId`, `ItemId`). Match them exhaustively; no wildcard arms.
- Core emits events and reads port resources (intents, beat clock, `SavePaths`). It never calls the shell.
- Errors: typed enums only at boundaries (persistence, tuning validation); `Result<(), BevyError>` with context inside systems; invariants through the sanity machinery. No `unwrap` or `expect` in core (clippy denies `unwrap_used`).
- Randomness comes from the seeded random resource only.
- Before writing any Bevy API, verify it against docs.rs for the version pinned in `Cargo.toml` (use the `bevy-api-check` agent).

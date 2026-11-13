---
paths:
  - pong_core/**
---
# pong_core rules (see docs/architecture.md, "Intra-crate structure")

- Never depend on or import `bevy_render`, `bevy_window`, `bevy_winit`, `bevy_audio`, `bevy_ui`, `wgpu`, or the `pong` shell crate. Core uses `bevy` with default features off.
- `kernel/` is ECS-free: pure functions and plain data over `bevy_math` types. No `App`, `World`, `Query`, systems, or schedules there. The AI calls these functions directly.
- `domain.rs` holds only types used by two or more features and no systems. Keep it under a dozen non-message types; each message has exactly one emitting feature, `Notice` excepted (ADR 0002).
- `flow/` holds states, phase sets, the step gate, and `FIXED_HZ`; it imports only kernel, domain, and Bevy. Features import kernel, domain, flow, and the public components, resources, and messages of features earlier in the order in `features/mod.rs`. Never import `app/` from a feature (ADR 0002).
- Every `FixedUpdate` system declares a fixed-step phase set: intent, effects, integrate, collide, rules, sanity. The step gate in `FixedFirst` is the one exception (ADR 0003). A system that serves, moves, collides, or judges the ball also joins `BallInPlay`, which runs only in `Playing` (ADR 0004). Never order against another feature's system by name.
- Rules owns every `MatchState` transition; the shell asks through ports (ADR 0003).
- Components own their invariants (clamping setters). Any feature may mutate through that API. Cross-feature triggers are events, never direct calls.
- Extension points are exhaustive enums (`EffectId` at C1, `ItemId` at C2, `BossId` at C3; none exists in M1). Match them exhaustively; no wildcard arms. For `EffectId`, ADR 0011 narrows this: one exhaustive match per crate registers effects; everywhere else an effect's behaviour is reached through its trait, never by matching on the id.
- Effects (ADR 0011, M2 spec 2.2 and 2.3): an effect implements the effect trait with no default methods, contributes only through composition channels, never imports another effect, and changes its parameters only at a flight start (serve or paddle hit). A field is a pure kernel function of the ball's motion and time in flight; physics and the AI's prediction evaluate the same composed flight.
- Core emits events and reads port resources (intents, `MatchControl` pause requests, `SavePaths`, the developer `Sandbox`; the beat clock at C4). It never calls the shell.
- Errors: typed enums only at boundaries (persistence, tuning validation); `Result<(), BevyError>` with context inside systems; invariants through the sanity machinery. No `unwrap` or `expect` in production core (clippy denies both). Test code may `expect` with a message that says what was expected (`clippy.toml`); the `testing` harness carries one module-level `#![expect(clippy::expect_used)]` for the same reason; `unwrap` stays denied everywhere.
- Randomness comes from the seeded random resource only.
- Before writing any Bevy API, verify it against the version pinned in `Cargo.toml` through the `bevy-api-check` agent: pinned crate source in the local cargo registry first, docs.rs as fallback. Behavior questions (what a reload does on failure) go to the agent too; it quotes the source. An API already used in this repository at the pinned version needs no new check: say in the handoff that no API new to the repository was written.

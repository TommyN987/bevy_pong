# M0: Spike

Specified 2026-10-30. Precondition for milestone one, per [architecture.md](../architecture.md).
Question answered: can a render-free Bevy core crate be built and tested headless, and is the workspace split worth it?

## Facts relied on

- Bevy 0.20.1 is current; feature collections `2d`, `ui`, `audio` exist; dev features are opt-in. [confirmed]
- `MinimalPlugins` runs without window, render, or audio. [confirmed]
- `bevy` with default features off plus ECS, state, asset, reflect, and time features compiles without `wgpu`. [probable; this spike verifies it]

## Work packages

- **0.1 MUST** Create a throwaway workspace outside the repo with `spike_core` and `spike_shell`. Done: both compile.
- **0.2 MUST** `spike_core` depends on `bevy` with default features off and only `bevy_state`, `bevy_asset`, `bevy_reflect`, and what they pull in. Done: `cargo tree -p spike_core` contains no `wgpu`, `winit`, or `bevy_render`.
- **0.3 MUST** One headless test builds an app with `MinimalPlugins` plus the asset plugin, deserializes an embedded RON struct, and reads it from a resource. Done: `cargo test -p spike_core` passes with no display.
- **0.4 MUST** Measure `cargo test -p spike_core` wall time cold and warm against a single-crate variant. Done: two numbers recorded.
- **0.5 MUST** Record the outcome as the first ADR: workspace kept, or collapsed to one crate. Done: `docs/adr/0001-crate-structure.md` exists.
- **0.6 WONT** Any gameplay code.

## Outcome handling

If 0.2 fails, the workspace collapses to a single crate. Every intra-crate rule in the architecture still holds; the test-speed justification is withdrawn and the ADR says so.

# ADR 0001: Workspace with a render-free core crate

Date: 2026-10-31
Status: accepted

## Context

`docs/architecture.md` proposes a two-crate workspace: `pong_core` (simulation, headless-testable, no render dependencies) and `pong` (shell). The split was justified by test speed and by making render types unreachable from simulation code, and it carried a precondition: a spike must show that `bevy` with default features off compiles and tests without `wgpu`, and must measure whether the split is worth it (`docs/specs/m0-spike.md`).

## Decision

Keep the workspace. `pong_core` depends on `bevy = "=0.20.1"` with `default-features = false` and features `std`, `async_executor`, `bevy_asset`, `bevy_state`. The shell depends on `bevy` with `2d`, `2d_api`, `ui`, `audio`.

## Evidence

Spike workspace (deleted after measurement): `spike_core` (render-free), `spike_shell` (render features, depends on core), `spike_single` (identical code to core, render features; the control). Each crate ran one headless test: `MinimalPlugins` + `AssetPlugin` + `StatesPlugin`, a custom `AssetLoader` deserializing a RON file embedded with `embedded_asset!`, polled until promoted into a resource, with a `Boot → Menu` state transition.

- Dependency graph (`cargo tree`): `spike_core` contains none of `wgpu`, `winit`, `bevy_render`, `bevy_window`, `bevy_winit`, `bevy_audio`, `bevy_ui`. The control contains `wgpu`, `winit`, `bevy_render`.
- The headless test passes in both crates; the asset loads and the state transition applies.
- `spike_shell` compiles against `spike_core` with `DefaultPlugins`.

Wall times from cargo's "Finished" lines, `cargo test -p <crate>`, separate target directories, dev profile with `opt-level = 1` for workspace crates and `3` for dependencies, Intel i7-9750H:

| Run | render-free core | single crate (control) |
|---|---|---|
| cold (deps from scratch) | 1 min 53 s | 13 min 23 s |
| warm, no change | 0.19 s | 9.27 s |
| incremental after touching `lib.rs` | 0.78 s | 26.35 s |
| target directory size | 1.2 GB | 8.3 GB |

The incremental row is the development loop. A simulation change in the core crate rebuilds and tests in under a second; the same change in a single crate costs about half a minute.

## Alternatives considered

- Single crate with feature-gated tests: rejected by the measurements above; every simulation test would pay the render stack's compile and link time.
- Workspace split by feature rather than core/shell: not measured; it does not change the render-free property, which is the one that pays.

## Consequences

- Phase 2 pins `bevy = "=0.20.1"` and the four core features above. `multi_threaded` was not needed for headless asset loading and stays off in core unless a later need appears.
- `StatesPlugin` is not in `MinimalPlugins`; the core plugin group and the testing module must add it.
- `embedded_asset!` requires `AssetPlugin` to be present first; plugin build order is a contract, not a convenience.
- The embedded asset path is `embedded://<crate>/<path relative to src/>`.
- The measurement script relied on `bc`, which is absent on the development machine; cargo's own timing lines were used instead. Any future timing recipe must not assume `bc`.

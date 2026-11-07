# ADR 0005: Dev builds on a pinned nightly with cranelift and mold

Date: 2026-11-07
Status: accepted

## Context

The user followed the optimisations section of Bevy's setup guide to shorten the edit-build loop.
Spec 1.1.1 had the toolchain on stable; the treadmill rule (CLAUDE.md) asks what each tool costs to keep current.

- Cranelift and `-Zshare-generics` need a nightly toolchain.
- With `mold`, `just dev-check` hung after `Compiling pong`: each statically linked test binary carried Bevy's full debug info (~1.4 GB), and `mold` linked about eleven at once at 3–4 GB each, so a 15 GB machine thrashed at ~96% I/O pressure (phase 4 report).
- Cranelift does not support catching panics: `catch_unwind` in cranelift-compiled code never catches, so both crash-file tests failed.

## Decision

The workspace builds on a dated nightly, `nightly-2026-11-06`, with cranelift for its own crates in dev builds and `clang` with `mold` as linker.

- `rust-toolchain.toml` pins the date and lists the `rustc-codegen-cranelift-preview` component; moving the date is a deliberate change.
- `.cargo/config.toml`: `clang` with `-fuse-ld=/usr/bin/mold` and `-Zshare-generics=y` for every build; cranelift for the dev profile, LLVM for dependencies.
- Tests build with LLVM (`[profile.test] codegen-backend = "llvm"`), so `catch_unwind` works in tests; `just run` keeps cranelift.
- Dependencies keep line tables only (`debug = "line-tables-only"`): test binaries shrink to ~0.5 GB, and a clean `just dev-check` peaks at ~8 GB.
- CI installs the same nightly and component, plus `clang` and `mold`.

## Alternatives considered

- A floating `nightly` channel: rejected; it changes on every `rustup update`, and a broken nightly stops the build unannounced.
- `split-debuginfo = "unpacked"` instead of line tables: offered, not taken; line tables were the smaller change and keep file and line in backtraces.

## Consequences

- Faster dev rebuilds and links, as Bevy's guide promises.
- A clean clone needs `clang` and `mold` installed, for release builds too.
- A nightly can break the build: each date move is a change to test, not an update.
- A debugger loses Bevy's variables; backtraces keep file and line inside Bevy.
- In dev builds, a panic in the workspace's own code cannot be caught by that code; Bevy's executors, compiled with LLVM, still catch system panics.

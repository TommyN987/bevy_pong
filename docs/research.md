# Research: Bevy and the Twist Space

Researched 2026-10-30 against [the frame](frame.md).
This document records findings. It makes no decisions.

Confidence labels: **[C]** confirmed from a primary source, **[P]** probable, **[U]** uncertain, **[?]** unknown.

## Bevy as of 2026-10-30

### Version and cadence

- Bevy 0.20.1 is current, released 2026-10-08. [C]
- Releases land about every four months with breaking changes: 0.18 in January 2026, 0.19 in June 2026, 0.20 in October 2026. [C]
- The 0.19 to 0.20 migration guide is still a draft with placeholder sections. [C]
- Third-party crates lag each release by days to months. [C]

### Ecosystem readiness for Bevy 0.20

| Crate | Purpose | Status |
|---|---|---|
| avian2d 0.8 | ECS-native 2D physics | ready [C] |
| moonshine-save 0.8 | save/load via Reflect | ready [C] |
| leafwing-input-manager 0.22 | action-mapped input | ready, published 2026-10-15 [C] |
| bevy_enhanced_input 0.27 | action-mapped input | ready [P] |
| bevy_kira_audio | audio with BPM clocks | ready [P] |
| bevy_hanabi 0.19 | GPU particles | lags, Bevy 0.19 only [C] |
| bevy_seedling | Firewheel audio, upstream candidate | last seen on Bevy 0.17 [U] |

### Features relevant to the frame

- **Neon look.** `Bloom` on `Camera2d` with HDR colors above 1.0, plus TonyMcMapface tonemapping and `DebandDither`, is the official 2D bloom recipe. [C]
- **Custom shaders.** `Material2d`, sprite materials, extended 2D materials, and `FullscreenMaterial` for post-processing exist. Shaders are now WESL; plain WGSL still works. [C]
- **UI.** Feathers and `bevy_ui_widgets` ship widgets and self-describe as experimental. Keyboard directional navigation exists since 0.18. [C]
- **Scenes.** BSN in code is in. A hot-reloadable `.bsn` asset format is listed as future work. [C]
- **Audio.** Built-in `bevy_audio` has no beat-aligned clock; its contributor notes say beat events need an audio-driver clock. Kira has BPM clocks with scheduled start times. A Bevy design note discusses replacing `bevy_audio` with Firewheel; no decision is recorded. [P]
- **Save.** moonshine-save states it does no versioning, validation, or backwards compatibility. [C]
- **ECS.** Observers, relationships, required components, resources as components, and panics-as-errors are in place. [C]
- **Dev loop.** `dynamic_linking` speeds iteration and must be gated off in release builds. Hot patching works through `bevy_simple_subsecond_system` plus the Dioxus CLI, binary crates only, experimental. [C]
- **Feature flags.** Bevy 0.20.1 exposes high-level `2d`, `3d`, `ui` collections and `2d_api`. `dev`, `dynamic_linking`, `hotpatching`, `bevy_ci_testing`, `bevy_remote`, and `file_watcher` are opt-in. [C]
- **CI.** The official `bevy_github_ci_template` runs fmt, clippy with warnings denied, and tests on stable Linux. Headless tests use `MinimalPlugins`. cargo-dist still releases as of early 2026. [P]
- **Linux.** The development machine has ALSA, X11, xkbcommon, Wayland, and the Vulkan loader. The Arch guide also lists pkgconf, libxcursor, libxrandr, libxi, and a pipewire-alsa shim; those were not checked. [?]

### Shared keyboard

- Most keyboards resolve about six simultaneous keys. [P]
- Arrow keys plus WASD is the combination developers report as safe. Modifier keys are counted separately. [P]
- Two players with up, down, and one action already use six keys. Any twist adding a second button per player risks ghosting. [P]
- Bevy reads physical key codes, so keyboard layout does not matter. [P]

## The twist space

Seven families were surveyed.

### A. Power-up chaos

- Mechanics surveyed: multi-ball, paddle grow and shrink, sticky catch, ghost ball, speed burst, reversed controls, temporary second wall.
- Prior art: Pong The Next Level, PowerPong, a CS50 student project where effects stack on each paddle hit. [C]
- Bevy areas: spawn and despawn, timers, observers, component-driven effects, relationships.
- Observed cost: each power-up must be understood by the AI or the AI becomes trivially beatable.

### B. Physics distortion

- Mechanics surveyed: gravity wells, wind, portals, rotating arena, magnet paddles, time dilation.
- Prior art: Psycho Pong rotates the field and accelerates rotation over time. [C]
- Bevy areas: avian2d or hand-rolled physics, custom WESL shaders, particles.
- Observed cost: distortions reduce ball predictability, which strains the frame's 9-year-old readability constraint.

### C. Escalation and progression

- Lethal League raises ball speed on every hit and displays the number; players describe the loop as risk-reward. [C]
- Windjammers 2 widens the super-shot timing window for the player who is behind. [C]
- Shufflepuck Café draws variety from opponent personalities rather than rules. [C]
- Pattern observed: a visible chaos meter driven by rally length or score, unlocks per win, boss paddles with personalities.
- Bevy areas: states and substates, persistence via Reflect, asset loading for unlockables.
- Observed cost: saved progression needs a schema that survives Bevy upgrades.

### D. Co-op geometry

- Mechanic surveyed: two paddles on one side against an AI wall or boss.
- Prior art: Pong The Next Level soccer mode puts a second paddle in front of the goal. [C]
- Research on mixed-skill family play favors asymmetric roles and interdependent progress over handicaps, and finds catch-up help must stay unobtrusive. [P, CHI 2024 study]
- Bevy areas: multi-entity player ownership, input mapping, AI pattern design.

### E. Theming as a twist

- Mechanic surveyed: palette, shape language, sound set, and small rule modifiers per theme on top of the neon baseline.
- Zelda-inspired elements surveyed: hearts for lives, a boomerang ball that returns, bombs that push paddles, dungeon rooms as arenas.
- K-pop-inspired elements surveyed: beat-driven arena where power-ups spawn on the downbeat, stage lights, lightstick paddles.
- Bevy areas: asset pipeline, hot reload via `file_watcher`, audio clocks, BSN once the asset format ships.
- Observed cost: Zelda and Blackpink are trademarks and copyrights. Non-commercial use reduces risk but does not remove it; shipping their music or art in a public repo is the exposure. [P]

### F. Approaches that break Pong's feel

- Rhythm Pong, where paddles or the ball move only on the beat. Hi-Fi Rush shows beat assists and soft off-beat penalties make rhythm approachable for non-rhythmic players. [C]
- Dungeon-master Pong, where one player is the arena and places hazards while the other plays. This turns a skill gap into a role.

### G. Inversion: how to make Pong impossible

- Invisible ball, swapped paddles, reversed controls, rotating arena, three balls, shrinking paddle.
- Each is a dial from zero to unplayable, which forms an escalation curve for family C.

### Assumptions versus constraints

- Pong conventions, not frame constraints: two paddles, one ball, rectangular arena, one-axis paddle motion, left-right symmetry.
- Real constraints: shared keyboard, Linux, 9-year-old readability.

### AI opponent patterns

- The standard recipe is reaction delay plus prediction error, with difficulty as a dial on both. [P, hobbyist sources]
- Error scaling with ball speed and bounce count, and adaptive difficulty when the AI dominates, are common extensions. [P]
- Fairness convention: the AI does not read the future ball position exactly. [P]

## Unknowns

- Whether the kids' keyboard supports two-player input. Testable with an anti-ghosting demo or a tiny Bevy app. [?]
- Audio direction: built-in, Kira, or wait for Firewheel. Beat-synced twists depend on it. [?]
- Tolerance for theming near real IP. [?]
- Whether bevy_hanabi reaches Bevy 0.20 before a particles milestone. [?]
- Whether screen-space distortion needs a full-screen pass or a mesh trick in Bevy. Not found in Bevy docs. [?]

## Sources

- [Bevy 0.20](https://bevy.org/news/bevy-0-20/), [Bevy 0.19](https://bevy.org/news/bevy-0-19/), [Bevy 0.18](https://bevy.org/news/bevy-0-18/)
- [Bevy feature flags](https://docs.rs/crate/bevy/latest/features), [Linux dependencies](https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md)
- [avian2d](https://docs.rs/avian2d/latest/avian2d/), [bevy_hanabi](https://docs.rs/crate/bevy_hanabi/latest), [moonshine-save](https://docs.rs/crate/moonshine-save/latest), [leafwing-input-manager 0.20 issue](https://github.com/Leafwing-Studios/leafwing-input-manager/issues/751), [Bevy Assets](https://bevy.org/assets/)
- [2D bloom example](https://bevy.org/examples/2d-rendering/bloom-2d/), [Material2d example](https://bevy.org/examples/shaders/shader-material-2d/)
- [CI template](https://raw.githubusercontent.com/bevyengine/bevy_github_ci_template/main/README.md), [how to test apps](https://docs.rs/crate/bevy/latest/source/tests/how_to_test_apps.rs), [bevy_dylib](https://docs.rs/crate/bevy_dylib/latest/source/src/lib.rs), [cargo-dist changelog](https://axodotdev.github.io/cargo-dist/changelog)
- [bevy_audio contributing notes](https://docs.rs/crate/bevy_audio/0.18.0/source/CONTRIBUTING.md), [Kira](https://docs.rs/crate/kira/latest/source/src/lib.rs), [Firewheel design note](https://hackmd.io/@bevy/rkwFscXObx)
- [hot patching](https://docs.rs/crate/bevy_simple_subsecond_system/0.1.3/source/readme.md)
- [keyboard rollover devlog](https://indifferentpenguin.itch.io/gamedaymayhem/devlog/677666/does-my-keyboard-support-2-player-input)
- [Pong variations](https://arcadeheroes.com/2012/12/03/as-arcade-heroes-turns-six-a-look-back-at-pong-variations/), [Pong The Next Level](https://strategywiki.org/wiki/Pong:_The_Next_Level), [Psycho Pong](https://aidenita.astrogeo.org/play/Psycho%20Pong)
- [Lethal League system](https://wiki.gbl.gg/w/Lethal_League/System), [Windjammers basics](https://sites.google.com/view/windjammersfrance/wiki/wiki-en/the-basics), [Shufflepuck Café](https://en.wikipedia.org/wiki/Shufflepuck_Caf%C3%A9)
- [Hi-Fi Rush accessibility](https://thetacomaledger.com/2023/02/27/the-ingenious-accessibility-of-hi-fi-rush/), [Crypt of the NecroDancer design](https://gamedeveloper.com/audio/game-design-deep-dive-finding-the-beat-in-i-crypt-of-the-necrodancer-i-)
- [mixed-skill play research](https://techandpeople.github.io/gaming), [Pong AI](https://codeincomplete.com/articles/javascript-pong/part5)

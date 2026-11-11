# app

The roof: the core plugin group, the error policy, and the Boot gate.
It depends on every feature and on flow; nothing depends on it.

## `CorePlugins` (`mod.rs`)

A `PluginGroup`: an ordered list of plugins the shell adds after `DefaultPlugins`,
and the harness after `MinimalPlugins` and `AssetPlugin`.

Build order and why:

| Plugin | Why here |
|---|---|
| `ErrorPolicyPlugin` | First, so nothing runs without the handler. |
| `SeededRandomPlugin` | Inserts `Random::seeded(seed)` before features draw from it. |
| `FlowPlugin` | States and phases before the features that name them. |
| `BootPlugin` | The gate out of `Boot`. |
| `TuningPlugin`, `SeatsPlugin`, `BallPlugin`, `PaddlePlugin`, `EffectsPlugin`, `PhysicsPlugin`, `RulesPlugin`, `AiPlugin`, `SanityPlugin`, `PersistencePlugin` | The declared feature order. |

`seed` decides every random draw in the core.
The shell passes fresh entropy each launch (`pong::seed`, plan 4.5); tests pass a constant.

Preconditions, the caller's to meet:
`AssetPlugin` is added first;
Bevy's `TimePlugin` is present;
nothing else sets the error handler;
a `MatchConfig` is inserted before entering `InMatch`.

`PluginGroup::set` replaces one member and keeps the order;
the harness uses it to force the Dev error policy.

## Error policy (`error_policy.rs`)

A system returning `Err`, or one whose `Res<T>` is missing, goes to the app's error handler.
`ErrorPolicy::Dev` panics, so a bug shows the first time;
`ErrorPolicy::Release` logs and skips the system, so a child never sees a crash.
`for_this_build` picks by `debug_assertions`.

The handler is set explicitly because Bevy's default, `match_severity`,
panics on a plain error in release too.
`App::set_error_handler` panics if called twice: the shell must not set its own.

Not every failure reaches the handler:
a `Single` query with zero or several matches skips its system silently.

## Boot gate (`boot.rs`)

`leave_boot` moves `Boot → Menu`.
It runs in `Update` under `in_state(AppState::Boot)` and `resource_exists` for `Tuning`, `Settings`, and `Profiles`,
so "wait until loaded" is a run condition, not a polled flag.
It lives in the roof because it reads a feature's type, which flow may not.

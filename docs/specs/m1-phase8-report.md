# M1 phase 8 report: UI flow

Started 2026-11-08. Branch `feat/ui`. Roles: Claude drives; the user reviews and commits.
Deviations from [m1-basic-pong.md](m1-basic-pong.md) and [m1-plan.md](m1-plan.md) per plan rule P.6, each with where it was recorded.

## Deviations

| Item | Deviation | Recorded |
|---|---|---|
| 1.4.9 | The game also writes in `Menu` whenever profiles or settings change, not only at match end; a failure retries at the next match end or menu change. Nothing is written during a match. Chosen by the user over a save port the shell sets (2026-11-08): a forgotten trigger would silently lose a change. | Spec 1.4.9 amended 2026-11-08. |
| Facts, `EditableText` | At 0.20.1 the widget takes keys only with the `TextInput` marker and input focus, and does not hide them from `ButtonInput<KeyCode>`; the menu reads its keys only while no name field has focus. | Spec facts amended 2026-11-08. |

## Decisions taken in this phase

- Plan 8.1 (2026-11-08, approved by the user): three parts, each a review stop. A: the core's profile creation and menu save trigger. B: menu input, screens, the match draft, name entry, settings, notice line. C: match HUD, pause, match over, and plan 8.3.
- Every menu screen is a vertical list and confirm picks, since A/Left backs out. The screens are a shell sub-state of `AppState::Menu`; a `MatchDraft` collects the picks and builds `MatchConfig`.
- Against the computer, the seat that confirmed the computer plays it on its own side and makes every later pick.
- The profile list is Guest, the saved profiles, then New; the right seat is not offered the left seat's profile.
- `Profiles::create` gives a new profile the id after the highest held, or 1 for the first; past `u32::MAX` it is refused as `NoProfileIdLeft`. A new profile goes last in the order.
- The menu trigger checks `is_changed` on `Profiles` and `Settings` itself: 0.20.1 has no `or` run condition (`or_else` short-circuits, and its second check would see a change one frame late). The first `Menu` frame always sees both as changed; the comparison with the last write keeps that from writing.
- The write failure notice now reads "The game could not save. It will try again.", since a retry no longer waits for a match.

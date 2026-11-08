# M1 phase 8 report: UI flow

Started 2026-11-08. Branch `feat/ui`. Roles: Claude drives; the user reviews and commits.
Deviations from [m1-basic-pong.md](m1-basic-pong.md) and [m1-plan.md](m1-plan.md) per plan rule P.6, each with where it was recorded.

## Deviations

| Item | Deviation | Recorded |
|---|---|---|
| 1.4.9 | The game also writes in `Menu` whenever profiles or settings change, not only at match end; a failure retries at the next match end or menu change. Nothing is written during a match. Chosen by the user over a save port the shell sets (2026-11-08): a forgotten trigger would silently lose a change. | Spec 1.4.9 amended 2026-11-08. |
| Facts, `EditableText` | At 0.20.1 the widget takes keys only with the `TextInput` marker and input focus, and does not hide them from `ButtonInput<KeyCode>`. The menu does not read its keys on the name screen at all, rather than checking focus: only Enter and Escape act there. Same effect, and nothing depends on when focus moves. | Spec facts amended 2026-11-08; this report. |
| Facts, fonts | Bevy's built-in font (a FiraMono subset) draws printable ASCII only, 95 code points: no ★, no arrows, no "ë". The game embeds DejaVu Sans Bold 2.37 (709 KB, Bitstream Vera licence, beside it in `pong/src/ui/fonts/`) in the default font's slot at `Startup`, so every text uses it. Chosen by the user (2026-11-08) over Adwaita Sans and an ASCII-only menu. An asset, not a crate. | Spec facts amended 2026-11-08; this report. |
| 1.6.3 | Names take Latin letters up to U+024F (all drawn by the font, checked), digits, spaces, and hyphens; a name is trimmed, and a blank one is not taken. | This report. |
| plan 4.5 | The dev entry starts a match only with `--vs`; bare `just run` shows the menus. Its notice log is gone, since the notice line shows every `Notice`; Escape keeps its match duties until part C. | This report. |
| 1.6.2, plan 8.1 | Paddles glow in their players' profile colours; no item named it, though the paddle view expected it in phase 8. Added to part C by the user (2026-11-08). | Spec 1.6.2 and plan 8.1 amended 2026-11-08. |

## Decisions taken in this phase

- Plan 8.1 (2026-11-08, approved by the user): three parts, each a review stop. A: the core's profile creation and menu save trigger. B: menu input, screens, the match draft, name entry, settings, notice line. C: match HUD, pause, match over, and plan 8.3.
- Every menu screen is a vertical list and confirm picks, since A/Left backs out. The screens are a shell sub-state of `AppState::Menu`; a `MatchDraft` collects the picks and builds `MatchConfig`.
- Against the computer, the seat that confirmed the computer plays it on its own side and makes every later pick.
- The profile list is Guest, the saved profiles, then New; the right seat is not offered the left seat's profile.
- `Profiles::create` gives a new profile the id after the highest held, or 1 for the first; past `u32::MAX` it is refused as `NoProfileIdLeft`. A new profile goes last in the order.
- The menu trigger checks `is_changed` on `Profiles` and `Settings` itself: 0.20.1 has no `or` run condition (`or_else` short-circuits, and its second check would see a change one frame late). The first `Menu` frame always sees both as changed; the comparison with the last write keeps that from writing.
- The write failure notice now reads "The game could not save. It will try again.", since a retry no longer waits for a match.
- Part B (2026-11-08): the menus in `pong::ui`. `ScreensPlugin` holds the game font, the boot screen, `MenuPlugin`, and `NoticeLinePlugin`.
  - `menu_input`: each seat's keys mean up, down, confirm, back; Escape backs out for anyone; a screen is driven by either seat or by one.
  - `menu`: the `MenuScreen` sub-state, `MenuList` (rows and cursor; the cursor stops at both ends), the private `MatchDraft`, and the choice logic as a pure function. A seat left empty by the computer is Guest in the `MatchConfig`, so a pick from an abandoned path never reaches the match.
  - `menu::name_entry`: the name field (Bevy's `TextInput` and `EditableText`, at most `ProfileName::MAX_CHARS`, filtered); Enter takes the name to the colours; back from the colours keeps it.
  - `menu::view`: a one-word heading per screen, the seat cue (the seat's up, down, and confirm keys) while one seat drives, one framed row per item with the cursor's row framed white. A seat's screens sit on its half; the name screen too, without a key cue, since its keys type.
  - `notice_line`: every `Notice` since the last match start, joined on one line at the foot of the menu.
  - `window`: Alt+Enter toggles the saved mode; the window follows `Settings`, and a fullscreen window is left on the monitor it opened on.
- The name field is not given a `Text`: `EditableText` lays out its own text, and both would write one layout (`bevy-api-check`).
- The glyph coverage of both fonts was read from their files' character maps by a script: the fonts are assets, not Bevy API.
- TDD: the green steps of the menu flow (13 tests) and of the name screen were larger than one test. Every rule they built was checked against a deliberate break: nothing withheld from the right seat, seat screens shared, the computer always against the left seat, abandoned picks reaching the match, no default points, no current volume, no trim, no cap, the name forgotten on back, seat keys navigating on the name screen, the cursor mark not moving, the right seat on the left half, notices never cleared or never shown, the window not following, a fullscreen window re-moded, the font not installed. Each fails a test. One break survived (the name screen without its seat), which exposed the key cue there; the test and the fix followed.
- Hand check (the user, 2026-11-08): a window saved by the last run opened fullscreen. Cause, read from the source (`bevy-api-check`; not observed in a run): the window opened fullscreen and the saved mode was applied in the first frames; on Wayland the compositor confirms the opening fullscreen only after that, Bevy skips a change to the mode the window reports (`bevy_winit` 0.20.1, `system.rs:368-371`) and records it as applied, so leaving fullscreen was dropped. Fix: the window opens in the saved mode. The core's `persistence::load::peek` reads a save file without the load policy (nothing moved, no notice); the shell reads the settings with it before building the app (`pong::window::opening_mode`). This also removes the moment of fullscreen before a saved window. The headless tests could not see it: they have no compositor.
- Part C (2026-11-08): the match screens in `pong::ui`, in `ScreensPlugin`.
  - `hud`: each side's score over its half for the whole match.
  - `pause`: Escape pauses wherever the paddles move; the screen offers Play and Menu, Play under the cursor each time; Escape, the back key, or Play resumes; Menu leaves the match, recording nothing. Its keys are read before the fixed steps, so rules acts within the frame (ADR 0003 note).
  - `match_over`: "<winner> wins!", the winner named by profile, by side for a Guest ("Left"), by stars for the computer; then each profile's gains, from a copy of the records taken on entering the match, so the screen shows what was recorded. Confirm by either seat, or Escape, returns to the main menu. No rematch: the spec asks for none.
  - `style`: the text size, the dim colour, and the framed row the menus and pause screen share.
  - Paddles (`views::paddle`): a material per paddle in its player's colour, scaled in linear light so its brightest channel is 3.0, the default paddle's; Guest and computer keep the default cyan. The rule is a pure function with unit tests; the system attaching it is checked by eye, as the views are.
- Plan 8.3: the dev entry and its tests are gone, and `just run` takes no arguments. Removal adds no behaviour, so it has no red step; the pause screen's resume after a focus loss is tested.
- Every Part C rule was checked against a deliberate break: Escape never pausing, Escape never resuming, Menu resuming, the pause cursor kept between pauses, no gains seen, Guests named "Guest", Escape ignored on match over, the score never updating, the profile colour ignored. Each fails a test.
- The two crash tests (`pong/tests/crash.rs`) passed the 30-second per-test limit twice in this phase, each time on the first test run after a rebuild, and passed in about a second on every rerun. Each forces a panic whose hook symbolizes a full backtrace from the 500 MB test binary, read cold from disk while some 330 other tests start. Likely also the unexplained exit 100 of phase 5. Not changed here: the limit is spec 1.5.2's, the user's to relax for these two tests.

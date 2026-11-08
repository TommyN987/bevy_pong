# M1 phase 9 report: Audio

Started 2026-11-08. Branch `feat/audio`. Roles: Claude drives; the user reviews and commits.
Deviations from [m1-basic-pong.md](m1-basic-pong.md) and [m1-plan.md](m1-plan.md) per plan rule P.6, each with where it was recorded.

## Deviations

| Item | Deviation | Recorded |
|---|---|---|
| Facts, audio source | Confirmed through the custom-source route, not bytes: the game's `Sound` asset implements Bevy's `Decodable` and hands rodio its samples. The bytes route needs Bevy's `wav` feature (adding `hound`), a WAV writer, and Bevy's decoder unwraps, so a missing format feature panics. | Spec facts amended 2026-11-08. |
| 1.6.3 | Every menu list shows at most six rows (`SHOWN_ROWS`), following the cursor one row at a time, opening with it in the middle short of either end; ▲ and ▼ show where rows are hidden. Found at the user's hand check (2026-11-08): volume's eleven rows ran off the screen, 90 and 100 below it, still reachable. Profile select, with no cap on profiles, would have followed. Chosen by the user over smaller rows, volume in twenties, and Bevy's own scrolling, which only the layout pass could test. | Spec 1.6.3 amended 2026-11-08. |
| Architecture, error policy | Audio init has no typed `Result` of ours: Bevy's `AudioPlugin` warns "No audio device found." and skips playback (`bevy-api-check`). Cue players then stay spawned, since Bevy's cleanup is skipped with playback: one entity per cue, a few hundred per match, harmless. | `docs/architecture.md` amended 2026-11-08; the audio boundary ADR at plan 11.2. |

## Decisions taken in this phase

- Plan 9.1 and 9.2 (2026-11-08, approved by the user as one proposal): `pong::audio`.
  - `sound`: `Sound`, a `Decodable` asset of mono `f32` samples at a rate, shared behind an `Arc`; `Playback`, one play of it, yields the samples in order. The whole sound is one span; `Some(0)` once played, as rodio's `Source` contract asks (`bevy-api-check` corrected a first guess of "samples left").
  - `synth`: `Synth`, the `SoundBank`. Each cue is sine notes at 44.1 kHz, peak 0.5, a 5 ms fade-in and a linear fade-out per note. Paddle: 660 Hz, 70 ms. Wall: 330 Hz, 50 ms. Point: a glide 880 to 220 Hz, 350 ms. Match won, after the user's hand check asked for something more victorious: silence for the point cue's length (the final point plays both; chosen by the user), three short G calls, a held C, a C major chord ringing out, about 1.3 s, with three overtones for a brass-like tone. A note holds any number of pitches, none for a rest; dividing by the voices and the overtone weights keeps every sample within the peak. The phase accumulates per sample, so a glide follows its pitch.
  - `cues`: the cue map. One `AudioPlayer` per `BallHitPaddle`, `BallHitWall`, `PointScored`, `MatchEnded` read, despawning once played. It imports cue names and `CueSounds`, never `synth` (spec 1.6.4 SHOULD).
  - `SoundsPlugin<B: SoundBank>` builds the four sounds when the app is built. After Bevy's `AudioPlugin` it registers `Sound` as a playable source (`add_audio_source`); without it, as in headless tests, only as an asset. `add_audio_source` registers the asset itself; registering it twice would drop the sounds added between (`bevy-api-check`). `main.rs` adds it after `DefaultPlugins`, since a plugin counts as added only once its build returns.
- Volume: each cue starts at the settings' volume, linear in the percent, so a change applies from the next cue. Not Bevy's `GlobalVolume`: it too applies only when a sound starts, and exists only with `AudioPlugin`, so a headless test could not read it.
- The final point sends `PointScored` and `MatchEnded` in one step; both cues play, as spec 1.6.4's one play per event says, the fanfare after the point's glide.
- The cue map tests run in a match's countdown: the core sends no event of its own there, and a `MatchEnded` outside a match is the core's invariant error (no `MatchConfig`).
- TDD: three cycles, red then green: playback, the synth's buffers, the cue map. Checked against deliberate breaks: no fade-in, no fade-out, a clipping peak, a near-silent peak, wall sounding like paddle, the wall cue mapped to the paddle's, `MatchEnded` unread, one cue per frame, a fixed volume, players that never despawn. Each fails a test. The fade-in break survived at first, since `sin(0) = 0` makes every first sample silent; the test now bounds the first millisecond.
- The fanfare and the scrolling list (2026-11-08), red then green each. Checked against deliberate breaks: the chord or the overtones not scaled, no wait, a short fanfare, a rest dividing by zero, no glide, the shown rows not following up or down, a list opening at the top, no clamp at the end, rows never hidden, either mark never shown. The overtone break and the glide break survived at first: samples stayed within full scale by luck of phase, and nothing read the pitch; the range test now bounds by the peak, and a test counts zero crossings, which a falling pitch makes ever fewer of. The clamp break survived too, since 80 opens on the last six rows either way; a test opens at 100.
- Hand checks (the user, 2026-11-08): the four cues audible and distinct; volume follows settings.
- Not tested, judged by ear and eye: how the sounds sound; that six rows fit the screen (the layout pass needs a window); the wiring with a real device (a headless test cannot open one).

//! The synthesized sound bank (spec 1.6.4): each cue is a few notes, made in
//! code when the app is built.

use std::f32::consts::TAU;
use std::num::NonZero;

use bevy::audio::{Sample, SampleRate};

use super::sound::Sound;
use super::{Cue, SoundBank};

/// Samples per second; a `u16`, so it widens to `f32` and to a
/// [`SampleRate`] exactly.
const HZ: NonZero<u16> = NonZero::new(44_100).expect("a rate above zero");
/// The loudest a sample gets: half of full scale, leaving room for sounds
/// that overlap.
pub const PEAK: f32 = 0.5;
/// How long a note takes to rise from silence, in seconds; a sound that
/// starts at full height clicks.
const FADE_IN: f32 = 0.005;
/// The point cue's length; the win cue waits it out, since the final point
/// plays both.
const POINT_SECONDS: f32 = 0.35;

/// A plain sine.
pub const PLAIN: &[f32] = &[1.0];
/// A sine with its next three overtones, fading: brighter, brass-like.
pub const BRASS: &[f32] = &[1.0, 0.5, 0.33, 0.25];

/// Pitches sounding together, fading out over `seconds`; none is a rest.
#[derive(Debug, Clone, Copy)]
pub struct Note {
    hz: &'static [f32],
    /// Where every pitch ends, as a multiple of where it starts: 1 holds it.
    glide: f32,
    seconds: f32,
}

/// `hz` held for `seconds`, fading out.
#[must_use]
pub const fn tone(hz: &'static [f32], seconds: f32) -> Note {
    Note {
        hz,
        glide: 1.0,
        seconds,
    }
}

/// Silence for `seconds`.
#[must_use]
pub const fn rest(seconds: f32) -> Note {
    tone(&[], seconds)
}

const G4: &[f32] = &[392.0];
const C5: &[f32] = &[523.25];
/// C major, C5 to C6.
const C_MAJOR: &[f32] = &[523.25, 659.25, 783.99, 1046.5];

/// Synthesizes every cue's sound.
#[derive(Debug, Clone, Copy, Default)]
pub struct Synth;

impl SoundBank for Synth {
    fn sound(&self, cue: Cue) -> Sound {
        let (notes, timbre): (&[Note], _) = match cue {
            Cue::PaddleHit => (&[tone(&[660.0], 0.07)], PLAIN),
            Cue::WallHit => (&[tone(&[330.0], 0.05)], PLAIN),
            Cue::Point => (
                &[Note {
                    hz: &[880.0],
                    glide: 0.25,
                    seconds: POINT_SECONDS,
                }],
                PLAIN,
            ),
            // Three short calls, a held note, a chord ringing out.
            Cue::MatchWon => (
                &[
                    rest(POINT_SECONDS),
                    tone(G4, 0.12),
                    rest(0.04),
                    tone(G4, 0.12),
                    rest(0.04),
                    tone(G4, 0.12),
                    rest(0.04),
                    tone(C5, 0.3),
                    tone(C_MAJOR, 0.65),
                ],
                BRASS,
            ),
        };
        phrase(notes, timbre)
    }
}

/// `notes` one after another in `timbre`, as a sound: how a cue outside the
/// bank's own, an effect's join cue, is made in the same voice.
#[must_use]
pub fn phrase(notes: &[Note], timbre: &[f32]) -> Sound {
    Sound::new(SampleRate::from(HZ), render(notes, timbre))
}

/// `notes` one after another, each pitch with the overtones `timbre`
/// weighs. The phase, in cycles, grows by the pitch over the rate each
/// sample: a glide's pitch then follows its path, which
/// `sin(TAU * hz * t)` with a changing `hz` would not. Dividing by the
/// voices and the weights keeps a chord within [`PEAK`].
fn render(notes: &[Note], timbre: &[f32]) -> Vec<Sample> {
    let step = 1.0 / f32::from(HZ.get());
    let weight: f32 = timbre.iter().sum();
    let mut samples = Vec::new();
    for note in notes {
        let voices: f32 = note.hz.iter().map(|_| 1.0).sum();
        let mut phases = vec![0.0_f32; note.hz.len()];
        let mut elapsed = 0.0_f32;
        while elapsed < note.seconds {
            let progress = elapsed / note.seconds;
            let envelope = (elapsed / FADE_IN).min(1.0) * (1.0 - progress);
            let bend = 1.0 + (note.glide - 1.0) * progress;
            let mut sum = 0.0;
            for (phase, hz) in phases.iter_mut().zip(note.hz) {
                for (overtone, gain) in (1_u8..).zip(timbre) {
                    sum += gain * (TAU * f32::from(overtone) * *phase).sin();
                }
                *phase = (*phase + hz * bend * step).fract();
            }
            let level = if phases.is_empty() {
                0.0
            } else {
                sum / (voices * weight)
            };
            samples.push(PEAK * envelope * level);
            elapsed += step;
        }
    }
    samples
}

#[cfg(test)]
mod tests {
    use super::{HZ, PEAK, Synth};
    use crate::audio::{Cue, SoundBank};

    /// Spec 1.6.4: buffers finite, non-silent, in range; the range is the
    /// synth's peak, which leaves headroom for cues that overlap.
    #[test]
    fn every_cue_sounds_finite_audible_and_within_the_peak() {
        for cue in Cue::ALL {
            let sound = Synth.sound(cue);
            let samples = sound.samples();
            assert!(!samples.is_empty(), "{cue:?} is empty");
            assert!(samples.iter().all(|s| s.is_finite()), "{cue:?}");
            assert!(
                samples.iter().all(|s| s.abs() <= PEAK),
                "{cue:?} passes the peak"
            );
            let peak = samples.iter().fold(0.0_f32, |peak, s| peak.max(s.abs()));
            assert!(peak > 0.1, "{cue:?} is silent: peak {peak}");
        }
    }

    /// A sound that jumps from or to silence clicks: it rises over its first
    /// millisecond, which `sin(0) = 0` alone would not show, and ends at
    /// silence.
    #[test]
    fn every_cue_rises_from_silence_and_ends_in_it() {
        let millisecond = usize::from(HZ.get()) / 1000;
        for cue in Cue::ALL {
            let sound = Synth.sound(cue);
            let samples = sound.samples();
            let rise = samples
                .iter()
                .take(millisecond)
                .fold(0.0_f32, |peak, s| peak.max(s.abs()));
            assert!(
                rise < 0.15,
                "{cue:?} jumps to {rise} in its first millisecond"
            );
            let last = samples.last();
            assert!(
                last.is_some_and(|s| s.abs() < 0.01),
                "{cue:?} ends at {last:?}"
            );
        }
    }

    /// The final point sends `PointScored` and `MatchEnded` together: the
    /// fanfare waits out the point's glide, then plays for over a second.
    #[test]
    fn the_win_waits_for_the_point_cue_then_plays_a_long_fanfare() {
        let point = Synth.sound(Cue::Point).samples().len();
        let win = Synth.sound(Cue::MatchWon);
        let (rest, fanfare) = win.samples().split_at(point.min(win.samples().len()));
        assert!(
            rest.iter().all(|&s| s == 0.0),
            "the win sounds over the point"
        );
        assert!(
            fanfare.len() > usize::from(HZ.get()),
            "the fanfare lasts {} samples",
            fanfare.len()
        );
    }

    /// A falling pitch crosses zero ever less often.
    #[test]
    fn the_point_cue_falls_in_pitch() {
        let sound = Synth.sound(Cue::Point);
        let quarter = sound.samples().len() / 4;
        let crossings = |part: &[f32]| {
            part.windows(2)
                .filter(|pair| (pair[0] < 0.0) != (pair[1] < 0.0))
                .count()
        };
        let first = crossings(&sound.samples()[..quarter]);
        let last = crossings(&sound.samples()[sound.samples().len() - quarter..]);
        assert!(
            first > 2 * last,
            "{first} crossings at the start, {last} at the end"
        );
    }

    /// Spec 1.6.4: four distinct sounds.
    #[test]
    fn the_four_cues_sound_different() {
        for (index, cue) in Cue::ALL.into_iter().enumerate() {
            for other in Cue::ALL.into_iter().skip(index + 1) {
                assert_ne!(
                    Synth.sound(cue).samples(),
                    Synth.sound(other).samples(),
                    "{cue:?} and {other:?}"
                );
            }
        }
    }
}

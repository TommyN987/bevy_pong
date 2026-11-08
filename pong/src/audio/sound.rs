//! A sound as samples, and how Bevy plays one (plan 9.1). Bevy's audio plays
//! any asset that hands out a sample iterator (`Decodable`), so a sound made
//! in code needs no file format.

use std::sync::Arc;
use std::time::Duration;

use bevy::asset::Asset;
use bevy::audio::{ChannelCount, Decodable, Sample, SampleRate, Source};
use bevy::reflect::TypePath;

/// A sound: mono samples at `rate`, each within ±1.
#[derive(Asset, TypePath, Clone, Debug)]
pub struct Sound {
    samples: Arc<[Sample]>,
    rate: SampleRate,
}

impl Sound {
    /// `samples`, played at `rate` per second.
    #[must_use]
    pub fn new(rate: SampleRate, samples: impl Into<Arc<[Sample]>>) -> Self {
        Self {
            samples: samples.into(),
            rate,
        }
    }

    /// The samples, in playing order.
    #[must_use]
    pub fn samples(&self) -> &[Sample] {
        &self.samples
    }

    /// How long the sound plays.
    #[must_use]
    pub fn duration(&self) -> Duration {
        const NANOS_PER_SECOND: u64 = 1_000_000_000;
        let samples = self.samples.len() as u64;
        Duration::from_nanos(samples.saturating_mul(NANOS_PER_SECOND) / u64::from(self.rate.get()))
    }
}

impl Decodable for Sound {
    type Decoder = Playback;

    /// Called on Bevy's side once per play. The playback owns a clone, since
    /// the audio thread pulls from it after this frame; the clone shares the
    /// samples (`Arc`), so a play copies none.
    fn decoder(&self) -> Playback {
        Playback {
            sound: self.clone(),
            next: 0,
        }
    }
}

/// One play of a [`Sound`]: its samples in order, then the end.
#[derive(Clone, Debug)]
pub struct Playback {
    sound: Sound,
    next: usize,
}

impl Iterator for Playback {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        let sample = self.sound.samples.get(self.next).copied()?;
        self.next += 1;
        Some(sample)
    }
}

impl Source for Playback {
    /// The whole sound is one span; `Some(0)` once it has played, as rodio
    /// requires of a source that names its span.
    fn current_span_len(&self) -> Option<usize> {
        let total = self.sound.samples.len();
        Some(if self.next < total { total } else { 0 })
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::MIN
    }

    fn sample_rate(&self) -> SampleRate {
        self.sound.rate
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.sound.duration())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::audio::{ChannelCount, Decodable, SampleRate, Source};

    use super::Sound;

    fn rate(hz: u32) -> SampleRate {
        SampleRate::new(hz).expect("a rate above zero")
    }

    #[test]
    fn a_playback_yields_the_sounds_samples_in_order_then_ends() {
        let sound = Sound::new(rate(8), vec![0.0, 0.5, -0.5, 0.25]);
        let played: Vec<f32> = sound.decoder().collect();
        assert_eq!(played, [0.0, 0.5, -0.5, 0.25]);
        assert_eq!(sound.decoder().count(), 4, "each play starts at the top");
    }

    #[test]
    fn a_playback_is_mono_at_the_sounds_rate_for_the_sounds_length() {
        let sound = Sound::new(rate(4), vec![0.0; 6]);
        let playback = sound.decoder();
        assert_eq!(playback.channels(), ChannelCount::MIN, "one channel");
        assert_eq!(playback.sample_rate(), rate(4));
        assert_eq!(sound.duration(), Duration::from_millis(1500));
        assert_eq!(playback.total_duration(), Some(Duration::from_millis(1500)));
    }

    /// Rodio's contract (`Source::current_span_len`): a span's total size,
    /// and `Some(0)` exactly when no sample is left, which `is_exhausted`
    /// reads.
    #[test]
    fn a_playback_is_one_span_the_sounds_length_then_zero_once_played() {
        let mut playback = Sound::new(rate(4), vec![0.0; 3]).decoder();
        assert_eq!(playback.current_span_len(), Some(3));
        playback.next();
        assert_eq!(
            playback.current_span_len(),
            Some(3),
            "the total, not the rest"
        );
        playback.by_ref().for_each(drop);
        assert_eq!(playback.current_span_len(), Some(0));
        assert!(playback.is_exhausted());
    }
}

//! The shell half of every effect (ADR 0011, M2 spec 2.3.3): how an effect
//! looks and sounds. The core half, in `pong_core`, is how it plays.
//!
//! One module per effect, as in the core. [`shell_half`] is this crate's one
//! match on [`EffectId`]: a new variant does not compile until its module
//! states an icon, a join cue, and a view, each of which may be none. The
//! HUD, the cue map, and the views reach an effect through the half it
//! returns and never match on the id themselves (M2 spec 2.3.8).

use bevy::app::{App, Plugin};
use bevy::color::Color;
use pong_core::domain::EffectId;

use crate::audio::sound::Sound;

pub mod shrink;
pub mod speed;
pub mod spin;
pub mod wind;

/// An effect's icon: one glyph of the game's font, in the effect's colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Icon {
    /// What the HUD draws; a glyph the game's font holds.
    pub glyph: &'static str,
    /// The colour it is drawn in.
    pub color: Color,
}

/// One effect's shell half. There is no `Default`: an effect states all
/// three parts, as the core's `Contribution` makes it state every channel.
pub struct ShellHalf {
    /// Shown in the HUD while the effect is in play; `None` for an effect
    /// the players are not told about.
    pub icon: Option<Icon>,
    /// Builds the sound played once when the effect joins the effects in
    /// play. A constructor, not the sound: the sounds are built once, when
    /// they are loaded, and asking for an effect's icon synthesizes nothing.
    pub join_cue: Option<fn() -> Sound>,
    /// Adds the systems that draw what the base views do not; `None` when
    /// the base views already show the effect.
    pub view: Option<fn(&mut App)>,
}

/// The shell's registration of every effect: the one place in this crate
/// that matches on [`EffectId`].
#[must_use]
pub fn shell_half(id: EffectId) -> ShellHalf {
    match id {
        EffectId::Speed => speed::shell_half(),
        EffectId::Shrink => shrink::shell_half(),
        EffectId::Wind => wind::shell_half(),
        EffectId::Spin => spin::shell_half(),
    }
}

/// Adds every effect's own view systems (M2 spec 2.3.11: an effect writes
/// systems only for what the base lacks).
pub struct EffectViewsPlugin;

impl Plugin for EffectViewsPlugin {
    fn build(&self, app: &mut App) {
        for id in EffectId::ALL {
            if let Some(view) = shell_half(id).view {
                view(app);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;

    use pong_core::domain::{EffectId, Mode};
    use pong_core::features::tuning::{EMBEDDED_RON, Tuning};

    use super::shell_half;
    use crate::audio::synth::PEAK;

    fn scheduled() -> Vec<EffectId> {
        let tuning = Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid");
        let chaos = Mode::Chaos {
            cap: NonZeroU8::MIN,
        };
        tuning.mode(chaos).bag().to_vec()
    }

    /// M2 spec 2.6.0: Speed's shell half is declared empty.
    #[test]
    fn speed_declares_an_empty_shell_half() {
        let half = shell_half(EffectId::Speed);
        assert!(half.icon.is_none() && half.join_cue.is_none() && half.view.is_none());
    }

    /// M2 spec 2.6.2 and R1: Wind states all three. Its direction is the
    /// one thing the base views cannot show, so it has a view of its own.
    #[test]
    fn wind_states_an_icon_a_join_cue_and_a_view() {
        let half = shell_half(EffectId::Wind);
        assert!(half.icon.is_some() && half.join_cue.is_some() && half.view.is_some());
    }

    /// M2 spec 2.6.4 and R1: Spin states all three. Which way a return was
    /// spun is the one thing the base views cannot show, so it has a view
    /// of its own.
    #[test]
    fn spin_states_an_icon_a_join_cue_and_a_view() {
        let half = shell_half(EffectId::Spin);
        assert!(half.icon.is_some() && half.join_cue.is_some() && half.view.is_some());
    }

    /// M2 spec 2.7.1 and R1: an effect the schedule can bring into play is
    /// announced, by an icon and a cue.
    #[test]
    fn every_scheduled_effect_has_an_icon_and_a_join_cue() {
        for id in scheduled() {
            let half = shell_half(id);
            assert!(half.icon.is_some(), "{id:?} has no icon");
            assert!(half.join_cue.is_some(), "{id:?} has no join cue");
        }
    }

    /// A viewer names the effects from their icons (M2 spec 2.7.1).
    #[test]
    fn no_two_effects_share_a_glyph() {
        let glyphs: Vec<&str> = EffectId::ALL
            .into_iter()
            .filter_map(|id| shell_half(id).icon)
            .map(|icon| icon.glyph)
            .collect();
        for (index, glyph) in glyphs.iter().enumerate() {
            assert!(!glyphs[index + 1..].contains(glyph), "{glyph} twice");
        }
    }

    /// As the base cues (spec 1.6.4): finite, audible, within the synth's
    /// peak, ending in silence.
    #[test]
    fn every_join_cue_sounds_finite_audible_within_the_peak_and_ends_in_silence() {
        for id in EffectId::ALL {
            let Some(join_cue) = shell_half(id).join_cue else {
                continue;
            };
            let sound = join_cue();
            let samples = sound.samples();
            assert!(samples.iter().all(|s| s.is_finite()), "{id:?}");
            assert!(samples.iter().all(|s| s.abs() <= PEAK), "{id:?}");
            let peak = samples.iter().fold(0.0_f32, |peak, s| peak.max(s.abs()));
            assert!(peak > 0.1, "{id:?} is silent: peak {peak}");
            assert!(
                samples.last().is_some_and(|s| s.abs() < 0.01),
                "{id:?} ends in a click"
            );
        }
    }
}

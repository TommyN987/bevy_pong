//! The cue map (spec 1.6.4): each core event plays its cue once, at the
//! settings' volume; an effect joining plays the effect's own. It knows cue names and their sounds, not how a sound is
//! made, so a different bank changes nothing here.

use bevy::audio::{AudioPlayer, PlaybackSettings, Volume};
use bevy::ecs::message::MessageReader;
use bevy::ecs::system::{Commands, Res, SystemParam};
use pong_core::domain::{BallHitPaddle, BallHitWall, EffectStarted, MatchEnded, PointScored};
use pong_core::features::persistence::model::{Settings, Volume as SavedVolume};

use super::{Cue, CueSounds};

/// The core events that have a sound (ADR 0010).
#[derive(SystemParam)]
pub(super) struct CoreEvents<'w, 's> {
    paddle_hits: MessageReader<'w, 's, BallHitPaddle>,
    wall_hits: MessageReader<'w, 's, BallHitWall>,
    points: MessageReader<'w, 's, PointScored>,
    wins: MessageReader<'w, 's, MatchEnded>,
    joins: MessageReader<'w, 's, EffectStarted>,
}

/// Spawns one player per event read. Each despawns once played; the volume
/// is the settings' when it starts, so a change applies from the next cue.
pub(super) fn play_cues(
    mut commands: Commands,
    sounds: Res<CueSounds>,
    settings: Res<Settings>,
    mut heard: CoreEvents,
) {
    let playback = PlaybackSettings::DESPAWN.with_volume(volume(settings.volume));
    let cues = heard
        .paddle_hits
        .read()
        .map(|_| Cue::PaddleHit)
        .chain(heard.wall_hits.read().map(|_| Cue::WallHit))
        .chain(heard.points.read().map(|_| Cue::Point))
        .chain(heard.wins.read().map(|_| Cue::MatchWon));
    for cue in cues {
        commands.spawn((AudioPlayer(sounds.get(cue).clone()), playback));
    }
    // An effect with no cue of its own joins in silence.
    for sound in heard
        .joins
        .read()
        .filter_map(|join| sounds.join(join.effect))
    {
        commands.spawn((AudioPlayer(sound.clone()), playback));
    }
}

/// The saved percent as a gain: 100 is the sound as made, 0 silence.
fn volume(saved: SavedVolume) -> Volume {
    Volume::Linear(f32::from(saved.percent()) / 100.0)
}

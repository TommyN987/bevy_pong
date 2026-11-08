//! The cue map (spec 1.6.4): each core event plays its cue once, at the
//! settings' volume. It knows cue names and their sounds, not how a sound is
//! made, so a different bank changes nothing here.

use bevy::audio::{AudioPlayer, PlaybackSettings, Volume};
use bevy::ecs::message::MessageReader;
use bevy::ecs::system::{Commands, Res};
use pong_core::domain::{BallHitPaddle, BallHitWall, MatchEnded, PointScored};
use pong_core::features::persistence::model::{Settings, Volume as SavedVolume};

use super::{Cue, CueSounds};

/// Spawns one player per event read. Each despawns once played; the volume
/// is the settings' when it starts, so a change applies from the next cue.
pub(super) fn play_cues(
    mut commands: Commands,
    sounds: Res<CueSounds>,
    settings: Res<Settings>,
    mut paddle_hits: MessageReader<BallHitPaddle>,
    mut wall_hits: MessageReader<BallHitWall>,
    mut points: MessageReader<PointScored>,
    mut wins: MessageReader<MatchEnded>,
) {
    let playback = PlaybackSettings::DESPAWN.with_volume(volume(settings.volume));
    let cues = paddle_hits
        .read()
        .map(|_| Cue::PaddleHit)
        .chain(wall_hits.read().map(|_| Cue::WallHit))
        .chain(points.read().map(|_| Cue::Point))
        .chain(wins.read().map(|_| Cue::MatchWon));
    for cue in cues {
        commands.spawn((AudioPlayer(sounds.get(cue).clone()), playback));
    }
}

/// The saved percent as a gain: 100 is the sound as made, 0 silence.
fn volume(saved: SavedVolume) -> Volume {
    Volume::Linear(f32::from(saved.percent()) / 100.0)
}

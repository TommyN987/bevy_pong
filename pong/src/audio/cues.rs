//! The cue map (spec 1.6.4): each core event plays its cue once, at the
//! settings' volume; an effect joining plays the effect's own. It knows cue
//! names and their sounds, not how a sound is made, so a different bank
//! changes nothing here.
//!
//! One cue is limited: the wall's (M2 spec 2.6.2). The core reports every
//! bounce, since each one happened; only how often it sounds is decided
//! here.

use std::time::Duration;

use bevy::audio::{AudioPlayer, PlaybackSettings, Volume};
use bevy::ecs::message::MessageReader;
use bevy::ecs::system::{Commands, Local, Res, SystemParam};
use bevy::time::Time;
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

/// The least time between two wall cues. Wind can pin a slow ball against a
/// wall, where it bounces every other step: unlimited, thirty cues a second.
/// Under any bounce play makes: wall to wall takes the ball over a quarter
/// of a second at its fastest.
const WALL_CUE_GAP: Duration = Duration::from_millis(150);

/// Whether a wall hit at `now` sounds, the last wall cue having played at
/// `last`.
fn wall_cue_is_due(now: Duration, last: Option<Duration>) -> bool {
    last.is_none_or(|last| now.saturating_sub(last) >= WALL_CUE_GAP)
}

/// Spawns one player per event read, and one at most for a frame's wall
/// hits, none if the last wall cue is under [`WALL_CUE_GAP`] old. Each
/// despawns once played; the volume is the settings' when it starts, so a
/// change applies from the next cue.
pub(super) fn play_cues(
    mut commands: Commands,
    sounds: Res<CueSounds>,
    settings: Res<Settings>,
    time: Res<Time>,
    mut last_wall_cue: Local<Option<Duration>>,
    mut heard: CoreEvents,
) {
    let playback = PlaybackSettings::DESPAWN.with_volume(volume(settings.volume));
    let wall_hit = heard.wall_hits.read().count() > 0;
    let wall_cue = wall_hit && wall_cue_is_due(time.elapsed(), *last_wall_cue);
    if wall_cue {
        *last_wall_cue = Some(time.elapsed());
    }
    let cues = heard
        .paddle_hits
        .read()
        .map(|_| Cue::PaddleHit)
        .chain(wall_cue.then_some(Cue::WallHit))
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

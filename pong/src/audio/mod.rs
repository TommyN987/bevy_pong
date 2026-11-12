//! Sound (spec 1.6.4): a cue for each core event, from a sound bank built
//! when the app is, and each effect's own cue when it joins (M2 spec 2.3.3).

mod cues;
pub mod sound;
pub mod synth;

use bevy::app::{App, Plugin, Update};
use bevy::asset::{AssetApp, Assets, Handle};
use bevy::audio::{AddAudioSource, AudioPlugin};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::schedule::common_conditions::resource_exists;
use pong_core::domain::EffectId;
use pong_core::features::persistence::model::Settings;

use self::sound::Sound;
use crate::effects::shell_half;

/// What a sound tells the players. The cue map knows only these names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    /// The ball hit a paddle.
    PaddleHit,
    /// The ball bounced off a wall.
    WallHit,
    /// A side took the point.
    Point,
    /// A side won the match.
    MatchWon,
}

impl Cue {
    /// Every cue, once.
    pub const ALL: [Self; 4] = [Self::PaddleHit, Self::WallHit, Self::Point, Self::MatchWon];
}

/// Makes the sound for each cue: synthesized now; read from embedded files,
/// if ever, without touching the cue map (spec 1.6.4 SHOULD).
pub trait SoundBank {
    /// The sound `cue` plays.
    fn sound(&self, cue: Cue) -> Sound;
}

/// The sound each cue plays, built once from a [`SoundBank`].
#[derive(Resource, Debug)]
pub struct CueSounds {
    paddle_hit: Handle<Sound>,
    wall_hit: Handle<Sound>,
    point: Handle<Sound>,
    match_won: Handle<Sound>,
    /// Each effect's join cue, for the effects that have one.
    joins: Vec<(EffectId, Handle<Sound>)>,
}

impl CueSounds {
    /// The sound played when `effect` joins the effects in play, if it has
    /// one.
    #[must_use]
    pub fn join(&self, effect: EffectId) -> Option<&Handle<Sound>> {
        self.joins
            .iter()
            .find(|(id, _)| *id == effect)
            .map(|(_, sound)| sound)
    }

    /// The sound `cue` plays.
    #[must_use]
    pub const fn get(&self, cue: Cue) -> &Handle<Sound> {
        match cue {
            Cue::PaddleHit => &self.paddle_hit,
            Cue::WallHit => &self.wall_hit,
            Cue::Point => &self.point,
            Cue::MatchWon => &self.match_won,
        }
    }
}

/// Plays a cue for each core event, from `B`'s sounds, made when the app is
/// built.
///
/// After Bevy's `AudioPlugin`, when there is one: it plays the sounds. Without
/// it, as in headless tests, the cues are still spawned, and nothing plays.
///
/// Precondition: the asset plugin; the core plugin group, for its messages and
/// `Settings`.
pub struct SoundsPlugin<B>(pub B);

impl<B: SoundBank + Send + Sync + 'static> Plugin for SoundsPlugin<B> {
    fn build(&self, app: &mut App) {
        // `add_audio_source` registers the asset too; registering it twice
        // would drop the sounds added between.
        if app.is_plugin_added::<AudioPlugin>() {
            app.add_audio_source::<Sound>();
        } else {
            app.init_asset::<Sound>();
        }
        let mut assets = app.world_mut().resource_mut::<Assets<Sound>>();
        // An effect's join cue is its own shell half's, not the bank's: the
        // bank knows the base cues by name, and no effect by name.
        let joins = EffectId::ALL
            .into_iter()
            .filter_map(|id| Some((id, assets.add(shell_half(id).join_cue?))))
            .collect();
        let mut add = |cue| assets.add(self.0.sound(cue));
        let sounds = CueSounds {
            paddle_hit: add(Cue::PaddleHit),
            wall_hit: add(Cue::WallHit),
            point: add(Cue::Point),
            match_won: add(Cue::MatchWon),
            joins,
        };
        // `Settings` exists from `Boot` on; the core sends no event before.
        app.insert_resource(sounds)
            .add_systems(Update, cues::play_cues.run_if(resource_exists::<Settings>));
    }
}

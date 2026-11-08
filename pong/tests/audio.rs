//! The cue map (spec 1.6.4, plan 9.2): one play per core event, at the
//! settings' volume. Headless, so with audio disabled: no `AudioPlugin`, the
//! cues spawn and nothing plays; how they sound is judged by ear.

use std::num::NonZeroU8;

use bevy::app::App;
use bevy::asset::Handle;
use bevy::audio::{AudioPlayer, PlaybackMode, PlaybackSettings, Volume};
use bevy::ecs::message::{Message, Messages};
use pong::audio::sound::Sound;
use pong::audio::synth::Synth;
use pong::audio::{Cue, CueSounds, SoundsPlugin};
use pong_core::domain::{
    BallHitPaddle, BallHitWall, Controller, MatchConfig, MatchEnded, PointScored, Seat, SeatPlayers,
};
use pong_core::features::persistence::model::{Settings, Volume as SavedVolume};
use pong_core::kernel::geometry::Side;
use pong_core::testing::{boot, enter_match, frame, headless_app};

/// A match's countdown, with the synthesized cues: the ball is still, so the
/// core sends no event of its own; a match, so the core takes `MatchEnded`.
fn audio_app() -> App {
    let mut app = headless_app(1);
    app.add_plugins(SoundsPlugin(Synth));
    boot(&mut app);
    enter_match(
        &mut app,
        MatchConfig {
            left: Controller::Human(Seat::Left),
            right: Controller::Human(Seat::Right),
            players: SeatPlayers::GUESTS,
            points_to_win: NonZeroU8::new(10).expect("not zero"),
        },
    );
    app
}

fn send<M: Message>(app: &mut App, message: M) {
    app.world_mut().resource_mut::<Messages<M>>().write(message);
}

/// Each cue spawned so far: its sound and how it plays.
fn cues(app: &mut App) -> Vec<(Handle<Sound>, PlaybackSettings)> {
    let mut query = app
        .world_mut()
        .query::<(&AudioPlayer<Sound>, &PlaybackSettings)>();
    query
        .iter(app.world())
        .map(|(player, settings)| (player.0.clone(), *settings))
        .collect()
}

fn sound_of(app: &App, cue: Cue) -> Handle<Sound> {
    app.world().resource::<CueSounds>().get(cue).clone()
}

fn handles(app: &mut App) -> Vec<Handle<Sound>> {
    cues(app).into_iter().map(|(handle, _)| handle).collect()
}

/// Sends one core event.
type Event = fn(&mut App);

/// Spec 1.6.4: one play command per event.
#[test]
fn each_core_event_plays_its_cue_once() {
    let events: [(Cue, Event); 4] = [
        (Cue::PaddleHit, |app| {
            send(app, BallHitPaddle { side: Side::Left });
        }),
        (Cue::WallHit, |app| send(app, BallHitWall)),
        (Cue::Point, |app| {
            send(
                app,
                PointScored {
                    scorer: Side::Right,
                },
            );
        }),
        (Cue::MatchWon, |app| {
            send(
                app,
                MatchEnded {
                    winner: Side::Left,
                    longest_rally: 3,
                },
            );
        }),
    ];
    for (cue, event) in events {
        let mut app = audio_app();
        assert!(cues(&mut app).is_empty(), "{cue:?}: a cue with no event");
        event(&mut app);
        frame(&mut app);
        assert_eq!(handles(&mut app), [sound_of(&app, cue)], "{cue:?}");
    }
}

#[test]
fn events_arriving_together_each_play_and_none_plays_again() {
    let mut app = audio_app();
    send(&mut app, BallHitPaddle { side: Side::Left });
    send(&mut app, BallHitPaddle { side: Side::Right });
    send(&mut app, BallHitWall);
    frame(&mut app);
    frame(&mut app);
    let (paddle, wall) = (sound_of(&app, Cue::PaddleHit), sound_of(&app, Cue::WallHit));
    let mut played = handles(&mut app);
    played.sort();
    let mut expected = vec![paddle.clone(), paddle, wall];
    expected.sort();
    assert_eq!(played, expected);
}

/// Volume from settings (spec 1.6.4), read when the cue plays; a cue
/// despawns once played, since nothing else ends it.
#[test]
fn a_cue_plays_at_the_volume_set_then_and_goes_when_done() {
    let mut app = audio_app();
    send(&mut app, BallHitWall);
    frame(&mut app);
    app.world_mut().resource_mut::<Settings>().volume = SavedVolume::new(30).expect("valid");
    send(&mut app, BallHitWall);
    frame(&mut app);
    let mut volumes: Vec<Volume> = cues(&mut app).iter().map(|(_, s)| s.volume).collect();
    volumes.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    assert_eq!(volumes, [Volume::Linear(0.3), Volume::Linear(0.8)]);
    assert!(
        cues(&mut app)
            .iter()
            .all(|(_, s)| matches!(s.mode, PlaybackMode::Despawn)),
        "a cue outlives its sound"
    );
}

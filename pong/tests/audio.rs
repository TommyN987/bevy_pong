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
    BallHitPaddle, BallHitWall, Controller, EffectId, EffectStarted, MatchConfig, MatchEnded, Mode,
    PointScored, Seat, SeatPlayers,
};
use pong_core::features::effects::ActiveEffects;
use pong_core::features::persistence::model::{Settings, Volume as SavedVolume};
use pong_core::features::tuning::Tuning;
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
            mode: Mode::Classic,
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

/// M2 spec 2.3.3: an effect's cue. It plays when the effect joins, once.
#[test]
fn an_effect_joining_plays_its_own_cue_once() {
    let mut app = audio_app();
    send(
        &mut app,
        EffectStarted {
            effect: EffectId::Shrink,
        },
    );
    frame(&mut app);
    frame(&mut app);
    let shrink = app
        .world()
        .resource::<CueSounds>()
        .join(EffectId::Shrink)
        .expect("Shrink has a join cue")
        .clone();
    assert_eq!(handles(&mut app), [shrink]);
}

/// An effect with no cue of its own joins in silence.
#[test]
fn an_effect_without_a_cue_joins_in_silence() {
    let mut app = audio_app();
    send(
        &mut app,
        EffectStarted {
            effect: EffectId::Speed,
        },
    );
    frame(&mut app);
    assert!(cues(&mut app).is_empty());
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
    // Paddle hits: two wall hits a frame apart would be one cue.
    send(&mut app, BallHitPaddle { side: Side::Left });
    frame(&mut app);
    app.world_mut().resource_mut::<Settings>().volume = SavedVolume::new(30).expect("valid");
    send(&mut app, BallHitPaddle { side: Side::Left });
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

/// The real path, not a message sent by hand: entering a Chaos match starts
/// its first bag effect, and that plays the effect's cue.
#[test]
fn entering_a_chaos_match_plays_the_joining_effects_cue() {
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
            mode: Mode::Chaos {
                cap: NonZeroU8::MIN,
            },
        },
    );
    frame(&mut app);
    // Which bag effect the match starts with is the seeded draw's to say.
    let tuning = app.world().resource::<Tuning>();
    let bag = tuning
        .mode(Mode::Chaos {
            cap: NonZeroU8::MIN,
        })
        .bag();
    let joined = *app
        .world()
        .resource::<ActiveEffects>()
        .ids()
        .iter()
        .find(|id| bag.contains(id))
        .expect("Chaos starts one bag effect");
    let its_cue = app
        .world()
        .resource::<CueSounds>()
        .join(joined)
        .expect("every bag effect has a join cue")
        .clone();
    assert_eq!(handles(&mut app), [its_cue]);
}

/// How many wall cues have been spawned so far.
fn wall_cues(app: &mut App) -> usize {
    let wall = sound_of(app, Cue::WallHit);
    handles(app)
        .into_iter()
        .filter(|handle| *handle == wall)
        .count()
}

/// M2 spec 2.6.2's sub-item: wind can pin a slow ball against a wall, where
/// it bounces every other step, thirty times a second. The cue is limited,
/// so a second of that is a handful of cues and not a buzz.
#[test]
fn a_ball_pinned_to_a_wall_plays_a_bounded_number_of_wall_cues_a_second() {
    let mut app = audio_app();
    for step in 0..60 {
        if step % 2 == 0 {
            send(&mut app, BallHitWall);
        }
        frame(&mut app);
    }
    let played = wall_cues(&mut app);
    assert!(
        (2..=8).contains(&played),
        "thirty wall hits in a second played {played} cues"
    );
}

/// Several wall hits in one frame are one sound.
#[test]
fn wall_hits_in_the_same_frame_play_one_cue() {
    let mut app = audio_app();
    for _ in 0..3 {
        send(&mut app, BallHitWall);
    }
    frame(&mut app);
    assert_eq!(wall_cues(&mut app), 1);
}

/// The limit is under any bounce play makes: wall to wall takes the ball
/// over a quarter of a second at its fastest, and each such bounce sounds.
#[test]
fn wall_hits_a_quarter_of_a_second_apart_each_play() {
    let mut app = audio_app();
    for _ in 0..4 {
        send(&mut app, BallHitWall);
        for _ in 0..15 {
            frame(&mut app);
        }
    }
    assert_eq!(wall_cues(&mut app), 4);
}

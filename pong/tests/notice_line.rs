//! The notice line (spec 1.7.3, plan 8.1): every `Notice` since the last
//! match start, shown on the menu, cleared when a match starts.

use std::num::NonZeroU8;

use bevy::app::App;
use bevy::ecs::message::Messages;
use bevy::ecs::query::With;
use bevy::ui::widget::Text;
use pong::ui::notice_line::{NoticeLine, NoticeLinePlugin, NoticeText};
use pong_core::domain::{Controller, MatchConfig, Notice, Seat, SeatPlayers};
use pong_core::features::persistence::io::SavePaths;
use pong_core::testing::{boot, frame, headless_app, leave_match, start_match};

fn notice_app(adjust: impl FnOnce(&mut App)) -> App {
    let mut app = headless_app(1);
    app.add_plugins(NoticeLinePlugin);
    adjust(&mut app);
    boot(&mut app);
    frame(&mut app);
    app
}

fn tell(app: &mut App, text: &str) {
    app.world_mut()
        .resource_mut::<Messages<Notice>>()
        .write(Notice {
            text: text.to_owned(),
        });
}

fn line(app: &App) -> String {
    app.world().resource::<NoticeLine>().text()
}

/// The text the menu shows.
fn shown(app: &mut App) -> Vec<String> {
    let mut texts = app.world_mut().query_filtered::<&Text, With<NoticeText>>();
    texts.iter(app.world()).map(|text| text.0.clone()).collect()
}

fn two_guests() -> MatchConfig {
    MatchConfig {
        left: Controller::Human(Seat::Left),
        right: Controller::Human(Seat::Right),
        players: SeatPlayers::GUESTS,
        points_to_win: NonZeroU8::MIN,
    }
}

#[test]
fn notices_pile_up_on_the_menu_and_clear_when_a_match_starts() {
    let mut app = notice_app(|_| {});
    assert_eq!(shown(&mut app), [""]);
    tell(&mut app, "One.");
    tell(&mut app, "Two.");
    frame(&mut app);
    assert_eq!(line(&app), "One. Two.");
    assert_eq!(shown(&mut app), ["One. Two."]);

    start_match(&mut app, two_guests());
    assert_eq!(line(&app), "");
    assert_eq!(
        shown(&mut app),
        Vec::<String>::new(),
        "shown during a match"
    );
    leave_match(&mut app);
    frame(&mut app);
    assert_eq!(shown(&mut app), [""]);
}

/// Spec 1.7.3: visible after a bad save at boot.
#[test]
fn a_notice_sent_while_booting_is_on_the_line() {
    let mut app = notice_app(|app| {
        app.world_mut().remove_resource::<SavePaths>();
    });
    assert_eq!(
        shown(&mut app),
        [
            "There is nowhere to save on this computer, so nothing will be kept after the game closes."
        ]
    );
}

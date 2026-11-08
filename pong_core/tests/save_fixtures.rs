//! Save fixtures (spec 1.4.9): one file per released version of each save
//! kind, kept forever, in `tests/fixtures/<kind>_v<N>.ron`. Every fixture
//! must still decode to the model it was written from; the count tests make
//! a version bump without its fixture fail.

use std::path::PathBuf;

use bevy::color::Srgba;
use pong_core::features::persistence::document::{
    PROFILES_VERSION, SETTINGS_VERSION, decode_profiles, decode_settings,
};
use pong_core::features::persistence::model::{ProfileId, Records, WindowMode};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixture(name: &str) -> String {
    let path = fixtures().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

/// The versions `kind` has fixtures for, sorted.
fn fixture_versions(kind: &str) -> Vec<u32> {
    let prefix = format!("{kind}_v");
    let mut versions: Vec<u32> = std::fs::read_dir(fixtures())
        .expect("the fixtures directory exists")
        .map(|entry| entry.expect("a readable entry").file_name())
        .filter_map(|name| {
            name.to_str()?
                .strip_prefix(&prefix)?
                .strip_suffix(".ron")?
                .parse()
                .ok()
        })
        .collect();
    versions.sort_unstable();
    versions
}

#[test]
fn every_released_profiles_version_has_exactly_one_fixture() {
    let expected: Vec<u32> = (1..=PROFILES_VERSION).collect();
    assert_eq!(fixture_versions("profiles"), expected);
}

#[test]
fn every_released_settings_version_has_exactly_one_fixture() {
    let expected: Vec<u32> = (1..=SETTINGS_VERSION).collect();
    assert_eq!(fixture_versions("settings"), expected);
}

#[test]
fn the_version_1_profiles_fixture_still_decodes_to_its_profiles() {
    let profiles = decode_profiles(&fixture("profiles_v1.ron")).expect("fixture decodes");
    let [zoe, max] = profiles.as_slice() else {
        panic!("expected two profiles, got {profiles:?}");
    };
    assert_eq!(zoe.id(), ProfileId(1));
    assert_eq!(zoe.name().as_str(), "Zoë");
    assert_eq!(zoe.color(), Srgba::rgb(1.0, 0.3, 0.25));
    assert_eq!(
        zoe.records(),
        Records {
            wins: 4,
            losses: 2,
            longest_rally: 17
        }
    );
    assert_eq!(max.id(), ProfileId(3));
    assert_eq!(max.name().as_str(), "Max");
    assert_eq!(max.color(), Srgba::rgb(0.2, 0.8, 0.9));
    assert_eq!(max.records(), Records::default());
}

#[test]
fn the_version_1_settings_fixture_still_decodes_to_its_settings() {
    let settings = decode_settings(&fixture("settings_v1.ron")).expect("fixture decodes");
    assert_eq!(settings.volume.percent(), 60);
    assert_eq!(settings.window_mode, WindowMode::Windowed);
}

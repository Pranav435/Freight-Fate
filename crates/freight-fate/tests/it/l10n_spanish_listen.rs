//! Real spoken lines run through the shipped Spanish catalog: lines a
//! Spanish listen heard half in English must come out whole.

use crate::states_city_support::*;
use ff_core::l10n::{Catalog, Translator};
use freight_fate::app::testing::TestApp;
use freight_fate::playtest::harness::{PlaytestHarness, StartDelivery};
use freight_fate::states::city::CityMenuState;

fn spanish() -> Translator {
    let po = include_str!("../../../../lang/es.po");
    Translator::new(Catalog::from_po("es", po).expect("es.po parses"))
}

/// Every English sentence of `line` that is still in its Spanish; short
/// ones (a key name, "Tab status menu") aside.
fn english_left(es: &Translator, line: &str) -> Vec<String> {
    let spanish = es.translate(line);
    line.split(". ")
        .map(|s| s.trim_end_matches('.'))
        .filter(|s| s.split_whitespace().count() >= 5 && spanish.contains(*s))
        .map(str::to_string)
        .collect()
}

fn arrival(city: &str, first_day_done: bool) -> Vec<String> {
    let mut app = TestApp::new();
    career(&mut app, "Playtest", city);
    if first_day_done {
        profile_mut(&mut app)
            .achievements
            .push("first_day".to_string());
    }
    let city = CityMenuState::new(&app.ctx, true);
    app.push_state(city);
    app.main_lines()
}

#[test]
fn every_yard_arrival_is_whole_spanish() {
    // Chicago's yard name is also a catalog entry, which once turned
    // "company yard: Chicago Company Yard" into the label said twice.
    let es = spanish();
    for city in [
        "Aberdeen", "Chicago", "Denver", "Atlanta", "Dallas", "Seattle",
    ] {
        for first_day_done in [false, true] {
            let lines = arrival(city, first_day_done);
            let said = lines.join(" ");
            assert!(said.contains("objective:"), "{said}");
            for line in &lines {
                assert_eq!(english_left(&es, line), Vec::<String>::new(), "{line}");
                let spanish = es.translate(line);
                assert!(
                    !spanish.contains("patio de la empresa patio de la empresa")
                        && !spanish.contains("Patio de la empresa patio"),
                    "{spanish}"
                );
            }
        }
    }
}

/// A facility named with its kind and town ("retail distribution hub
/// Amazon Fulfillment Center KRB1 in Kenosha") stays English for now.
fn names_a_facility(sentence: &str) -> bool {
    sentence.contains(" in ")
        && sentence.split(" in ").count() > 1
        && ["hub ", "terminal ", "center ", "Center "]
            .iter()
            .any(|kind| sentence.contains(kind))
}

#[test]
fn the_chicago_start_and_kenosha_drive_are_whole_spanish() {
    let es = spanish();
    for first_day_done in [false, true] {
        let mut harness = PlaytestHarness::new();
        let setup = StartDelivery::named("Chicago Listen");
        let setup = if first_day_done {
            setup.configure(|p| p.achievements.push("first_day".to_string()))
        } else {
            setup
        };
        harness.start_delivery(setup);
        let lines = harness.app.main_lines();
        let said = lines.join(" ");
        assert!(said.contains("Chicago Company Yard"), "{said}");
        assert!(said.contains("toward Kenosha"), "{said}");
        let mut leaks = Vec::new();
        // The dispatch board's market and offer rows are a known follow-up.
        for line in lines
            .iter()
            .filter(|l| !l.starts_with("Dispatch board.") && !l.starts_with("Accept assigned"))
        {
            let left: Vec<String> = english_left(&es, line)
                .into_iter()
                .filter(|s| !names_a_facility(s))
                .collect();
            leaks.extend(left);
        }
        assert_eq!(leaks, Vec::<String>::new());
    }
}

#[test]
fn the_keyboard_help_is_whole_spanish() {
    let es = spanish();
    let mut harness = PlaytestHarness::new();
    harness.start_delivery(StartDelivery::named("Help Listen"));
    harness.clear_speech();
    harness.with_drive(|drive, ctx| drive.speak_keyboard_help(ctx));
    let lines = harness.app.main_lines();
    assert!(lines.join(" ").contains("tuning keys"), "{lines:?}");
    for line in &lines {
        assert_eq!(english_left(&es, line), Vec::<String>::new());
    }
}

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

fn arrival(first_day_done: bool) -> Vec<String> {
    let mut app = TestApp::new();
    career(&mut app, "Playtest", "Aberdeen");
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
fn the_aberdeen_arrival_is_whole_spanish() {
    let es = spanish();
    for first_day_done in [false, true] {
        let lines = arrival(first_day_done);
        let said = lines.join(" ");
        assert!(said.contains("objective:"), "{said}");
        for line in &lines {
            assert_eq!(english_left(&es, line), Vec::<String>::new(), "{line}");
        }
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

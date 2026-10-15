//! Chains in the existing assists: stop planning's no-chains reminder and
//! facility stopping assistance's roadside chain work. Neither ever buys
//! anything.

use ff_core::sim::weather::WeatherKind;

use freight_fate::app::testing::TestApp;
use freight_fate::app::SharedState;

use crate::states_driving_menus_support::*;

/// Career hours at noon on a day of the career year (day 80 is hour 0).
fn on_day(doy: f64) -> Option<f64> {
    Some((doy - 80.0) * 24.0 + 12.0)
}

/// A Denver-Silverthorne run on I-70 under a clear sky, at mile `at`, with a
/// chain-law area over `areas`, on a day of the career year.
fn i70_drive(
    app: &mut TestApp,
    doy: f64,
    chains: bool,
    at: f64,
    areas: Vec<(f64, f64)>,
) -> SharedState {
    let drive = a_drive_between(app, "Denver", "Silverthorne", "Carrier");
    {
        let p = app.ctx.profile.as_mut().expect("a career");
        p.set_money(1000.0);
        p.set_chains_owned(chains);
        p.set_chain_wear_pct(0.0);
        p.set_chain_repair_links(2);
    }
    with_drive(&drive, |d| {
        d.trip.weather.current = WeatherKind::Clear;
        d.trip.career_hours = on_day(doy);
        d.trip.game_minutes = 0.0;
        d.trip.chain_law_areas = areas;
        d.trip.position_mi = at;
        d.trip.truck.chain_wear_pct = 0.0;
        d.trip.truck.chains_on = false;
        d.trip.truck.velocity_mps = 20.0;
        d.trip.zones = Vec::new();
    });
    app.clear_speech();
    drive
}

fn money(app: &TestApp) -> f64 {
    app.ctx.profile.as_ref().expect("a career").money()
}

fn owned(app: &TestApp) -> bool {
    app.ctx.profile.as_ref().expect("a career").chains_owned()
}

fn remind(app: &mut TestApp, drive: &SharedState) {
    drive_and_ctx(drive, app, |d, ctx| d.update_chain_reminder(ctx));
    app.ctx.run_deferred();
}

fn reminders(app: &TestApp) -> Vec<String> {
    app.event_lines()
        .into_iter()
        .filter(|s| s.starts_with("Chain law ahead"))
        .collect()
}

#[test]
fn test_departure_names_a_chain_law_in_season_and_the_garage_here() {
    let mut app = TestApp::new();
    app.ctx.settings.hos_planning_hints = true;
    let drive = i70_drive(&mut app, 288.0, false, 0.0, vec![(20.0, 26.0)]);
    let line = drive_and_ctx(&drive, &mut app, |d, ctx| d.departure_chain_reminder(ctx));
    assert_eq!(
        line.as_deref(),
        Some(
            "I-70 has a chain law in season on this route. You have no chains. The garage here \
             sells them."
        )
    );
    assert!((money(&app) - 1000.0).abs() < 0.01);
    assert!(!owned(&app));
    // Off, or with chains aboard, or in July: nothing.
    app.ctx.settings.hos_planning_hints = false;
    assert!(drive_and_ctx(&drive, &mut app, |d, ctx| d.departure_chain_reminder(ctx)).is_none());
    app.ctx.settings.hos_planning_hints = true;
    with_drive(&drive, |d| d.trip.career_hours = on_day(196.0));
    assert!(drive_and_ctx(&drive, &mut app, |d, ctx| d.departure_chain_reminder(ctx)).is_none());
}

#[test]
fn test_the_road_reminder_plays_once_per_corridor_and_never_spends() {
    let mut app = TestApp::new();
    app.ctx.settings.hos_planning_hints = true;
    app.ctx.settings.imperial_units = true;
    // Areas at mile 20 (exit 254 at 18.7 before it) and 40 (exit 234 at 38).
    let drive = i70_drive(
        &mut app,
        288.0,
        false,
        5.0,
        vec![(20.0, 26.0), (40.0, 46.0)],
    );
    let clock = app.fake_pacer_clock();
    remind(&mut app, &drive);
    let said = reminders(&app);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].starts_with("Chain law ahead on I-70 at exit 254."),
        "{said:?}"
    );
    assert!(said[0].contains("You have no chains"), "{said:?}");
    assert!(said[0].contains("in Denver, 5 miles back."), "{said:?}");
    // Ticking on through the approach doesn't repeat it.
    for at in [6.0, 10.0, 15.0, 19.0] {
        with_drive(&drive, |d| d.trip.position_mi = at);
        remind(&mut app, &drive);
    }
    assert_eq!(reminders(&app).len(), 1, "{:?}", reminders(&app));
    // The next separate corridor plays again.
    clock.advance(120.0); // the first line has long finished
    app.clear_speech();
    with_drive(&drive, |d| d.trip.position_mi = 27.0);
    remind(&mut app, &drive);
    let said = reminders(&app);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].starts_with("Chain law ahead on I-70 at exit 234."),
        "{said:?}"
    );
    assert!((money(&app) - 1000.0).abs() < 0.01);
    assert!(!owned(&app));
    assert_eq!(with_drive(&drive, |d| d.trip.position_mi), 27.0);
    assert_eq!(with_drive(&drive, |d| d.trip.truck.velocity_mps), 20.0);

    drop(drive);
    drop(app);

    // No mapped exit before the area: no exit clause, and metric distances.
    let mut app = TestApp::new();
    app.ctx.settings.hos_planning_hints = true;
    app.ctx.settings.imperial_units = false;
    let drive = i70_drive(&mut app, 288.0, false, 2.0, vec![(10.0, 16.0)]);
    remind(&mut app, &drive);
    let said = reminders(&app);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(!said[0].contains("exit"), "{said:?}");
    assert!(
        said[0].starts_with("Chain law ahead on I-70. You have no chains."),
        "{said:?}"
    );
    assert!(said[0].contains("kilometers back."), "{said:?}");
}

#[test]
fn test_the_reminder_is_silent_when_it_has_nothing_to_say() {
    let quiet = |hints: bool, chains: bool, doy: f64, buy: bool| {
        let mut app = TestApp::new();
        app.ctx.settings.hos_planning_hints = hints;
        let drive = i70_drive(&mut app, doy, chains, 5.0, vec![(20.0, 26.0)]);
        if buy {
            app.ctx.profile.as_mut().unwrap().set_chains_owned(true);
        }
        remind(&mut app, &drive);
        reminders(&app)
    };
    assert!(
        quiet(false, false, 288.0, false).is_empty(),
        "stop planning off"
    );
    assert!(quiet(true, true, 288.0, false).is_empty(), "chains aboard");
    assert!(
        quiet(true, false, 288.0, true).is_empty(),
        "chains bought first"
    );
    assert!(quiet(true, false, 196.0, false).is_empty(), "July");
    assert_eq!(
        quiet(true, false, 288.0, false).len(),
        1,
        "the control case"
    );
}

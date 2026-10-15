//! Driving by the picture alone.
//!
//! A sighted driver that never hears a word: every decision it makes is
//! read off the visual display's capture of the drive -- the speedometer,
//! the speed-limit sign, the curve warning, the road bending on screen, the
//! truck's place between the lane lines, the traffic ahead and on the
//! radar, the blind-spot lights -- and every action goes in as the same key
//! presses a player
//! makes, through the real game frame. Every assist that would drive for it
//! is off (the realistic preset, lane keeping off, curve assist off), so
//! steering, speed and passing are all its own.
//!
//! If the picture leaves out something a driver needs, this driver misses
//! it: it runs wide in the bend, rear-ends the slow car, or gets pulled over.
//! That is how the blind-spot lights came to exist: the first version of
//! this driver passed the slow car, saw it a few hundred feet back on the
//! radar, moved over, and was billed a sideswipe, because the game counts a
//! vehicle as holding its lane well behind where it is drawn. The lights
//! read the game's own clearance check.
//!
//! Set `FREIGHT_FATE_VISUALS_SHOTS` to also save a flipbook of the drive.

use ff_core::data::world::get_world;
use ff_core::message_log::MessageLog;
use ff_core::sim::traffic_manager::TrafficVehicle;
use freight_fate::playtest::harness::{PlaytestHarness, StartDelivery};
use freight_fate::playtest::road::{find_feature_seeded, RoadOptions};
use freight_fate::states::base::{InputEvent, Key};
use freight_fate::visuals::scene_drive::{DriveView, RoadAhead};
use freight_fate::visuals::{Scene, Screen, Visuals};

const DT: f64 = 1.0 / 60.0;
const FT_PER_MI: f32 = 5280.0;

/// Which pedal is down, which way the wheel is held.
#[derive(Default)]
struct Controls {
    pedal: Option<Key>,
    wheel: Option<Key>,
}

#[derive(Default)]
struct SightedDriver {
    controls: Controls,
    /// The lane it means to be in.
    target_lane: Option<i32>,
    /// The advisory of the bend it is slowing for, once it has read the sign.
    bend_mph: Option<f64>,
    in_bend: bool,
    /// Gap to the vehicle ahead in its lane last frame, feet.
    last_gap: Option<f32>,
    closing_fps: f32,
    passes: u32,
}

/// What the dashboard's curve panel says, as a distance in feet.
fn shown_distance_ft(text: &str) -> Option<f32> {
    let number: String = text
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .filter(|c| *c != ',')
        .collect();
    let value: f32 = number.parse().ok()?;
    Some(if text.contains("mile") {
        value * FT_PER_MI
    } else {
        value
    })
}

/// How hard the road on screen bends right around the truck, 1/ft.
fn bend_under_truck(view: &DriveView) -> f32 {
    view.road
        .curvature
        .iter()
        .enumerate()
        .filter(|(i, _)| (-80.0..250.0).contains(&RoadAhead::sample_s(*i)))
        .map(|(_, k)| k.abs())
        .fold(0.0, f32::max)
}

impl SightedDriver {
    fn lane_clear(view: &DriveView, lane: i32, behind_ft: f32, ahead_ft: f32) -> bool {
        !view
            .traffic
            .iter()
            .any(|v| v.lane == lane && v.ahead_ft > -behind_ft && v.ahead_ft < ahead_ft)
    }

    /// One frame's look at the screen; returns the keys to press and release.
    fn decide(&mut self, view: &DriveView) -> Vec<InputEvent> {
        let hud = &view.hud;
        let lanes = &view.lanes;
        let speed = hud.speed_mph;
        let speed_fps = (speed * 1.4667) as f32;
        let lane = lanes.truck_lane;
        let target_lane = *self.target_lane.get_or_insert(lane);

        // Speed: the posted limit on the sign, a little under.
        let limit = hud.limit_mph.unwrap_or(55.0);
        let mut target = limit - 2.0;

        // Bends: read the warning sign's advisory once braking for it from
        // here would start to take most of the distance shown.
        if let Some((_, advisory, distance, _)) = &hud.curve {
            if let Some(ft) = shown_distance_ft(distance) {
                let (v, a) = (speed_fps, *advisory as f32 * 1.4667);
                let needed = ((v * v - a * a).max(0.0) / (2.0 * 4.5)) + 700.0;
                if ft < needed {
                    self.bend_mph = Some(*advisory as f64);
                }
            }
        }
        let bending = bend_under_truck(view) > 1.0 / 5000.0;
        if bending && self.bend_mph.is_some() {
            self.in_bend = true;
        }
        if self.in_bend && !bending {
            // Through it.
            self.in_bend = false;
            self.bend_mph = None;
        }
        if let Some(mph) = self.bend_mph {
            target = target.min(mph - 3.0);
        }

        // Traffic: the nearest vehicle drawn ahead in the truck's own lane.
        let gap = view
            .traffic
            .iter()
            .filter(|v| v.lane == lane && v.ahead_ft > 0.0)
            .map(|v| v.ahead_ft)
            .fold(f32::INFINITY, f32::min);
        if gap.is_finite() {
            if let Some(last) = self.last_gap {
                let now = (last - gap) / DT as f32;
                self.closing_fps = self.closing_fps * 0.9 + now * 0.1;
            }
            self.last_gap = Some(gap);
        } else {
            self.last_gap = None;
            self.closing_fps = 0.0;
        }
        let closing = gap.is_finite() && self.closing_fps > 3.0 && gap < 900.0;
        if closing && target_lane == lane && !self.in_bend {
            // Pass on the left when the radar shows the left lane open.
            if lane + 1 < lanes.count
                && !lanes.blocked_left
                && Self::lane_clear(view, lane + 1, 200.0, 500.0)
            {
                self.target_lane = Some(lane + 1);
                self.passes += 1;
            }
        }
        if target_lane > 0 && target_lane == lane && !closing {
            // Keep right once the right lane is clear well past the car passed.
            if !lanes.blocked_right && Self::lane_clear(view, target_lane - 1, 250.0, 700.0) {
                self.target_lane = Some(target_lane - 1);
            }
        }
        if target_lane == lane && gap.is_finite() {
            // Keep a few seconds back: ease off while the gap shrinks on
            // screen, and drop below the car's pace inside the gap.
            let want_gap = (speed_fps * 3.5).max(180.0);
            if gap < want_gap * 1.8 && self.closing_fps > 0.0 {
                target = target.min(speed - (self.closing_fps / 1.4667) as f64 - 2.0);
            }
            if gap < want_gap {
                target = target.min(speed - 4.0);
            }
        }

        let mut events = Vec::new();
        let pedal = if speed < target - 1.0 {
            Some(Key::Up)
        } else if speed > target + 1.5 {
            Some(Key::Down)
        } else {
            None
        };
        Self::hold(&mut self.controls.pedal, pedal, &mut events);

        // Steering: keep the truck, as drawn between the lane lines, on the
        // centre of the lane it wants, looking ahead along its heading.
        let target_lane = self.target_lane.unwrap_or(lane);
        let w = lanes.lane_width_ft;
        let u = -(lane as f32 + 0.5) * w + lanes.offset_ft;
        let want = -(target_lane as f32 + 0.5) * w;
        // Where the truck will be in a second and a half on its present
        // heading: steering to that, not to where it is, keeps it from
        // swinging past the lane it is moving into.
        let error = u - want + lanes.yaw.sin() * speed_fps * 1.5;
        let wheel = if error > 0.6 {
            Some(Key::Left)
        } else if error < -0.6 {
            Some(Key::Right)
        } else {
            None
        };
        Self::hold(&mut self.controls.wheel, wheel, &mut events);
        events
    }

    fn hold(slot: &mut Option<Key>, want: Option<Key>, events: &mut Vec<InputEvent>) {
        if *slot == want {
            return;
        }
        if let Some(key) = slot.take() {
            events.push(InputEvent::key_up(key));
        }
        if let Some(key) = want {
            events.push(InputEvent::key(key));
            *slot = Some(key);
        }
    }
}

/// What happened on a drive by sight, as the game (not the picture) saw it.
#[derive(Debug, Default)]
struct Report {
    miles: f64,
    seconds: f64,
    passes: u32,
    lanes_used: std::collections::BTreeSet<i32>,
    ended_in_lane: i64,
    over_limit_s: f64,
    off_road_s: f64,
    widest_ft: f32,
    slowest_in_bends: f64,
    closest_ft: f64,
    passed_slow_car: bool,
    pulled_over: bool,
    damage: f64,
    cargo_damage: f64,
}

/// Put a slow car in the truck's lane a little way ahead.
fn slow_car_ahead(h: &mut PlaytestHarness, ahead_mi: f64, mph: f64) {
    h.with_drive(|d, _| {
        let pos = d.trip.position_mi;
        let mut slow =
            TrafficVehicle::new("slow-car", pos + ahead_mi, mph, mph, 0, "cruising", "car");
        slow.lane = d.lane.lane;
        d.trip.traffic_manager.vehicles.push(slow);
    });
}

/// Drive by sight until `end_mi`, through the real game frame.
fn drive_by_sight(h: &mut PlaytestHarness, end_mi: f64, shots_prefix: &str) -> Report {
    let shots = std::env::var_os("FREIGHT_FATE_VISUALS_SHOTS").map(std::path::PathBuf::from);
    let mut visuals = Visuals::new();
    let log = MessageLog::new();
    let mut driver = SightedDriver::default();
    let mut report = Report {
        slowest_in_bends: f64::INFINITY,
        closest_ft: f64::INFINITY,
        ..Default::default()
    };
    let (start_mi, start_damage, start_cargo) = h.read_drive(|d| {
        (
            d.trip.position_mi,
            d.truck().damage_pct,
            d.truck().cargo_damage_pct,
        )
    });
    let mut frames = 0u32;
    let mut settled_frames = 0u32;
    let mut shot = 0;
    while frames < 60 * 60 * 10 {
        let scene = Scene::capture(&h.app.ctx);
        let Screen::Drive {
            view,
            overlay: None,
        } = &scene.screen
        else {
            panic!("the road view went away mid-drive: {:?}", scene.screen);
        };
        let events = driver.decide(view);
        for event in &events {
            h.app.handle_event(event);
        }
        h.app.frame(DT);
        frames += 1;
        report.lanes_used.insert(view.lanes.truck_lane);
        // Settled: in the lane it means to be in, for two seconds.
        if driver.target_lane == Some(view.lanes.truck_lane) {
            settled_frames += 1;
        } else {
            settled_frames = 0;
        }

        // The referee reads the game, not the picture.
        let (pos, speed, limit, off, offset, in_bend, pulled, slow_gap_ft) = h.read_drive(|d| {
            let pos = d.trip.position_mi;
            let slow = d
                .trip
                .npc_vehicles()
                .iter()
                .find(|v| v.key == "slow-car")
                .map(|v| ((v.position_mi - pos) * 5280.0, v.lane == d.lane.lane));
            (
                pos,
                d.truck().speed_mph(),
                d.enforced_limit_prev.unwrap_or(65.0),
                d.off_pavement(),
                (d.lane.offset * 6.0).abs() as f32,
                d.trip
                    .curve_at(pos)
                    .is_some_and(|c| !c.connector && c.advisory_mph < 55),
                d.pull_over.is_some(),
                slow,
            )
        });
        if speed > limit + 5.0 {
            report.over_limit_s += DT;
        }
        if off {
            report.off_road_s += DT;
        }
        if settled_frames > 120 {
            report.widest_ft = report.widest_ft.max(offset);
        }
        if in_bend {
            report.slowest_in_bends = report.slowest_in_bends.min(speed);
        }
        if let Some((gap, same_lane)) = slow_gap_ft {
            if same_lane && gap > 0.0 {
                report.closest_ft = report.closest_ft.min(gap);
            }
            report.passed_slow_car |= gap < -150.0;
        }
        report.pulled_over |= pulled;
        if let Some(dir) = &shots {
            // A frame every fifteen seconds, and every three while passing
            // or in a bend.
            let busy = view.lanes.truck_lane != 0 || bend_under_truck(view) > 1.0 / 5000.0;
            let every = if busy { 180 } else { 900 };
            if frames.is_multiple_of(every) && shot < 16 {
                std::fs::create_dir_all(dir).expect("shots dir");
                crate::visuals_frames::shoot(
                    &mut visuals,
                    scene.clone(),
                    &log,
                    (1800, 1280),
                    &dir.join(format!("{shots_prefix}_{shot:02}.bmp")),
                );
                shot += 1;
            }
        }
        if pos > end_mi {
            break;
        }
    }
    let (end_mi_now, damage, cargo, lane) = h.read_drive(|d| {
        (
            d.trip.position_mi,
            d.truck().damage_pct,
            d.truck().cargo_damage_pct,
            d.lane.lane,
        )
    });
    report.miles = end_mi_now - start_mi;
    report.seconds = frames as f64 * DT;
    report.passes = driver.passes;
    report.ended_in_lane = lane;
    report.damage = damage - start_damage;
    report.cargo_damage = cargo - start_cargo;
    println!("{shots_prefix}: {report:?}");
    report
}

/// Every assist that would drive for the player, off.
fn by_hand() -> RoadOptions {
    RoadOptions {
        assists: Some("realistic".to_string()),
        lane_keeping: Some("off".to_string()),
        curve_assist: Some(false),
        predictive_cruise: Some(false),
        descent: Some("off".to_string()),
        ..Default::default()
    }
}

fn the_driver_kept_it_clean(r: &Report) {
    assert!(!r.pulled_over, "nobody pulled the truck over");
    assert!(
        r.over_limit_s < 3.0,
        "kept to the posted limit: {:.1} s over",
        r.over_limit_s
    );
    assert!(
        r.off_road_s < 0.5,
        "stayed on the pavement: {:.1} s off",
        r.off_road_s
    );
    assert!(
        r.widest_ft < 6.0,
        "held its lane: {:.1} ft from centre at worst",
        r.widest_ft
    );
    assert!(r.damage < 1.0, "no crash: +{:.1}% truck damage", r.damage);
    assert!(
        r.cargo_damage < 1.0,
        "the load rode well: +{:.1}% cargo damage",
        r.cargo_damage
    );
}

#[test]
#[cfg_attr(ci_quick, ignore = "sweep: drives five miles of mountain road")]
fn by_sight_alone_a_driver_steers_a_mountain_bend_and_follows_a_slow_car() {
    // Denver toward Salt Lake City: a 40 mph bend four miles from the
    // staged start, on road that narrows to one lane each way.
    let opts = RoadOptions {
        feature: "curve".to_string(),
        seed: Some(7),
        trip_seed: Some(7),
        ..by_hand()
    };
    let pairs = vec![("Denver".to_string(), "Salt Lake City".to_string())];
    let hit = find_feature_seeded(get_world(), &pairs, "curve", &opts)
        .into_iter()
        .next()
        .expect("a bend on the way to Salt Lake City");
    let mut h = PlaytestHarness::new();
    h.app.ctx.settings.speed_keeper = false;
    h.app.ctx.settings.pedal_latch = "off".to_string();
    h.start_road_feature(&hit, &opts);
    h.app.ctx.settings.automatic_emergency_braking = false;
    h.app.ctx.settings.stop_and_go_assist = false;
    assert_eq!(h.app.ctx.settings.lane_keeping, "off");
    slow_car_ahead(&mut h, 0.18, 44.0);
    let (advisory, bend_end) = h.read_drive(|d| {
        let bend = d
            .trip
            .curve_at(hit.at_mi + 0.01)
            .expect("the bend the search found");
        (
            bend.advisory_mph as f64,
            bend.start_mi.max(bend.end_mi) + 0.5,
        )
    });
    let r = drive_by_sight(&mut h, bend_end, "bend");
    assert!(
        r.miles > 3.0,
        "the drive got through the bend and beyond: {r:?}"
    );
    the_driver_kept_it_clean(&r);
    assert!(
        r.slowest_in_bends <= advisory + 2.0,
        "slowed for the {advisory:.0} mph bend: {:.0} mph",
        r.slowest_in_bends
    );
    assert!(
        r.closest_ft > 80.0,
        "never closed on the slow car: {:.0} ft",
        r.closest_ft
    );
}

#[test]
#[cfg_attr(ci_quick, ignore = "sweep: drives three miles of interstate")]
fn by_sight_alone_a_driver_passes_a_slow_car_and_keeps_right() {
    let mut h = PlaytestHarness::new();
    {
        let s = &mut h.app.ctx.settings;
        let opts = by_hand();
        s.apply_driving_assistance_preset(opts.assists.as_deref().unwrap_or_default());
        s.lane_keeping = "off".to_string();
        s.curve_speed_assist = false;
        s.predictive_cruise = false;
        s.descent_speed_control = "off".to_string();
        s.speed_keeper = false;
        s.pedal_latch = "off".to_string();
        s.automatic_emergency_braking = false;
        s.stop_and_go_assist = false;
    }
    h.start_delivery(StartDelivery::named("Visual Display"));
    h.with_drive(|d, _| {
        d.departure_checked = true;
        d.trip.position_mi = 24.0;
        d.trip.traffic_manager.vehicles.clear();
    });
    h.prepare_for_driving(58.0);
    let lanes = h.read_drive(|d| d.lane.lane_count);
    assert!(
        lanes >= 2,
        "an interstate with room to pass ({lanes} lanes)"
    );
    slow_car_ahead(&mut h, 0.15, 35.0);
    let r = drive_by_sight(&mut h, 27.0, "pass");
    the_driver_kept_it_clean(&r);
    assert!(r.passes >= 1, "the slow car on screen was passed: {r:?}");
    assert!(
        r.lanes_used.contains(&1),
        "the pass used the left lane: {r:?}"
    );
    assert!(r.passed_slow_car, "the slow car fell behind: {r:?}");
    assert_eq!(
        r.ended_in_lane, 0,
        "back in the right lane after the pass: {r:?}"
    );
}

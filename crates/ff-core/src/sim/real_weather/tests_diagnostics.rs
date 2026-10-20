//! Why a live-weather fetch failed has to reach the log, once, and the
//! game must not flood api.weather.gov. Live weather read unavailable on one
//! Windows machine (2026-10-19) with nothing in the log beyond a bare
//! sentence, while another NWS client on the same machine worked.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use super::super::real_traffic::wall_time;

fn obs_at(now: f64) -> Observation {
    Observation::new("Clear", 0.0, Some(10.0), Some(10.0)).observed_at(now)
}

fn fixed(t: f64) -> Clock {
    Arc::new(move || t)
}

type LatestFn = Box<dyn Fn(&str) -> Result<Value, String> + Send + Sync>;

/// Answers `/points` (or refuses it, with `points_ok` false), lists two
/// stations, and answers each station's latest observation from `latest`.
struct ScriptedTransport {
    points_ok: bool,
    latest: LatestFn,
    hold: Duration,
    peak: AtomicUsize,
}

impl ScriptedTransport {
    fn new(latest: impl Fn(&str) -> Result<Value, String> + Send + Sync + 'static) -> Self {
        Self {
            points_ok: true,
            latest: Box::new(latest),
            hold: Duration::ZERO,
            peak: AtomicUsize::new(0),
        }
    }
}

impl HttpTransport for ScriptedTransport {
    fn get(&self, url: &str, _: &[(&str, &str)], _: f64) -> Result<Vec<u8>, TransportError> {
        self.peak.fetch_max(FetchSlot::in_use(), Ordering::SeqCst);
        std::thread::sleep(self.hold);
        let doc = if url.contains("/points/") {
            if !self.points_ok {
                return Err(TransportError::new(
                    "The server answered with error 403. (GET api.weather.gov/points/1,2: Forbidden)",
                ));
            }
            serde_json::json!({"properties": {"observationStations": "https://x/stations"}})
        } else if url.ends_with("/stations") {
            serde_json::json!({"observationStations": ["https://x/stations/KAAA", "https://x/stations/KBBB"]})
        } else {
            (self.latest)(url).map_err(TransportError::new)?
        };
        Ok(serde_json::to_vec(&doc).unwrap())
    }

    fn post(
        &self,
        _: &str,
        _: &[u8],
        _: &[(&str, &str)],
        _: f64,
    ) -> Result<Vec<u8>, TransportError> {
        Err(TransportError::new("no POST on the NWS API"))
    }
}

#[test]
fn the_points_lookup_asks_for_the_address_nws_serves_without_a_redirect() {
    assert_eq!(NwsFetcher::point_path(41.88, -87.63), "41.88,-87.63");
    assert_eq!(
        NwsFetcher::point_path(39.739_236, -104.990_251),
        "39.7392,-104.9903"
    );
    assert_eq!(NwsFetcher::point_path(35.0, -90.0), "35.0,-90.0");
}

#[test]
fn a_failed_points_lookup_says_which_step_failed_and_why() {
    let mut transport = ScriptedTransport::new(|_| Err("unused".to_string()));
    transport.points_ok = false;
    let err = NwsFetcher::new(Arc::new(transport))
        .default_fetch(34.5, -91.55)
        .unwrap_err();
    assert!(err.starts_with("points lookup: "), "{err}");
    assert!(err.contains("403") && err.contains("Forbidden"), "{err}");
}

#[test]
fn when_every_station_fails_each_one_is_named_in_the_error() {
    let transport = ScriptedTransport::new(|url| {
        Err(if url.contains("KAAA") {
            "The server answered with error 404.".to_string()
        } else {
            "The connection timed out.".to_string()
        })
    });
    let err = NwsFetcher::new(Arc::new(transport))
        .default_fetch(34.5, -91.55)
        .unwrap_err();
    assert_eq!(
        err,
        "every station failed: KAAA: The server answered with error 404.; \
         KBBB: The connection timed out."
    );
}

#[test]
fn a_failure_is_logged_once_per_stretch_and_again_when_the_reason_changes() {
    let now = 2_000_000.0;
    let answers = Arc::new(Mutex::new(vec![
        Err("A".to_string()),
        Err("A".to_string()),
        Err("A".to_string()),
        Err("B".to_string()),
        Ok(obs_at(now)),
        Err("B".to_string()),
    ]));
    let queue = answers.clone();
    let p = RealWeatherProvider::new(Arc::new(move |_, _| lock_unpoisoned(&queue).remove(0)))
        .with_clock(fixed(0.0))
        .with_wall_clock(fixed(now));
    let outcomes: Vec<WorkerOutcome> = (0..6)
        .map(|_| p.worker("city:stuttgart", 34.5, -91.55))
        .collect();
    assert_eq!(
        outcomes,
        vec![
            WorkerOutcome::Failed,
            WorkerOutcome::FailedSilent,
            WorkerOutcome::FailedSilent,
            WorkerOutcome::Failed,
            WorkerOutcome::Cached,
            WorkerOutcome::Failed,
        ]
    );
}

#[test]
fn route_planning_bursts_never_run_more_than_the_cap_of_fetches_at_once() {
    let now = wall_time();
    let stamp = chrono::DateTime::from_timestamp(now as i64, 0)
        .unwrap()
        .to_rfc3339();
    let mut transport = ScriptedTransport::new(move |_| {
        Ok(serde_json::json!({"properties": {"textDescription": "Clear", "timestamp": stamp}}))
    });
    transport.hold = Duration::from_millis(20);
    let transport = Arc::new(transport);
    let p = RealWeatherProvider::with_nws(transport.clone());
    // A dozen distinct places, all asked for in the same instant.
    for i in 0..12 {
        let lat = 30.0 + f64::from(i);
        p.request(&format!("city:{i}"), lat, -90.0);
    }
    p.join_background();
    let peak = transport.peak.load(Ordering::SeqCst);
    assert!(peak >= 1, "no fetch ran");
    assert!(peak <= MAX_CONCURRENT_FETCHES, "{peak} fetches ran at once");
    assert!((0..12).all(|i| p.get(&format!("city:{i}")).is_some()));
}

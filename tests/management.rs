#![cfg(not(target_arch = "wasm32"))]
use ryggattack::management::Status;
use std::time::{Duration, Instant};
#[test]
fn readiness_fails_before_first_tick_after_stall_and_during_shutdown() {
    let mut status = Status::default();
    for path in ["/healthz", "/readyz"] {
        assert_eq!(status.response(path, false).0, 503);
        status.tick_at = Some(Instant::now());
        assert_eq!(status.response(path, false).0, 200);
        assert_eq!(status.response(path, true).0, 503);
        status.tick_at = Some(Instant::now() - Duration::from_secs(2));
        assert_eq!(status.response(path, false).0, 503);
        status.tick_at = None;
    }
}
#[test]
fn metrics_are_aggregate_only_and_unknown_routes_are_not_probes() {
    let status = Status {
        ticks: 64,
        rooms: 2,
        players: 3,
        connections: 2,
        ..Default::default()
    };
    let (code, text) = status.response("/metrics", false);
    assert_eq!(code, 200);
    assert!(text.contains("ryggattack_ticks_total 64\n"));
    assert!(text.contains("ryggattack_rooms 2\n"));
    assert!(!text.contains('{'), "no identifying metric labels");
    for path in ["/", "/metrics?x=1", "/healthz/", "/readyz?x=1"] {
        assert_eq!(status.response(path, false).0, 404);
    }
}

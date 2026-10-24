use ryggattack::rooms::{Lobby, Phase, RoomSettings};
#[test]
fn readiness_countdown_resets_and_match_locks_roster_until_same_lobby_returns() {
    let mut r = Lobby::new(1, "0123456789abcdef0123".into(), 10, "A".into());
    r.ready(10, true).unwrap();
    assert_eq!(r.phase, Phase::Countdown { ticks_left: 128 });
    for _ in 0..127 {
        assert!(!r.advance());
    }
    r.join(11, "B".into()).unwrap();
    assert_eq!(r.phase, Phase::Lobby);
    assert!(r.humans.iter().all(|h| !h.ready));
    r.ready(10, true).unwrap();
    r.ready(11, true).unwrap();
    r.ready(11, false).unwrap();
    assert_eq!(r.phase, Phase::Lobby);
    r.ready(11, true).unwrap();
    r.configure(
        10,
        RoomSettings {
            bots: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(r.humans.iter().all(|h| !h.ready));
    r.ready(10, true).unwrap();
    r.ready(11, true).unwrap();
    for _ in 0..127 {
        assert!(!r.advance());
    }
    assert!(r.advance());
    assert_eq!(r.phase, Phase::Running);
    assert!(r.join(12, "C".into()).is_err());
    assert!(r.configure(10, RoomSettings::default()).is_err());
    assert!(r.ready(10, false).is_err());
    r.finish();
    assert_eq!(r.id, 1);
    assert_eq!(r.humans.len(), 2);
    assert_eq!(r.settings.bots, 1);
    assert!(r.humans.iter().all(|h| !h.ready));
    r.ready(10, true).unwrap();
    r.ready(11, true).unwrap();
    r.leave(10);
    assert_eq!(r.host, 11);
    assert_eq!(r.phase, Phase::Lobby);
    r.leave(11);
    assert!(r.humans.is_empty());
}
#[test]
fn solo_zero_bots_stays_offline_practice() {
    let mut r = Lobby::new(1, "0123456789abcdef0123".into(), 10, "A".into());
    r.configure(
        10,
        RoomSettings {
            bots: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(r.ready(10, true).is_err());
    assert_eq!(r.phase, Phase::Lobby);
}
#[test]
fn private_lobby_replaces_bots_without_using_names_as_identity() {
    let mut room = Lobby::new(1, "0123456789abcdef0123".into(), 10, "Ada".into());
    assert_eq!(room.host, 10);
    assert_eq!(room.settings.bots, 3);
    assert_eq!(room.join(11, "Ada".into()), Ok(1));
    assert_eq!(room.settings.bots, 2);
    assert_ne!(room.humans[0].id, room.humans[1].id);
    room.join(12, "C".into()).unwrap();
    room.join(13, "D".into()).unwrap();
    assert_eq!(room.join(14, "E".into()), Err("room full"));
    assert_ne!(room.configure(11, RoomSettings::default()), Ok(()));
}

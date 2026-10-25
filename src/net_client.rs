//! Shared native/browser adapter. Latest-state mailbox and stop-and-wait input
//! bound queues even if rendering stalls. No client simulation or hit claims.
use crate::{input::ControlIntent, network::*};
use std::{
    ops::ControlFlow,
    sync::{Arc, Mutex},
};
use web_time::{Duration, Instant};
// The renderer constructs routes for all four corners immediately. Validate
// the complete 5x5 undirected grid topology before publishing any map to it.
fn valid_topology(edges: &[(usize, usize)]) -> bool {
    if edges.is_empty() || edges.len() > 40 {
        return false;
    }
    let mut seen = std::collections::HashSet::new();
    let mut adjacent = [0u32; 25];
    for &(a, b) in edges {
        if a >= 25
            || b >= 25
            || a == b
            || !((a / 5 == b / 5 && a.abs_diff(b) == 1) || a.abs_diff(b) == 5)
            || !seen.insert((a.min(b), a.max(b)))
        {
            return false;
        }
        adjacent[a] |= 1 << b;
        adjacent[b] |= 1 << a;
    }
    let mut reached = 1u32;
    loop {
        let next = adjacent
            .iter()
            .enumerate()
            .filter(|(i, _)| reached & (1 << i) != 0)
            .fold(reached, |all, (_, neighbors)| all | neighbors);
        if next == reached {
            break;
        }
        reached = next;
    }
    reached == (1 << 25) - 1
}
// These are presentation-safety limits, deliberately wider than gameplay's
// arena/timers. Validate before publishing, then canonicalize carts for HUD and
// interpolation, which index by seat. Body and cart IDs are separate namespaces.
fn valid_frame(frame: &Frame) -> bool {
    fn bounded(values: &[f32], low: f32, high: f32) -> bool {
        values
            .iter()
            .all(|v| v.is_finite() && (low..=high).contains(v))
    }
    fn unit(values: &[f32]) -> bool {
        bounded(values, -1.01, 1.01)
            && (0.98..=1.02).contains(&values.iter().map(|v| v * v).sum::<f32>())
    }
    if frame.carts.len() != 4
        || frame.bodies.len() > 1024
        || frame
            .winner
            .is_some_and(|id| id >= 4 || !frame.round_finished || frame.wins[id] == 0)
        || frame.wins.iter().any(|w| *w > 7)
        || !bounded(&[frame.transition, frame.until_drop], 0., 60.)
        || frame.names.iter().flatten().any(|name| {
            name.trim().is_empty()
                || name.chars().count() > 24
                || name.chars().any(char::is_control)
        })
    {
        return false;
    }
    let mut carts = [false; 4];
    for cart in &frame.carts {
        if cart.id >= 4
            || std::mem::replace(&mut carts[cart.id], true)
            || (!cart.active && cart.alive)
            || !bounded(&cart.position, -1024., 1024.)
            || !unit(&cart.forward)
            || !bounded(
                &[
                    cart.shot_cooldown,
                    cart.speed_seconds,
                    cart.all_hits_seconds,
                ],
                0.,
                60.,
            )
        {
            return false;
        }
    }
    let mut ids = std::collections::HashSet::new();
    frame.bodies.iter().all(|body| {
        body.id > 0
            && body.id <= 9_007_199_254_740_991
            && ids.insert(body.id)
            && body.kind <= 4
            && match body.kind {
                0 => body.owner.is_some_and(|seat| seat < 4) && body.prop.is_none(),
                3 => {
                    body.owner.is_none()
                        && body.prop.is_some_and(|prop| {
                            prop.variant < 2
                                && prop.scale.is_finite()
                                && (0.75..=2.2).contains(&prop.scale)
                        })
                }
                _ => body.owner.is_none() && body.prop.is_none(),
            }
            && bounded(&body.position, -1024., 1024.)
            && unit(&body.rotation)
            && bounded(&body.scale, 0., 128.)
    })
}
fn valid_lobby(lobby: &crate::rooms::Lobby) -> bool {
    use crate::rooms::Phase;
    let valid_name = |s: &str| {
        !s.trim().is_empty() && s.chars().count() <= 24 && !s.chars().any(char::is_control)
    };
    let mut ids = std::collections::HashSet::new();
    let mut seats = std::collections::HashSet::new();
    lobby.id > 0
        && valid_code(&lobby.code)
        && !lobby.humans.is_empty()
        && lobby.humans.len() <= 4
        && lobby.settings.valid(lobby.humans.len())
        && lobby
            .humans
            .iter()
            .any(|h| h.id == lobby.host && h.connected)
        && lobby.humans.iter().all(|h| {
            h.id > 0
                && h.seat < 4
                && ids.insert(h.id)
                && seats.insert(h.seat)
                && valid_name(&h.name)
                && if h.connected {
                    h.reconnect_seconds.is_none()
                } else {
                    h.reconnect_seconds.is_some_and(|s| (1..=60).contains(&s))
                }
        })
        && match lobby.phase {
            Phase::Countdown { ticks_left } => {
                (1..=128).contains(&ticks_left)
                    && lobby.humans.iter().all(|h| h.ready && h.connected)
            }
            _ => true,
        }
}
#[cfg(any(target_arch = "wasm32", test))]
fn claim_key(address: &str, code: &str) -> String {
    format!(
        "ryggattack.claim.{}",
        serde_json::to_string(&(address, code)).unwrap()
    )
}
#[cfg(any(target_arch = "wasm32", test))]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredClaim {
    room: u64,
    participant: u64,
    seat: usize,
    credential: String,
}
#[cfg(any(target_arch = "wasm32", test))]
impl StoredClaim {
    fn parse(text: &str) -> Option<Self> {
        if text.len() > 512 {
            return None;
        }
        let c: Self = serde_json::from_str(text).ok()?;
        (c.room > 0 && c.participant > 0 && c.seat < 4 && valid_credential(&c.credential))
            .then_some(c)
    }
}
type Welcome = (usize, u64, u64, Vec<(usize, usize)>, String, String);
#[derive(Default)]
struct Inbox {
    opened: bool,
    disconnected: bool,
    welcome: Option<Welcome>,
    lobby: Option<crate::rooms::Lobby>,
    notice: Option<String>,
    frame: Option<Frame>,
    frame_barrier: bool,
    error: Option<String>,
}
pub struct NetworkClient {
    sender: Option<ewebsock::WsSender>,
    address: String,
    inbox: Arc<Mutex<Inbox>>,
    name: String,
    room_code: Option<String>,
    pub participant: Option<u64>,
    pub room: Option<u64>,
    pub lobby: Option<crate::rooms::Lobby>,
    pub notice: String,
    pub seat: Option<usize>,
    pub edges: Option<Vec<(usize, usize)>>,
    pub frame: Option<Frame>,
    pub previous: Option<Frame>,
    /// Presentation invalidation survives coalesced Lobby/Welcome + State.
    pub(crate) visual_epoch: u64,
    pub received: Instant,
    pub status: String,
    pub rtt_ms: Option<f64>,
    sequence: u64,
    sent: Instant,
    acknowledged: bool,
    failed: bool,
    credential: Option<String>,
    retry_started: Option<Instant>,
    retry_at: Instant,
    retry_count: u32,
    suppress_until_neutral: bool,
    awaiting_welcome: bool,
}
impl NetworkClient {
    pub fn connect(address: &str, name: &str) -> Result<Self, String> {
        Self::connect_room(address, name, None)
    }
    pub fn connect_room(address: &str, name: &str, room: Option<&str>) -> Result<Self, String> {
        decode_client(
            &serde_json::to_string(&ClientMessage::Hello {
                version: VERSION,
                name: name.into(),
                room: room.map(str::to_owned),
            })
            .unwrap(),
        )
        .map_err(str::to_owned)?;
        let inbox = Arc::new(Mutex::new(Inbox::default()));
        let now = Instant::now();
        #[allow(unused_mut)]
        let mut client = Self {
            sender: None,
            room_code: room.map(str::to_owned),
            participant: None,
            room: None,
            lobby: None,
            notice: String::new(),
            address: address.into(),
            inbox,
            name: name.into(),
            seat: None,
            edges: None,
            frame: None,
            previous: None,
            visual_epoch: 0,
            received: now,
            status: "Connecting…".into(),
            rtt_ms: None,
            sequence: 0,
            sent: now,
            acknowledged: true,
            failed: false,
            credential: None,
            retry_started: None,
            retry_at: now,
            retry_count: 0,
            suppress_until_neutral: false,
            awaiting_welcome: true,
        };
        #[cfg(target_arch = "wasm32")]
        if let Some(code) = room {
            let stored = web_sys::window()
                .and_then(|w| w.session_storage().ok().flatten())
                .and_then(|s| s.get_item(&claim_key(address, code)).ok().flatten())
                .and_then(|s| StoredClaim::parse(&s));
            if let Some(c) = stored {
                client.room = Some(c.room);
                client.participant = Some(c.participant);
                client.seat = Some(c.seat);
                client.credential = Some(c.credential);
                client.suppress_until_neutral = true;
                client.status = "Reconnecting saved seat…".into();
            }
        }
        Ok(client)
    }
    pub fn command(&mut self, command: ClientMessage) -> Result<(), String> {
        if self.failed || self.seat.is_none() || self.awaiting_welcome {
            return Err("not connected".into());
        }
        if matches!(
            command,
            ClientMessage::Hello { .. }
                | ClientMessage::Reconnect { .. }
                | ClientMessage::Input { .. }
        ) {
            return Err("not a lobby command".into());
        }
        let text = serde_json::to_string(&command).unwrap();
        decode_client(&text).map_err(str::to_owned)?;
        self.sender
            .as_mut()
            .ok_or("not connected")?
            .send(ewebsock::WsMessage::Text(text));
        self.notice.clear();
        Ok(())
    }
    fn clear_claim(&self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(code) = &self.room_code
            && let Some(storage) =
                web_sys::window().and_then(|w| w.session_storage().ok().flatten())
        {
            let _ = storage.remove_item(&claim_key(&self.address, code));
        }
    }
    fn save_claim(&self) {
        #[cfg(target_arch = "wasm32")]
        if let (Some(code), Some(room), Some(participant), Some(seat), Some(credential), Some(w)) = (
            &self.room_code,
            self.room,
            self.participant,
            self.seat,
            &self.credential,
            web_sys::window(),
        ) {
            if let Ok(Some(storage)) = w.session_storage() {
                let text = serde_json::to_string(&StoredClaim {
                    room,
                    participant,
                    seat,
                    credential: credential.clone(),
                })
                .unwrap();
                let _ = storage.set_item(&claim_key(&self.address, code), &text);
            }
            // Invitation only in the address bar; credential stays private.
            if let (Ok(search), Ok(path), Ok(history)) =
                (w.location().search(), w.location().pathname(), w.history())
                && let Ok(params) = web_sys::UrlSearchParams::new_with_str(&search)
            {
                params.set("room", code);
                let _ = history.replace_state_with_url(
                    &wasm_bindgen::JsValue::NULL,
                    "",
                    Some(&format!("{path}?{}", params.to_string())),
                );
            }
        }
    }
    fn fail_terminal(&mut self, status: String) {
        self.clear_claim();
        self.credential = None;
        if let Some(mut sender) = self.sender.take() {
            sender.close();
        }
        self.failed = true;
        self.retry_started = None;
        self.notice.clear();
        self.lobby = None;
        self.frame = None;
        self.previous = None;
        self.status = status;
    }
    pub fn leave(&mut self) {
        self.clear_claim();
        let _ = self.command(ClientMessage::Leave);
        if let Some(sender) = &mut self.sender {
            sender.close();
        }
        self.failed = true;
        self.credential = None;
        self.retry_started = None;
        self.seat = None;
        self.participant = None;
        self.room = None;
        self.lobby = None;
        self.frame = None;
        self.previous = None;
        self.status = "Left room; create or join another".into();
    }
    fn open(address: &str, mailbox: Arc<Mutex<Inbox>>) -> Result<ewebsock::WsSender, String> {
        let options = ewebsock::Options {
            max_incoming_frame_size: MAX_STATE_BYTES,
            ..Default::default()
        };
        ewebsock::ws_connect(
            address.into(),
            options,
            Box::new(move |event| {
                let mut inbox = mailbox.lock().unwrap();
                match event {
                    ewebsock::WsEvent::Opened => inbox.opened = true,
                    ewebsock::WsEvent::Message(ewebsock::WsMessage::Text(text)) => {
                        let msg = if text.len() <= MAX_STATE_BYTES {
                            serde_json::from_str::<ServerMessage>(&text).ok()
                        } else {
                            None
                        };
                        match msg {
                            Some(ServerMessage::Welcome {
                                version,
                                seat,
                                edges,
                                room,
                                participant,
                                code,
                                credential,
                            }) if version == VERSION
                                && seat < 4
                                && room > 0
                                && participant > 0
                                && valid_code(&code)
                                && valid_credential(&credential)
                                && valid_topology(&edges) =>
                            {
                                inbox.frame_barrier = true;
                                inbox.frame = None;
                                inbox.welcome =
                                    Some((seat, room, participant, edges, code, credential))
                            }
                            Some(ServerMessage::State(mut frame)) if valid_frame(&frame) => {
                                frame.carts.sort_unstable_by_key(|cart| cart.id);
                                inbox.frame = Some(frame)
                            }
                            Some(ServerMessage::Error(err)) => inbox.error = Some(err),
                            Some(ServerMessage::Lobby(lobby)) if valid_lobby(&lobby) => {
                                if lobby.phase != crate::rooms::Phase::Running {
                                    inbox.frame_barrier = true;
                                    inbox.frame = None;
                                }
                                inbox.lobby = Some(lobby)
                            }
                            Some(ServerMessage::Notice(notice)) if notice.len() <= 256 => {
                                inbox.notice = Some(notice)
                            }
                            _ => inbox.error = Some("Invalid server protocol".into()),
                        }
                    }
                    ewebsock::WsEvent::Error(_) | ewebsock::WsEvent::Closed => {
                        inbox.disconnected = true;
                    }
                    _ => {}
                }
                if inbox.error.is_some() || inbox.disconnected {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            }),
        )
    }
    fn retry(&mut self) {
        if let Some(mut sender) = self.sender.take() {
            sender.close();
        }
        self.inbox = Arc::new(Mutex::new(Inbox::default()));
        let started = *self.retry_started.get_or_insert_with(Instant::now);
        if self.credential.is_none()
            || started.elapsed() >= Duration::from_secs(55)
            || self.retry_count >= 12
        {
            self.failed = true;
            self.status =
                "Backend unavailable or reconnect expired; leave and join a lobby again".into();
            return;
        }
        self.retry_count += 1;
        self.retry_at =
            Instant::now() + Duration::from_millis((250u64 << self.retry_count.min(4)).min(4000));
        self.sequence = 0;
        self.awaiting_welcome = true;
        self.acknowledged = true;
        self.suppress_until_neutral = true;
        self.previous = None;
        self.status = format!(
            "Retrying {}/12 — bot takeover; seat reserved up to 60s",
            self.retry_count
        );
    }
    /// Returns true only when an intent was sent; caller may retain fire taps.
    pub fn pump(&mut self, intent: ControlIntent) -> bool {
        if self.failed || Instant::now() < self.retry_at {
            return false;
        }
        if self
            .retry_started
            .is_some_and(|s| s.elapsed() >= Duration::from_secs(55))
        {
            self.retry();
            return false;
        }
        let intent = if self.suppress_until_neutral {
            if intent == ControlIntent::default() {
                self.suppress_until_neutral = false;
            }
            ControlIntent::default()
        } else {
            intent
        };
        // Do not open a socket while the caller is still constructing Bevy's
        // scene/plugins. The transport clock starts only when it can be polled.
        if self.sender.is_none() {
            match Self::open(&self.address, self.inbox.clone()) {
                Ok(sender) => self.sender = Some(sender),
                Err(_) => {
                    self.retry();
                    return false;
                }
            }
            self.sent = Instant::now();
        }
        let inbox = std::mem::take(&mut *self.inbox.lock().unwrap());
        // Latest-state coalescing may hide Lobby/Countdown behind Running.
        // Preserve that boundary even when no non-running lobby reaches pump.
        if inbox.frame_barrier {
            self.visual_epoch = self.visual_epoch.wrapping_add(1);
            self.frame = None;
            self.previous = None;
            self.acknowledged = true;
        }
        if let Some((seat, room, participant, edges, code, credential)) = inbox.welcome {
            if self.participant.is_some_and(|id| id != participant)
                || self.room.is_some_and(|id| id != room)
                || self.seat.is_some_and(|s| s != seat)
            {
                self.fail_terminal("Invalid server protocol".into());
                return false;
            }
            self.seat = Some(seat);
            self.room = Some(room);
            self.participant = Some(participant);
            self.edges = Some(edges);
            self.room_code = Some(code);
            self.credential = Some(credential);
            self.retry_started = None;
            self.retry_count = 0;
            self.awaiting_welcome = false;
            self.received = Instant::now();
            self.sequence = 0;
            self.acknowledged = true;
            self.frame = None;
            self.previous = None;
            self.notice.clear();
            self.status = format!("Online — seat {}", seat + 1);
            self.save_claim();
        }
        if inbox.disconnected && inbox.error.is_none() {
            self.retry();
            return false;
        }
        if let Some(error) = inbox.error {
            self.fail_terminal(error);
            return false;
        }
        let sender = self.sender.as_mut().expect("opened above");
        if inbox.opened {
            // Scene/plugin startup may delay the first pump. Start the Hello
            // acknowledgement deadline when Hello is actually sent, not when
            // construction began before the first rendered frame.
            self.sent = Instant::now();
            sender.send(ewebsock::WsMessage::Text(
                serde_json::to_string(&if let (Some(credential), Some(participant), Some(room)) =
                    (&self.credential, self.participant, &self.room_code)
                {
                    ClientMessage::Reconnect {
                        version: VERSION,
                        room: room.clone(),
                        participant,
                        credential: credential.clone(),
                    }
                } else {
                    ClientMessage::Hello {
                        version: VERSION,
                        name: self.name.clone(),
                        room: self.room_code.clone(),
                    }
                })
                .unwrap(),
            ));
            self.status = if self.credential.is_some() {
                "Reconnecting — bot takeover until seat restored…"
            } else {
                "Waiting for seat…"
            }
            .into();
        }
        if let Some(notice) = inbox.notice {
            self.notice = notice;
        }
        if let Some(lobby) = inbox.lobby {
            if Some(lobby.id) != self.room
                || self.room_code.as_ref() != Some(&lobby.code)
                || !lobby
                    .humans
                    .iter()
                    .any(|h| Some(h.id) == self.participant && Some(h.seat) == self.seat)
            {
                self.fail_terminal("Invalid server protocol".into());
                return false;
            }
            if lobby.phase != crate::rooms::Phase::Running {
                self.frame = None;
                self.previous = None;
                self.acknowledged = true;
            }
            self.lobby = Some(lobby);
            self.received = Instant::now();
        }
        let running = self
            .lobby
            .as_ref()
            .is_none_or(|l| l.phase == crate::rooms::Phase::Running);
        if let Some(frame) = inbox.frame.filter(|_| running) {
            if let Some(seat) = self.seat
                && !self.acknowledged
                && frame.ack[seat] >= self.sequence
            {
                self.rtt_ms = Some(self.sent.elapsed().as_secs_f64() * 1000.0);
                self.acknowledged = true;
            }
            if self.frame.as_ref().is_none_or(|old| frame.tick > old.tick) {
                self.previous = self.frame.take();
                self.frame = Some(frame);
                self.received = Instant::now();
            }
        }
        if ((self.awaiting_welcome || !self.acknowledged)
            && self.sent.elapsed() > Duration::from_secs(5))
            || (self.seat.is_some()
                && self.received.elapsed() > Duration::from_secs(5)
                && self.retry_started.is_none())
        {
            self.retry();
            return false;
        }
        if self.seat.is_some()
            && !self.awaiting_welcome
            && running
            && self.acknowledged
            && (self.sequence == 0 || self.sent.elapsed() >= Duration::from_millis(62))
        {
            self.sequence += 1;
            self.sent = Instant::now();
            self.acknowledged = false;
            sender.send(ewebsock::WsMessage::Text(
                serde_json::to_string(&ClientMessage::Input {
                    sequence: self.sequence,
                    horizontal: intent.horizontal,
                    vertical: intent.vertical,
                    fire: intent.fire,
                })
                .unwrap(),
            ));
            return true;
        }
        false
    }
}
#[cfg(test)]
mod resilience_tests {
    use super::*;
    #[test]
    fn visual_descriptors_validate_after_wire_decode() {
        let mut sim = crate::headless::Simulation::new(Default::default()).unwrap();
        let mut frame = sim.network_frame();
        frame.bodies.truncate(1);
        let body = &mut frame.bodies[0];
        body.kind = 0;
        body.prop = None;
        for seat in 0..4 {
            frame.bodies[0].owner = Some(seat);
            let decoded: Frame =
                serde_json::from_str(&serde_json::to_string(&frame).unwrap()).unwrap();
            assert!(valid_frame(&decoded));
        }
        for owner in [None, Some(4), Some(usize::MAX)] {
            frame.bodies[0].owner = owner;
            assert!(!valid_frame(&frame));
        }
        frame.bodies[0].owner = None;
        frame.bodies[0].kind = 3;
        assert!(!valid_frame(&frame));
        for kind in [
            PropKind::Pine,
            PropKind::Birch,
            PropKind::Bush,
            PropKind::Rock,
            PropKind::Stump,
            PropKind::Fern,
        ] {
            for variant in 0..2 {
                for scale in [0.75, 1.15, 1.5, 2.2] {
                    frame.bodies[0].prop = Some(PropVisual {
                        kind,
                        variant,
                        scale,
                    });
                    let decoded: Frame =
                        serde_json::from_str(&serde_json::to_string(&frame).unwrap()).unwrap();
                    assert!(valid_frame(&decoded));
                }
            }
        }
        let good = frame.clone();
        for scale in [0., 0.749, 2.201, f32::NAN, f32::INFINITY] {
            frame.bodies[0].prop.as_mut().unwrap().scale = scale;
            assert!(!valid_frame(&frame));
        }
        frame = good.clone();
        for variant in [2, 255] {
            frame.bodies[0].prop.as_mut().unwrap().variant = variant;
            assert!(!valid_frame(&frame));
        }
        frame = good.clone();
        frame.bodies[0].owner = Some(0);
        assert!(!valid_frame(&frame));
        for kind in [0, 1, 2, 4] {
            frame = good.clone();
            frame.bodies[0].kind = kind;
            frame.bodies[0].owner = (kind == 0).then_some(0);
            assert!(!valid_frame(&frame), "prop on non-scenery kind {kind}");
        }
        for kind in [1, 2, 4] {
            frame = good.clone();
            frame.bodies[0].kind = kind;
            frame.bodies[0].prop = None;
            assert!(valid_frame(&frame));
            frame.bodies[0].owner = Some(0);
            assert!(!valid_frame(&frame));
        }
        let wire = serde_json::to_value(&good.bodies[0]).unwrap();
        for (field, value) in [
            ("kind", serde_json::json!("Unknown")),
            ("variant", serde_json::json!(256)),
            ("scale", serde_json::json!("large")),
            ("extra", serde_json::json!(true)),
        ] {
            let mut bad = wire.clone();
            bad["prop"][field] = value;
            assert!(serde_json::from_value::<Body>(bad).is_err());
        }
        for owner in [
            serde_json::json!(-1),
            serde_json::json!(0.5),
            serde_json::json!("Ada"),
        ] {
            let mut bad = wire.clone();
            bad["owner"] = owner;
            assert!(serde_json::from_value::<Body>(bad).is_err());
        }
        let mut oversized = good.clone();
        oversized.bodies = (1..=1025)
            .map(|id| {
                let mut body = good.bodies[0].clone();
                body.id = id;
                body
            })
            .collect();
        assert!(!valid_frame(&oversized));
    }
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn coalesced_match_boundary_discards_old_winner_before_pump() {
        use crate::rooms::Phase;
        for fresh_state in [false, true] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = format!("ws://{}", listener.local_addr().unwrap());
            let (release, hold) = std::sync::mpsc::channel();
            let server = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                let mut ws = tungstenite::accept(stream).unwrap();
                let mut sim = crate::headless::Simulation::new(Default::default()).unwrap();
                let mut frame = sim.network_frame();
                frame.tick = 90;
                frame.winner = Some(0);
                frame.wins[0] = 1;
                frame.round_finished = true;
                let mut lobby = crate::rooms::Lobby::new(1, "a".repeat(20), 1, "Ada".into());
                lobby.humans[0].ready = true;
                let mut messages = vec![
                    ServerMessage::Welcome {
                        version: VERSION,
                        seat: 0,
                        room: 1,
                        participant: 1,
                        edges: sim.network_edges(),
                        code: "a".repeat(20),
                        credential: "b".repeat(64),
                    },
                    ServerMessage::State(frame),
                ];
                for phase in [
                    Phase::Lobby,
                    Phase::Countdown { ticks_left: 32 },
                    Phase::Running,
                ] {
                    lobby.phase = phase;
                    messages.push(ServerMessage::Lobby(lobby.clone()));
                }
                if fresh_state {
                    let mut next = sim.network_frame();
                    next.tick = 1;
                    messages.push(ServerMessage::State(next));
                }
                messages.push(ServerMessage::Notice("barrier delivered".into()));
                for message in messages {
                    ws.send(tungstenite::Message::Text(
                        serde_json::to_string(&message).unwrap().into(),
                    ))
                    .unwrap();
                }
                hold.recv_timeout(std::time::Duration::from_secs(10)).ok();
            });
            let mut client = NetworkClient::connect(&address, "Ada").unwrap();
            client.pump(Default::default());
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while client.inbox.lock().unwrap().notice.as_deref() != Some("barrier delivered") {
                assert!(
                    std::time::Instant::now() < deadline,
                    "callback did not receive boundary"
                );
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            client.pump(Default::default());
            release.send(()).unwrap();
            server.join().unwrap();
            assert_eq!(client.seat, Some(0));
            assert_eq!(client.lobby.as_ref().unwrap().phase, Phase::Running);
            if fresh_state {
                let frame = client.frame.as_ref().expect("post-barrier frame");
                assert_eq!(frame.tick, 1);
                assert_eq!(frame.winner, None);
            } else {
                assert!(
                    client.frame.is_none(),
                    "coalescing resurrected the previous match winner"
                );
            }
            assert!(client.previous.is_none());
            assert!(
                client.visual_epoch > 0,
                "renderer must observe a coalesced boundary"
            );
            client.leave();
        }
    }
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn terminal_identity_mismatches_revoke_claim_close_transport_and_clear_notice() {
        for mismatch in ["welcome", "lobby", "server error"] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let mut c = NetworkClient::connect_room(
                &format!("ws://{}", listener.local_addr().unwrap()),
                "Ada",
                Some(&"a".repeat(20)),
            )
            .unwrap();
            c.participant = Some(1);
            c.room = Some(1);
            c.seat = Some(0);
            c.credential = Some("a".repeat(64));
            c.notice = "not connected".into();
            {
                let mut inbox = c.inbox.lock().unwrap();
                match mismatch {
                    "welcome" => {
                        inbox.welcome = Some((0, 1, 2, vec![], "a".repeat(20), "b".repeat(64)))
                    }
                    "lobby" => {
                        inbox.lobby =
                            Some(crate::rooms::Lobby::new(2, "a".repeat(20), 1, "Ada".into()))
                    }
                    _ => inbox.error = Some("room expired".into()),
                }
            }
            assert!(!c.pump(Default::default()));
            assert!(c.failed, "{mismatch}");
            assert!(
                c.credential.is_none(),
                "terminal {mismatch} retained private claim"
            );
            assert!(c.sender.is_none(), "terminal {mismatch} retained transport");
            assert!(
                c.notice.is_empty(),
                "terminal {mismatch} retained stale notice"
            );
        }
    }
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn restored_welcome_clears_retry_command_notice() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut c = NetworkClient::connect("ws://127.0.0.1:1", "Ada").unwrap();
        c.address = format!("ws://{}", listener.local_addr().unwrap());
        c.notice = "not connected".into();
        c.inbox.lock().unwrap().welcome = Some((0, 1, 1, vec![], "a".repeat(20), "b".repeat(64)));
        c.pump(Default::default());
        assert!(c.status.starts_with("Online"));
        assert!(
            c.notice.is_empty(),
            "successful reconnect is masked by a stale command error"
        );
    }
    #[test]
    fn stored_claim_is_private_scoped_and_validated() {
        assert_ne!(claim_key("ws://a", "room"), claim_key("ws://b", "room"));
        assert_ne!(claim_key("ws://a", "room"), claim_key("ws://a", "other"));
        let good =
            serde_json::json!({"room":1,"participant":2,"seat":0,"credential":"a".repeat(64)});
        assert!(StoredClaim::parse(&good.to_string()).is_some());
        for (k, v) in [
            ("participant", serde_json::json!(0)),
            ("seat", serde_json::json!(4)),
            ("credential", serde_json::json!("invite")),
        ] {
            let mut invalid = good.clone();
            invalid[k] = v;
            assert!(StoredClaim::parse(&invalid.to_string()).is_none());
        }
    }
    #[test]
    fn retries_are_bounded_and_leave_disables_reconnect() {
        let mut c = NetworkClient::connect("ws://127.0.0.1:1", "Ada").unwrap();
        c.credential = Some("a".repeat(64));
        for _ in 0..12 {
            c.retry();
            assert!(!c.failed);
        }
        c.retry();
        assert!(c.failed);
        assert!(c.status.contains("unavailable"));
        c.leave();
        assert!(c.credential.is_none());
        assert!(!c.pump(Default::default()));
        let mut c = NetworkClient::connect("ws://127.0.0.1:1", "Ada").unwrap();
        c.credential = Some("a".repeat(64));
        c.retry_started = Some(Instant::now() - Duration::from_secs(55));
        c.retry();
        assert!(c.failed);
    }
}

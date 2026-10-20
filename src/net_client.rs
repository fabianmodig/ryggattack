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
        && lobby.humans.iter().any(|h| h.id == lobby.host)
        && lobby.humans.iter().all(|h| {
            h.id > 0
                && h.seat < 4
                && ids.insert(h.id)
                && seats.insert(h.seat)
                && valid_name(&h.name)
        })
        && match lobby.phase {
            Phase::Countdown { ticks_left } => {
                (1..=128).contains(&ticks_left) && lobby.humans.iter().all(|h| h.ready)
            }
            _ => true,
        }
}
type Welcome = (usize, u64, u64, Vec<(usize, usize)>);
#[derive(Default)]
struct Inbox {
    opened: bool,
    welcome: Option<Welcome>,
    lobby: Option<crate::rooms::Lobby>,
    notice: Option<String>,
    frame: Option<Frame>,
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
    pub received: Instant,
    pub status: String,
    pub rtt_ms: Option<f64>,
    sequence: u64,
    sent: Instant,
    acknowledged: bool,
    failed: bool,
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
        Ok(Self {
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
            received: now,
            status: "Connecting…".into(),
            rtt_ms: None,
            sequence: 0,
            sent: now,
            acknowledged: true,
            failed: false,
        })
    }
    pub fn command(&mut self, command: ClientMessage) -> Result<(), String> {
        if self.failed || self.seat.is_none() {
            return Err("not connected".into());
        }
        if matches!(
            command,
            ClientMessage::Hello { .. } | ClientMessage::Input { .. }
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
    pub fn leave(&mut self) {
        let _ = self.command(ClientMessage::Leave);
        if let Some(sender) = &mut self.sender {
            sender.close();
        }
        self.failed = true;
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
                            }) if version == VERSION
                                && seat < 4
                                && room > 0
                                && participant > 0
                                && valid_topology(&edges) =>
                            {
                                inbox.welcome = Some((seat, room, participant, edges))
                            }
                            Some(ServerMessage::State(mut frame)) if valid_frame(&frame) => {
                                frame.carts.sort_unstable_by_key(|cart| cart.id);
                                inbox.frame = Some(frame)
                            }
                            Some(ServerMessage::Error(err)) => inbox.error = Some(err),
                            Some(ServerMessage::Lobby(lobby)) if valid_lobby(&lobby) => {
                                inbox.lobby = Some(lobby)
                            }
                            Some(ServerMessage::Notice(notice)) if notice.len() <= 256 => {
                                inbox.notice = Some(notice)
                            }
                            _ => inbox.error = Some("Invalid server protocol".into()),
                        }
                    }
                    ewebsock::WsEvent::Error(_) => {
                        inbox.error = Some("Connection failed; check backend address".into())
                    }
                    ewebsock::WsEvent::Closed => {
                        inbox.error =
                            Some("Disconnected; join a lobby again as a new participant".into())
                    }
                    _ => {}
                }
                if inbox.error.is_some() {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            }),
        )
    }
    /// Returns true only when an intent was sent; caller may retain fire taps.
    pub fn pump(&mut self, intent: ControlIntent) -> bool {
        if self.failed {
            return false;
        }
        // Do not open a socket while the caller is still constructing Bevy's
        // scene/plugins. The transport clock starts only when it can be polled.
        if self.sender.is_none() {
            match Self::open(&self.address, self.inbox.clone()) {
                Ok(sender) => self.sender = Some(sender),
                Err(error) => {
                    self.status = format!("Connection failed: {error}");
                    self.failed = true;
                    return false;
                }
            }
            self.sent = Instant::now();
        }
        let sender = self.sender.as_mut().expect("opened above");
        let inbox = std::mem::take(&mut *self.inbox.lock().unwrap());
        if let Some(error) = inbox.error {
            self.status = error;
            self.failed = true;
            sender.close();
            return false;
        }
        if inbox.opened {
            // Scene/plugin startup may delay the first pump. Start the Hello
            // acknowledgement deadline when Hello is actually sent, not when
            // construction began before the first rendered frame.
            self.sent = Instant::now();
            sender.send(ewebsock::WsMessage::Text(
                serde_json::to_string(&ClientMessage::Hello {
                    version: VERSION,
                    name: self.name.clone(),
                    room: self.room_code.clone(),
                })
                .unwrap(),
            ));
            self.status = "Waiting for seat…".into();
        }
        if let Some((seat, room, participant, edges)) = inbox.welcome {
            self.seat = Some(seat);
            self.room = Some(room);
            self.participant = Some(participant);
            self.edges = Some(edges);
            self.status = format!("Online — seat {}", seat + 1);
        }
        if let Some(notice) = inbox.notice {
            self.notice = notice;
        }
        if let Some(lobby) = inbox.lobby {
            if Some(lobby.id) != self.room
                || !lobby
                    .humans
                    .iter()
                    .any(|h| Some(h.id) == self.participant && Some(h.seat) == self.seat)
            {
                self.failed = true;
                self.status = "Invalid server protocol".into();
                sender.close();
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
        if (self.seat.is_none() || !self.acknowledged)
            && self.sent.elapsed() > Duration::from_secs(5)
        {
            self.failed = true;
            sender.close();
            self.status = "Server timeout; leave and join a lobby again".into();
            return false;
        }
        if self.seat.is_some()
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

//! Localhost-only development service: one continuously running four-seat match.
//! No public auth, rooms, reconnect, deployment, or asset-free server claim.
use crate::{
    headless::{Simulation, SimulationConfig, TICK_DURATION},
    input::ControlIntent,
    network::*,
};
use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};
use tungstenite::{Message, WebSocket, protocol::WebSocketConfig};
// Allocation-free framing observer, not a second protocol validator (tungstenite
// owns that). It times partial headers/payloads and fragmented messages across
// control frames; completed traffic/idle sockets have no active deadline.
#[derive(Default)]
struct WireProgress {
    header: [u8; 14],
    used: usize,
    payload: u64,
    fragmented: bool,
    since: Option<Instant>,
}
impl WireProgress {
    fn observe(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if self.payload > 0 {
                self.payload -= 1;
                if self.payload == 0 {
                    self.finish();
                }
                continue;
            }
            self.since.get_or_insert_with(Instant::now);
            self.header[self.used] = byte;
            self.used += 1;
            if self.used < 2 {
                continue;
            }
            let tag = self.header[1] & 127;
            let extended = match tag {
                126 => 2,
                127 => 8,
                _ => 0,
            };
            let needed = 2 + extended + if self.header[1] & 128 != 0 { 4 } else { 0 };
            if self.used != needed {
                continue;
            }
            self.payload = match tag {
                126 => u16::from_be_bytes(self.header[2..4].try_into().unwrap()) as u64,
                127 => u64::from_be_bytes(self.header[2..10].try_into().unwrap()),
                n => n as u64,
            };
            if self.payload == 0 {
                self.finish();
            }
        }
    }
    fn finish(&mut self) {
        if self.header[0] & 8 == 0 {
            self.fragmented = self.header[0] & 128 == 0;
        }
        if !self.fragmented {
            self.since = None;
        }
        self.used = 0;
    }
    fn expired(&self) -> bool {
        self.since
            .is_some_and(|s| s.elapsed() >= Duration::from_secs(1))
    }
}
// Every blocking read rechecks the same absolute deadline, including reads
// inside tungstenite's continuation-frame loop (an inactivity timeout cannot).
struct Transport {
    stream: TcpStream,
    deadline: Option<Instant>,
    quota: Option<(usize, usize)>,
    wire: Option<WireProgress>,
    http_matched: usize,
}
impl Read for Transport {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(deadline) = self.deadline {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .filter(|d| !d.is_zero())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::TimedOut, "absolute admission deadline")
                })?;
            self.stream.set_read_timeout(Some(remaining))?;
        }
        let limit = if let Some((bytes, calls)) = &mut self.quota {
            if *bytes == 0 || *calls == 0 {
                return Err(io::ErrorKind::WouldBlock.into());
            }
            *calls -= 1;
            buf.len().min(*bytes)
        } else {
            buf.len()
        };
        // During HTTP upgrade do not read ahead into WebSocket bytes before
        // enabling the framing observer. This path runs only in admission workers.
        let limit = if self.wire.is_none() {
            let n = self.stream.peek(&mut buf[..limit])?;
            let mut matched = self.http_matched;
            let mut end = n;
            for (index, &byte) in buf[..n].iter().enumerate() {
                matched = if byte == b"\r\n\r\n"[matched] {
                    matched + 1
                } else if byte == b'\r' {
                    1
                } else {
                    0
                };
                if matched == 4 {
                    end = index + 1;
                    break;
                }
            }
            end
        } else {
            limit
        };
        let n = self.stream.read(&mut buf[..limit])?;
        if self.wire.is_none() {
            for &byte in &buf[..n] {
                self.http_matched = if byte == b"\r\n\r\n"[self.http_matched] {
                    self.http_matched + 1
                } else if byte == b'\r' {
                    1
                } else {
                    0
                };
            }
        }
        if let Some((bytes, _)) = &mut self.quota {
            *bytes -= n;
        }
        if let Some(wire) = &mut self.wire {
            wire.observe(&buf[..n]);
        }
        Ok(n)
    }
}
impl Write for Transport {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}
struct Peer {
    socket: WebSocket<Transport>,
    name: String,
    sequence: u64,
    last_input: Instant,
    budget: usize,
}
pub struct Server {
    origins: Arc<Vec<String>>,
    listener: TcpListener,
    pending: Arc<AtomicUsize>,
    tx: mpsc::SyncSender<(WebSocket<Transport>, String)>,
    rx: Receiver<(WebSocket<Transport>, String)>,
    peers: [Option<Peer>; 4],
    simulation: Simulation,
    ticks: u64,
    victory_ticks: u32,
}
impl Server {
    pub fn bind(address: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind(address)?;
        // Origin checks complement loopback binding; neither authenticates native clients.
        if !listener.local_addr()?.ip().is_loopback() {
            return Err(std::io::Error::other(
                "development server must bind loopback",
            ));
        }
        listener.set_nonblocking(true)?;
        let origins: Vec<String> = std::env::var("RYGGATTACK_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://127.0.0.1:8000,http://localhost:8000".into())
            .split(',')
            .map(|s| s.trim().to_owned())
            .collect();
        for origin in &origins {
            let valid = origin
                .parse::<tungstenite::http::Uri>()
                .ok()
                .is_some_and(|uri| {
                    matches!(uri.scheme_str(), Some("http" | "https"))
                        && uri
                            .authority()
                            .is_some_and(|a| !a.as_str().contains(['*', '@']))
                        && origin
                            == &format!(
                                "{}://{}",
                                uri.scheme_str().unwrap(),
                                uri.authority().unwrap()
                            )
                });
            if !valid {
                return Err(std::io::Error::other(
                    "expected exact http(s) web origins, without path or wildcard",
                ));
            }
        }
        let (tx, rx) = mpsc::sync_channel(4);
        Ok(Self {
            origins: Arc::new(origins),
            listener,
            pending: Arc::new(AtomicUsize::new(0)),
            tx,
            rx,
            peers: Default::default(),
            simulation: Simulation::new(SimulationConfig {
                human_seats: [true; 4],
                bots: 0,
                ..Default::default()
            })
            .unwrap(),
            ticks: 0,
            victory_ticks: 0,
        })
    }
    pub fn local_addr(&self) -> SocketAddr {
        self.listener.local_addr().unwrap()
    }
    pub fn run(mut self) -> ! {
        let mut deadline = Instant::now();
        loop {
            self.tick();
            deadline += TICK_DURATION;
            let now = Instant::now();
            if now < deadline {
                std::thread::sleep(deadline - now);
            } else if now.duration_since(deadline) > TICK_DURATION * 4 {
                deadline = now;
                eprintln!("tick overrun: discarded wall-clock debt");
            }
        }
    }
    // tungstenite's Callback API fixes ErrorResponse as an unboxed HTTP response.
    #[allow(clippy::result_large_err)]
    pub fn tick(&mut self) {
        // Handshakes have fixed concurrency, queue, and IO deadlines. Never block
        // the authoritative tick on a half-open HTTP/WebSocket handshake.
        for _ in 0..8 {
            let Ok((stream, _)) = self.listener.accept() else {
                break;
            };
            if self.pending.load(Ordering::Relaxed) >= 8 {
                continue;
            }
            self.pending.fetch_add(1, Ordering::Relaxed);
            let count = self.pending.clone();
            let tx = self.tx.clone();
            let origins = self.origins.clone();
            let accepted = Instant::now();
            std::thread::spawn(move || {
                let _ = stream.set_nodelay(true);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
                let config = WebSocketConfig::default()
                    .read_buffer_size(MAX_INPUT_BYTES)
                    .max_message_size(Some(MAX_INPUT_BYTES))
                    .max_frame_size(Some(MAX_INPUT_BYTES))
                    .write_buffer_size(0)
                    .max_write_buffer_size(MAX_STATE_BYTES * 2);
                if let Ok(mut socket) = tungstenite::accept_hdr_with_config(
                    Transport {
                        stream,
                        deadline: Some(accepted + Duration::from_secs(1)),
                        quota: None,
                        wire: None,
                        http_matched: 0,
                    },
                    move |request: &tungstenite::handshake::server::Request, response| {
                        // No Origin is the explicit native-client policy. Browsers must
                        // supply exactly one allowlisted Origin; null is never native.
                        let values: Vec<_> = request.headers().get_all("Origin").iter().collect();
                        if values.is_empty()
                            || (values.len() == 1
                                && values[0]
                                    .to_str()
                                    .ok()
                                    .is_some_and(|s| origins.iter().any(|o| o == s)))
                        {
                            Ok(response)
                        } else {
                            Err(tungstenite::http::Response::builder()
                                .status(403)
                                .body(Some("Origin not allowed".into()))
                                .unwrap())
                        }
                    },
                    Some(config),
                ) {
                    // WASM may spend several seconds creating the first scene
                    // before it can pump Hello. Keep that admission grace bounded,
                    // separately from the shorter HTTP upgrade IO timeout.
                    socket.get_mut().deadline = Some(accepted + Duration::from_secs(10));
                    socket.get_mut().wire = Some(WireProgress::default());
                    // Validate Hello before allocating a seat.
                    let hello = socket.read();
                    if let Ok(Message::Text(text)) = hello {
                        match decode_client(&text) {
                            Ok(ClientMessage::Hello { .. }) => {
                                // Transfer the validated hello and socket through
                                // the bounded admission queue; tick owns seat allocation.
                                socket.get_mut().deadline = None;
                                let _ = socket.get_mut().stream.set_nonblocking(true);
                                let _ = tx.try_send((socket, text.to_string()));
                            }
                            _ => {
                                let _ = socket.send(Message::text(
                                    serde_json::to_string(&ServerMessage::Error(
                                        "invalid hello/version/name".into(),
                                    ))
                                    .unwrap(),
                                ));
                            }
                        }
                    }
                }
                count.fetch_sub(1, Ordering::Relaxed);
            });
        }
        while let Ok((mut socket, text)) = self.rx.try_recv() {
            let Ok(ClientMessage::Hello { name, .. }) = decode_client(&text) else {
                continue;
            };
            if let Some(seat) = self.peers.iter().position(Option::is_none) {
                let welcome = ServerMessage::Welcome {
                    version: VERSION,
                    seat,
                    edges: self.simulation.network_edges(),
                };
                if socket
                    .send(Message::text(serde_json::to_string(&welcome).unwrap()))
                    .is_ok()
                {
                    self.peers[seat] = Some(Peer {
                        socket,
                        name,
                        sequence: 0,
                        last_input: Instant::now(),
                        budget: 0,
                    });
                }
            } else {
                let _ = socket.send(Message::text(
                    serde_json::to_string(&ServerMessage::Error("development match full".into()))
                        .unwrap(),
                ));
            }
        }
        for seat in 0..4 {
            let mut remove = false;
            if let Some(peer) = self.peers[seat].as_mut() {
                peer.budget = 0;
                // Also bound work *inside* read(): empty non-FIN fragments never
                // reach the completed-message budget. At most 1KiB new wire bytes
                // and four nonblocking reads per peer/tick, plus a 1KiB buffer.
                peer.socket.get_mut().quota = Some((MAX_INPUT_BYTES, 4));
                loop {
                    if peer
                        .socket
                        .get_ref()
                        .wire
                        .as_ref()
                        .is_some_and(WireProgress::expired)
                    {
                        remove = true;
                        break;
                    }
                    match peer.socket.read() {
                        Ok(Message::Text(text)) => {
                            peer.budget += 1;
                            let result = decode_client(&text);
                            if peer.budget > 8 || !matches!(result, Ok(ClientMessage::Input { .. }))
                            {
                                let _ = peer.socket.send(Message::text(
                                    serde_json::to_string(&ServerMessage::Error(
                                        "invalid input or rate exceeded".into(),
                                    ))
                                    .unwrap(),
                                ));
                                remove = true;
                                break;
                            }
                            if let Ok(ClientMessage::Input {
                                sequence,
                                horizontal,
                                vertical,
                                fire,
                            }) = result
                                && sequence > peer.sequence
                            {
                                peer.sequence = sequence;
                                peer.last_input = Instant::now();
                                self.simulation
                                    .submit(
                                        seat,
                                        ControlIntent {
                                            horizontal,
                                            vertical,
                                            fire,
                                        },
                                    )
                                    .unwrap();
                            }
                        }
                        Ok(Message::Close(_)) => {
                            remove = true;
                            break;
                        }
                        Ok(Message::Ping(_)) => {
                            if peer.socket.flush().is_err() {
                                remove = true;
                            }
                            break;
                        }
                        Ok(_) => {
                            remove = true;
                            break;
                        }
                        Err(tungstenite::Error::Io(err))
                            if err.kind() == std::io::ErrorKind::WouldBlock =>
                        {
                            break;
                        }
                        Err(_) => {
                            remove = true;
                            break;
                        }
                    }
                }
                if peer
                    .socket
                    .get_ref()
                    .wire
                    .as_ref()
                    .is_some_and(WireProgress::expired)
                {
                    remove = true;
                }
                if peer.last_input.elapsed() > Duration::from_secs(1) {
                    let _ = self.simulation.submit(seat, ControlIntent::default());
                }
            }
            if remove {
                self.peers[seat] = None;
                self.simulation
                    .submit(seat, ControlIntent::default())
                    .unwrap();
            }
        }
        self.simulation.step();
        // Online service policy only: offline keeps its explicit restart UI.
        // Reuse the shared fresh-game reset, retaining seats, topology, tick and
        // monotonic replication IDs rather than constructing another Simulation.
        if self
            .simulation
            .app
            .world()
            .resource::<crate::game::Round>()
            .winner
            .is_some()
        {
            self.victory_ticks += 1;
            if self.victory_ticks >= 3 * crate::simulation::TICK_HZ {
                use bevy::ecs::system::RunSystemOnce;
                self.simulation
                    .app
                    .world_mut()
                    .run_system_once(crate::game::start_new_game)
                    .unwrap();
                self.victory_ticks = 0;
            }
        } else {
            self.victory_ticks = 0;
        }
        self.ticks += 1;
        if self.ticks.is_multiple_of(4) {
            let mut frame = self.simulation.network_frame();
            for seat in 0..4 {
                if let Some(peer) = &self.peers[seat] {
                    frame.ack[seat] = peer.sequence;
                    frame.names[seat] = Some(peer.name.clone());
                }
            }
            let text = serde_json::to_string(&ServerMessage::State(frame)).unwrap();
            assert!(text.len() <= MAX_STATE_BYTES, "replication budget exceeded");
            for seat in 0..4 {
                if let Some(peer) = self.peers[seat].as_mut() {
                    // A slow socket is dropped instead of accumulating snapshots.
                    if peer.socket.send(Message::text(text.clone())).is_err() {
                        self.peers[seat] = None;
                        self.simulation
                            .submit(seat, ControlIntent::default())
                            .unwrap();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wire_progress_preserves_fragment_deadline_across_ping_and_buffered_messages() {
        let mut wire = WireProgress::default();
        wire.observe(&[0x81, 0x80, 0, 0, 0, 0, 0x01]);
        assert!(
            wire.since.is_some(),
            "complete message followed by partial header in same read"
        );
        wire.observe(&[0x80, 0, 0, 0, 0]);
        let started = wire.since.unwrap();
        wire.observe(&[0x89, 0x80, 0, 0, 0, 0]);
        assert_eq!(
            wire.since,
            Some(started),
            "Ping cannot renew incomplete message"
        );
        wire.observe(&[0x80, 0x80, 0, 0, 0, 0]);
        assert!(wire.since.is_none(), "FIN continuation completes message");
        // Fragmented header and extended payload length, no allocation.
        wire.observe(&[0x81, 0xfe, 0, 126, 0, 0]);
        assert!(wire.since.is_some());
        wire.observe(&[0, 0]);
        wire.observe(&[0; 125]);
        assert!(wire.since.is_some());
        wire.observe(&[0]);
        assert!(wire.since.is_none());
        wire.observe(&[0x81, 0xff, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0]);
        assert!(wire.since.is_none());
    }
    #[test]
    fn victory_displays_then_fresh_match_accepts_new_native_client() {
        let mut server = Server::bind("127.0.0.1:0").unwrap();
        {
            let world = server.simulation.app.world_mut();
            world.resource_mut::<crate::game::Round>().wins = [2, 0, 0, 0];
            for mut player in world.query::<&mut crate::players::Player>().iter_mut(world) {
                if player.id != 0 {
                    player.alive = false;
                }
            }
        }
        server.tick();
        assert_eq!(server.simulation.snapshot().winner, Some(0));
        for _ in 0..128 {
            server.tick();
        }
        assert_eq!(
            server.simulation.snapshot().winner,
            Some(0),
            "result must remain visible"
        );
        for _ in 0..128 {
            server.tick();
        }
        let snapshot = server.simulation.snapshot();
        assert_eq!(
            snapshot.winner, None,
            "continuous service froze at first match victory"
        );
        assert_eq!(snapshot.wins, [0; 4]);
        assert!(snapshot.carts.iter().all(|c| c.alive));
        let mut client = crate::net_client::NetworkClient::connect(
            &format!("ws://{}", server.local_addr()),
            "next match",
        )
        .unwrap();
        for _ in 0..300 {
            client.pump(ControlIntent::default());
            server.tick();
            if client.frame.as_ref().is_some_and(|f| f.ack[0] > 0) {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let frame = client.frame.unwrap();
        assert!(frame.tick > snapshot.tick);
        assert_eq!(frame.winner, None);
        assert_eq!(frame.wins, [0; 4]);
        assert!(
            frame.ack[0] > 0,
            "new client must control its assigned seat after restart"
        );
    }
    #[test]
    fn raw_empty_continuation_flood_has_bounded_tick_work() {
        let mut server = Server::bind("127.0.0.1:0").unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut attacker = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (stream, _) = listener.accept().unwrap();
        stream.set_nonblocking(true).unwrap();
        let socket = WebSocket::from_raw_socket(
            Transport {
                stream,
                deadline: None,
                quota: None,
                wire: Some(WireProgress::default()),
                http_matched: 0,
            },
            tungstenite::protocol::Role::Server,
            None,
        );
        server.peers[0] = Some(Peer {
            socket,
            name: "attacker".into(),
            sequence: 0,
            last_input: Instant::now(),
            budget: 0,
        });
        let mut flood = vec![0x01, 0x80, 0, 0, 0, 0];
        for _ in 0..10000 {
            flood.extend_from_slice(&[0, 0x80, 0, 0, 0, 0]);
        }
        attacker.write_all(&flood).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        let start = Instant::now();
        server.tick();
        assert!(
            start.elapsed() < Duration::from_millis(100),
            "fragment flood stalled tick"
        );
        assert_eq!(server.ticks, 1);
        let remaining = server.peers[0]
            .as_ref()
            .unwrap()
            .socket
            .get_ref()
            .stream
            .peek(&mut [0; 1]);
        assert!(
            matches!(remaining, Ok(n) if n > 0),
            "one tick drained arbitrary raw continuation frames: {remaining:?}"
        );
    }
}

//! Localhost-only development service: one continuously running four-seat match.
//! No public auth, rooms, reconnect, deployment, or asset-free server claim.
use crate::{
    headless::{Simulation, SimulationConfig, TICK_DURATION},
    input::ControlIntent,
    network::*,
};
use std::{
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};
use tungstenite::{Message, WebSocket, protocol::WebSocketConfig};
struct Peer {
    socket: WebSocket<TcpStream>,
    name: String,
    sequence: u64,
    last_input: Instant,
    budget: usize,
}
pub struct Server {
    listener: TcpListener,
    pending: Arc<AtomicUsize>,
    tx: mpsc::SyncSender<(WebSocket<TcpStream>, String)>,
    rx: Receiver<(WebSocket<TcpStream>, String)>,
    peers: [Option<Peer>; 4],
    simulation: Simulation,
    ticks: u64,
}
impl Server {
    pub fn bind(address: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind(address)?;
        // This milestone has no origin/auth policy; never accidentally expose it.
        if !listener.local_addr()?.ip().is_loopback() {
            return Err(std::io::Error::other(
                "development server must bind loopback",
            ));
        }
        listener.set_nonblocking(true)?;
        let (tx, rx) = mpsc::sync_channel(4);
        Ok(Self {
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
            std::thread::spawn(move || {
                let _ = stream.set_nodelay(true);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
                let config = WebSocketConfig::default()
                    .max_message_size(Some(MAX_INPUT_BYTES))
                    .max_frame_size(Some(MAX_INPUT_BYTES))
                    .write_buffer_size(0)
                    .max_write_buffer_size(MAX_STATE_BYTES * 2);
                if let Ok(mut socket) = tungstenite::accept_with_config(stream, Some(config)) {
                    // WASM may spend several seconds creating the first scene
                    // before it can pump Hello. Keep that admission grace bounded,
                    // separately from the shorter HTTP upgrade IO timeout.
                    let _ = socket
                        .get_mut()
                        .set_read_timeout(Some(Duration::from_secs(10)));
                    // Validate Hello before allocating a seat.
                    let hello = socket.read();
                    if let Ok(Message::Text(text)) = hello {
                        match decode_client(&text) {
                            Ok(ClientMessage::Hello { .. }) => {
                                // Transfer the validated hello and socket through
                                // the bounded admission queue; tick owns seat allocation.
                                let _ = socket.get_mut().set_nonblocking(true);
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
                loop {
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

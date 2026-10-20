//! Shared native/browser adapter. Latest-state mailbox and stop-and-wait input
//! bound queues even if rendering stalls. No client simulation or hit claims.
use crate::{input::ControlIntent, network::*};
use std::{
    ops::ControlFlow,
    sync::{Arc, Mutex},
};
use web_time::{Duration, Instant};
#[derive(Default)]
struct Inbox {
    opened: bool,
    welcome: Option<(usize, Vec<(usize, usize)>)>,
    frame: Option<Frame>,
    error: Option<String>,
}
pub struct NetworkClient {
    sender: Option<ewebsock::WsSender>,
    address: String,
    inbox: Arc<Mutex<Inbox>>,
    name: String,
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
        decode_client(
            &serde_json::to_string(&ClientMessage::Hello {
                version: VERSION,
                name: name.into(),
            })
            .unwrap(),
        )
        .map_err(str::to_owned)?;
        let inbox = Arc::new(Mutex::new(Inbox::default()));
        let now = Instant::now();
        Ok(Self {
            sender: None,
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
                            }) if version == VERSION
                                && seat < 4
                                && edges.len() <= 40
                                && edges.iter().all(|&(a, b)| a < 25 && b < 25) =>
                            {
                                inbox.welcome = Some((seat, edges))
                            }
                            Some(ServerMessage::State(frame))
                                if frame.carts.len() == 4 && frame.bodies.len() <= 1024 =>
                            {
                                inbox.frame = Some(frame)
                            }
                            Some(ServerMessage::Error(err)) => inbox.error = Some(err),
                            _ => inbox.error = Some("Invalid server protocol".into()),
                        }
                    }
                    ewebsock::WsEvent::Error(_) => {
                        inbox.error = Some("Connection failed; check backend address".into())
                    }
                    ewebsock::WsEvent::Closed => {
                        inbox.error =
                            Some("Disconnected; reload/relaunch to join a free seat".into())
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
                })
                .unwrap(),
            ));
            self.status = "Waiting for seat…".into();
        }
        if let Some((seat, edges)) = inbox.welcome {
            self.seat = Some(seat);
            self.edges = Some(edges);
            self.status = format!("Online — seat {}", seat + 1);
        }
        if let Some(frame) = inbox.frame {
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
            self.status = "Server timeout; reload/relaunch to reconnect".into();
            return false;
        }
        if self.seat.is_some()
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

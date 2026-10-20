//! Private management listener. Never route this port through the public ingress.
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Status {
    pub tick_at: Option<Instant>,
    pub ticks: u64,
    pub rooms: usize,
    pub players: usize,
    pub connections: usize,
    pub pending: usize,
    pub overruns: u64,
    pub rejected: u64,
}
impl Status {
    pub fn response(&self, route: &str, stopping: bool) -> (u16, String) {
        let healthy = !stopping
            && self
                .tick_at
                .is_some_and(|t| t.elapsed() < Duration::from_secs(2));
        match route {
            "/healthz" | "/readyz" => (
                if healthy { 200 } else { 503 },
                if healthy { "ok\n" } else { "not ready\n" }.into(),
            ),
            "/metrics" => (
                200,
                format!(
                    "ryggattack_rooms {}\nryggattack_players {}\nryggattack_connections {}\nryggattack_pending_admissions {}\nryggattack_ticks_total {}\nryggattack_tick_overruns_total {}\nryggattack_rate_rejections_total {}\n",
                    self.rooms,
                    self.players,
                    self.connections,
                    self.pending,
                    self.ticks,
                    self.overruns,
                    self.rejected
                ),
            ),
            _ => (404, "not found\n".into()),
        }
    }
}
pub fn serve(
    address: &str,
    status: Arc<Mutex<Status>>,
    stop: &'static AtomicBool,
) -> std::io::Result<(SocketAddr, std::thread::JoinHandle<()>)> {
    let listener = TcpListener::bind(address)?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let thread = std::thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(20));
                continue;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
            let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
            // Absolute deadline and fixed total allocation, no keep-alive, no per-request thread.
            let until = Instant::now() + Duration::from_millis(100);
            let mut request = [0u8; 1024];
            let mut used = 0;
            while used < request.len() && Instant::now() < until {
                let _ = stream.set_read_timeout(Some(
                    until
                        .saturating_duration_since(Instant::now())
                        .max(Duration::from_millis(1)),
                ));
                match stream.read(&mut request[used..]) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => used += n,
                }
                if request[..used].windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let text = std::str::from_utf8(&request[..used]).unwrap_or("");
            let route = text
                .lines()
                .next()
                .and_then(|s| s.strip_prefix("GET "))
                .and_then(|s| s.strip_suffix(" HTTP/1.1"))
                .unwrap_or("");
            let (code, body) = status
                .lock()
                .unwrap()
                .response(route, stop.load(Ordering::Relaxed));
            let reason = match code {
                200 => "OK",
                503 => "Service Unavailable",
                _ => "Not Found",
            };
            let _ = write!(
                stream,
                "HTTP/1.1 {code} {reason}\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    Ok((address, thread))
}

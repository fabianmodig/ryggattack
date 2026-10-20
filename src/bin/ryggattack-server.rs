#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let address = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8787".into());
    let server =
        ryggattack::server::Server::bind(&address).expect("bind loopback development service");
    println!(
        "Ryggattack development WebSocket service ws://{} (private rooms; no auth/reconnect)",
        server.local_addr()
    );
    server.run();
}
#[cfg(target_arch = "wasm32")]
fn main() {}

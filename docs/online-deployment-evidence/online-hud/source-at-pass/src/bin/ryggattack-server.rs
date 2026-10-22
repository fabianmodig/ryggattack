#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::io::Result<()> {
    use std::sync::atomic::{AtomicBool, Ordering};
    static STOP: AtomicBool = AtomicBool::new(false);
    extern "C" fn stop(_: libc::c_int) {
        STOP.store(true, Ordering::Relaxed);
    }
    // The signal handler only sets a lock-free flag; IO/cleanup runs on normal threads.
    unsafe {
        if libc::signal(libc::SIGTERM, stop as *const () as libc::sighandler_t) == libc::SIG_ERR
            || libc::signal(libc::SIGINT, stop as *const () as libc::sighandler_t) == libc::SIG_ERR
        {
            return Err(std::io::Error::last_os_error());
        }
    }
    let production = match std::env::var("RYGGATTACK_MODE").as_deref() {
        Err(_) | Ok("development") => false,
        Ok("production") => true,
        _ => return Err(std::io::Error::other("invalid RYGGATTACK_MODE")),
    };
    let address = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8787".into());
    let server = if production {
        ryggattack::server::Server::bind_production(&address)?
    } else {
        ryggattack::server::Server::bind(&address)?
    };
    let management_address =
        std::env::var("RYGGATTACK_MANAGEMENT_BIND").unwrap_or_else(|_| "127.0.0.1:0".into());
    let (management, thread) =
        ryggattack::management::serve(&management_address, server.status(), &STOP)?;
    println!(
        "{}",
        serde_json::json!({"event":"listening","mode":if production {"production"} else {"development"},"websocket":format!("ws://{}", server.local_addr()),"management":management.to_string(),"service":"private rooms; secure reconnect; no accounts"})
    );
    server.run_until(&STOP);
    thread
        .join()
        .map_err(|_| std::io::Error::other("management thread failed"))?;
    Ok(())
}
#[cfg(target_arch = "wasm32")]
fn main() {}

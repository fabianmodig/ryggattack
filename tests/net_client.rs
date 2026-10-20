#![cfg(not(target_arch = "wasm32"))]
use ryggattack::{net_client::NetworkClient, network::*};
use std::{
    net::TcpListener,
    time::{Duration, Instant},
};
use tungstenite::Message;

fn grid() -> Vec<(usize, usize)> {
    let mut edges = Vec::new();
    for n in 0..25 {
        if n % 5 < 4 {
            edges.push((n, n + 1));
        }
        if n < 20 {
            edges.push((n, n + 5));
        }
    }
    edges
}
fn backend(messages: Vec<ServerMessage>) -> NetworkClient {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("ws://{}", listener.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut ws = tungstenite::accept(stream).unwrap();
        let _ = ws.read().unwrap();
        for message in messages {
            let _ = ws.send(Message::text(serde_json::to_string(&message).unwrap()));
        }
        std::thread::sleep(Duration::from_millis(200));
    });
    let mut client = NetworkClient::connect(&address, "validation").unwrap();
    let deadline = Instant::now() + Duration::from_millis(150);
    while Instant::now() < deadline {
        client.pump(Default::default());
        if client.status == "Invalid server protocol" {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    thread.join().unwrap();
    client
}
fn welcome(edges: Vec<(usize, usize)>) -> ServerMessage {
    ServerMessage::Welcome {
        version: VERSION,
        seat: 0,
        edges,
    }
}
#[test]
fn malicious_backend_states_are_rejected_and_cart_order_is_canonical() {
    let mut sim =
        ryggattack::headless::Simulation::new(ryggattack::headless::SimulationConfig::default())
            .unwrap();
    let good = sim.network_frame();
    type Mutation = (&'static str, Box<dyn Fn(&mut Frame)>);
    let cases: Vec<Mutation> = vec![
        ("duplicate carts", Box::new(|f| f.carts[1].id = 0)),
        ("cart range", Box::new(|f| f.carts[3].id = 4)),
        (
            "duplicate bodies",
            Box::new(|f| f.bodies.push(f.bodies[0].clone())),
        ),
        ("zero body ID", Box::new(|f| f.bodies[0].id = 0)),
        ("unsupported kind", Box::new(|f| f.bodies[0].kind = 5)),
        (
            "unbounded cart position",
            Box::new(|f| f.carts[0].position[0] = 1e30),
        ),
        (
            "invalid forward",
            Box::new(|f| f.carts[0].forward = [0.; 3]),
        ),
        (
            "unbounded cooldown",
            Box::new(|f| f.carts[0].shot_cooldown = 1e30),
        ),
        (
            "negative buff",
            Box::new(|f| f.carts[0].speed_seconds = -1.),
        ),
        (
            "invalid rotation",
            Box::new(|f| f.bodies[0].rotation = [0.; 4]),
        ),
        ("negative scale", Box::new(|f| f.bodies[0].scale[0] = -1.)),
        (
            "unbounded body position",
            Box::new(|f| f.bodies[0].position[0] = 1e30),
        ),
        ("invalid winner", Box::new(|f| f.winner = Some(4))),
        ("invalid score", Box::new(|f| f.wins[0] = u32::MAX)),
        ("invalid timer", Box::new(|f| f.until_drop = -1.)),
        (
            "unbounded name",
            Box::new(|f| f.names[0] = Some("x".repeat(1000))),
        ),
    ];
    for (name, mutate) in cases {
        let mut frame = good.clone();
        mutate(&mut frame);
        let client = backend(vec![welcome(grid()), ServerMessage::State(frame)]);
        assert_eq!(client.status, "Invalid server protocol", "accepted {name}");
        assert!(
            client.frame.is_none(),
            "invalid state reached renderer: {name}"
        );
    }
    let mut shuffled = good.clone();
    shuffled.carts.reverse();
    let client = backend(vec![welcome(grid()), ServerMessage::State(shuffled)]);
    assert_eq!(
        client
            .frame
            .unwrap()
            .carts
            .iter()
            .map(|c| c.id)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
}

#[test]
fn malicious_backend_topologies_fail_before_renderer() {
    let valid = grid();
    let mut duplicate = valid.clone();
    duplicate[0] = (6, 5); // duplicate of (5, 6), without exceeding edge-count cap
    let mut self_edge = valid.clone();
    self_edge[0] = (0, 0);
    let mut wrap = valid.clone();
    wrap[0] = (4, 5);
    let mut out_of_range = valid.clone();
    out_of_range[0] = (0, 25);
    let isolated = valid
        .iter()
        .copied()
        .filter(|&(a, b)| a != 0 && b != 0)
        .collect();
    let disconnected = valid
        .iter()
        .copied()
        .filter(|&(a, b)| !(a < 10 && b >= 10))
        .collect();
    for (name, edges) in [
        ("empty", vec![]),
        ("duplicate", duplicate),
        ("self", self_edge),
        ("row wrap", wrap),
        ("range", out_of_range),
        ("isolated start", isolated),
        ("disconnected", disconnected),
    ] {
        let client = backend(vec![welcome(edges)]);
        assert_eq!(client.status, "Invalid server protocol", "accepted {name}");
        assert!(client.edges.is_none(), "invalid topology reached renderer");
    }
    let client = backend(vec![welcome(valid)]);
    assert_eq!(client.seat, Some(0));
    assert!(client.edges.is_some());
}

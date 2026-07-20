use std::net::UdpSocket;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let event = args.get(1).map(|s| s.as_str()).unwrap_or("help");
    let port: u16 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(12345);

    match event {
        "grab" | "drop" => {
            send_gesture(event, port);
        }
        "sequence" => {
            println!("[Sim] Sending GRAB then DROP with 2s delay...\n");
            send_gesture("Grab", port);
            std::thread::sleep(std::time::Duration::from_secs(2));
            send_gesture("Drop", port);
        }
        _ => {
            println!("PalmSync Gesture Simulator");
            println!("==========================");
            println!();
            println!("Usage: gesture_sim <command> [port]");
            println!();
            println!("Commands:");
            println!("  grab       Send a GRAB event");
            println!("  drop       Send a DROP event");
            println!("  sequence   Send GRAB then DROP (2s delay)");
            println!();
            println!("Port defaults to 12345");
            println!();
            println!("Examples:");
            println!("  gesture_sim grab");
            println!("  gesture_sim drop 12345");
            println!("  gesture_sim sequence");
        }
    }
}

fn send_gesture(event: &str, port: u16) {
    let event_name = match event {
        "grab" => "Grab",
        "drop" => "Drop",
        other => other,
    };

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let json = format!(
        r#"{{"event":"{}","timestamp_ms":{},"confidence":0.95}}"#,
        event_name, timestamp
    );

    let dest = format!("127.0.0.1:{}", port);
    let socket = UdpSocket::bind("127.0.0.1:0").expect("failed to bind UDP socket");

    match socket.send_to(json.as_bytes(), &dest) {
        Ok(n) => {
            println!("[Sim] Sent {} event ({} bytes) -> {}", event_name, n, dest);
            println!("[Sim] Payload: {}", json);
        }
        Err(e) => {
            eprintln!("[Sim] Failed to send: {}", e);
        }
    }
}

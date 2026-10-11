use palmsync_shared::{DeviceReport, GesturePacket};
use serde::Deserialize;
use tokio::net::UdpSocket;

/// What arrives on the local UDP port (127.0.0.1 only).
pub enum IpcMessage {
    /// GRAB / DROP from the camera (Eye) or the UI buttons.
    Gesture(GesturePacket),
    /// A problem the UI's system check found: `{"report": {...}}`.
    Report(DeviceReport),
}

#[derive(Deserialize)]
struct ReportPacket {
    report: DeviceReport,
}

pub struct IpcListener {
    socket: UdpSocket,
}

impl IpcListener {
    pub async fn bind(port: u16) -> anyhow::Result<Self> {
        let addr = format!("127.0.0.1:{}", port);
        let socket = UdpSocket::bind(&addr).await?;
        log::info!("IPC listener bound to {}", addr);
        Ok(Self { socket })
    }

    pub async fn recv(&self) -> anyhow::Result<IpcMessage> {
        let mut buf = vec![0u8; 16 * 1024];
        let n = self.socket.recv(&mut buf).await?;
        parse(std::str::from_utf8(&buf[..n])?)
    }
}

fn parse(text: &str) -> anyhow::Result<IpcMessage> {
    if let Ok(p) = serde_json::from_str::<ReportPacket>(text) {
        return Ok(IpcMessage::Report(p.report));
    }
    Ok(IpcMessage::Gesture(serde_json::from_str(text)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gestures_and_reports() {
        let g = parse(r#"{"event":"Grab","timestamp_ms":1,"confidence":0.9}"#).unwrap();
        assert!(matches!(g, IpcMessage::Gesture(_)));

        let r = parse(
            r#"{"report":{"machine":"PC2","level":"warn","source":"sistem","message":"Kamera izni kapalı","timestamp":5}}"#,
        )
        .unwrap();
        match r {
            IpcMessage::Report(r) => assert_eq!(r.message, "Kamera izni kapalı"),
            _ => panic!("expected report"),
        }

        assert!(parse("garbage").is_err());
    }
}

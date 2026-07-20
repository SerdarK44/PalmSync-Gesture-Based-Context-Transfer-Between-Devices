use palmsync_shared::GesturePacket;
use tokio::net::UdpSocket;

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

    pub async fn recv_gesture(&self) -> anyhow::Result<GesturePacket> {
        let mut buf = vec![0u8; 4096];
        let n = self.socket.recv(&mut buf).await?;
        let text = std::str::from_utf8(&buf[..n])?;
        let packet: GesturePacket = serde_json::from_str(text)?;
        Ok(packet)
    }
}

using System.Net;
using System.Net.Sockets;
using System.Text;

namespace PalmSync.Tray.Services;

/// Sends GRAB/DROP gesture packets to the local daemon over UDP — the same
/// wire format the C++ Eye produces. Lets the user capture/restore from the
/// UI without a camera or command line.
public static class GestureSender
{
    public static void Send(string eventName, ushort ipcPort)
    {
        // Rust GestureEvent enum expects "Grab" / "Drop"
        var normalized = eventName.Equals("drop", StringComparison.OrdinalIgnoreCase)
            ? "Drop" : "Grab";

        var ts = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        var json = $"{{\"event\":\"{normalized}\",\"timestamp_ms\":{ts},\"confidence\":0.95}}";

        using var udp = new UdpClient();
        var bytes = Encoding.UTF8.GetBytes(json);
        udp.Send(bytes, bytes.Length, new IPEndPoint(IPAddress.Loopback, ipcPort));
    }
}

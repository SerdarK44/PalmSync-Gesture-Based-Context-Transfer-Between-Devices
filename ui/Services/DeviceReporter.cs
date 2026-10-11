using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Text.Json;

namespace PalmSync.Tray.Services;

/// Sends a problem report to the local daemon (UDP, same port as gestures).
/// The daemon forwards it to the HOST when this computer joined a session —
/// so the host's `cihaz-raporlari.log` collects every machine's problems —
/// or writes it there directly when this computer is the host. Reports sent
/// before the connection is up are queued by the daemon.
public static class DeviceReporter
{
    public static void Send(string level, string source, string message)
    {
        try
        {
            if (!DaemonManager.IsRunning) return;
            var packet = new
            {
                report = new
                {
                    machine = Environment.MachineName,
                    level,           // "error" | "warn" | "info"
                    source,          // "sistem" | "kamera" | "arayuz"
                    message = message.Length > 1900 ? message[..1900] + "…" : message,
                    timestamp = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(),
                },
            };
            var bytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(packet));
            using var udp = new UdpClient();
            udp.Send(bytes, bytes.Length, new IPEndPoint(IPAddress.Loopback, SessionController.CurrentIpcPort()));
        }
        catch (Exception ex)
        {
            AppLog.Debug("Rapor daemon'a iletilemedi: " + ex.Message);
        }
    }
}

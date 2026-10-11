using System.Net;
using System.Net.NetworkInformation;
using System.Net.Sockets;

namespace PalmSync.Tray.Services;

public static class NetworkInfo
{
    // Adapters that look physical but are actually virtual / tunnels.
    // Embedding one of these IPs in the session code makes the host
    // unreachable from the real LAN, so they're excluded.
    private static readonly string[] VirtualHints =
    {
        "virtual", "vmware", "virtualbox", "vbox", "hyper-v", "vethernet",
        "loopback", "wsl", "tap", "tunnel", "tailscale", "zerotier", "vpn",
        "docker", "bluetooth", "pseudo", "npcap", "wan miniport",
    };

    /// This machine's LAN IPv4 address, chosen to be reachable by another
    /// computer on the same network. Returns null if not connected to a LAN.
    public static IPAddress? GetLanIp()
    {
        var candidates = GetCandidates();
        if (candidates.Count == 0) return null;
        if (candidates.Count == 1) return candidates[0];

        // Multiple real adapters (e.g. Wi-Fi + Ethernet both up): pick the one
        // the OS actually routes outbound traffic through.
        var routed = GetRoutedIp();
        if (routed != null && candidates.Any(c => c.Equals(routed)))
            return routed;

        return candidates[0];
    }

    /// All plausible LAN IPv4 addresses (for diagnostics / multi-NIC hosts).
    public static List<IPAddress> GetCandidates()
    {
        var withGateway = new List<IPAddress>();
        var withoutGateway = new List<IPAddress>();

        foreach (var ni in NetworkInterface.GetAllNetworkInterfaces())
        {
            if (ni.OperationalStatus != OperationalStatus.Up) continue;
            if (ni.NetworkInterfaceType is not (NetworkInterfaceType.Ethernet
                or NetworkInterfaceType.Wireless80211)) continue;
            if (IsVirtual(ni)) continue;

            var props = ni.GetIPProperties();
            var hasGateway = props.GatewayAddresses.Any(g =>
                g.Address.AddressFamily == AddressFamily.InterNetwork
                && !g.Address.Equals(IPAddress.Any));

            foreach (var ua in props.UnicastAddresses)
            {
                if (ua.Address.AddressFamily != AddressFamily.InterNetwork) continue;
                if (IPAddress.IsLoopback(ua.Address)) continue;
                if (!IsPrivate(ua.Address)) continue;

                if (hasGateway) withGateway.Add(ua.Address);
                else withoutGateway.Add(ua.Address);
            }
        }

        // Adapters with a gateway are the ones actually on the LAN → first.
        withGateway.AddRange(withoutGateway);
        return withGateway;
    }

    private static bool IsVirtual(NetworkInterface ni)
    {
        var text = (ni.Description + " " + ni.Name).ToLowerInvariant();
        return VirtualHints.Any(h => text.Contains(h));
    }

    /// The local IP the OS would use to reach the internet — reflects the real
    /// routing decision. No packet is actually sent (UDP connect just sets the
    /// route). Falls back to null on failure.
    private static IPAddress? GetRoutedIp()
    {
        try
        {
            using var socket = new Socket(AddressFamily.InterNetwork,
                SocketType.Dgram, ProtocolType.Udp);
            socket.Connect("8.8.8.8", 65530);
            return (socket.LocalEndPoint as IPEndPoint)?.Address;
        }
        catch
        {
            return null;
        }
    }

    /// TCP reachability probe with a timeout — used to tell "host down /
    /// firewalled / wrong network" apart from "connected but wrong password".
    public static bool CanReach(string host, int port, int timeoutMs = 2500)
    {
        try
        {
            using var client = new System.Net.Sockets.TcpClient();
            var task = client.ConnectAsync(host, port);
            return task.Wait(timeoutMs) && client.Connected;
        }
        catch
        {
            return false;
        }
    }

    private static bool IsPrivate(IPAddress addr)
    {
        var b = addr.GetAddressBytes();
        return b[0] switch
        {
            10 => true,
            172 => b[1] >= 16 && b[1] <= 31,
            192 => b[1] == 168,
            _ => false,
        };
    }
}

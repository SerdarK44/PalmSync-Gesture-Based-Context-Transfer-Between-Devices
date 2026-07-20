using System.IO;
using System.Net;

namespace PalmSync.Tray.Services;

/// High-level session operations shared by the Home UI: start hosting,
/// join a session, and stop. Writes config then (re)launches the daemon.
public static class SessionController
{
    private const ushort DefaultPort = 9001;

    public record HostInfo(string Code, string Password, string Ip);

    /// Begin hosting. Detects the LAN IP, generates a password, writes config
    /// as master, and launches the daemon. Returns the shareable code+password.
    public static HostInfo StartHosting()
    {
        var ip = NetworkInfo.GetLanIp()
            ?? throw new InvalidOperationException(
                "No local network connection found. Connect to Wi-Fi or Ethernet and try again.");

        var password = GeneratePassword();
        var code = SessionCode.Encode(ip, DefaultPort);

        var cfg = ConfigManager.Load();
        cfg.Role = "master";
        cfg.MasterAddr = ip.ToString();
        cfg.MasterPort = DefaultPort;
        cfg.Password = password;
        ConfigManager.Save(cfg);

        DaemonManager.Restart();
        return new HostInfo(code, password, ip.ToString());
    }

    /// Join a hosted session from a code + password.
    public static void Join(string code, string password)
    {
        if (!SessionCode.TryDecode(code, out var ip, out var port))
            throw new ArgumentException("That code doesn't look right. Check it and try again.");

        if (string.IsNullOrWhiteSpace(password))
            throw new ArgumentException("Please enter the password.");

        var cfg = ConfigManager.Load();
        cfg.Role = "slave";
        cfg.MasterAddr = ip.ToString();
        cfg.MasterPort = port;
        cfg.Password = password.Trim();
        ConfigManager.Save(cfg);

        DaemonManager.Restart();
    }

    /// Resume a previously saved session (used at login). Only acts if a real
    /// session was saved to disk; never launches a phantom daemon on a fresh
    /// install. Swallows launch errors so the tray always comes up.
    public static bool ResumeIfConfigured()
    {
        if (!File.Exists(ConfigManager.ConfigPath)) return false;

        var cfg = ConfigManager.Load();
        if (cfg.Role != "master" && cfg.Role != "slave") return false;
        if (string.IsNullOrEmpty(cfg.Password)) return false;

        try
        {
            DaemonManager.Restart();
            return true;
        }
        catch
        {
            return false;
        }
    }

    /// If currently hosting, rebuild the shareable code from saved config.
    public static HostInfo? CurrentHostInfo()
    {
        var cfg = ConfigManager.Load();
        if (cfg.Role != "master") return null;
        if (!IPAddress.TryParse(cfg.MasterAddr, out var ip)) return null;
        return new HostInfo(SessionCode.Encode(ip, cfg.MasterPort), cfg.Password, ip.ToString());
    }

    public static string CurrentRole() => ConfigManager.Load().Role;

    public static ushort CurrentIpcPort() => ConfigManager.Load().IpcPort;

    public static void Stop()
    {
        DaemonManager.Stop();
        var cfg = ConfigManager.Load();
        cfg.Role = "off";
        ConfigManager.Save(cfg);
    }

    // 8 chars, unambiguous alphabet (no 0/O/1/I/L)
    private static string GeneratePassword()
    {
        const string alphabet = "ABCDEFGHJKMNPQRSTUVWXYZ23456789";
        var bytes = new byte[8];
        System.Security.Cryptography.RandomNumberGenerator.Fill(bytes);
        return new string(bytes.Select(b => alphabet[b % alphabet.Length]).ToArray());
    }
}

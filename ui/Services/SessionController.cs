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
                "Yerel ağ bağlantısı bulunamadı. Wi-Fi'ya ya da kabloya bağlanıp tekrar dene.");

        var password = GeneratePassword();
        var code = SessionCode.Encode(ip, DefaultPort);

        var cfg = ConfigManager.Load();
        cfg.Role = "master";
        cfg.MasterAddr = ip.ToString();
        cfg.MasterPort = DefaultPort;
        cfg.Password = password;
        ConfigManager.Save(cfg);

        DaemonManager.Restart();

        // Make sure inbound connections can reach us (self-heals a missing
        // installer rule; one UAC prompt only if the rule doesn't exist yet).
        try { FirewallHelper.EnsureRule(DaemonManager.DaemonPath); } catch { }

        AppLog.Info($"🖥 Oturum başlatıldı (bu bilgisayar: {ip}, kod: {code})");
        return new HostInfo(code, password, ip.ToString());
    }

    public record JoinTarget(string Ip, ushort Port);

    /// Join a hosted session from a code + password. Returns the decoded target
    /// so the caller can probe reachability for a clear error message.
    public static JoinTarget Join(string code, string password)
    {
        if (!SessionCode.TryDecode(code, out var ip, out var port))
            throw new ArgumentException("Bu kod geçerli görünmüyor. Kontrol edip tekrar dene.");

        if (string.IsNullOrWhiteSpace(password))
            throw new ArgumentException("Lütfen şifreyi gir.");

        var cfg = ConfigManager.Load();
        cfg.Role = "slave";
        cfg.MasterAddr = ip.ToString();
        cfg.MasterPort = port;
        cfg.Password = password.Trim();
        ConfigManager.Save(cfg);

        DaemonManager.Restart();
        AppLog.Info($"🖥 Oturuma katılınıyor ({ip}:{port})");
        return new JoinTarget(ip.ToString(), port);
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
            AppLog.Info($"Önceki oturum sürdürüldü (rol: {cfg.Role})");
            return true;
        }
        catch (Exception ex)
        {
            AppLog.Error("Önceki oturum sürdürülemedi", ex);
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
        AppLog.Info("⏹ Oturum sonlandırıldı");
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

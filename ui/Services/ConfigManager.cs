using System.IO;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace PalmSync.Tray.Services;

// Field names must match the Rust serde serialization in shared/src/config.rs
public class PalmSyncConfig
{
    [JsonPropertyName("node_id")]
    public string NodeId { get; set; } = MakeNodeId();

    // node_id must be non-empty and only [A-Za-z0-9_-] (the daemon validates
    // incoming ids and rejects anything else). Machine names can contain
    // spaces or non-ASCII (e.g. Turkish) chars, so sanitize + add a short
    // random suffix for uniqueness.
    private static string MakeNodeId()
    {
        var raw = Environment.MachineName;
        var sb = new System.Text.StringBuilder();
        foreach (var c in raw)
            if (char.IsAsciiLetterOrDigit(c) || c is '-' or '_') sb.Append(c);
        var name = sb.Length > 0 ? sb.ToString() : "pc";
        return $"{name}-{Guid.NewGuid():N}"[..Math.Min(40, name.Length + 9)];
    }

    // "off" until the user starts or joins a session
    [JsonPropertyName("role")]
    public string Role { get; set; } = "off";

    [JsonPropertyName("master_addr")]
    public string MasterAddr { get; set; } = "127.0.0.1";

    [JsonPropertyName("master_port")]
    public ushort MasterPort { get; set; } = 9001;

    [JsonPropertyName("password")]
    public string Password { get; set; } = "palmsync";

    [JsonPropertyName("ipc_port")]
    public ushort IpcPort { get; set; } = 12345;

    [JsonPropertyName("log_level")]
    public string LogLevel { get; set; } = "info";

    [JsonPropertyName("log_dir")]
    public string LogDir { get; set; } = AppPaths.DefaultLogDir;

    /// Documents\PalmSync — the daemon writes received files under it.
    [JsonPropertyName("data_dir")]
    public string DataDir { get; set; } = AppPaths.DataDir;

    /// Verbose logs everywhere + the full system-check report goes to the host.
    [JsonPropertyName("debug_mode")]
    public bool DebugMode { get; set; }

    [JsonPropertyName("log_max_size_mb")]
    public ulong LogMaxSizeMb { get; set; } = 10;

    [JsonPropertyName("enable_notifications")]
    public bool EnableNotifications { get; set; } = true;
}

public static class ConfigManager
{
    private static readonly JsonSerializerOptions Options = new()
    {
        WriteIndented = true,
    };

    public static string ConfigDir =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "PalmSync");

    public static string ConfigPath => Path.Combine(ConfigDir, "config.json");

    public static PalmSyncConfig Load()
    {
        try
        {
            if (File.Exists(ConfigPath))
            {
                var json = File.ReadAllText(ConfigPath);
                return JsonSerializer.Deserialize<PalmSyncConfig>(json) ?? new PalmSyncConfig();
            }
        }
        catch
        {
            // fall through to defaults
        }
        return new PalmSyncConfig();
    }

    public static void Save(PalmSyncConfig config)
    {
        Directory.CreateDirectory(ConfigDir);
        var json = JsonSerializer.Serialize(config, Options);
        File.WriteAllText(ConfigPath, json);
    }
}

using System.IO;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace PalmSync.Tray.Services;

// Field names must match the Rust serde serialization in shared/src/config.rs
public class PalmSyncConfig
{
    [JsonPropertyName("node_id")]
    public string NodeId { get; set; } = Environment.MachineName;

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
    public string LogDir { get; set; } =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "PalmSync", "logs");

    [JsonPropertyName("log_max_size_mb")]
    public ulong LogMaxSizeMb { get; set; } = 50;

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

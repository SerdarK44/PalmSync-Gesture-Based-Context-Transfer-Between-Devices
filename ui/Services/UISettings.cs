using System.IO;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace PalmSync.Tray.Services;

/// UI-only preferences (the daemon never reads these).
public class UISettings
{
    /// "system" (follow Windows), "light" or "dark".
    [JsonPropertyName("theme")]
    public string Theme { get; set; } = "system";

    /// The first-run "Sistem kontrolü ve izinler" setup was completed.
    [JsonPropertyName("setup_done")]
    public bool SetupDone { get; set; }

    /// "ask" (show "new version — install"), "auto" (install by itself) or "off".
    [JsonPropertyName("update_mode")]
    public string UpdateMode { get; set; } = "ask";

    /// A version the user chose to skip ("Bu sürümü atla").
    [JsonPropertyName("skipped_version")]
    public string? SkippedVersion { get; set; }
}

public static class UISettingsManager
{
    private static readonly JsonSerializerOptions Options = new() { WriteIndented = true };

    private static string SettingsPath =>
        Path.Combine(ConfigManager.ConfigDir, "ui-settings.json");

    public static UISettings Load()
    {
        try
        {
            if (File.Exists(SettingsPath))
                return JsonSerializer.Deserialize<UISettings>(File.ReadAllText(SettingsPath)) ?? new();
        }
        catch { }
        return new();
    }

    public static void Save(UISettings s)
    {
        Directory.CreateDirectory(ConfigManager.ConfigDir);
        File.WriteAllText(SettingsPath, JsonSerializer.Serialize(s, Options));
    }
}

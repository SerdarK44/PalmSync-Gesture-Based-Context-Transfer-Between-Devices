using System.IO;
using System.Text;

namespace PalmSync.Tray.Services;

/// The user-visible folder layout under Documents:
///
///   Belgeler\PalmSync\
///     Loglar\                    all log files (daemon, kamera, arayüz, güvenlik, cihaz raporları)
///       sistem-kontrolu\         one report per system check
///       BENİOKU.txt              what each file is
///     Gelen Dosyalar\            files received from the other computer
///
/// Logs used to live hidden in %LOCALAPPDATA%\PalmSync\logs.
public static class AppPaths
{
    public static string DataDir =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments), "PalmSync");

    public static string DefaultLogDir => Path.Combine(DataDir, "Loglar");
    public static string ReceivedDir => Path.Combine(DataDir, "Gelen Dosyalar");
    public static string HealthDir => Path.Combine(AppLog.LogDir, "sistem-kontrolu");

    /// Where logs were kept before (still the daemon's own default).
    public static string LegacyLogDir => Path.Combine(ConfigManager.ConfigDir, "logs");

    /// Create the folders, point the config at them and move old logs over.
    /// Safe to call at every start.
    public static void EnsureLayout()
    {
        try
        {
            Directory.CreateDirectory(DefaultLogDir);
            Directory.CreateDirectory(ReceivedDir);

            var cfg = ConfigManager.Load();
            var changed = false;
            var legacy = Path.GetFullPath(LegacyLogDir).TrimEnd('\\');
            if (string.IsNullOrWhiteSpace(cfg.LogDir) ||
                Path.GetFullPath(cfg.LogDir).TrimEnd('\\').Equals(legacy, StringComparison.OrdinalIgnoreCase))
            {
                cfg.LogDir = DefaultLogDir;
                changed = true;
                MoveLegacyLogs(legacy, DefaultLogDir);
            }
            if (!string.Equals(cfg.DataDir, DataDir, StringComparison.OrdinalIgnoreCase))
            {
                cfg.DataDir = DataDir;
                changed = true;
            }
            if (changed && File.Exists(ConfigManager.ConfigPath)) ConfigManager.Save(cfg);

            Directory.CreateDirectory(cfg.LogDir);
            Directory.CreateDirectory(Path.Combine(cfg.LogDir, "sistem-kontrolu"));
            WriteReadme(cfg.LogDir);
        }
        catch
        {
            // Never block startup over folder housekeeping.
        }
    }

    private static void MoveLegacyLogs(string from, string to)
    {
        if (!Directory.Exists(from)) return;
        foreach (var f in Directory.EnumerateFiles(from))
        {
            try
            {
                var dest = Path.Combine(to, Path.GetFileName(f));
                if (!File.Exists(dest)) File.Move(f, dest);
            }
            catch { /* in use by an old daemon: leave it */ }
        }
    }

    private static void WriteReadme(string logDir)
    {
        var path = Path.Combine(logDir, "BENİOKU.txt");
        if (File.Exists(path)) return;
        var text = new StringBuilder()
            .AppendLine("PalmSync kayıt (log) klasörü")
            .AppendLine("============================")
            .AppendLine()
            .AppendLine("palmsync-brain.log     Arka plan servisi: bağlantı, yakalama, geri yükleme")
            .AppendLine("palmsync-eye.log       Kamera modülü: kamera açılışı ve el hareketleri")
            .AppendLine("palmsync-ui.log        Arayüz: kullanıcının yaptıkları ve arayüz hataları")
            .AppendLine("audit.log              Güvenlik: bağlantı denemeleri")
            .AppendLine("cihaz-raporlari.log    (Ev sahibi bilgisayarda) oturumdaki TÜM bilgisayarların")
            .AppendLine("                       bildirdiği sorunlar — diğer bilgisayardaki kamera/dosya")
            .AppendLine("                       sorunları da buraya gelir")
            .AppendLine("sistem-kontrolu\\       Her sistem kontrolünün raporu (son 20 tanesi saklanır)")
            .AppendLine()
            .AppendLine("*.log.1, *.log.2 ...   Boyut sınırı dolunca arşivlenen eski kayıtlar")
            .AppendLine()
            .AppendLine("Kayıtları en kolay PalmSync → 📜 Kayıtlar penceresinden okuyabilirsin.")
            .AppendLine("Hata bildirirken oradaki 📦 Dışa aktar ile tek bir .zip oluştur.")
            .ToString();
        try { File.WriteAllText(path, text, new UTF8Encoding(true)); } catch { }
    }
}

using System.IO;
using System.Text;

namespace PalmSync.Tray.Services;

/// The tray UI's own log (palmsync-ui.log), in the same line format as the
/// daemon and the camera so the log viewer can merge all three. INFO messages
/// are written in plain Turkish: they show up as-is in "Önemli olaylar" and
/// in the Home screen activity feed.
public static class AppLog
{
    private const long MaxBytes = 2 * 1024 * 1024;
    private static readonly object Lock = new();

    public static string LogDir
    {
        get
        {
            try { return ConfigManager.Load().LogDir; }
            catch { return AppPaths.DefaultLogDir; }
        }
    }

    public static string FilePath => Path.Combine(LogDir, "palmsync-ui.log");

    public static void Info(string message) => Write("INFO", message);
    public static void Warn(string message) => Write("WARN", message);
    public static void Error(string message, Exception? ex = null) =>
        Write("ERROR", ex == null ? message : $"{message}: {ex.Message}\n{ex}");
    public static void Debug(string message) => Write("DEBUG", message);

    private static void Write(string level, string message)
    {
        try
        {
            var line = $"{DateTime.Now:yyyy-MM-dd HH:mm:ss.fff} [{level}] ui: " +
                       message.Replace("\r", "").Replace("\n", "\n    ") + Environment.NewLine;
            lock (Lock)
            {
                var path = FilePath;
                Directory.CreateDirectory(Path.GetDirectoryName(path)!);
                var fi = new FileInfo(path);
                if (fi.Exists && fi.Length > MaxBytes)
                    File.Move(path, path + ".1", overwrite: true);

                using var fs = new FileStream(path, FileMode.Append, FileAccess.Write,
                    FileShare.ReadWrite | FileShare.Delete);
                var bytes = Encoding.UTF8.GetBytes(line);
                fs.Write(bytes, 0, bytes.Length);
            }
        }
        catch
        {
            // Logging must never take the app down.
        }
    }
}

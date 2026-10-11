using System.Globalization;
using System.IO;
using System.Text;
using System.Text.RegularExpressions;

namespace PalmSync.Tray.Services;

public enum LogLevel { Trace, Debug, Info, Warn, Error }

/// One log entry (a line plus any indented continuation lines).
public sealed class LogEntry
{
    public DateTime Time { get; init; }
    public LogLevel Level { get; init; }
    public string Source { get; init; } = "";      // Daemon / Kamera / Arayüz / Güvenlik
    public string Component { get; init; } = "";   // short target, e.g. "daemon", "master"
    public string Message { get; set; } = "";
    /// Plain-Turkish description when this is a meaningful user-level event
    /// (capture, restore, connection, camera...), otherwise null.
    public string? Summary { get; init; }

    public bool IsEvent => Summary != null || Level >= LogLevel.Warn;
    /// Today: "14:08:47"; older days get the date too so history isn't confusing.
    public string TimeText => Time == DateTime.MinValue ? ""
        : Time.Date == DateTime.Today ? Time.ToString("HH:mm:ss")
        : Time.ToString("dd.MM HH:mm:ss");
    public string DateText => Time == DateTime.MinValue ? "" : Time.ToString("dd.MM.yyyy");
    public string LevelText => Level switch
    {
        LogLevel.Error => "HATA",
        LogLevel.Warn => "UYARI",
        LogLevel.Info => "BİLGİ",
        LogLevel.Debug => "DEBUG",
        _ => "TRACE",
    };
    public string DisplayText => Summary ?? Message;
    public string FirstLine => First(Message);
    public string DisplayFirstLine => First(DisplayText);

    private static string First(string s)
    {
        var nl = s.IndexOf('\n');
        return nl < 0 ? s : s[..nl] + "  …";
    }

    public string ToLine() =>
        $"{Time:yyyy-MM-dd HH:mm:ss.fff} [{Level.ToString().ToUpperInvariant()}] {Source}/{Component}: {Message}";
}

/// A log file being followed: remembers how far it has been read so each
/// refresh only parses the new bytes (the old viewer and the status poller
/// re-read the whole, ever-growing file every 2 seconds).
public sealed class LogSource
{
    public string Name { get; }
    public string Path { get; }
    private long _offset;
    private LogEntry? _last;

    public LogSource(string name, string path) { Name = name; Path = path; }

    /// Read the last <paramref name="maxBytes"/> of the file (first call) or
    /// whatever was appended since the previous call.
    public List<LogEntry> ReadNew(long maxBytes = 2 * 1024 * 1024)
    {
        var result = new List<LogEntry>();
        try
        {
            if (!File.Exists(Path)) { _offset = 0; return result; }

            using var fs = new FileStream(Path, FileMode.Open, FileAccess.Read,
                FileShare.ReadWrite | FileShare.Delete);
            if (fs.Length < _offset) { _offset = 0; _last = null; }   // rotated / truncated

            var start = Math.Max(_offset, fs.Length - maxBytes);
            var skipPartial = start > _offset || (start > 0 && _offset == 0);
            fs.Seek(start, SeekOrigin.Begin);

            var bytes = new byte[fs.Length - start];
            int read = 0;
            while (read < bytes.Length)
            {
                int n = fs.Read(bytes, read, bytes.Length - read);
                if (n == 0) break;
                read += n;
            }

            // Only consume complete lines; a half-written last line is read next time.
            int lastNl = Array.LastIndexOf(bytes, (byte)'\n', Math.Max(0, read - 1));
            if (lastNl < 0) return result;
            _offset = start + lastNl + 1;

            var text = Encoding.UTF8.GetString(bytes, 0, lastNl + 1);
            var lines = text.Split('\n');
            int i = 0;
            if (skipPartial && lines.Length > 0) i = 1;   // started mid-line

            for (; i < lines.Length; i++)
            {
                var line = lines[i].TrimEnd('\r');
                if (line.Length == 0) continue;
                var entry = LogParser.Parse(line, Name);
                if (entry == null)
                {
                    // continuation (indented line, stack trace...)
                    if (_last != null) _last.Message += "\n" + line.Trim();
                    continue;
                }
                result.Add(entry);
                _last = entry;
            }
        }
        catch (IOException) { }
        catch (UnauthorizedAccessException) { }
        return result;
    }

    public void Reset() { _offset = 0; _last = null; }
}

public static class LogParser
{
    // 2026-10-11 14:08:47.123 [INFO] palmsync_daemon::daemon: message
    private static readonly Regex Std = new(
        @"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}(?:\.\d{1,3})?) \[(\w+)\] ([^:\s]+(?:::[^:\s]+)*): ?(.*)$",
        RegexOptions.Compiled);

    // 2026-07-20T22:31:29.716567800+03:00 [AUTH] node_id=... success=true
    private static readonly Regex Audit = new(
        @"^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2}:\d{2})(\.\d+)?\S* \[(\w+)\] (.*)$",
        RegexOptions.Compiled);

    public static LogEntry? Parse(string line, string fileSource)
    {
        var m = Std.Match(line);
        if (m.Success)
        {
            DateTime.TryParseExact(m.Groups[1].Value,
                new[] { "yyyy-MM-dd HH:mm:ss.fff", "yyyy-MM-dd HH:mm:ss.ff", "yyyy-MM-dd HH:mm:ss.f", "yyyy-MM-dd HH:mm:ss" },
                CultureInfo.InvariantCulture, DateTimeStyles.None, out var t);
            var level = ParseLevel(m.Groups[2].Value);
            var target = m.Groups[3].Value;
            var msg = m.Groups[4].Value;
            var source = SourceOf(target, fileSource);
            return new LogEntry
            {
                Time = t,
                Level = level,
                Source = source,
                Component = ShortTarget(target),
                Message = msg,
                Summary = Friendly.Describe(source, msg),
            };
        }

        var a = Audit.Match(line);
        if (a.Success)
        {
            DateTime.TryParseExact(a.Groups[1].Value + " " + a.Groups[2].Value, "yyyy-MM-dd HH:mm:ss",
                CultureInfo.InvariantCulture, DateTimeStyles.None, out var t);
            var kind = a.Groups[4].Value;
            var msg = a.Groups[5].Value;
            var failed = msg.Contains("success=false");
            return new LogEntry
            {
                Time = t,
                Level = kind == "SECURITY" || failed ? LogLevel.Warn : LogLevel.Info,
                Source = "Güvenlik",
                Component = kind.ToLowerInvariant(),
                Message = msg,
                Summary = kind == "AUTH"
                    ? (failed ? "🔒 Bağlantı reddedildi (yanlış şifre?)" : "🔒 Cihaz kimliği doğrulandı")
                    : null,
            };
        }

        // Indented continuation lines belong to the previous entry. Anything
        // else that doesn't parse (legacy format) is kept as its own entry.
        if (line.StartsWith(' ') || line.StartsWith('\t')) return null;
        return new LogEntry { Level = LogLevel.Info, Source = fileSource, Message = line };
    }

    private static LogLevel ParseLevel(string s) => s.ToUpperInvariant() switch
    {
        "ERROR" => LogLevel.Error,
        "WARN" or "WARNING" => LogLevel.Warn,
        "DEBUG" => LogLevel.Debug,
        "TRACE" => LogLevel.Trace,
        _ => LogLevel.Info,
    };

    private static string SourceOf(string target, string fileSource)
    {
        if (target == "eye") return "Kamera";
        if (target == "ui") return "Arayüz";
        if (target == "palmsync_report") return "Cihazlar";
        if (target.StartsWith("palmsync_networking")) return "Ağ";
        return fileSource;
    }

    private static string ShortTarget(string target)
    {
        var i = target.LastIndexOf("::", StringComparison.Ordinal);
        return i >= 0 ? target[(i + 2)..] : target;
    }
}

/// Turns technical log messages into short Turkish descriptions of what
/// happened, for the "Önemli olaylar" view and the Home screen activity feed.
public static class Friendly
{
    public static string? Describe(string source, string msg)
    {
        string After(string marker) =>
            msg[(msg.IndexOf(marker, StringComparison.Ordinal) + marker.Length)..].Trim();

        static string FileOf(string spec)
        {
            // "C:\proj:main.rs line 0" -> "main.rs"
            var s = spec;
            var line = s.LastIndexOf(" line ", StringComparison.Ordinal);
            if (line > 0) s = s[..line];
            var colon = s.LastIndexOf(':');
            if (colon >= 0 && colon < s.Length - 1) s = s[(colon + 1)..];
            return s.Length == 0 ? "çalışma alanı" : s;
        }

        if (source == "Arayüz") return msg;   // the UI already logs plain Turkish
        if (source == "Cihazlar") return "📡 " + msg;   // "[PC-2] kamera: ..." from a machine in the session

        if (msg.Contains("PalmSync daemon started")) return "▶ Arka plan servisi başladı";
        if (msg.Contains("PalmSync Eye started")) return "📷 Kamera modülü başladı";
        if (msg.StartsWith("Starting in MASTER mode")) return "🖥 Bu bilgisayar oturumu barındırıyor";
        if (msg.StartsWith("Starting in SLAVE mode")) return "🖥 Oturuma bağlanılıyor";
        if (msg.Contains("GRAB captured:")) return "✊ Yakalandı: " + FileOf(After("GRAB captured:"));
        if (msg.Contains("DROP restoring:")) return "🖐 Geri yükleniyor: " + FileOf(After("DROP restoring:"));
        if (msg.StartsWith("Wrote received file to")) return "📄 Gelen dosya kaydedildi: " + After("Wrote received file to");
        if (msg.StartsWith("Opening browser tab:")) return "🌐 Tarayıcı sekmesi açıldı: " + After("Opening browser tab:");
        if (msg.Contains(" authenticated from ")) return "🔗 Diğer cihaz bağlandı (" + After("authenticated from") + ")";
        if (msg.StartsWith("Authenticated with master")) return "🔗 Oturuma bağlanıldı";
        if (msg.StartsWith("Slave ") && msg.EndsWith(" disconnected")) return "⚠ Diğer cihazın bağlantısı koptu";
        if (msg.StartsWith("Disconnected from master") || msg.StartsWith("Lost connection"))
            return "⚠ Oturumla bağlantı koptu, yeniden deneniyor";
        if (msg.StartsWith("Connection failed")) return "⚠ Oturuma bağlanılamadı, tekrar deneniyor";
        if (msg.Contains("Rejected handshake")) return "🔒 Bağlantı reddedildi — şifre/kod yanlış olabilir";
        if (msg.StartsWith("No state available")) return "ℹ Geri yüklenecek bir şey yok — önce diğer bilgisayarda yakala";
        if (msg.Contains("Nothing to capture")) return "ℹ Yakalanacak bir şey yok — VS Code ya da tarayıcı önde olmalı";
        if (msg.StartsWith("File restore failed")) return "⚠ Dosya açılamadı: " + After("File restore failed:");
        if (msg.StartsWith("Could not locate")) return "⚠ Dosya diskte bulunamadı, sadece adı gönderildi";
        if (msg.Contains(">>> GRAB")) return "📷 Kamera: yakalama hareketi (açık el → yumruk)";
        if (msg.Contains(">>> DROP")) return "📷 Kamera: bırakma hareketi (yumruk → açık el)";
        if (msg.StartsWith("Camera ") && msg.Contains(" opened via ")) return "📷 Kamera açıldı";
        if (msg.StartsWith("Camera init failed")) return "⚠ Kamera açılamadı — bağlı mı, başka uygulama kullanıyor mu?";
        if (msg.StartsWith("Only a monochrome camera")) return "⚠ Sadece kızılötesi kamera bulundu, hareketler çalışmayabilir";
        return null;
    }
}

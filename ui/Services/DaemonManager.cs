using System.Diagnostics;
using System.IO;

namespace PalmSync.Tray.Services;

/// Runs palmsync-daemon.exe as a hidden child process inside the user's
/// session. Running here (not as a session-0 service) is what lets the daemon
/// read the user's VS Code window title and clipboard, and needs no admin.
public static class DaemonManager
{
    private static Process? _proc;
    private static bool _stopping;
    private static readonly object Lock = new();

    public static bool IsRunning
    {
        get
        {
            lock (Lock)
            {
                return _proc is { HasExited: false };
            }
        }
    }

    public static string DaemonPath
    {
        get
        {
            // UI lives in {app}\ui; daemon is one level up in {app}
            var uiDir = AppContext.BaseDirectory;
            var candidate = Path.GetFullPath(Path.Combine(uiDir, "..", "palmsync-daemon.exe"));
            if (File.Exists(candidate)) return candidate;

            // Dev fallback: same directory
            var sameDir = Path.Combine(uiDir, "palmsync-daemon.exe");
            return File.Exists(sameDir) ? sameDir : candidate;
        }
    }

    /// (Re)start the daemon. Config (role, password, master addr) must already
    /// be written; the daemon reads it on launch.
    public static void Restart()
    {
        if (!File.Exists(DaemonPath))
            throw new FileNotFoundException(
                "palmsync-daemon.exe bulunamadı. PalmSync'i yeniden kur.", DaemonPath);

        Stop();

        // Kill any stray daemon from a previous UI instance / crash
        foreach (var p in Process.GetProcessesByName("palmsync-daemon"))
        {
            try { p.Kill(true); p.WaitForExit(2000); } catch { /* ignore */ }
            finally { p.Dispose(); }
        }

        var psi = new ProcessStartInfo
        {
            FileName = DaemonPath,
            WorkingDirectory = Path.GetDirectoryName(DaemonPath),
            UseShellExecute = false,
            CreateNoWindow = true,
        };

        lock (Lock)
        {
            _stopping = false;
            _proc = Process.Start(psi);
            if (_proc != null)
            {
                _proc.EnableRaisingEvents = true;
                var p = _proc;
                p.Exited += (_, _) =>
                {
                    // Only unexpected exits are worth a warning.
                    if (!_stopping)
                        AppLog.Warn($"Arka plan servisi beklenmedik şekilde kapandı (çıkış kodu {p.ExitCode}). " +
                                    "Ayrıntı için Daemon kayıtlarına bak.");
                };
                AppLog.Debug($"Daemon başlatıldı (PID {_proc.Id})");
            }
        }
    }

    public static void Stop()
    {
        lock (Lock)
        {
            _stopping = true;
            if (_proc is { HasExited: false })
            {
                try { _proc.Kill(true); _proc.WaitForExit(2000); }
                catch { /* ignore */ }
            }
            _proc?.Dispose();
            _proc = null;
        }
    }
}

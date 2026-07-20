using System.Diagnostics;
using System.IO;

namespace PalmSync.Tray.Services;

/// Runs palmsync-eye.exe (the camera / hand-gesture detector) as a child
/// process. It opens the webcam and sends GRAB/DROP to the local daemon over
/// UDP — fist = grab, open hand = drop.
public static class EyeManager
{
    private static Process? _proc;
    private static readonly object Lock = new();

    public static bool IsRunning
    {
        get { lock (Lock) { return _proc is { HasExited: false }; } }
    }

    public static string EyePath
    {
        get
        {
            var uiDir = AppContext.BaseDirectory;
            var candidate = Path.GetFullPath(Path.Combine(uiDir, "..", "palmsync-eye.exe"));
            if (File.Exists(candidate)) return candidate;
            var sameDir = Path.Combine(uiDir, "palmsync-eye.exe");
            return File.Exists(sameDir) ? sameDir : candidate;
        }
    }

    public static bool IsAvailable => File.Exists(EyePath);

    public static void Start(ushort ipcPort)
    {
        if (!IsAvailable)
            throw new FileNotFoundException("palmsync-eye.exe not found.", EyePath);

        Stop();

        var psi = new ProcessStartInfo
        {
            FileName = EyePath,
            Arguments = $"--port {ipcPort}",
            WorkingDirectory = Path.GetDirectoryName(EyePath),
            UseShellExecute = false,
        };

        lock (Lock) { _proc = Process.Start(psi); }
    }

    public static void Stop()
    {
        lock (Lock)
        {
            if (_proc is { HasExited: false })
            {
                try { _proc.Kill(true); _proc.WaitForExit(2000); } catch { }
            }
            _proc?.Dispose();
            _proc = null;
        }

        foreach (var p in Process.GetProcessesByName("palmsync-eye"))
        {
            try { p.Kill(true); } catch { } finally { p.Dispose(); }
        }
    }
}

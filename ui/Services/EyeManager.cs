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
    private static readonly System.Text.StringBuilder Stderr = new();

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

    /// Try to start the camera process. Returns null on success, or a
    /// human-readable error string on failure (including Windows Camera
    /// Privacy hints when relevant).
    /// Always passes --preview-pipe so the CameraPreviewService can show the
    /// live camera image (with the detected hand) in the Home window.
    public static async Task<string?> TryStart(ushort ipcPort)
    {
        if (!IsAvailable)
            return
                $"Kamera modülü (palmsync-eye.exe) bulunamadı.\n\n" +
                $"Beklenen konum:\n{EyePath}\n\n" +
                $"Kamera özelliği için PalmSync kurulumunu yeniden çalıştır.";

        Stop();

        var args = $"--port {ipcPort} --preview-pipe \\\\.\\pipe\\{CameraPreviewService.PipeName} " +
                   $"--log-dir \"{AppLog.LogDir.TrimEnd('\\')}\"";
        if (ConfigManager.Load().DebugMode) args += " --verbose";

        var psi = new ProcessStartInfo
        {
            FileName = EyePath,
            // Headless by default: cv::namedWindow / cv::waitKey require a
            // HighGUI event loop that crashes when spawned from a GUI tray app.
            // Pass --window to palmsync-eye.exe manually if you need the preview.
            Arguments = args,
            WorkingDirectory = Path.GetDirectoryName(EyePath),
            UseShellExecute = false,
            RedirectStandardError = true,
            StandardErrorEncoding = System.Text.Encoding.UTF8,  // eye writes UTF-8 (—, →)
            CreateNoWindow = true,
        };

        lock (Stderr) Stderr.Clear();

        Process proc;
        lock (Lock)
        {
            _proc = Process.Start(psi)
                ?? throw new InvalidOperationException("Process.Start returned null.");
            proc = _proc;
        }

        // Drain stderr continuously. Previously nobody read it while the camera
        // ran: once the pipe buffer filled up (every gesture logs a line)
        // palmsync-eye blocked on its next log write and the camera froze.
        proc.ErrorDataReceived += (_, e) =>
        {
            if (e.Data == null) return;
            lock (Stderr)
            {
                if (Stderr.Length < 16 * 1024) Stderr.AppendLine(e.Data);
            }
        };
        proc.BeginErrorReadLine();

        // Give the process up to 3 s to open the camera (slow UVC cameras can
        // take 2+ s for the first frame; DirectShow probing also adds time).
        await Task.Delay(3000);

        if (!proc.HasExited)
        {
            AppLog.Info("📷 Kamera ile el hareketi açıldı");
            return null;  // still running → success
        }

        var reason = ExitReason(proc);
        // When the eye itself logged why (no webcam etc.), don't log it twice.
        lock (Stderr)
        {
            if (Stderr.ToString().Contains("Camera init failed")) AppLog.Debug("Kamera başlatılamadı (ayrıntı Kamera kayıtlarında)");
            else AppLog.Warn("Kamera başlatılamadı: " + reason);
        }
        // Let the host know too (cihaz-raporlari.log on the host computer).
        ReportFailure("Kamera başlatılamadı", reason);
        return reason;
    }

    /// Send a camera failure to the host's log, as one line.
    public static void ReportFailure(string what, string reason)
    {
        var lines = reason.Split('\n', StringSplitOptions.RemoveEmptyEntries)
            .Select(l => l.Trim()).Where(l => l.Length > 0).ToList();
        var summary = lines.FirstOrDefault() ?? "";
        var detail = lines.LastOrDefault(l => l.Contains("FAIL", StringComparison.OrdinalIgnoreCase)
                                             || l.Contains("failed", StringComparison.OrdinalIgnoreCase));
        DeviceReporter.Send("error", "kamera", $"{what}: {summary}{(detail != null && detail != summary ? " | " + detail : "")}");
    }

    /// Explanation for a camera process that has already exited (null if it
    /// is still running or was never started). Used to tell the user WHY the
    /// camera stopped instead of silently switching the toggle off.
    public static string? LastExitReason()
    {
        lock (Lock)
        {
            if (_proc is not { HasExited: true }) return null;
            return ExitReason(_proc);
        }
    }

    private static string ExitReason(Process proc)
    {
        proc.WaitForExit();  // flushes the async stderr reader
        string stderr;
        lock (Stderr) stderr = Stderr.ToString().Replace("[Eye] ", "");

        // 0xC0000135 = STATUS_DLL_NOT_FOUND, 0xC000007B = bad image: the
        // Visual C++ runtime or an OpenCV DLL is missing on this computer.
        if (proc.ExitCode is unchecked((int)0xC0000135) or unchecked((int)0xC000007B))
            return
                $"Kamera modülü başlayamadı: gerekli bir DLL eksik (çıkış 0x{proc.ExitCode:X8}).\n\n" +
                "Çözüm: Microsoft Visual C++ Redistributable (x64) kur:\n" +
                "https://aka.ms/vs/17/release/vc_redist.x64.exe\n" +
                "ya da PalmSync'in son kurulumunu yeniden kur.\n\n" +
                "Windows 'N' sürümlerinde ayrıca Media Feature Pack gerekir.";

        if (stderr.Contains("Camera init failed") || stderr.Contains("Failed to open camera"))
        {
            // Show the last lines (the summary) so the dialog stays compact.
            var lines = stderr.Trim().Split('\n');
            var diag = string.Join('\n', lines.TakeLast(6));
            if (lines.Length > 6) diag = $"(… önceki {lines.Length - 6} satır Kayıtlar > Kamera'da)\n" + diag;
            return
                "Webcam açılamadı.\n\n" +
                "Kontrol et:\n" +
                "  • Kamera takılı mı? Teams / Zoom / Discord kamerayı kullanıyor olabilir.\n" +
                "  • Ayarlar → Gizlilik ve güvenlik → Kamera: üç anahtar da AÇIK mı?\n\n" +
                "Tanı:\n" + diag;
        }

        if (stderr.Contains("IPC init failed"))
            return "Kamera ile daemon arasındaki kanal açılamadı. Bir oturumun açık olduğundan emin olup tekrar dene.";

        if (string.IsNullOrWhiteSpace(stderr))
            return
                $"Kamera modülü başlamadı (çıkış {proc.ExitCode}).\n\n" +
                "Olası nedenler:\n" +
                "  • Windows uygulama denetimi (WDAC/Akıllı Uygulama Denetimi) exe'yi engelledi\n" +
                "  • Bir OpenCV DLL'i eksik → PalmSync kurulumunu yeniden çalıştır";

        return $"Kamera modülü beklenmedik şekilde durdu (çıkış {proc.ExitCode}).\n\n" +
               string.Join('\n', stderr.Trim().Split('\n').TakeLast(8));
    }

    public static void Stop()
    {
        lock (Lock)
        {
            if (_proc is { HasExited: false })
            {
                AppLog.Info("📷 Kamera kapatıldı");
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

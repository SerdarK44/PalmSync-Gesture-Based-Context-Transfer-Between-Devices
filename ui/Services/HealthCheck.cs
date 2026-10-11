using System.Diagnostics;
using System.IO;
using System.Net;
using System.Net.Sockets;
using System.Security.Cryptography.X509Certificates;
using System.Text;
using System.Text.Json;
using Microsoft.Win32;

namespace PalmSync.Tray.Services;

public enum CheckStatus { Ok, Warn, Fail, Skipped }

/// Result of one system check, plus how to fix it if it isn't OK.
public sealed class CheckResult
{
    public required string Id { get; init; }
    public required string Category { get; init; }
    public required string Title { get; init; }
    public CheckStatus Status { get; init; }
    public string Detail { get; init; } = "";
    /// What the user can do when it can't be fixed automatically.
    public string? Hint { get; init; }
    /// Fix that needs no admin rights (registry under HKCU, folders, config).
    public Func<Task>? Fix { get; init; }
    /// Fix that needs admin rights; all of these run in one UAC prompt.
    public Elevation.Step? AdminFix { get; init; }
    /// Not a fix but a helpful page to open (Windows settings, download...).
    public string? OpenUri { get; init; }

    public bool CanFix => Status is CheckStatus.Warn or CheckStatus.Fail && (Fix != null || AdminFix != null);
    public bool CanOpen => OpenUri != null && Status is CheckStatus.Warn or CheckStatus.Fail;
    public bool HasHint => Hint != null && Status is CheckStatus.Warn or CheckStatus.Fail;
    /// 🔐 = needs administrator rights (UAC prompt).
    public string FixText => Fix == null && AdminFix != null ? "🔐 Düzelt" : "Düzelt";
    public string Icon => Status switch
    {
        CheckStatus.Ok => "✅",
        CheckStatus.Warn => "⚠️",
        CheckStatus.Fail => "❌",
        _ => "➖",
    };
    public string StatusText => Status switch
    {
        CheckStatus.Ok => "Tamam",
        CheckStatus.Warn => "Uyarı",
        CheckStatus.Fail => "Sorun",
        _ => "Atlandı",
    };
}

/// "Sistem kontrolü": finds what typically breaks PalmSync on a freshly
/// installed computer — missing files / DLLs, blocked executables, camera
/// missing, blocked by privacy settings or used by another app, firewall,
/// certificate, ports, folders, notifications, autostart — fixes what it can,
/// writes a report to Documents\PalmSync\Loglar\sistem-kontrolu and sends
/// whatever is still broken to the HOST's log.
public static class HealthCheck
{
    private static readonly SemaphoreSlim Gate = new(1, 1);

    public static IReadOnlyList<CheckResult> Last { get; private set; } = Array.Empty<CheckResult>();
    public static DateTime LastRun { get; private set; }
    public static int ProblemCount => Last.Count(r => r.Status is CheckStatus.Fail or CheckStatus.Warn);
    public static event Action? Changed;

    private const string ConsentStore = @"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\webcam";

    private static string AppDir => Path.GetDirectoryName(DaemonManager.DaemonPath)!;

    // ── Running ─────────────────────────────────────────────

    /// Run every check. `probeCamera` actually opens the webcam (a few
    /// seconds) unless PalmSync's own camera is already running.
    public static async Task<IReadOnlyList<CheckResult>> RunAsync(string reason, bool probeCamera = true)
    {
        await Gate.WaitAsync();
        try
        {
            AppLog.Debug($"Sistem kontrolü başladı ({reason})");
            var results = await Task.Run(async () =>
            {
                var list = new List<CheckResult>();
                void Add(Func<CheckResult> check, string id, string category, string title)
                {
                    try { list.Add(check()); }
                    catch (Exception ex)
                    {
                        list.Add(new CheckResult
                        {
                            Id = id, Category = category, Title = title, Status = CheckStatus.Warn,
                            Detail = "Kontrol yapılamadı: " + ex.Message,
                        });
                    }
                }

                Add(CheckFiles, "files", "Dosyalar", "Program dosyaları");
                Add(CheckHandModels, "models", "Dosyalar", "Kamera 2.0 el modelleri");
                Add(CheckVcRuntime, "vcruntime", "Dosyalar", "Visual C++ çalışma zamanı");
                Add(() => CheckRuns("daemon", DaemonManager.DaemonPath, "Arka plan servisi çalışabiliyor"), "daemon", "Dosyalar", "Arka plan servisi çalışabiliyor");
                Add(() => CheckRuns("eye", EyeManager.EyePath, "Kamera modülü çalışabiliyor"), "eye", "Dosyalar", "Kamera modülü çalışabiliyor");
                Add(CheckMediaFoundation, "mf", "Dosyalar", "Windows medya bileşenleri");

                var devices = Add2(list, CheckCameraDevice, "camdev", "Kamera", "Kamera donanımı");
                Add(CheckCameraPrivacy, "campriv", "Kamera", "Kamera izni (Windows gizlilik)");
                Add(CheckCameraInUse, "camuse", "Kamera", "Kamerayı başka uygulama kullanıyor mu");
                if (probeCamera) list.Add(await CheckCameraOpens(devices));

                Add(CheckNetwork, "net", "Ağ", "Yerel ağ bağlantısı");
                Add(CheckFirewall, "fw", "Ağ", "Güvenlik duvarı izni");
                Add(CheckPorts, "ports", "Ağ", "PalmSync portları");

                Add(CheckCertificate, "cert", "İzinler", "PalmSync sertifikası");
                Add(CheckSmartAppControl, "sac", "İzinler", "Akıllı Uygulama Denetimi");
                Add(CheckNotifications, "notif", "İzinler", "Bildirimler");
                Add(CheckAutostart, "autostart", "İzinler", "Oturum açınca başlatma");

                Add(CheckConfig, "config", "Sistem", "Ayar dosyası");
                Add(CheckFolders, "folders", "Sistem", "Belgeler\\PalmSync klasörleri");
                Add(CheckApps, "apps", "Sistem", "Aktarım için uygulamalar");
                return list;
            });

            Last = results;
            LastRun = DateTime.Now;
            WriteReport(reason, results);

            var problems = results.Where(r => r.Status is CheckStatus.Fail or CheckStatus.Warn).ToList();
            if (problems.Count == 0)
                AppLog.Info($"🔧 Sistem kontrolü: her şey yolunda ({results.Count} kontrol)");
            else
            {
                AppLog.Info($"🔧 Sistem kontrolü: {problems.Count} sorun ({string.Join(", ", problems.Select(p => p.Title))})");
                foreach (var p in problems)
                    AppLog.Warn($"Sistem kontrolü — {p.Title}: {p.Detail}");
            }

            Changed?.Invoke();
            return results;
        }
        finally
        {
            Gate.Release();
        }
    }

    private static T Add2<T>(List<CheckResult> list, Func<(CheckResult, T)> check, string id, string category, string title)
    {
        try
        {
            var (r, extra) = check();
            list.Add(r);
            return extra;
        }
        catch (Exception ex)
        {
            list.Add(new CheckResult { Id = id, Category = category, Title = title, Status = CheckStatus.Warn, Detail = "Kontrol yapılamadı: " + ex.Message });
            return default!;
        }
    }

    /// Fix everything fixable: user-level fixes first, then all admin fixes
    /// in one UAC prompt, then check again. Returns a short summary.
    public static async Task<string> FixAllAsync(IEnumerable<CheckResult>? only = null)
    {
        var targets = (only ?? Last).Where(r => r.CanFix).ToList();
        if (targets.Count == 0) return "Otomatik düzeltilebilecek bir sorun yok.";

        int fixedCount = 0;
        var failed = new List<string>();
        foreach (var r in targets.Where(r => r.Fix != null))
        {
            try
            {
                await r.Fix!();
                AppLog.Info($"✅ Düzeltildi: {r.Title}");
                fixedCount++;
            }
            catch (Exception ex)
            {
                AppLog.Warn($"Düzeltilemedi: {r.Title} — {ex.Message}");
                failed.Add(r.Title);
            }
        }

        var adminSteps = targets.Where(r => r.AdminFix != null).Select(r => r.AdminFix!).ToList();
        var declined = false;
        if (adminSteps.Count > 0)
        {
            var res = await Elevation.RunAsync(adminSteps);
            if (res == null) declined = true;
            else
            {
                fixedCount += res.Count(x => x.Ok);
                failed.AddRange(res.Where(x => !x.Ok).Select(x => x.Title));
            }
        }

        await RunAsync("düzeltme sonrası", probeCamera: true);
        SendProblemsToHost();

        var sb = new StringBuilder($"{fixedCount} düzeltme uygulandı.");
        if (declined) sb.Append(" Yönetici izni verilmediği için bazı düzeltmeler yapılamadı.");
        if (failed.Count > 0) sb.Append(" Düzeltilemeyen: " + string.Join(", ", failed) + ".");
        if (ProblemCount > 0) sb.Append($" Kalan sorun: {ProblemCount} (ayrıntılar ana bilgisayarın loguna gönderildi).");
        return sb.ToString();
    }

    /// Send what's still wrong to the daemon → the host's cihaz-raporlari.log.
    /// In debug mode the full report (incl. passing checks) is sent.
    public static void SendProblemsToHost()
    {
        if (Last.Count == 0) return;
        var debug = ConfigManager.Load().DebugMode;
        var problems = Last.Where(r => r.Status is CheckStatus.Fail or CheckStatus.Warn).ToList();

        DeviceReporter.Send(problems.Count == 0 ? "info" : "warn", "sistem",
            problems.Count == 0
                ? $"Sistem kontrolü: her şey yolunda ({Last.Count} kontrol)"
                : $"Sistem kontrolü: {problems.Count} sorun — {string.Join(", ", problems.Select(p => p.Title))}");

        foreach (var r in debug ? Last : problems)
        {
            var level = r.Status switch { CheckStatus.Fail => "error", CheckStatus.Warn => "warn", _ => "info" };
            var text = $"{r.Title}: {r.StatusText} — {r.Detail}";
            if (r.Hint != null && r.Status != CheckStatus.Ok) text += $" | Öneri: {r.Hint}";
            DeviceReporter.Send(level, "sistem", text);
        }
    }

    private static void WriteReport(string reason, IReadOnlyList<CheckResult> results)
    {
        try
        {
            var dir = AppPaths.HealthDir;
            Directory.CreateDirectory(dir);
            var sb = new StringBuilder();
            sb.AppendLine("PalmSync — Sistem kontrolü");
            sb.AppendLine($"Tarih     : {DateTime.Now:dd.MM.yyyy HH:mm:ss}");
            sb.AppendLine($"Bilgisayar: {Environment.MachineName}   Windows: {Environment.OSVersion.Version}");
            sb.AppendLine($"Neden     : {reason}");
            sb.AppendLine($"Sonuç     : {results.Count(r => r.Status == CheckStatus.Ok)} tamam, " +
                          $"{results.Count(r => r.Status == CheckStatus.Warn)} uyarı, " +
                          $"{results.Count(r => r.Status == CheckStatus.Fail)} sorun");
            foreach (var g in results.GroupBy(r => r.Category))
            {
                sb.AppendLine().AppendLine($"── {g.Key} ──");
                foreach (var r in g)
                {
                    sb.AppendLine($"  [{r.StatusText,-7}] {r.Title}: {r.Detail}");
                    if (r.Hint != null && r.Status is CheckStatus.Warn or CheckStatus.Fail)
                        sb.AppendLine($"            Öneri: {r.Hint}");
                }
            }
            File.WriteAllText(Path.Combine(dir, $"sistem-kontrolu-{DateTime.Now:yyyy-MM-dd_HH-mm-ss}.txt"),
                sb.ToString(), new UTF8Encoding(true));

            foreach (var old in new DirectoryInfo(dir).GetFiles("sistem-kontrolu-*.txt")
                         .OrderByDescending(f => f.Name).Skip(20))
                old.Delete();
        }
        catch (Exception ex)
        {
            AppLog.Debug("Sistem kontrolü raporu yazılamadı: " + ex.Message);
        }
    }

    // ── Files ───────────────────────────────────────────────

    private static readonly string[] RequiredFiles =
    {
        "palmsync-daemon.exe", "palmsync-eye.exe",
        "opencv_core4.dll", "opencv_imgproc4.dll", "opencv_videoio4.dll",
        "opencv_highgui4.dll", "opencv_imgcodecs4.dll",
        "jpeg62.dll", "libpng16.dll", "z.dll", "tiff.dll", "liblzma.dll",
        "libwebp.dll", "libwebpdecoder.dll", "libwebpdemux.dll", "libwebpmux.dll", "libsharpyuv.dll",
        // face masking (objdetect and what it pulls in)
        "opencv_objdetect4.dll", "opencv_dnn4.dll", "opencv_calib3d4.dll", "opencv_features2d4.dll",
        "opencv_flann4.dll", "libprotobuf.dll", "abseil_dll.dll",
    };

    private const string FaceModel = "haarcascade_frontalface_default.xml";

    private static CheckResult CheckFiles()
    {
        var missing = RequiredFiles.Where(f => !File.Exists(Path.Combine(AppDir, f))).ToList();
        var noFaceModel = !File.Exists(Path.Combine(AppDir, FaceModel)) ||
                          !File.Exists(Path.Combine(AppDir, "haarcascade_profileface.xml"));
        return new CheckResult
        {
            Id = "files", Category = "Dosyalar", Title = "Program dosyaları",
            Status = missing.Count > 0 ? CheckStatus.Fail : noFaceModel ? CheckStatus.Warn : CheckStatus.Ok,
            Detail = missing.Count > 0
                ? $"Eksik: {string.Join(", ", missing)} (klasör: {AppDir})"
                : noFaceModel
                    ? $"Yüz modeli ({FaceModel}) yok — yüz maskeleme kapalı, el kilidi için önce AÇIK el gerekir"
                    : $"{RequiredFiles.Length + 2} dosya yerinde ({AppDir})",
            Hint = missing.Count == 0 && !noFaceModel ? null
                : "PalmSync kurulumunu yeniden çalıştır. Antivirüs dosya silmiş olabilir — PalmSync klasörünü istisnalara ekle.",
        };
    }

    /// Camera 2.0 hand models: present and byte-identical to the pinned
    /// manifest. Without them the eye silently falls back to the classic
    /// skin-colour detector, which works far worse — the user should know.
    private static CheckResult CheckHandModels()
    {
        const string title = "Kamera 2.0 el modelleri";
        var dir = Path.Combine(AppDir, "models");
        var manifest = Path.Combine(dir, "models.json");
        if (!File.Exists(manifest))
            return new CheckResult
            {
                Id = "models", Category = "Dosyalar", Title = title, Status = CheckStatus.Warn,
                Detail = "models\\models.json yok — kamera eski (ten rengi) algılayıcıyla çalışır",
                Hint = "PalmSync kurulumunu yeniden çalıştır.",
            };

        using var doc = System.Text.Json.JsonDocument.Parse(File.ReadAllText(manifest));
        var problems = new List<string>();
        int count = 0;
        foreach (var asset in doc.RootElement.GetProperty("assets").EnumerateArray())
        {
            var file = asset.GetProperty("file").GetString() ?? "";
            var path = Path.Combine(dir, file);
            count++;
            if (!File.Exists(path)) { problems.Add($"{file} eksik"); continue; }
            if (asset.TryGetProperty("bytes", out var bytes) && new FileInfo(path).Length != bytes.GetInt64())
            {
                problems.Add($"{file} boyutu hatalı");
                continue;
            }
            using var stream = File.OpenRead(path);
            var hash = Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(stream));
            if (!string.Equals(hash, asset.GetProperty("sha256").GetString(), StringComparison.OrdinalIgnoreCase))
                problems.Add($"{file} bozuk (SHA-256 uyuşmuyor)");
        }

        return new CheckResult
        {
            Id = "models", Category = "Dosyalar", Title = title,
            Status = problems.Count == 0 ? CheckStatus.Ok : CheckStatus.Warn,
            Detail = problems.Count == 0
                ? $"{count} dosya doğrulandı (SHA-256)"
                : string.Join(", ", problems) + " — kamera eski (ten rengi) algılayıcıyla çalışır",
            Hint = problems.Count == 0 ? null
                : "PalmSync kurulumunu yeniden çalıştır. Antivirüs dosya silmiş olabilir — PalmSync klasörünü istisnalara ekle.",
        };
    }

    private static CheckResult CheckVcRuntime()
    {
        var sys = Environment.GetFolderPath(Environment.SpecialFolder.System);
        string[] dlls = { "msvcp140.dll", "vcruntime140.dll", "vcruntime140_1.dll" };
        var missing = dlls.Where(d => !File.Exists(Path.Combine(AppDir, d)) && !File.Exists(Path.Combine(sys, d))).ToList();
        return new CheckResult
        {
            Id = "vcruntime", Category = "Dosyalar", Title = "Visual C++ çalışma zamanı",
            Status = missing.Count == 0 ? CheckStatus.Ok : CheckStatus.Fail,
            Detail = missing.Count == 0 ? "msvcp140 / vcruntime140 mevcut" : "Eksik: " + string.Join(", ", missing),
            Hint = missing.Count == 0 ? null : "Düzelt'e bas: Microsoft Visual C++ Redistributable indirilip kurulur (internet gerekir).",
            AdminFix = missing.Count == 0 ? null : new Elevation.Step("Visual C++ çalışma zamanı",
                "  $f = Join-Path $env:TEMP 'vc_redist.x64.exe'\n" +
                "  Invoke-WebRequest 'https://aka.ms/vs/17/release/vc_redist.x64.exe' -OutFile $f -UseBasicParsing\n" +
                "  $p = Start-Process $f '/install /quiet /norestart' -Wait -PassThru\n" +
                "  if ($p.ExitCode -ne 0 -and $p.ExitCode -ne 3010 -and $p.ExitCode -ne 1638) { throw \"vc_redist çıkış kodu $($p.ExitCode)\" }"),
        };
    }

    /// Start the exe with --version: catches WDAC/Smart App Control blocks,
    /// missing DLLs and broken binaries before the user hits them.
    private static CheckResult CheckRuns(string id, string exe, string title)
    {
        if (!File.Exists(exe))
            return new CheckResult { Id = id, Category = "Dosyalar", Title = title, Status = CheckStatus.Fail, Detail = "Dosya yok: " + exe, Hint = "PalmSync kurulumunu yeniden çalıştır." };
        try
        {
            using var p = Process.Start(new ProcessStartInfo(exe, "--version")
            {
                UseShellExecute = false, CreateNoWindow = true,
                RedirectStandardOutput = true, RedirectStandardError = true,
                WorkingDirectory = Path.GetDirectoryName(exe)!,
            })!;
            var output = p.StandardOutput.ReadToEndAsync();
            if (!p.WaitForExit(8000))
            {
                try { p.Kill(true); } catch { }
                return new CheckResult { Id = id, Category = "Dosyalar", Title = title, Status = CheckStatus.Warn, Detail = "8 saniyede yanıt vermedi" };
            }
            if (p.ExitCode == 0)
                return new CheckResult { Id = id, Category = "Dosyalar", Title = title, Status = CheckStatus.Ok, Detail = output.Result.Trim() };

            var code = unchecked((uint)p.ExitCode);
            var (detail, hint) = code switch
            {
                0xC0000135 => ("Gerekli bir DLL eksik (0xC0000135)", "Visual C++ çalışma zamanını kur ya da PalmSync'i yeniden kur."),
                0xC000007B => ("DLL'ler bozuk ya da yanlış mimari (0xC000007B)", "PalmSync'i yeniden kur."),
                _ => ($"Çıkış kodu 0x{code:X8}", "Kayıtlardaki ayrıntıya bak; PalmSync'i yeniden kurmayı dene."),
            };
            return new CheckResult { Id = id, Category = "Dosyalar", Title = title, Status = CheckStatus.Fail, Detail = detail, Hint = hint };
        }
        catch (System.ComponentModel.Win32Exception ex) when (ex.NativeErrorCode is 4551 or 1260 or 225)
        {
            // 4551: Application Control (WDAC / Smart App Control), 1260: group policy, 225: virus
            return new CheckResult
            {
                Id = id, Category = "Dosyalar", Title = title, Status = CheckStatus.Fail,
                Detail = $"Windows bu programı engelliyor ({ex.Message})",
                Hint = ex.NativeErrorCode == 225
                    ? "Antivirüs dosyayı engelledi: PalmSync klasörünü antivirüs istisnalarına ekle."
                    : "Sertifikayı güvenilir yap (Düzelt) ve Akıllı Uygulama Denetimi açıksa kapat.",
                AdminFix = CertificateFix(),
            };
        }
    }

    private static CheckResult CheckMediaFoundation()
    {
        var ok = File.Exists(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.System), "mfplat.dll"));
        return new CheckResult
        {
            Id = "mf", Category = "Dosyalar", Title = "Windows medya bileşenleri",
            Status = ok ? CheckStatus.Ok : CheckStatus.Fail,
            Detail = ok ? "Media Foundation mevcut" : "Media Foundation yok (Windows 'N' sürümü) — kamera açılamaz",
            Hint = ok ? null : "Ayarlar → Uygulamalar → İsteğe bağlı özellikler → 'Medya Özellik Paketi'ni ekle, sonra yeniden başlat.",
            OpenUri = ok ? null : "ms-settings:optionalfeatures",
        };
    }

    // ── Camera ──────────────────────────────────────────────

    private record PnpCamera(string Name, string Status, string InstanceId, int Problem);

    private static (CheckResult, List<PnpCamera>) CheckCameraDevice()
    {
        var cams = ListPnpCameras();
        var ok = cams.Where(c => c.Status == "OK").ToList();
        var disabled = cams.Where(c => c.Problem == 22).ToList();
        var broken = cams.Where(c => c.Status is "Error" or "Degraded" && c.Problem != 22).ToList();

        if (ok.Count > 0)
            return (new CheckResult
            {
                Id = "camdev", Category = "Kamera", Title = "Kamera donanımı", Status = CheckStatus.Ok,
                Detail = string.Join(", ", ok.Select(c => c.Name)),
            }, cams);

        if (disabled.Count > 0)
            return (new CheckResult
            {
                Id = "camdev", Category = "Kamera", Title = "Kamera donanımı", Status = CheckStatus.Fail,
                Detail = "Kamera Aygıt Yöneticisi'nde devre dışı: " + string.Join(", ", disabled.Select(c => c.Name)),
                Hint = "Düzelt'e bas: kamera yeniden etkinleştirilir.",
                AdminFix = new Elevation.Step("Kamerayı etkinleştir", string.Join("\n", disabled.Select(c =>
                    $"  Enable-PnpDevice -InstanceId '{Elevation.Escape(c.InstanceId)}' -Confirm:$false"))),
            }, cams);

        if (broken.Count > 0)
            return (new CheckResult
            {
                Id = "camdev", Category = "Kamera", Title = "Kamera donanımı", Status = CheckStatus.Fail,
                Detail = "Kamera sürücüsü hata veriyor: " + string.Join(", ", broken.Select(c => $"{c.Name} (kod {c.Problem})")),
                Hint = "Aygıt Yöneticisi'nde kamerayı kaldırıp bilgisayarı yeniden başlat ya da üreticinin sürücüsünü kur.",
                OpenUri = "devmgmt.msc",
            }, cams);

        var seen = cams.Where(c => c.Status == "Unknown").Select(c => c.Name).Distinct().ToList();
        return (new CheckResult
        {
            Id = "camdev", Category = "Kamera", Title = "Kamera donanımı", Status = CheckStatus.Fail,
            Detail = seen.Count > 0
                ? "Kamera takılı değil (daha önce görülen: " + string.Join(", ", seen) + ")"
                : "Windows hiç kamera görmüyor",
            Hint = "Kamerayı tak (USB kameralarda başka bir USB girişi dene). Dizüstünde kamera kapağı / F tuşu kilidi olabilir.",
        }, cams);
    }

    private static List<PnpCamera> ListPnpCameras()
    {
        var json = RunPowerShell(
            "Get-PnpDevice -Class Camera,Image -ErrorAction SilentlyContinue | " +
            "Select-Object FriendlyName,Status,InstanceId,@{n='Problem';e={[int]$_.Problem}} | ConvertTo-Json -Compress");
        var list = new List<PnpCamera>();
        if (string.IsNullOrWhiteSpace(json)) return list;
        using var doc = JsonDocument.Parse(json);
        var items = doc.RootElement.ValueKind == JsonValueKind.Array
            ? doc.RootElement.EnumerateArray().ToList()
            : new List<JsonElement> { doc.RootElement };
        foreach (var e in items)
        {
            string S(string n) => e.TryGetProperty(n, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() ?? "" : "";
            int problem = e.TryGetProperty("Problem", out var pv) && pv.ValueKind == JsonValueKind.Number ? pv.GetInt32() : 0;
            list.Add(new PnpCamera(S("FriendlyName"), S("Status"), S("InstanceId"), problem));
        }
        return list;
    }

    private static CheckResult CheckCameraPrivacy()
    {
        // Organisation policy: 2 = force deny, cannot be changed by us.
        var policy = Registry.GetValue(@"HKEY_LOCAL_MACHINE\SOFTWARE\Policies\Microsoft\Windows\AppPrivacy", "LetAppsAccessCamera", null);
        if (policy is int p && p == 2)
            return new CheckResult
            {
                Id = "campriv", Category = "Kamera", Title = "Kamera izni (Windows gizlilik)", Status = CheckStatus.Fail,
                Detail = "Kuruluş ilkesi (Grup İlkesi) uygulamaların kameraya erişimini yasaklıyor",
                Hint = "Bu bilgisayarın yöneticisinden (okul/şirket BT) kamera iznini açmasını iste.",
            };

        var device = ReadConsent(Registry.LocalMachine, ConsentStore);
        var apps = ReadConsent(Registry.CurrentUser, ConsentStore);
        var desktop = ReadConsent(Registry.CurrentUser, ConsentStore + @"\NonPackaged");
        var eyeKey = ConsentStore + @"\NonPackaged\" + EyeManager.EyePath.Replace('\\', '#');
        var eyeOnly = ReadConsent(Registry.CurrentUser, eyeKey);

        var denied = new List<string>();
        if (device == "Deny") denied.Add("'Kamera erişimi' (cihaz geneli)");
        if (apps == "Deny") denied.Add("'Uygulamaların kameraya erişmesine izin ver'");
        if (desktop == "Deny") denied.Add("'Masaüstü uygulamalarının kameraya erişmesine izin ver'");
        if (eyeOnly == "Deny") denied.Add("PalmSync kamera modülü için özel olarak");

        if (denied.Count == 0)
            return new CheckResult { Id = "campriv", Category = "Kamera", Title = "Kamera izni (Windows gizlilik)", Status = CheckStatus.Ok, Detail = "Masaüstü uygulamalarına kamera izni açık" };

        return new CheckResult
        {
            Id = "campriv", Category = "Kamera", Title = "Kamera izni (Windows gizlilik)", Status = CheckStatus.Fail,
            Detail = "Kapalı: " + string.Join(", ", denied),
            Hint = "Düzelt'e bas ya da Ayarlar → Gizlilik ve güvenlik → Kamera'dan üç anahtarı da aç.",
            OpenUri = "ms-settings:privacy-webcam",
            Fix = apps == "Deny" || desktop == "Deny" || eyeOnly == "Deny"
                ? () =>
                {
                    if (apps == "Deny") Registry.SetValue(@"HKEY_CURRENT_USER\" + ConsentStore, "Value", "Allow");
                    if (desktop == "Deny") Registry.SetValue(@"HKEY_CURRENT_USER\" + ConsentStore + @"\NonPackaged", "Value", "Allow");
                    if (eyeOnly == "Deny") Registry.SetValue(@"HKEY_CURRENT_USER\" + eyeKey, "Value", "Allow");
                    return Task.CompletedTask;
                }
                : null,
            AdminFix = device == "Deny"
                ? new Elevation.Step("Kamera erişimi (cihaz geneli)",
                    $"  Set-ItemProperty -Path 'HKLM:\\{ConsentStore}' -Name Value -Value 'Allow'")
                : null,
        };
    }

    private static string? ReadConsent(RegistryKey root, string path)
    {
        using var k = root.OpenSubKey(path);
        return k?.GetValue("Value") as string;
    }

    /// Windows records which apps are using the camera right now
    /// (LastUsedTimeStop == 0 while in use). Only one app can open a webcam.
    private static CheckResult CheckCameraInUse()
    {
        var users = new List<string>();
        void Scan(string path, bool nonPackaged)
        {
            using var k = Registry.CurrentUser.OpenSubKey(path);
            if (k == null) return;
            foreach (var name in k.GetSubKeyNames())
            {
                if (name == "NonPackaged") continue;
                using var sub = k.OpenSubKey(name);
                if (sub?.GetValue("LastUsedTimeStart") is long start && start != 0 &&
                    sub.GetValue("LastUsedTimeStop") is long stop && stop == 0)
                {
                    var app = nonPackaged ? Path.GetFileName(name.Replace('#', '\\')) : name.Split('_')[0];
                    if (!app.Contains("palmsync", StringComparison.OrdinalIgnoreCase)) users.Add(app);
                }
            }
        }
        Scan(ConsentStore, false);
        Scan(ConsentStore + @"\NonPackaged", true);

        return users.Count == 0
            ? new CheckResult { Id = "camuse", Category = "Kamera", Title = "Kamerayı başka uygulama kullanıyor mu", Status = CheckStatus.Ok, Detail = "Hayır" }
            : new CheckResult
            {
                Id = "camuse", Category = "Kamera", Title = "Kamerayı başka uygulama kullanıyor mu", Status = CheckStatus.Warn,
                Detail = "Kamerayı şu an kullanan: " + string.Join(", ", users.Distinct()),
                Hint = "Webcam'i aynı anda tek uygulama açabilir: bu uygulamada kamerayı kapat ya da uygulamayı kapat.",
            };
    }

    private static async Task<CheckResult> CheckCameraOpens(List<PnpCamera>? devices)
    {
        const string title = "Kamera açılıyor mu";
        if (EyeManager.IsRunning)
            return new CheckResult { Id = "camopen", Category = "Kamera", Title = title, Status = CheckStatus.Ok, Detail = "Kamera şu an PalmSync tarafından kullanılıyor" };
        if (!EyeManager.IsAvailable)
            return new CheckResult { Id = "camopen", Category = "Kamera", Title = title, Status = CheckStatus.Skipped, Detail = "Kamera modülü yok" };
        if (devices != null && !devices.Any(d => d.Status == "OK"))
            return new CheckResult { Id = "camopen", Category = "Kamera", Title = title, Status = CheckStatus.Skipped, Detail = "Kamera bağlı değil" };

        try
        {
            using var p = Process.Start(new ProcessStartInfo(EyeManager.EyePath, $"--probe --log-dir \"{AppLog.LogDir}\"")
            {
                UseShellExecute = false, CreateNoWindow = true,
                RedirectStandardOutput = true, RedirectStandardError = true,
                StandardErrorEncoding = Encoding.UTF8,
                WorkingDirectory = Path.GetDirectoryName(EyeManager.EyePath)!,
            })!;
            var stdout = p.StandardOutput.ReadToEndAsync();
            var stderr = p.StandardError.ReadToEndAsync();
            var exited = await Task.Run(() => p.WaitForExit(30000));
            if (!exited)
            {
                try { p.Kill(true); } catch { }
                return new CheckResult { Id = "camopen", Category = "Kamera", Title = title, Status = CheckStatus.Warn, Detail = "Kamera 30 saniyede açılamadı (sürücü takıldı olabilir)", Hint = "Kamerayı çıkarıp tak ya da bilgisayarı yeniden başlat." };
            }
            var outText = await stdout;
            if (p.ExitCode == 0)
            {
                var mono = outText.Contains("mono=1");
                return new CheckResult
                {
                    Id = "camopen", Category = "Kamera", Title = title,
                    Status = mono ? CheckStatus.Warn : CheckStatus.Ok,
                    Detail = mono ? "Sadece kızılötesi (Windows Hello) kamera açıldı — el hareketleri çalışmaz"
                                  : "Kamera açıldı ve görüntü geldi (" + outText.Trim().Replace("PROBE_OK ", "") + ")",
                    Hint = mono ? "Normal (renkli) bir webcam tak." : null,
                };
            }
            return new CheckResult
            {
                Id = "camopen", Category = "Kamera", Title = title, Status = CheckStatus.Fail,
                Detail = p.ExitCode == 3 ? "Kamera açıldı ama görüntü gelmedi" : "Kamera açılamadı (DirectShow / MSMF)",
                Hint = "Kamerayı kullanan başka uygulamayı kapat, kamera iznini kontrol et; olmazsa kamerayı çıkarıp tak.",
            };
        }
        catch (Exception ex)
        {
            return new CheckResult { Id = "camopen", Category = "Kamera", Title = title, Status = CheckStatus.Fail, Detail = "Kamera modülü başlatılamadı: " + ex.Message };
        }
    }

    // ── Network ─────────────────────────────────────────────

    private static CheckResult CheckNetwork()
    {
        var ip = NetworkInfo.GetLanIp();
        return new CheckResult
        {
            Id = "net", Category = "Ağ", Title = "Yerel ağ bağlantısı",
            Status = ip != null ? CheckStatus.Ok : CheckStatus.Fail,
            Detail = ip != null ? "Adres: " + ip : "Wi-Fi / kablolu ağ bağlantısı yok",
            Hint = ip != null ? null : "İki bilgisayarı aynı Wi-Fi ağına bağla.",
            OpenUri = ip != null ? null : "ms-settings:network",
        };
    }

    private static CheckResult CheckFirewall()
    {
        var ok = FirewallHelper.RuleExists();
        var daemon = DaemonManager.DaemonPath;
        return new CheckResult
        {
            Id = "fw", Category = "Ağ", Title = "Güvenlik duvarı izni",
            Status = ok ? CheckStatus.Ok : CheckStatus.Warn,
            Detail = ok ? "PalmSync gelen bağlantı kuralı var" : "Kural yok — bu bilgisayar oturum açarsa diğeri bağlanamaz",
            Hint = ok ? null : "Düzelt'e bas (yönetici izni ister).",
            AdminFix = ok ? null : new Elevation.Step("Güvenlik duvarı izni",
                $"  netsh advfirewall firewall add rule name=\"PalmSync\" dir=in action=allow program=\"{daemon}\" enable=yes profile=any | Out-Null\n" +
                "  netsh advfirewall firewall add rule name=\"PalmSync (port)\" dir=in action=allow protocol=TCP localport=9001 profile=any | Out-Null"),
        };
    }

    private static CheckResult CheckPorts()
    {
        const string title = "PalmSync portları";
        if (DaemonManager.IsRunning)
            return new CheckResult { Id = "ports", Category = "Ağ", Title = title, Status = CheckStatus.Ok, Detail = "Arka plan servisi portları kullanıyor" };

        var cfg = ConfigManager.Load();
        var problems = new List<string>();
        bool udpFree = true;
        try { using var u = new UdpClient(new IPEndPoint(IPAddress.Loopback, cfg.IpcPort)); }
        catch (SocketException) { udpFree = false; problems.Add($"UDP {cfg.IpcPort} (kamera → servis) başka bir programda"); }
        try { var t = new TcpListener(IPAddress.Any, 9001); t.Start(); t.Stop(); }
        catch (SocketException) { problems.Add("TCP 9001 (oturum) başka bir programda"); }

        return new CheckResult
        {
            Id = "ports", Category = "Ağ", Title = title,
            Status = problems.Count == 0 ? CheckStatus.Ok : CheckStatus.Fail,
            Detail = problems.Count == 0 ? $"UDP {cfg.IpcPort} ve TCP 9001 boş" : string.Join("; ", problems),
            Hint = problems.Count == 0 ? null : udpFree ? "9001 portunu kullanan programı kapat." : "Düzelt'e bas: boş bir port seçilir.",
            Fix = udpFree ? null : () =>
            {
                using var probe = new UdpClient(new IPEndPoint(IPAddress.Loopback, 0));
                var free = (ushort)((IPEndPoint)probe.Client.LocalEndPoint!).Port;
                probe.Dispose();
                var c = ConfigManager.Load();
                c.IpcPort = free;
                ConfigManager.Save(c);
                AppLog.Info($"Kamera → servis portu {free} olarak değiştirildi");
                return Task.CompletedTask;
            },
        };
    }

    // ── Permissions ─────────────────────────────────────────

    private static string CertPath => Path.Combine(AppDir, "PalmSync-PublicCert.cer");

    private static Elevation.Step? CertificateFix() => File.Exists(CertPath)
        ? new Elevation.Step("PalmSync sertifikası",
            $"  Import-Certificate -FilePath '{Elevation.Escape(CertPath)}' -CertStoreLocation Cert:\\LocalMachine\\Root | Out-Null\n" +
            $"  Import-Certificate -FilePath '{Elevation.Escape(CertPath)}' -CertStoreLocation Cert:\\LocalMachine\\TrustedPublisher | Out-Null")
        : null;

    private static CheckResult CheckCertificate()
    {
        const string title = "PalmSync sertifikası";
        if (!File.Exists(CertPath))
            return new CheckResult { Id = "cert", Category = "İzinler", Title = title, Status = CheckStatus.Skipped, Detail = "Sertifika dosyası yok (geliştirici kurulumu)" };

        using var cert = new X509Certificate2(CertPath);
        var thumb = cert.Thumbprint;
        bool In(StoreName name)
        {
            using var store = new X509Store(name, StoreLocation.LocalMachine);
            store.Open(OpenFlags.ReadOnly);
            return store.Certificates.Find(X509FindType.FindByThumbprint, thumb, false).Count > 0;
        }
        var ok = In(StoreName.Root) && In(StoreName.TrustedPublisher);
        return new CheckResult
        {
            Id = "cert", Category = "İzinler", Title = title,
            Status = ok ? CheckStatus.Ok : CheckStatus.Warn,
            Detail = ok ? "Güvenilir" : "Güvenilir değil — Windows / antivirüs programı engelleyebilir",
            Hint = ok ? null : "Düzelt'e bas (yönetici izni ister).",
            AdminFix = ok ? null : CertificateFix(),
        };
    }

    private static CheckResult CheckSmartAppControl()
    {
        var v = Registry.GetValue(@"HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\CI\Policy", "VerifiedAndReputablePolicyState", 0);
        var on = v is int i && i == 1;
        return new CheckResult
        {
            Id = "sac", Category = "İzinler", Title = "Akıllı Uygulama Denetimi",
            Status = on ? CheckStatus.Warn : CheckStatus.Ok,
            Detail = on ? "Açık — imzası tanınmayan programları engelleyebilir" : "Kapalı / değerlendirme modunda",
            Hint = on ? "Program engellenirse: Windows Güvenliği → Uygulama ve tarayıcı denetimi → Akıllı Uygulama Denetimi → Kapalı." : null,
            OpenUri = on ? "windowsdefender://appbrowser" : null,
        };
    }

    private static CheckResult CheckNotifications()
    {
        var global = Registry.GetValue(@"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\PushNotifications", "ToastEnabled", 1);
        var off = global is int i && i == 0;
        return new CheckResult
        {
            Id = "notif", Category = "İzinler", Title = "Bildirimler",
            Status = off ? CheckStatus.Warn : CheckStatus.Ok,
            Detail = off ? "Windows bildirimleri kapalı — 'Yakalandı / Geri yüklendi' bildirimleri görünmez" : "Açık",
            Fix = off ? () =>
            {
                Registry.SetValue(@"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\PushNotifications", "ToastEnabled", 1, RegistryValueKind.DWord);
                return Task.CompletedTask;
            } : null,
        };
    }

    private const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";

    private static CheckResult CheckAutostart()
    {
        bool Has(RegistryKey root) { using var k = root.OpenSubKey(RunKey); return k?.GetValue("PalmSync") != null; }
        var ok = Has(Registry.LocalMachine) || Has(Registry.CurrentUser);
        return new CheckResult
        {
            Id = "autostart", Category = "İzinler", Title = "Oturum açınca başlatma",
            Status = ok ? CheckStatus.Ok : CheckStatus.Warn,
            Detail = ok ? "PalmSync Windows ile başlıyor" : "Kayıtlı değil — bilgisayar açılınca oturum sürdürülmez",
            Fix = ok ? null : () =>
            {
                var exe = Environment.ProcessPath ?? Path.Combine(AppContext.BaseDirectory, "palmsync-ui.exe");
                Registry.SetValue(@"HKEY_CURRENT_USER\" + RunKey, "PalmSync", $"\"{exe}\" --silent");
                return Task.CompletedTask;
            },
        };
    }

    // ── System ──────────────────────────────────────────────

    private static CheckResult CheckConfig()
    {
        const string title = "Ayar dosyası";
        var path = ConfigManager.ConfigPath;
        if (!File.Exists(path))
            return new CheckResult { Id = "config", Category = "Sistem", Title = title, Status = CheckStatus.Ok, Detail = "Henüz yok (ilk oturumda oluşturulur)" };
        try
        {
            var cfg = JsonSerializer.Deserialize<PalmSyncConfig>(File.ReadAllText(path))
                      ?? throw new JsonException("boş dosya");
            var badId = string.IsNullOrEmpty(cfg.NodeId) || !cfg.NodeId.All(c => char.IsAsciiLetterOrDigit(c) || c is '-' or '_');
            return new CheckResult
            {
                Id = "config", Category = "Sistem", Title = title,
                Status = badId ? CheckStatus.Fail : CheckStatus.Ok,
                Detail = badId ? $"Cihaz kimliği geçersiz ('{cfg.NodeId}') — bağlantı reddedilir" : "Geçerli",
                Fix = badId ? () =>
                {
                    var c = ConfigManager.Load();
                    c.NodeId = new PalmSyncConfig().NodeId;
                    ConfigManager.Save(c);
                    return Task.CompletedTask;
                } : null,
            };
        }
        catch (Exception ex)
        {
            return new CheckResult
            {
                Id = "config", Category = "Sistem", Title = title, Status = CheckStatus.Fail,
                Detail = "Bozuk: " + ex.Message,
                Hint = "Düzelt'e bas: bozuk dosya yedeklenip yenisi oluşturulur.",
                Fix = () =>
                {
                    File.Move(path, path + $".bozuk-{DateTime.Now:yyyyMMddHHmmss}", true);
                    ConfigManager.Save(new PalmSyncConfig());
                    return Task.CompletedTask;
                },
            };
        }
    }

    private static CheckResult CheckFolders()
    {
        var dirs = new[] { AppLog.LogDir, AppPaths.HealthDir, AppPaths.ReceivedDir };
        var bad = new List<string>();
        foreach (var d in dirs)
        {
            try
            {
                Directory.CreateDirectory(d);
                var probe = Path.Combine(d, ".yazma-testi");
                File.WriteAllText(probe, "ok");
                File.Delete(probe);
            }
            catch (Exception ex) { bad.Add($"{d} ({ex.Message})"); }
        }
        return new CheckResult
        {
            Id = "folders", Category = "Sistem", Title = "Belgeler\\PalmSync klasörleri",
            Status = bad.Count == 0 ? CheckStatus.Ok : CheckStatus.Fail,
            Detail = bad.Count == 0 ? AppPaths.DataDir : "Yazılamıyor: " + string.Join("; ", bad),
            Hint = bad.Count == 0 ? null
                : "Belgeler klasörü salt okunur ya da 'Denetimli klasör erişimi' engelliyor: Windows Güvenliği → Fidye yazılımı koruması'ndan PalmSync'e izin ver.",
            OpenUri = bad.Count == 0 ? null : "windowsdefender://ransomwareprotection",
        };
    }

    /// Which editors/browsers a received file or page can open in here. Purely
    /// informational: the daemon falls back to an equivalent app, the file's
    /// default program or a screenshot, so nothing is required.
    private static CheckResult CheckApps()
    {
        var pf = Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles);
        var local = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        var editors = new List<string>();
        if (FindCodeCli() != null) editors.Add("VS Code");
        if (File.Exists(Path.Combine(pf, @"Sublime Text\sublime_text.exe")) ||
            File.Exists(Path.Combine(pf, @"Sublime Text 3\sublime_text.exe"))) editors.Add("Sublime Text");
        if (File.Exists(Path.Combine(pf, @"Notepad++\notepad++.exe"))) editors.Add("Notepad++");
        if (File.Exists(Path.Combine(local, @"Programs\cursor\Cursor.exe"))) editors.Add("Cursor");
        editors.Add("Not Defteri");

        string browser = "?";
        try
        {
            using var k = Registry.CurrentUser.OpenSubKey(
                @"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice");
            var progId = k?.GetValue("ProgId") as string ?? "";
            browser = progId switch
            {
                var p when p.StartsWith("ChromeHTML") => "Chrome",
                var p when p.StartsWith("BraveHTML") => "Brave",
                var p when p.StartsWith("MSEdgeHTM") => "Edge",
                var p when p.StartsWith("FirefoxURL") => "Firefox",
                var p when p.StartsWith("Opera") => "Opera",
                var p when p.StartsWith("VivaldiHTM") => "Vivaldi",
                "" => "Edge (Windows varsayılanı)",
                var p => p,
            };
        }
        catch { }

        return new CheckResult
        {
            Id = "apps", Category = "Sistem", Title = "Aktarım için uygulamalar",
            Status = CheckStatus.Ok,
            Detail = $"Düzenleyiciler: {string.Join(", ", editors)} · Varsayılan tarayıcı: {browser}. " +
                     "Gelen içeriğin uygulaması burada yoksa eşdeğeri, yoksa varsayılan program, o da yoksa ekran görüntüsü açılır.",
        };
    }

    private static string? FindCodeCli()
    {
        foreach (var dir in (Environment.GetEnvironmentVariable("PATH") ?? "").Split(';', StringSplitOptions.RemoveEmptyEntries))
        {
            try { var p = Path.Combine(dir.Trim(), "code.cmd"); if (File.Exists(p)) return p; } catch { }
        }
        var candidates = new[]
        {
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), @"Programs\Microsoft VS Code\bin\code.cmd"),
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles), @"Microsoft VS Code\bin\code.cmd"),
        };
        return candidates.FirstOrDefault(File.Exists);
    }

    // ── Helpers ─────────────────────────────────────────────

    private static string RunPowerShell(string command)
    {
        using var p = Process.Start(new ProcessStartInfo("powershell.exe",
            $"-NoProfile -NonInteractive -Command \"[Console]::OutputEncoding=[Text.Encoding]::UTF8; {command.Replace("\"", "\\\"")}\"")
        {
            UseShellExecute = false, CreateNoWindow = true,
            RedirectStandardOutput = true, StandardOutputEncoding = Encoding.UTF8,
        })!;
        var output = p.StandardOutput.ReadToEnd();
        p.WaitForExit(15000);
        return output.Trim();
    }
}

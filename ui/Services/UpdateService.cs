using System.Diagnostics;
using System.IO;
using System.Net.Http;
using System.Reflection;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Text.Json;

namespace PalmSync.Tray.Services;

public record UpdateInfo(Version Version, string Tag, string Notes, string SetupUrl, string? ShaUrl, long Size, string PageUrl);

/// In-app updates from GitHub Releases.
///
/// release.ps1 publishes `PalmSync-Setup-x.y.z.exe` + `.sha256` as a release
/// on GitHub. The app checks the latest release at start-up and every few
/// hours; if it is newer than the running version it offers it (or installs
/// it by itself when the user chose automatic updates). The download is
/// checked against the published SHA-256 — and, if the setup is signed,
/// against the PalmSync certificate — before it is run.
public static class UpdateService
{
    /// The GitHub repository releases are published to (owner/name).
    public const string Repo = "SerdarK44/PalmSync-Gesture-Based-Context-Transfer-Between-Devices";

    private static readonly HttpClient Http = CreateClient();
    private static readonly SemaphoreSlim Gate = new(1, 1);

    public static Version Current { get; } = ReadCurrentVersion();
    public static UpdateInfo? Available { get; private set; }
    public static DateTime LastCheck { get; private set; }
    public static string? LastError { get; private set; }

    /// Raised when `Available` changes or a download makes progress.
    public static event Action? Changed;
    /// 0..100 while downloading, -1 otherwise.
    public static int DownloadPercent { get; private set; } = -1;

    private static HttpClient CreateClient()
    {
        var c = new HttpClient { Timeout = TimeSpan.FromMinutes(10) };
        c.DefaultRequestHeaders.UserAgent.ParseAdd("PalmSync-Updater");
        c.DefaultRequestHeaders.Accept.ParseAdd("application/vnd.github+json");
        return c;
    }

    private static Version ReadCurrentVersion()
    {
        var info = typeof(UpdateService).Assembly
            .GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion;
        // "0.2.0+commit" -> "0.2.0"
        var text = info?.Split('+', '-')[0];
        return Version.TryParse(text, out var v) ? v
            : typeof(UpdateService).Assembly.GetName().Version ?? new Version(0, 0, 0);
    }

    public static string CurrentText => $"{Current.Major}.{Current.Minor}.{Current.Build}";

    // ── Checking ────────────────────────────────────────────

    /// Ask GitHub for the latest release. Returns the update if it is newer
    /// than this build (and not skipped by the user), else null.
    public static async Task<UpdateInfo?> CheckAsync(bool manual = false)
    {
        await Gate.WaitAsync();
        try
        {
            LastError = null;
            using var resp = await Http.GetAsync($"https://api.github.com/repos/{Repo}/releases/latest");
            LastCheck = DateTime.Now;
            if (resp.StatusCode == System.Net.HttpStatusCode.NotFound)
            {
                SetAvailable(null);
                if (manual) AppLog.Info("Güncelleme denetlendi: GitHub'da henüz yayınlanmış sürüm yok");
                return null;
            }
            resp.EnsureSuccessStatusCode();

            using var doc = JsonDocument.Parse(await resp.Content.ReadAsStringAsync());
            var root = doc.RootElement;
            var tag = root.GetProperty("tag_name").GetString() ?? "";
            if (!Version.TryParse(tag.TrimStart('v', 'V'), out var latest))
                throw new InvalidDataException($"Release etiketi sürüm değil: '{tag}'");

            string? setupUrl = null, shaUrl = null;
            long size = 0;
            foreach (var a in root.GetProperty("assets").EnumerateArray())
            {
                var name = a.GetProperty("name").GetString() ?? "";
                var url = a.GetProperty("browser_download_url").GetString();
                if (name.StartsWith("PalmSync-Setup-", StringComparison.OrdinalIgnoreCase) &&
                    name.EndsWith(".exe", StringComparison.OrdinalIgnoreCase))
                {
                    setupUrl = url;
                    size = a.GetProperty("size").GetInt64();
                }
                else if (name.EndsWith(".exe.sha256", StringComparison.OrdinalIgnoreCase))
                    shaUrl = url;
            }

            var newer = latest > Current;
            var skipped = !manual && UISettingsManager.Load().SkippedVersion == latest.ToString();
            if (!newer || setupUrl == null || skipped)
            {
                SetAvailable(null);
                if (manual)
                    AppLog.Info(newer && setupUrl == null
                        ? $"v{latest} yayınlanmış ama kurulum dosyası eklenmemiş"
                        : $"Güncelleme denetlendi: en güncel sürüm kullanılıyor (v{CurrentText})");
                return null;
            }

            var info = new UpdateInfo(latest, tag,
                root.TryGetProperty("body", out var b) ? b.GetString() ?? "" : "",
                setupUrl, shaUrl, size,
                root.TryGetProperty("html_url", out var h) ? h.GetString() ?? "" : "");
            if (Available?.Version != info.Version)
                AppLog.Info($"⬆ Yeni sürüm var: v{latest} (kurulu: v{CurrentText})");
            SetAvailable(info);
            return info;
        }
        catch (Exception ex)
        {
            LastError = ex.Message;
            AppLog.Debug("Güncelleme denetlenemedi: " + ex.Message);   // offline is normal
            return null;
        }
        finally
        {
            Gate.Release();
        }
    }

    private static void SetAvailable(UpdateInfo? info)
    {
        Available = info;
        Changed?.Invoke();
    }

    // ── Installing ──────────────────────────────────────────

    /// Download, verify, start the installer silently and close PalmSync so
    /// the files can be replaced. The installer starts PalmSync again.
    /// Returns an error message, or null when the installer was started.
    public static async Task<string?> DownloadAndInstallAsync(UpdateInfo info)
    {
        var dir = Path.Combine(Path.GetTempPath(), "PalmSync", "update");
        Directory.CreateDirectory(dir);
        var file = Path.Combine(dir, $"PalmSync-Setup-{info.Version}.exe");

        try
        {
            AppLog.Info($"⬇ v{info.Version} indiriliyor…");
            using (var resp = await Http.GetAsync(info.SetupUrl, HttpCompletionOption.ResponseHeadersRead))
            {
                resp.EnsureSuccessStatusCode();
                var total = resp.Content.Headers.ContentLength ?? info.Size;
                await using var src = await resp.Content.ReadAsStreamAsync();
                await using var dst = File.Create(file);
                var buf = new byte[81920];
                long done = 0;
                int n;
                while ((n = await src.ReadAsync(buf)) > 0)
                {
                    await dst.WriteAsync(buf.AsMemory(0, n));
                    done += n;
                    var pct = total > 0 ? (int)(done * 100 / total) : -1;
                    if (pct != DownloadPercent) { DownloadPercent = pct; Changed?.Invoke(); }
                }
            }

            var problem = await VerifyAsync(info, file);
            if (problem != null)
            {
                File.Delete(file);
                AppLog.Error($"İndirilen güncelleme doğrulanamadı, kurulmadı: {problem}");
                return "İndirilen dosya doğrulanamadı, güvenlik için kurulmadı.\n\n" + problem;
            }

            var log = Path.Combine(AppLog.LogDir, "guncelleme-kurulum.log");
            AppLog.Info($"🔄 v{info.Version} kuruluyor (Windows yönetici izni isteyecek)");
            Process.Start(new ProcessStartInfo(file,
                $"/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS /LOG=\"{log}\"")
            {
                UseShellExecute = true,   // the setup asks for admin rights itself
            });
            return null;
        }
        catch (System.ComponentModel.Win32Exception ex) when (ex.NativeErrorCode == 1223)
        {
            AppLog.Warn("Güncelleme kurulumu iptal edildi (yönetici izni verilmedi)");
            return "Güncelleme için Windows'un yönetici iznine 'Evet' demen gerekiyor.";
        }
        catch (Exception ex)
        {
            AppLog.Error("Güncelleme indirilemedi/kurulamadı", ex);
            return "Güncelleme indirilemedi: " + ex.Message;
        }
        finally
        {
            DownloadPercent = -1;
            Changed?.Invoke();
        }
    }

    /// SHA-256 must match the published `.sha256`; if the setup carries an
    /// Authenticode signature, its certificate must be the PalmSync one.
    private static async Task<string?> VerifyAsync(UpdateInfo info, string file)
    {
        if (info.ShaUrl == null) return "Sürüm sayfasında SHA-256 özeti (.sha256) yok.";

        var expected = (await Http.GetStringAsync(info.ShaUrl)).Trim().Split(' ', '\t')[0].ToLowerInvariant();
        string actual;
        await using (var fs = File.OpenRead(file))
            actual = Convert.ToHexString(await SHA256.HashDataAsync(fs)).ToLowerInvariant();
        if (expected != actual) return $"SHA-256 uyuşmuyor (beklenen {expected[..12]}…, gelen {actual[..12]}…).";

        try
        {
#pragma warning disable SYSLIB0057
            using var signer = new X509Certificate2(X509Certificate.CreateFromSignedFile(file));
#pragma warning restore SYSLIB0057
            var ourCert = Path.Combine(AppContext.BaseDirectory, "..", "PalmSync-PublicCert.cer");
            if (File.Exists(ourCert))
            {
#pragma warning disable SYSLIB0057
                using var ours = new X509Certificate2(ourCert);
#pragma warning restore SYSLIB0057
                if (!string.Equals(signer.Thumbprint, ours.Thumbprint, StringComparison.OrdinalIgnoreCase))
                    return $"Kurulum dosyası başka bir sertifikayla imzalanmış ({signer.Subject}).";
            }
        }
        catch (CryptographicException)
        {
            // Not signed: the SHA-256 check above is what protects us.
        }
        return null;
    }

    /// Download + install, then close PalmSync (the installer reopens it).
    /// Must be called on the UI thread.
    public static async Task InstallAndExitAsync(UpdateInfo info, bool quiet = false)
    {
        var error = await DownloadAndInstallAsync(info);
        if (error == null)
        {
            System.Windows.Application.Current.Shutdown();
            return;
        }
        if (!quiet)
            System.Windows.MessageBox.Show(error, "PalmSync — Güncelleme",
                System.Windows.MessageBoxButton.OK, System.Windows.MessageBoxImage.Warning);
    }

    public static void Skip(UpdateInfo info)
    {
        var s = UISettingsManager.Load();
        s.SkippedVersion = info.Version.ToString();
        UISettingsManager.Save(s);
        AppLog.Info($"v{info.Version} atlandı (bir sonraki sürümde tekrar sorulur)");
        SetAvailable(null);
    }

    // ── Background schedule ─────────────────────────────────

    private static System.Threading.Timer? _timer;

    /// Check shortly after start-up, then every 6 hours.
    public static void StartBackgroundChecks(Func<UpdateInfo, Task> onAvailable)
    {
        _timer?.Dispose();
        _timer = new System.Threading.Timer(async _ =>
        {
            if (UISettingsManager.Load().UpdateMode == "off") return;
            var info = await CheckAsync();
            if (info != null) await onAvailable(info);
        }, null, TimeSpan.FromSeconds(15), TimeSpan.FromHours(6));
    }
}

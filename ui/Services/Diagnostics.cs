using System.IO;
using System.Text;

namespace PalmSync.Tray.Services;

/// Produces a plain-language connection report. The user runs this on BOTH
/// machines when a session won't connect; comparing the two reports pinpoints
/// the cause (wrong IP, firewall, different network, daemon not running).
public static class Diagnostics
{
    public static string Run()
    {
        var sb = new StringBuilder();
        var cfg = ConfigManager.Load();

        void H(string s) => sb.AppendLine().AppendLine("── " + s + " ──");
        void L(string s) => sb.AppendLine(s);

        L("PalmSync Sorun Giderme Raporu");
        L(DateTime.Now.ToString("dd.MM.yyyy HH:mm:ss"));
        L("Bilgisayar: " + Environment.MachineName);
        L("Windows: " + Environment.OSVersion.VersionString);

        H("Bu bilgisayarın ağ adresleri");
        var candidates = NetworkInfo.GetCandidates();
        if (candidates.Count == 0)
            L("  ⚠ Yerel IPv4 adresi yok — Wi-Fi / kabloya bağlı değil mi?");
        else
        {
            var chosen = NetworkInfo.GetLanIp();
            foreach (var ip in candidates)
                L($"  {ip}{(ip.Equals(chosen) ? "   <- oturum için kullanılan" : "")}");
            if (candidates.Count > 1)
                L("  (Birden fazla ağ var — iki bilgisayar bunlardan AYNI birinde olmalı.)");
        }

        H("Oturum");
        L("  Rol: " + cfg.Role switch { "master" => "ev sahibi", "slave" => "katılımcı", _ => "yok" });
        L("  Arka plan servisi çalışıyor: " + (DaemonManager.IsRunning ? "evet" : "HAYIR"));
        if (cfg.Role == "master")
            L($"  {cfg.MasterPort} portunda dinleniyor");
        else if (cfg.Role == "slave")
            L($"  Ev sahibine bağlanılıyor: {cfg.MasterAddr}:{cfg.MasterPort}");
        else
            L("  Aktif oturum yok (önce bir oturum başlat ya da katıl).");

        H("Güvenlik duvarı (EV SAHİBİ bilgisayarda önemli)");
        L("  PalmSync gelen bağlantı kuralı: " +
          (FirewallHelper.RuleExists() ? "var ✓" : "YOK ✗ (ev sahibi bağlantı kabul edemez)"));

        if (cfg.Role == "slave" && !string.IsNullOrEmpty(cfg.MasterAddr))
        {
            H("Ev sahibine erişim");
            bool ok = NetworkInfo.CanReach(cfg.MasterAddr, cfg.MasterPort);
            L($"  {cfg.MasterAddr}:{cfg.MasterPort} — {(ok ? "ERİŞİLEBİLİR ✓ (ağ sorunsuz)" : "ERİŞİLEMİYOR ✗")}");
            L(ok
                ? "  => Hâlâ bağlanmıyorsa iki bilgisayardaki şifre farklıdır."
                : "  => Ev sahibinde güvenlik duvarı, farklı ağ ya da modemde istemci yalıtımı.");
        }

        H("Kamera");
        L("  Kamera modülü: " + (EyeManager.IsAvailable ? "kurulu ✓" : "BULUNAMADI ✗"));
        L("  Çalışıyor: " + (EyeManager.IsRunning ? "evet" : "hayır"));
        try
        {
            var cams = CameraDeviceService.GetCamerasAsync().GetAwaiter().GetResult();
            if (cams.Count == 0) L("  ⚠ Windows hiç kamera görmüyor");
            foreach (var c in cams) L("  • " + c.Name);
        }
        catch (Exception ex) { L("  (kamera listesi alınamadı: " + ex.Message + ")"); }

        H("Sistem kontrolü");
        if (HealthCheck.Last.Count == 0) L("  Henüz çalıştırılmadı (🔧 Sistem kontrolü).");
        else
        {
            L($"  {HealthCheck.LastRun:HH:mm:ss} itibarıyla:");
            foreach (var r in HealthCheck.Last.Where(r => r.Status != CheckStatus.Ok))
                L($"  {r.StatusText,-7} {r.Title}: {r.Detail}");
            if (HealthCheck.ProblemCount == 0) L("  Her şey yolunda ✓");
        }

        H("Son uyarı ve hatalar (kayıtlardan)");
        var problems = new List<LogEntry>();
        foreach (var (name, file) in new[] { ("Daemon", "palmsync-brain.log"), ("Kamera", "palmsync-eye.log"), ("Arayüz", "palmsync-ui.log") })
            problems.AddRange(new LogSource(name, Path.Combine(cfg.LogDir, file))
                .ReadNew(512 * 1024).Where(e => e.Level >= LogLevel.Warn));
        var last = problems.OrderBy(e => e.Time).TakeLast(12).ToList();
        if (last.Count == 0) L("  Yok ✓");
        foreach (var e in last)
            L($"  {e.Time:dd.MM HH:mm:ss} {e.LevelText,-5} {e.Source,-7} {e.FirstLine}");

        return sb.ToString();
    }
}

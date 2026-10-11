using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Text;

namespace PalmSync.Tray.Services;

/// Runs several administrator fixes (firewall, certificate, camera policy,
/// VC++ runtime...) in ONE elevated PowerShell, i.e. a single UAC prompt
/// instead of one per fix. Each step reports OK/HATA to a result file that
/// the UI reads back and logs.
public static class Elevation
{
    public record Step(string Title, string Script);
    public record StepResult(string Title, bool Ok, string Detail);

    /// Returns null if the user declined the UAC prompt.
    public static async Task<List<StepResult>?> RunAsync(IReadOnlyList<Step> steps)
    {
        if (steps.Count == 0) return new();

        var dir = Path.Combine(Path.GetTempPath(), "PalmSync");
        Directory.CreateDirectory(dir);
        var id = Guid.NewGuid().ToString("N")[..8];
        var script = Path.Combine(dir, $"duzelt-{id}.ps1");
        var results = Path.Combine(dir, $"duzelt-{id}.txt");

        var sb = new StringBuilder();
        sb.AppendLine("$ErrorActionPreference = 'Stop'");
        sb.AppendLine("$ProgressPreference = 'SilentlyContinue'");
        sb.AppendLine($"$out = '{Escape(results)}'");
        sb.AppendLine("function Report($t, $ok, $d) { Add-Content -LiteralPath $out -Encoding UTF8 -Value (\"{0}`t{1}`t{2}\" -f $t, $ok, $d) }");
        foreach (var s in steps)
        {
            sb.AppendLine("try {");
            sb.AppendLine(s.Script);
            sb.AppendLine($"  Report '{Escape(s.Title)}' 'OK' ''");
            sb.AppendLine("} catch {");
            sb.AppendLine($"  Report '{Escape(s.Title)}' 'HATA' ($_.Exception.Message -replace \"`r|`n\", ' ')");
            sb.AppendLine("}");
        }
        // UTF-8 with BOM: Windows PowerShell 5.1 reads BOM-less scripts as ANSI.
        await File.WriteAllTextAsync(script, sb.ToString(), new UTF8Encoding(true));

        try
        {
            var psi = new ProcessStartInfo("powershell.exe",
                $"-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File \"{script}\"")
            {
                UseShellExecute = true,   // required for Verb=runas
                Verb = "runas",
                WindowStyle = ProcessWindowStyle.Hidden,
            };
            AppLog.Info($"🔐 Yönetici izni isteniyor ({steps.Count} düzeltme: {string.Join(", ", steps.Select(s => s.Title))})");
            using var p = Process.Start(psi);
            if (p != null) await p.WaitForExitAsync().WaitAsync(TimeSpan.FromMinutes(10));
        }
        catch (Win32Exception ex) when (ex.NativeErrorCode == 1223)
        {
            AppLog.Warn("Yönetici izni verilmedi (UAC iptal edildi) — yönetici gerektiren düzeltmeler yapılmadı");
            return null;
        }
        catch (Exception ex)
        {
            AppLog.Error("Yönetici düzeltmeleri çalıştırılamadı", ex);
            return steps.Select(s => new StepResult(s.Title, false, ex.Message)).ToList();
        }
        finally
        {
            try { File.Delete(script); } catch { }
        }

        var list = new List<StepResult>();
        try
        {
            if (File.Exists(results))
            {
                foreach (var line in await File.ReadAllLinesAsync(results))
                {
                    var parts = line.Split('\t');
                    if (parts.Length >= 2)
                        list.Add(new StepResult(parts[0].TrimStart('﻿'), parts[1] == "OK", parts.Length > 2 ? parts[2] : ""));
                }
                File.Delete(results);
            }
        }
        catch { }

        foreach (var s in steps.Where(s => list.All(r => r.Title != s.Title)))
            list.Add(new StepResult(s.Title, false, "sonuç alınamadı"));
        foreach (var r in list)
        {
            if (r.Ok) AppLog.Info($"✅ Düzeltildi: {r.Title}");
            else AppLog.Warn($"Düzeltilemedi: {r.Title} — {r.Detail}");
        }
        return list;
    }

    /// Escape for a single-quoted PowerShell string.
    public static string Escape(string s) => s.Replace("'", "''");
}

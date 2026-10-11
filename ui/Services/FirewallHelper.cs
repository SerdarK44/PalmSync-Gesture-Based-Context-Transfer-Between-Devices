using System.Diagnostics;

namespace PalmSync.Tray.Services;

/// Ensures the inbound firewall rule for the daemon exists. The installer adds
/// it, but this self-heals when the daemon runs from a non-installed location,
/// the installer rule was removed, or install ran without admin.
public static class FirewallHelper
{
    private const string RuleName = "PalmSync";

    public static bool RuleExists()
    {
        try
        {
            var psi = new ProcessStartInfo
            {
                FileName = "netsh",
                Arguments = $"advfirewall firewall show rule name=\"{RuleName}\"",
                UseShellExecute = false,
                RedirectStandardOutput = true,
                CreateNoWindow = true,
            };
            using var p = Process.Start(psi);
            if (p == null) return false;
            var outp = p.StandardOutput.ReadToEnd();
            p.WaitForExit(4000);
            // Localized "No rules match" — detect by absence of the rule name
            return outp.Contains(RuleName, StringComparison.OrdinalIgnoreCase)
                   && !outp.Contains("No rules", StringComparison.OrdinalIgnoreCase);
        }
        catch
        {
            return false;
        }
    }

    /// Add the rule via an elevated netsh (one UAC prompt). Returns true if the
    /// rule exists afterwards. No-op if it already exists.
    public static bool EnsureRule(string daemonPath)
    {
        if (RuleExists()) return true;

        var args =
            $"advfirewall firewall add rule name=\"{RuleName}\" dir=in action=allow " +
            $"program=\"{daemonPath}\" enable=yes profile=any";

        try
        {
            var psi = new ProcessStartInfo
            {
                FileName = "netsh",
                Arguments = args,
                UseShellExecute = true,   // required for Verb=runas
                Verb = "runas",           // UAC elevation
                CreateNoWindow = true,
                WindowStyle = ProcessWindowStyle.Hidden,
            };
            using var p = Process.Start(psi);
            p?.WaitForExit(15000);
            return RuleExists();
        }
        catch
        {
            // User declined UAC, or netsh failed
            return false;
        }
    }
}

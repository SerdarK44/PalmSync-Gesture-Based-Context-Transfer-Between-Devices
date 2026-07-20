using System.IO;

namespace PalmSync.Tray.Services;

public enum PeerStatus
{
    Waiting,    // hosting but nobody connected / slave not yet connected
    Connected,  // a peer is connected
}

/// Derives live connection state by tailing the daemon log. Cheap and robust
/// enough for a status indicator; the daemon is the source of truth.
public static class LogWatcher
{
    public static PeerStatus GetStatus(bool isMaster)
    {
        var lines = TailLines(96);

        // Walk newest → oldest; the first decisive marker wins.
        for (var i = lines.Count - 1; i >= 0; i--)
        {
            var line = lines[i];
            if (isMaster)
            {
                if (line.Contains("authenticated from")) return PeerStatus.Connected;
                if (line.Contains("disconnected")) return PeerStatus.Waiting;
            }
            else
            {
                if (line.Contains("Authenticated with master")) return PeerStatus.Connected;
                if (line.Contains("Disconnected from master")
                    || line.Contains("Connection failed")) return PeerStatus.Waiting;
            }
        }
        return PeerStatus.Waiting;
    }

    private static List<string> TailLines(int max)
    {
        var result = new List<string>();
        try
        {
            var path = Path.Combine(ConfigManager.ConfigDir, "logs", "palmsync-brain.log");
            if (!File.Exists(path)) return result;

            using var fs = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite);
            using var reader = new StreamReader(fs);
            string? line;
            while ((line = reader.ReadLine()) != null)
            {
                result.Add(line);
                if (result.Count > max) result.RemoveAt(0);
            }
        }
        catch
        {
            // best effort
        }
        return result;
    }
}

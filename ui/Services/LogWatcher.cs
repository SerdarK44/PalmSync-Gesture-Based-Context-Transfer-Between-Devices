using System.IO;

namespace PalmSync.Tray.Services;

public enum PeerStatus
{
    Waiting,    // hosting but nobody connected / slave not yet connected
    Connected,  // a peer is connected
}

/// Derives live connection state by following the daemon log. Cheap and
/// robust enough for a status indicator; the daemon is the source of truth.
/// Reads only newly appended lines on each call (it used to re-read the
/// whole file every 2 s).
public static class LogWatcher
{
    private static readonly object Lock = new();
    private static LogSource? _source;
    private static PeerStatus _master = PeerStatus.Waiting;
    private static PeerStatus _slave = PeerStatus.Waiting;

    public static PeerStatus GetStatus(bool isMaster)
    {
        lock (Lock)
        {
            var path = Path.Combine(AppLog.LogDir, "palmsync-brain.log");
            if (_source == null || _source.Path != path)
            {
                _source = new LogSource("Daemon", path);
                _master = _slave = PeerStatus.Waiting;
            }

            foreach (var e in _source.ReadNew(512 * 1024))
            {
                var line = e.Message;
                if (line.Contains("PalmSync daemon started"))
                {
                    _master = _slave = PeerStatus.Waiting;   // markers from an older run don't count
                }
                else if (line.Contains("authenticated from")) _master = PeerStatus.Connected;
                else if (line.StartsWith("Slave ") && line.EndsWith(" disconnected")) _master = PeerStatus.Waiting;
                else if (line.Contains("Authenticated with master")) _slave = PeerStatus.Connected;
                else if (line.Contains("Disconnected from master") || line.Contains("Connection failed")
                         || line.StartsWith("Lost connection")) _slave = PeerStatus.Waiting;
            }
            return isMaster ? _master : _slave;
        }
    }
}

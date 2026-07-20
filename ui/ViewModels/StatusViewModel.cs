using System.Diagnostics;
using System.IO;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using PalmSync.Tray.Services;

namespace PalmSync.Tray.ViewModels;

public class StatusViewModel : ViewModelBase, IDisposable
{
    private readonly DispatcherTimer _timer;

    private string _connectionStatus = "Checking...";
    private Brush _statusBrush = Brushes.Gray;
    private string _connectionDetails = "";
    private string _cpuUsage = "—";
    private string _memoryUsage = "—";
    private string _lastLogLine = "—";

    public string ConnectionStatus { get => _connectionStatus; set => Set(ref _connectionStatus, value); }
    public Brush StatusBrush { get => _statusBrush; set => Set(ref _statusBrush, value); }
    public string ConnectionDetails { get => _connectionDetails; set => Set(ref _connectionDetails, value); }
    public string CpuUsage { get => _cpuUsage; set => Set(ref _cpuUsage, value); }
    public string MemoryUsage { get => _memoryUsage; set => Set(ref _memoryUsage, value); }
    public string LastLogLine { get => _lastLogLine; set => Set(ref _lastLogLine, value); }

    public ICommand RefreshCommand { get; }

    public StatusViewModel()
    {
        RefreshCommand = new RelayCommand(Refresh);
        _timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(3) };
        _timer.Tick += (_, _) => Refresh();
        _timer.Start();
        Refresh();
    }

    private void Refresh()
    {
        var config = ConfigManager.Load();

        if (!DaemonManager.IsRunning)
        {
            ConnectionStatus = "No Active Session";
            StatusBrush = Brushes.Gray;
            ConnectionDetails = "Open PalmSync and start or join a session.";
        }
        else
        {
            var isMaster = config.Role == "master";
            var peer = LogWatcher.GetStatus(isMaster);
            var connected = peer == PeerStatus.Connected;

            StatusBrush = connected ? Brushes.SeaGreen : Brushes.DarkOrange;
            ConnectionStatus = (config.Role, connected) switch
            {
                ("master", true) => "Hosting — device connected",
                ("master", false) => "Hosting — waiting",
                ("slave", true) => "Connected",
                ("slave", false) => "Connecting...",
                _ => "Running",
            };
            ConnectionDetails = isMaster
                ? $"Hosting on port {config.MasterPort} — IPC udp/{config.IpcPort}"
                : $"Master: {config.MasterAddr}:{config.MasterPort} — IPC udp/{config.IpcPort}";
        }

        RefreshProcessMetrics();
        RefreshLastLog(config.LogDir);
    }

    private void RefreshProcessMetrics()
    {
        try
        {
            var procs = Process.GetProcessesByName("palmsync-daemon");
            if (procs.Length == 0)
            {
                CpuUsage = "—";
                MemoryUsage = "not running";
                return;
            }

            long totalMem = 0;
            foreach (var p in procs)
            {
                totalMem += p.WorkingSet64;
                p.Dispose();
            }
            MemoryUsage = $"{totalMem / (1024.0 * 1024.0):F1} MB";
            CpuUsage = $"{procs.Length} process(es)";
        }
        catch
        {
            CpuUsage = "—";
            MemoryUsage = "—";
        }
    }

    private void RefreshLastLog(string logDir)
    {
        try
        {
            var logFile = Path.Combine(logDir, "palmsync-brain.log");
            if (!File.Exists(logFile))
            {
                LastLogLine = "(no log file)";
                return;
            }

            // Read only the tail without loading the whole file
            using var fs = new FileStream(logFile, FileMode.Open, FileAccess.Read, FileShare.ReadWrite);
            var readLen = (int)Math.Min(4096, fs.Length);
            fs.Seek(-readLen, SeekOrigin.End);
            var buf = new byte[readLen];
            fs.ReadExactly(buf, 0, readLen);
            var text = System.Text.Encoding.UTF8.GetString(buf);
            var lines = text.Split('\n', StringSplitOptions.RemoveEmptyEntries);
            LastLogLine = lines.Length > 0 ? lines[^1].Trim() : "(empty)";
        }
        catch
        {
            LastLogLine = "(log unavailable)";
        }
    }

    public void Dispose() => _timer.Stop();
}

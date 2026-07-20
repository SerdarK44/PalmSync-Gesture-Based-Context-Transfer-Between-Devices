using System.Windows;
using System.Windows.Input;
using System.Windows.Threading;
using PalmSync.Tray.Services;
using PalmSync.Tray.Views;

namespace PalmSync.Tray.ViewModels;

public class TrayViewModel : ViewModelBase, IDisposable
{
    private readonly DispatcherTimer _statusTimer;
    private string _statusText = "PalmSync";

    public string StatusText
    {
        get => _statusText;
        set => Set(ref _statusText, value);
    }

    public ICommand OpenHomeCommand { get; }
    public ICommand OpenSettingsCommand { get; }
    public ICommand OpenLogsCommand { get; }
    public ICommand ExitCommand { get; }

    public TrayViewModel()
    {
        OpenHomeCommand = new RelayCommand(OpenHome);
        OpenSettingsCommand = new RelayCommand(OpenSettings);
        OpenLogsCommand = new RelayCommand(OpenLogs);
        ExitCommand = new RelayCommand(() => Application.Current.Shutdown());

        _statusTimer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(5) };
        _statusTimer.Tick += (_, _) => RefreshStatus();
    }

    public void StartStatusPolling()
    {
        RefreshStatus();
        _statusTimer.Start();
    }

    private void RefreshStatus()
    {
        if (!DaemonManager.IsRunning)
        {
            StatusText = "PalmSync — no active session";
            return;
        }

        var role = SessionController.CurrentRole();
        var peer = LogWatcher.GetStatus(role == "master");
        StatusText = (role, peer) switch
        {
            ("master", PeerStatus.Connected) => "PalmSync — hosting, device connected",
            ("master", _) => "PalmSync — hosting, waiting",
            ("slave", PeerStatus.Connected) => "PalmSync — connected",
            ("slave", _) => "PalmSync — connecting...",
            _ => "PalmSync",
        };
    }

    private static void ShowSingleton<T>() where T : Window, new()
    {
        var existing = Application.Current.Windows.OfType<T>().FirstOrDefault();
        if (existing != null)
        {
            existing.Activate();
            return;
        }
        new T().Show();
    }

    private void OpenHome() => ShowSingleton<HomeWindow>();
    private void OpenSettings() => ShowSingleton<SettingsWindow>();
    private void OpenLogs() => ShowSingleton<LogViewerWindow>();

    public void Dispose() => _statusTimer.Stop();
}

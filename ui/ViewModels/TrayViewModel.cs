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
    public ICommand OpenTroubleshootCommand { get; }
    public ICommand OpenSystemCheckCommand { get; }
    public ICommand ExitCommand { get; }

    public TrayViewModel()
    {
        OpenHomeCommand = new RelayCommand(OpenHome);
        OpenSettingsCommand = new RelayCommand(OpenSettings);
        OpenLogsCommand = new RelayCommand(OpenLogs);
        OpenTroubleshootCommand = new RelayCommand(() => ShowSingleton<TroubleshootWindow>());
        OpenSystemCheckCommand = new RelayCommand(() => SystemCheckWindow.ShowWindow());
        ExitCommand = new RelayCommand(() =>
        {
            AppLog.Info("PalmSync kapatıldı");
            Application.Current.Shutdown();
        });

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
            StatusText = "PalmSync — aktif oturum yok";
            return;
        }

        var role = SessionController.CurrentRole();
        var peer = LogWatcher.GetStatus(role == "master");
        StatusText = (role, peer) switch
        {
            ("master", PeerStatus.Connected) => "PalmSync — oturum açık, cihaz bağlı",
            ("master", _) => "PalmSync — oturum açık, cihaz bekleniyor",
            ("slave", PeerStatus.Connected) => "PalmSync — bağlı",
            ("slave", _) => "PalmSync — bağlanıyor...",
            _ => "PalmSync",
        };
    }

    public static void ShowSingleton<T>() where T : Window, new()
    {
        var existing = Application.Current.Windows.OfType<T>().FirstOrDefault();
        if (existing != null)
        {
            if (existing.WindowState == WindowState.Minimized) existing.WindowState = WindowState.Normal;
            existing.Show();
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

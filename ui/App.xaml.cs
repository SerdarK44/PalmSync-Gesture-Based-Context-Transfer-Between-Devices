using System.Threading;
using System.Windows;
using Hardcodet.Wpf.TaskbarNotification;
using PalmSync.Tray.Services;
using PalmSync.Tray.ViewModels;
using PalmSync.Tray.Views;

namespace PalmSync.Tray;

public partial class App : Application
{
    private const string MutexName = "PalmSync.Tray.SingleInstance";
    private const string ShowEventName = "PalmSync.Tray.ShowHome";

    private TaskbarIcon? _trayIcon;
    private TrayViewModel? _trayViewModel;
    private Mutex? _singleInstance;
    private EventWaitHandle? _showEvent;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        // Only one tray app per user. A second launch (desktop icon while the
        // tray is already running) used to start another copy that restarted
        // and killed the running daemon; now it just brings PalmSync forward.
        _singleInstance = new Mutex(true, MutexName, out var isFirst);
        if (!isFirst)
        {
            try { EventWaitHandle.OpenExisting(ShowEventName).Set(); } catch { }
            Shutdown();
            return;
        }
        _showEvent = new EventWaitHandle(false, EventResetMode.AutoReset, ShowEventName);
        ThreadPool.RegisterWaitForSingleObject(_showEvent,
            (_, _) => Dispatcher.BeginInvoke(new Action(TrayViewModel.ShowSingleton<HomeWindow>)),
            null, Timeout.Infinite, false);

        // Documents\PalmSync\{Loglar, Gelen Dosyalar}; moves logs from the old
        // hidden %LOCALAPPDATA% location. Must run before anything logs.
        AppPaths.EnsureLayout();
        AppLog.Info("PalmSync arayüzü başladı");

        DispatcherUnhandledException += (_, args) =>
        {
            AppLog.Error("Beklenmeyen arayüz hatası", args.Exception);
            MessageBox.Show(args.Exception.Message, "PalmSync — hata",
                MessageBoxButton.OK, MessageBoxImage.Error);
            args.Handled = true;
        };
        AppDomain.CurrentDomain.UnhandledException += (_, args) =>
            AppLog.Error("Beklenmeyen hata", args.ExceptionObject as Exception);
        TaskScheduler.UnobservedTaskException += (_, args) =>
        {
            AppLog.Error("Arka plan görevi hatası", args.Exception);
            args.SetObserved();
        };

        ThemeService.ApplySaved();

        _trayViewModel = new TrayViewModel();
        _trayIcon = (TaskbarIcon)FindResource("TrayIconResource");
        _trayIcon.DataContext = _trayViewModel;
        _trayViewModel.StartStatusPolling();

        // Resume a previously active session (at login this happens silently,
        // in the tray). A manual launch always shows the Home window — before,
        // opening PalmSync with a saved session showed nothing at all.
        SessionController.ResumeIfConfigured();
        var silent = e.Args.Contains("--silent");

        if (e.Args.Contains("--updated"))
        {
            AppLog.Info($"✅ PalmSync v{UpdateService.CurrentText} sürümüne güncellendi");
            _trayIcon.ShowBalloonTip("PalmSync güncellendi",
                $"v{UpdateService.CurrentText} kuruldu ve oturum sürdürüldü.", BalloonIcon.Info);
        }

        // Updates from GitHub Releases: ask (banner + balloon) or install by itself.
        _trayIcon.TrayBalloonTipClicked += (_, _) => TrayViewModel.ShowSingleton<HomeWindow>();
        UpdateService.StartBackgroundChecks(info => Dispatcher.InvokeAsync(async () =>
        {
            if (UISettingsManager.Load().UpdateMode == "auto")
            {
                AppLog.Info($"Otomatik güncelleme: v{info.Version} kuruluyor");
                await UpdateService.InstallAndExitAsync(info, quiet: true);
            }
            else
            {
                _trayIcon?.ShowBalloonTip("PalmSync — yeni sürüm",
                    $"v{info.Version} hazır. Yüklemek için tıkla.", BalloonIcon.Info);
            }
        }).Task.Unwrap());
        if (!silent)
            new HomeWindow().Show();

        // First run on this PC: walk through permissions + problems. Later
        // starts: check quietly in the background (the Home header shows a
        // badge if something is wrong) and report problems to the host.
        if (!UISettingsManager.Load().SetupDone && !silent)
            SystemCheckWindow.ShowWindow(firstRun: true);
        else
            _ = StartupCheckAsync();
    }

    private static async Task StartupCheckAsync()
    {
        try
        {
            // The camera probe flashes the webcam light, so at a normal start
            // it only runs in debug mode (the camera is tested anyway when a
            // session starts it).
            await HealthCheck.RunAsync("başlangıç", probeCamera: ConfigManager.Load().DebugMode);
            if (DaemonManager.IsRunning)
            {
                await Task.Delay(3000);   // let the daemon bind its IPC port
                HealthCheck.SendProblemsToHost();
            }
        }
        catch (Exception ex)
        {
            AppLog.Error("Başlangıç sistem kontrolü yapılamadı", ex);
        }
    }

    protected override void OnExit(ExitEventArgs e)
    {
        if (_trayViewModel != null)
        {
            // The daemon lives in this user session; don't leave it orphaned.
            DaemonManager.Stop();
            EyeManager.Stop();
        }
        _trayViewModel?.Dispose();
        _trayIcon?.Dispose();
        _showEvent?.Dispose();
        _singleInstance?.Dispose();
        base.OnExit(e);
    }
}

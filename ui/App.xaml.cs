using System.Windows;
using Hardcodet.Wpf.TaskbarNotification;
using PalmSync.Tray.Services;
using PalmSync.Tray.ViewModels;
using PalmSync.Tray.Views;

namespace PalmSync.Tray;

public partial class App : Application
{
    private TaskbarIcon? _trayIcon;
    private TrayViewModel? _trayViewModel;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        DispatcherUnhandledException += (_, args) =>
        {
            LogError(args.Exception);
            MessageBox.Show(args.Exception.Message, "PalmSync error",
                MessageBoxButton.OK, MessageBoxImage.Error);
            args.Handled = true;
        };
        AppDomain.CurrentDomain.UnhandledException += (_, args) =>
            LogError(args.ExceptionObject as Exception);

        ThemeService.ApplySystemTheme();

        _trayViewModel = new TrayViewModel();
        _trayIcon = (TaskbarIcon)FindResource("TrayIconResource");
        _trayIcon.DataContext = _trayViewModel;
        _trayViewModel.StartStatusPolling();

        // Resume a previously active session silently at login. If there was
        // none, show the Home window so the user sees what to do next.
        var resumed = SessionController.ResumeIfConfigured();
        if (!resumed && !e.Args.Contains("--silent"))
            new HomeWindow().Show();
    }

    protected override void OnExit(ExitEventArgs e)
    {
        // The daemon lives in this user session; don't leave it orphaned.
        DaemonManager.Stop();
        _trayViewModel?.Dispose();
        _trayIcon?.Dispose();
        base.OnExit(e);
    }

    private static void LogError(Exception? ex)
    {
        if (ex == null) return;
        try
        {
            var dir = ConfigManager.ConfigDir;
            System.IO.Directory.CreateDirectory(dir);
            System.IO.File.AppendAllText(
                System.IO.Path.Combine(dir, "ui-error.log"),
                $"{DateTime.Now:o} {ex}\n\n");
        }
        catch { /* nothing we can do */ }
    }
}

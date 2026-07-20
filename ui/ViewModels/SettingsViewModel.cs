using System.Net.Sockets;
using System.Windows;
using System.Windows.Input;
using PalmSync.Tray.Services;

namespace PalmSync.Tray.ViewModels;

public class SettingsViewModel : ViewModelBase
{
    private readonly PalmSyncConfig _config;

    private string _role;
    private string _masterAddr;
    private string _masterPort;
    private string _password;
    private string _ipcPort;
    private string _logLevel;
    private bool _enableNotifications;
    private bool _isDarkTheme;

    public event Action? RequestClose;

    public string Role { get => _role; set => Set(ref _role, value); }
    public List<string> Roles { get; } = new() { "master", "slave" };
    public string MasterAddr { get => _masterAddr; set => Set(ref _masterAddr, value); }
    public string MasterPort { get => _masterPort; set => Set(ref _masterPort, value); }
    public string Password { get => _password; set => Set(ref _password, value); }
    public string IpcPort { get => _ipcPort; set => Set(ref _ipcPort, value); }
    public string LogLevel { get => _logLevel; set => Set(ref _logLevel, value); }
    public bool EnableNotifications { get => _enableNotifications; set => Set(ref _enableNotifications, value); }

    public bool IsDarkTheme
    {
        get => _isDarkTheme;
        set
        {
            if (Set(ref _isDarkTheme, value))
                ThemeService.Apply(value ? AppTheme.Dark : AppTheme.Light);
        }
    }

    public List<string> LogLevels { get; } = new() { "trace", "debug", "info", "warn", "error" };

    public ICommand SaveCommand { get; }
    public ICommand CancelCommand { get; }
    public ICommand TestConnectionCommand { get; }

    public SettingsViewModel()
    {
        _config = ConfigManager.Load();
        _role = _config.Role;
        _masterAddr = _config.MasterAddr;
        _masterPort = _config.MasterPort.ToString();
        _password = _config.Password;
        _ipcPort = _config.IpcPort.ToString();
        _logLevel = _config.LogLevel;
        _enableNotifications = _config.EnableNotifications;
        _isDarkTheme = ThemeService.Current == AppTheme.Dark;

        SaveCommand = new RelayCommand(Save);
        CancelCommand = new RelayCommand(() => RequestClose?.Invoke());
        TestConnectionCommand = new AsyncRelayCommand(TestConnection);
    }

    private void Save()
    {
        if (!ushort.TryParse(MasterPort, out var masterPort))
        {
            MessageBox.Show("Master port must be a number between 1-65535.", "PalmSync",
                MessageBoxButton.OK, MessageBoxImage.Warning);
            return;
        }
        if (!ushort.TryParse(IpcPort, out var ipcPort))
        {
            MessageBox.Show("IPC port must be a number between 1-65535.", "PalmSync",
                MessageBoxButton.OK, MessageBoxImage.Warning);
            return;
        }

        _config.Role = Role;
        _config.MasterAddr = MasterAddr.Trim();
        _config.MasterPort = masterPort;
        _config.Password = Password;
        _config.IpcPort = ipcPort;
        _config.LogLevel = LogLevel;
        _config.EnableNotifications = EnableNotifications;

        try
        {
            ConfigManager.Save(_config);
        }
        catch (Exception ex)
        {
            MessageBox.Show(
                $"Config could not be saved: {ex.Message}\n\nWriting to ProgramData may require administrator rights.",
                "PalmSync", MessageBoxButton.OK, MessageBoxImage.Error);
            return;
        }

        // Apply immediately if a session is active
        if (DaemonManager.IsRunning)
            DaemonManager.Restart();

        RequestClose?.Invoke();
    }

    private async Task TestConnection()
    {
        if (!ushort.TryParse(MasterPort, out var port))
        {
            MessageBox.Show("Invalid port.", "PalmSync", MessageBoxButton.OK, MessageBoxImage.Warning);
            return;
        }

        try
        {
            using var client = new TcpClient();
            var connectTask = client.ConnectAsync(MasterAddr.Trim(), port);
            var completed = await Task.WhenAny(connectTask, Task.Delay(3000));
            if (completed != connectTask || !client.Connected)
                throw new TimeoutException("Connection timed out (3s)");

            MessageBox.Show($"Connected to {MasterAddr}:{port} successfully!", "PalmSync",
                MessageBoxButton.OK, MessageBoxImage.Information);
        }
        catch (Exception ex)
        {
            MessageBox.Show($"Connection failed: {ex.Message}", "PalmSync",
                MessageBoxButton.OK, MessageBoxImage.Error);
        }
    }
}

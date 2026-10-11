using System.Diagnostics;
using System.IO;
using System.Net.Sockets;
using System.Windows;
using System.Windows.Input;
using PalmSync.Tray.Services;

namespace PalmSync.Tray.ViewModels;

public class SettingsViewModel : ViewModelBase
{
    private readonly PalmSyncConfig _config;
    private readonly UISettings _uiSettings;
    private readonly AppTheme _themeAtOpen;
    private bool _saved;

    private string _role;
    private string _masterAddr;
    private string _masterPort;
    private string _password;
    private string _ipcPort;
    private string _logLevel;
    private bool _enableNotifications;
    private bool _isDarkTheme;
    private bool _debugMode;

    public event Action? RequestClose;

    public string Role { get => _role; set => Set(ref _role, value); }
    public List<string> Roles { get; } = new() { "master", "slave", "off" };
    public string MasterAddr { get => _masterAddr; set => Set(ref _masterAddr, value); }
    public string MasterPort { get => _masterPort; set => Set(ref _masterPort, value); }
    public string Password { get => _password; set => Set(ref _password, value); }
    public string IpcPort { get => _ipcPort; set => Set(ref _ipcPort, value); }
    public string LogLevel { get => _logLevel; set => Set(ref _logLevel, value); }
    public bool EnableNotifications { get => _enableNotifications; set => Set(ref _enableNotifications, value); }
    public bool DebugMode { get => _debugMode; set => Set(ref _debugMode, value); }

    public bool IsDarkTheme
    {
        get => _isDarkTheme;
        set
        {
            // Live preview; reverted on Cancel.
            if (Set(ref _isDarkTheme, value))
                ThemeService.Apply(value ? AppTheme.Dark : AppTheme.Light);
        }
    }

    public List<string> LogLevels { get; } = new() { "error", "warn", "info", "debug", "trace" };

    public string LogDir => _config.LogDir;
    public string VersionText => $"Sürüm {UpdateService.CurrentText} — bulutsuz P2P çalışma alanı senkronizasyonu";

    private string _updateMode;
    private string _updateStatus = "";
    public bool UpdateAsk { get => _updateMode == "ask"; set { if (value) SetUpdateMode("ask"); } }
    public bool UpdateAuto { get => _updateMode == "auto"; set { if (value) SetUpdateMode("auto"); } }
    public bool UpdateOff { get => _updateMode == "off"; set { if (value) SetUpdateMode("off"); } }
    public string UpdateStatus { get => _updateStatus; set => Set(ref _updateStatus, value); }
    public ICommand CheckUpdateCommand { get; }

    private void SetUpdateMode(string mode)
    {
        _updateMode = mode;
        OnPropertyChanged(nameof(UpdateAsk));
        OnPropertyChanged(nameof(UpdateAuto));
        OnPropertyChanged(nameof(UpdateOff));
    }

    private async Task CheckUpdateAsync()
    {
        UpdateStatus = "Denetleniyor…";
        var info = await UpdateService.CheckAsync(manual: true);
        UpdateStatus = info != null
            ? $"v{info.Version} hazır — ana penceredeki 'Şimdi yükle' ile kur"
            : UpdateService.LastError != null
                ? "GitHub'a ulaşılamadı: " + UpdateService.LastError
                : $"En güncel sürüm kullanılıyor (v{UpdateService.CurrentText})";
    }

    public ICommand SaveCommand { get; }
    public ICommand CancelCommand { get; }
    public ICommand TestConnectionCommand { get; }
    public ICommand OpenLogsCommand { get; }
    public ICommand OpenLogFolderCommand { get; }
    public ICommand OpenSystemCheckCommand { get; }

    public SettingsViewModel()
    {
        _config = ConfigManager.Load();
        _uiSettings = UISettingsManager.Load();
        _role = _config.Role;
        _masterAddr = _config.MasterAddr;
        _masterPort = _config.MasterPort.ToString();
        _password = _config.Password;
        _ipcPort = _config.IpcPort.ToString();
        _logLevel = _config.LogLevel;
        _enableNotifications = _config.EnableNotifications;
        _debugMode = _config.DebugMode;
        _themeAtOpen = ThemeService.Current;
        _updateMode = _uiSettings.UpdateMode;
        CheckUpdateCommand = new AsyncRelayCommand(CheckUpdateAsync);
        _isDarkTheme = _themeAtOpen == AppTheme.Dark;

        SaveCommand = new RelayCommand(Save);
        CancelCommand = new RelayCommand(() => RequestClose?.Invoke());
        TestConnectionCommand = new AsyncRelayCommand(TestConnection);
        OpenLogsCommand = new RelayCommand(TrayViewModel.ShowSingleton<Views.LogViewerWindow>);
        OpenSystemCheckCommand = new RelayCommand(() => Views.SystemCheckWindow.ShowWindow());
        OpenLogFolderCommand = new RelayCommand(() =>
        {
            try
            {
                Directory.CreateDirectory(_config.LogDir);
                Process.Start(new ProcessStartInfo("explorer.exe", $"\"{_config.LogDir}\"") { UseShellExecute = true });
            }
            catch { }
        });
    }

    /// Called when the window closes: undo the live theme preview unless saved.
    public void OnClosed()
    {
        if (!_saved && ThemeService.Current != _themeAtOpen)
            ThemeService.Apply(_themeAtOpen);
    }

    private void Save()
    {
        if (!ushort.TryParse(MasterPort, out var masterPort) || masterPort == 0)
        {
            MessageBox.Show("Port 1–65535 arasında bir sayı olmalı.", "PalmSync",
                MessageBoxButton.OK, MessageBoxImage.Warning);
            return;
        }
        if (!ushort.TryParse(IpcPort, out var ipcPort) || ipcPort < 1024)
        {
            MessageBox.Show("Kamera portu 1024–65535 arasında bir sayı olmalı.", "PalmSync",
                MessageBoxButton.OK, MessageBoxImage.Warning);
            return;
        }

        var needsRestart =
            _config.Role != Role || _config.MasterAddr != MasterAddr.Trim() || _config.MasterPort != masterPort ||
            _config.Password != Password || _config.IpcPort != ipcPort || _config.LogLevel != LogLevel ||
            _config.EnableNotifications != EnableNotifications || _config.DebugMode != DebugMode;
        var debugChanged = _config.DebugMode != DebugMode;

        _config.Role = Role;
        _config.MasterAddr = MasterAddr.Trim();
        _config.MasterPort = masterPort;
        _config.Password = Password;
        _config.IpcPort = ipcPort;
        _config.LogLevel = LogLevel;
        _config.EnableNotifications = EnableNotifications;
        _config.DebugMode = DebugMode;

        try
        {
            ConfigManager.Save(_config);
            _uiSettings.Theme = IsDarkTheme ? "dark" : "light";
            _uiSettings.UpdateMode = _updateMode;
            UISettingsManager.Save(_uiSettings);
        }
        catch (Exception ex)
        {
            AppLog.Error("Ayarlar kaydedilemedi", ex);
            MessageBox.Show($"Ayarlar kaydedilemedi: {ex.Message}", "PalmSync",
                MessageBoxButton.OK, MessageBoxImage.Error);
            return;
        }

        _saved = true;
        AppLog.Info($"⚙ Ayarlar kaydedildi (kayıt seviyesi: {LogLevel})");
        if (debugChanged)
            AppLog.Info(DebugMode ? "🐞 Hata ayıklama modu AÇILDI" : "🐞 Hata ayıklama modu kapatıldı");

        // Apply immediately if a session is active (the daemon reads config at start).
        if (needsRestart && DaemonManager.IsRunning)
        {
            try { DaemonManager.Restart(); }
            catch (Exception ex) { AppLog.Error("Daemon yeniden başlatılamadı", ex); }
        }

        RequestClose?.Invoke();
    }

    private async Task TestConnection()
    {
        if (!ushort.TryParse(MasterPort, out var port))
        {
            MessageBox.Show("Geçersiz port.", "PalmSync", MessageBoxButton.OK, MessageBoxImage.Warning);
            return;
        }

        try
        {
            using var client = new TcpClient();
            var connectTask = client.ConnectAsync(MasterAddr.Trim(), port);
            var completed = await Task.WhenAny(connectTask, Task.Delay(3000));
            if (completed != connectTask || !client.Connected)
                throw new TimeoutException("3 saniye içinde yanıt yok");

            MessageBox.Show($"{MasterAddr}:{port} adresine ulaşıldı ✓", "PalmSync",
                MessageBoxButton.OK, MessageBoxImage.Information);
        }
        catch (Exception ex)
        {
            MessageBox.Show($"Bağlanılamadı: {ex.Message}", "PalmSync",
                MessageBoxButton.OK, MessageBoxImage.Error);
        }
    }
}

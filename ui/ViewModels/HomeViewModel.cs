using System.Collections.ObjectModel;
using System.IO;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using PalmSync.Tray.Services;

namespace PalmSync.Tray.ViewModels;

public enum HomeState { Idle, ConnectInput, Hosting, Connected }

public class HomeViewModel : ViewModelBase, IDisposable
{
    private const int RecentCount = 5;

    private readonly DispatcherTimer _timer;
    private readonly Action<ImageSource> _onFrame;
    private readonly List<LogSource> _activitySources;

    private HomeState _state = HomeState.Idle;
    private string _code = "";
    private string _password = "";
    private string _hostIp = "";
    private string _inputCode = "";
    private string _inputPassword = "";
    private string _statusText = "";
    private string _pillText = "";
    private Brush _statusBrush = Brushes.Gray;
    private string _flash = "";
    private int _waitingCycles;
    private bool _cameraOn;
    private string? _cameraError;
    private ImageSource? _previewFrame;

    public HomeState State
    {
        get => _state;
        set
        {
            if (Set(ref _state, value))
            {
                OnPropertyChanged(nameof(IsIdle));
                OnPropertyChanged(nameof(IsConnectInput));
                OnPropertyChanged(nameof(IsHosting));
                OnPropertyChanged(nameof(IsConnected));
                OnPropertyChanged(nameof(ShowSyncButtons));
            }
        }
    }

    public bool IsIdle => State == HomeState.Idle;
    public bool IsConnectInput => State == HomeState.ConnectInput;
    public bool IsHosting => State == HomeState.Hosting;
    public bool IsConnected => State == HomeState.Connected;
    public bool ShowSyncButtons => State is HomeState.Hosting or HomeState.Connected;

    public string Code { get => _code; set => Set(ref _code, value); }
    public string Password { get => _password; set => Set(ref _password, value); }
    public string HostIp { get => _hostIp; set => Set(ref _hostIp, value); }
    public string InputCode { get => _inputCode; set => Set(ref _inputCode, value); }
    public string InputPassword { get => _inputPassword; set => Set(ref _inputPassword, value); }
    public string StatusText { get => _statusText; set => Set(ref _statusText, value); }
    /// Short status for the header badge ("Bağlı", "Bekleniyor"...).
    public string PillText { get => _pillText; set => Set(ref _pillText, value); }
    public Brush StatusBrush { get => _statusBrush; set => Set(ref _statusBrush, value); }
    public string Flash { get => _flash; set => Set(ref _flash, value); }

    public bool CameraOn
    {
        get => _cameraOn;
        set
        {
            if (Set(ref _cameraOn, value))
            {
                OnPropertyChanged(nameof(CameraButtonText));
                OnPropertyChanged(nameof(CameraStateText));
            }
        }
    }
    public string CameraButtonText => _cameraOn ? "Kapat" : "Aç";
    public bool CameraAvailable => EyeManager.IsAvailable;
    public string CameraStateText =>
        !CameraAvailable ? "Kamera modülü kurulu değil"
        : _cameraOn ? "Açık — elini kameraya göster"
        : _cameraError != null ? "⚠ " + _cameraError
        : "Kapalı — butonlar yine çalışır";

    private void SetCameraError(string? error)
    {
        _cameraError = error;
        OnPropertyChanged(nameof(CameraStateText));
    }

    public ImageSource? PreviewFrame { get => _previewFrame; set => Set(ref _previewFrame, value); }

    /// Latest meaningful events from the logs (newest first).
    public ObservableCollection<LogEntry> RecentEvents { get; } = new();
    public bool NoRecentEvents => RecentEvents.Count == 0;

    public ICommand StartHostingCommand { get; }
    public ICommand ShowConnectCommand { get; }
    public ICommand JoinCommand { get; }
    public ICommand BackCommand { get; }
    public ICommand GrabCommand { get; }
    public ICommand RestoreCommand { get; }
    public ICommand StopCommand { get; }
    public ICommand ToggleCameraCommand { get; }
    public ICommand TroubleshootCommand { get; }
    public ICommand OpenLogsCommand { get; }
    public ICommand OpenSettingsCommand { get; }
    public ICommand CopyCodeCommand { get; }
    public ICommand CopyPasswordCommand { get; }
    public ICommand OpenSystemCheckCommand { get; }
    public ICommand InstallUpdateCommand { get; }
    public ICommand SkipUpdateCommand { get; }

    /// "New version available" banner (GitHub Releases).
    public bool UpdateVisible => UpdateService.Available != null;
    public string UpdateText => UpdateService.Available is { } u
        ? $"Yeni sürüm hazır: v{u.Version}  (kurulu: v{UpdateService.CurrentText})" : "";
    public string UpdateButtonText => UpdateService.DownloadPercent >= 0
        ? $"İndiriliyor… %{UpdateService.DownloadPercent}" : "Şimdi yükle";

    /// Header badge: number of problems found by the last system check.
    public bool HasHealthProblems => HealthCheck.ProblemCount > 0;
    public string HealthText => HealthCheck.ProblemCount == 0 ? "🔧" : $"⚠ {HealthCheck.ProblemCount}";

    public HomeViewModel()
    {
        StartHostingCommand = new RelayCommand(StartHosting);
        ShowConnectCommand = new RelayCommand(() => State = HomeState.ConnectInput);
        JoinCommand = new AsyncRelayCommand(Join);
        BackCommand = new RelayCommand(() => State = HomeState.Idle);
        GrabCommand = new AsyncRelayCommand(GrabAsync);
        RestoreCommand = new RelayCommand(() => SendGesture("drop", "Geri yükleme istendi ✓"));
        StopCommand = new RelayCommand(Stop);
        ToggleCameraCommand = new AsyncRelayCommand(() => ToggleCameraAsync(userInitiated: true));
        TroubleshootCommand = new RelayCommand(TrayViewModel.ShowSingleton<Views.TroubleshootWindow>);
        OpenLogsCommand = new RelayCommand(TrayViewModel.ShowSingleton<Views.LogViewerWindow>);
        OpenSettingsCommand = new RelayCommand(TrayViewModel.ShowSingleton<Views.SettingsWindow>);
        CopyCodeCommand = new RelayCommand(() => TrySetClipboard(Code, "Kod kopyalandı"));
        CopyPasswordCommand = new RelayCommand(() => TrySetClipboard(Password, "Şifre kopyalandı"));
        OpenSystemCheckCommand = new RelayCommand(() => Views.SystemCheckWindow.ShowWindow());
        InstallUpdateCommand = new AsyncRelayCommand(async () =>
        {
            if (UpdateService.Available is { } u) await UpdateService.InstallAndExitAsync(u);
        });
        SkipUpdateCommand = new RelayCommand(() =>
        {
            if (UpdateService.Available is { } u) UpdateService.Skip(u);
        });
        UpdateService.Changed += OnUpdateChanged;
        HealthCheck.Changed += OnHealthChanged;

        // Subscribe to camera preview frames from eye.exe (via named pipe)
        _onFrame = frame =>
            Application.Current.Dispatcher.BeginInvoke(() =>
            {
                if (CameraOn) PreviewFrame = frame;
            });
        CameraPreviewService.FrameReceived += _onFrame;

        // The camera keeps running when the window is closed; reflect that
        // when the window is reopened and resume the preview.
        if (EyeManager.IsRunning)
        {
            _cameraOn = true;
            _ = CameraPreviewService.StartAsync();
        }

        var dir = AppLog.LogDir;
        _activitySources = new()
        {
            new LogSource("Daemon", Path.Combine(dir, "palmsync-brain.log")),
            new LogSource("Kamera", Path.Combine(dir, "palmsync-eye.log")),
            new LogSource("Arayüz", Path.Combine(dir, "palmsync-ui.log")),
            new LogSource("Cihazlar", Path.Combine(dir, "cihaz-raporlari.log")),
        };

        _timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(2) };
        _timer.Tick += (_, _) => RefreshStatus();

        RestoreFromExistingSession();
        RefreshActivity();
        _timer.Start();
    }

    private void RestoreFromExistingSession()
    {
        if (!DaemonManager.IsRunning) return;

        var role = SessionController.CurrentRole();
        if (role == "master")
        {
            var info = SessionController.CurrentHostInfo();
            if (info != null)
            {
                Code = info.Code;
                Password = info.Password;
                HostIp = info.Ip;
                State = HomeState.Hosting;
            }
        }
        else if (role == "slave")
        {
            State = HomeState.Connected;
        }
        RefreshStatus();
    }

    private void StartHosting()
    {
        try
        {
            var info = SessionController.StartHosting();
            Code = info.Code;
            Password = info.Password;
            HostIp = info.Ip;
            _waitingCycles = 0;
            State = HomeState.Hosting;
            SetStatus("Diğer bilgisayarın bağlanması bekleniyor…", "Bekleniyor", Brushes.DarkOrange);
            _ = ReportHealthAfterSessionStartAsync();

            if (CameraAvailable && !CameraOn)
                _ = ToggleCameraAsync(userInitiated: false);
        }
        catch (Exception ex)
        {
            AppLog.Error("Oturum başlatılamadı", ex);
            MessageBox.Show(ex.Message, "PalmSync", MessageBoxButton.OK, MessageBoxImage.Warning);
        }
    }

    private async Task Join()
    {
        SessionController.JoinTarget target;
        try
        {
            target = SessionController.Join(InputCode, InputPassword);
        }
        catch (Exception ex)
        {
            AppLog.Warn("Oturuma katılınamadı: " + ex.Message);
            MessageBox.Show(ex.Message, "PalmSync", MessageBoxButton.OK, MessageBoxImage.Warning);
            return;
        }

        _waitingCycles = 0;
        State = HomeState.Connected;
        SetStatus("Bağlanılıyor…", "Bağlanıyor", Brushes.DarkOrange);
        InputPassword = "";
        _ = ReportHealthAfterSessionStartAsync();

        if (CameraAvailable && !CameraOn)
            _ = ToggleCameraAsync(userInitiated: false);

        // Probe reachability so we can tell the user *why* if it won't connect.
        var reachable = await Task.Run(() => NetworkInfo.CanReach(target.Ip, target.Port));

        if (!reachable && State == HomeState.Connected)
        {
            AppLog.Warn($"Ev sahibi bilgisayara ulaşılamıyor ({target.Ip}:{target.Port})");
            SetStatus(
                $"Diğer bilgisayara ({target.Ip}) ulaşılamıyor. Olası nedenler: o bilgisayarda " +
                "güvenlik duvarı, farklı Wi-Fi / ağ ya da başka bir ağdan alınmış kod. " +
                "Ev sahibi bilgisayarda ❔ Sorun giderme'yi aç.",
                "Ulaşılamıyor", Brushes.Firebrick);
        }
    }

    private async Task GrabAsync()
    {
        // Hide the PalmSync window so the previously-active app (VS Code,
        // browser) regains focus before the daemon captures it. Without this,
        // GetForegroundWindow() returns PalmSync itself and the capture comes
        // back empty (no file, no URL — just clipboard at best).
        var win = Application.Current.Windows.OfType<Views.HomeWindow>().FirstOrDefault();
        win?.Hide();
        await Task.Delay(350);   // wait for OS focus switch
        SendGesture("grab", "Yakalama istendi ✓");
        await Task.Delay(300);   // let the UDP land before showing the window again
        win?.Show();
    }

    private void SendGesture(string kind, string confirmation)
    {
        try
        {
            GestureSender.Send(kind, SessionController.CurrentIpcPort());
            AppLog.Debug($"Buton: {kind} gönderildi");
            ShowFlash(confirmation);
        }
        catch (Exception ex)
        {
            AppLog.Error($"'{kind}' gönderilemedi", ex);
            MessageBox.Show($"Gönderilemedi: {ex.Message}", "PalmSync",
                MessageBoxButton.OK, MessageBoxImage.Warning);
        }
    }

    private void ShowFlash(string text)
    {
        Flash = text;
        var t = new DispatcherTimer { Interval = TimeSpan.FromSeconds(2.5) };
        t.Tick += (_, _) => { if (Flash == text) Flash = ""; t.Stop(); };
        t.Start();
    }

    /// Auto-start (session start/join) must not interrupt with a dialog when
    /// there's simply no webcam: the reason is shown in the camera card and
    /// the logs instead. Only an explicit click on "Aç" shows the full dialog.
    private async Task ToggleCameraAsync(bool userInitiated)
    {
        if (CameraOn)
        {
            EyeManager.Stop();
            await CameraPreviewService.StopAsync();
            PreviewFrame = null;
            CameraOn = false;
            return;
        }

        try
        {
            // Start listening before eye.exe starts so the preview appears at once
            // (eye also retries the pipe every ~2 s, so the order isn't critical).
            await CameraPreviewService.StartAsync();

            var error = await EyeManager.TryStart(SessionController.CurrentIpcPort());
            if (error != null)
            {
                await CameraPreviewService.StopAsync();
                CameraOn = false;
                SetCameraError(FirstLine(error) + " Ayrıntı için Aç'a bas.");
                if (userInitiated)
                    MessageBox.Show(error, "PalmSync — Kamera", MessageBoxButton.OK, MessageBoxImage.Warning);
            }
            else
            {
                SetCameraError(null);
                CameraOn = true;
            }
        }
        catch (Exception ex)
        {
            AppLog.Error("Kamera başlatılamadı", ex);
            SetCameraError("Kamera başlatılamadı.");
            if (userInitiated)
                MessageBox.Show($"Kamera başlatılamadı:\n{ex.Message}", "PalmSync",
                    MessageBoxButton.OK, MessageBoxImage.Warning);
            CameraOn = false;
        }
    }

    private void Stop()
    {
        EyeManager.Stop();
        _ = CameraPreviewService.StopAsync();
        PreviewFrame = null;
        CameraOn = false;
        SessionController.Stop();
        Code = "";
        Password = "";
        State = HomeState.Idle;
    }

    private void SetStatus(string text, string pill, Brush brush)
    {
        StatusText = text;
        PillText = pill;
        StatusBrush = brush;
    }

    private void RefreshStatus()
    {
        RefreshActivity();

        // If the camera process died on its own, reset the toggle and tell
        // the user why (previously it just silently switched off).
        if (CameraOn && !EyeManager.IsRunning)
        {
            CameraOn = false;
            PreviewFrame = null;
            var reason = EyeManager.LastExitReason();
            if (reason != null)
            {
                AppLog.Warn("Kamera kendiliğinden kapandı: " + reason);
                EyeManager.ReportFailure("Kamera kendiliğinden kapandı", reason);
                SetCameraError("Kamera kapandı: " + FirstLine(reason));
            }
        }

        if (State is not (HomeState.Hosting or HomeState.Connected)) return;

        if (!DaemonManager.IsRunning)
        {
            SetStatus("Arka plan servisi çalışmıyor. Oturumu bitirip yeniden başlat; " +
                      "nedenini 📜 Kayıtlar'da görebilirsin.", "Durdu", Brushes.Firebrick);
            return;
        }

        var isMaster = State == HomeState.Hosting;
        var peer = LogWatcher.GetStatus(isMaster);

        if (peer == PeerStatus.Connected)
        {
            _waitingCycles = 0;
            SetStatus(isMaster ? "Diğer bilgisayar bağlandı ✓  Artık yakalayıp geri yükleyebilirsin."
                               : "Oturuma bağlandın ✓  Artık yakalayıp geri yükleyebilirsin.",
                      "Bağlı", Brushes.SeaGreen);
            return;
        }

        // Keep the "unreachable" explanation from Join() instead of overwriting it.
        if (PillText == "Ulaşılamıyor") return;

        _waitingCycles++;
        // ~2s per cycle; after ~12s of no connection, surface a hint.
        if (_waitingCycles >= 6)
        {
            SetStatus(isMaster
                ? "Hâlâ bekleniyor. Diğer bilgisayarın kodu ve şifreyi aynen girdiğinden ve iki bilgisayarın aynı Wi-Fi'da olduğundan emin ol."
                : "Ev sahibine henüz ulaşılamadı. Aynı Wi-Fi / ağda olduğunuzu ve kod + şifrenin doğru olduğunu kontrol et.",
                isMaster ? "Bekleniyor" : "Bağlanıyor", Brushes.DarkOrange);
        }
        else
        {
            SetStatus(isMaster ? "Diğer bilgisayarın bağlanması bekleniyor…" : "Bağlanılıyor…",
                      isMaster ? "Bekleniyor" : "Bağlanıyor", Brushes.DarkOrange);
        }
    }

    /// Pull new lines from the logs and keep the last few user-level events.
    private void RefreshActivity()
    {
        var fresh = _activitySources
            .SelectMany(s => s.ReadNew(256 * 1024))
            .Where(e => e.Summary != null && e.Level >= LogLevel.Info && e.Time.Date == DateTime.Today)
            // On the host, its own reports already show up as UI lines; and the
            // per-check lines are summarised by "🔧 Sistem kontrolü: N sorun".
            .Where(e => !(e.Source == "Cihazlar" && e.Message.StartsWith($"[{Environment.MachineName}]")))
            .Where(e => !e.Message.StartsWith("Sistem kontrolü — "))
            .OrderBy(e => e.Time)
            .ToList();
        if (fresh.Count == 0) return;

        foreach (var e in fresh)
        {
            if (RecentEvents.Count > 0 && RecentEvents[0].DisplayFirstLine == e.DisplayFirstLine) continue;
            RecentEvents.Insert(0, e);
        }
        while (RecentEvents.Count > RecentCount)
            RecentEvents.RemoveAt(RecentEvents.Count - 1);
        OnPropertyChanged(nameof(NoRecentEvents));
    }

    private static string FirstLine(string s)
    {
        var line = s.Split('\n', StringSplitOptions.RemoveEmptyEntries).FirstOrDefault()?.Trim() ?? s;
        return line.EndsWith('.') ? line : line + ".";
    }

    private void OnUpdateChanged() =>
        Application.Current.Dispatcher.BeginInvoke(() =>
        {
            OnPropertyChanged(nameof(UpdateVisible));
            OnPropertyChanged(nameof(UpdateText));
            OnPropertyChanged(nameof(UpdateButtonText));
        });

    private void OnHealthChanged() =>
        Application.Current.Dispatcher.BeginInvoke(() =>
        {
            OnPropertyChanged(nameof(HasHealthProblems));
            OnPropertyChanged(nameof(HealthText));
        });

    /// A session just started: (re)check this PC and send the problems to
    /// the daemon, which queues them until the host is connected — so the
    /// host's log shows e.g. "[PC-2] Kamera izni kapalı".
    private static async Task ReportHealthAfterSessionStartAsync()
    {
        try
        {
            await Task.Delay(4000);   // daemon restarted: wait for its IPC port
            if (HealthCheck.Last.Count == 0 || DateTime.Now - HealthCheck.LastRun > TimeSpan.FromMinutes(10))
                await HealthCheck.RunAsync("oturum başladı", probeCamera: false);
            HealthCheck.SendProblemsToHost();
        }
        catch (Exception ex)
        {
            AppLog.Error("Sistem kontrolü sonucu gönderilemedi", ex);
        }
    }

    private void TrySetClipboard(string text, string confirmation)
    {
        try
        {
            if (string.IsNullOrEmpty(text)) return;
            Clipboard.SetText(text);
            ShowFlash(confirmation);
        }
        catch { /* clipboard busy */ }
    }

    public void Dispose()
    {
        _timer.Stop();
        CameraPreviewService.FrameReceived -= _onFrame;
        HealthCheck.Changed -= OnHealthChanged;
        UpdateService.Changed -= OnUpdateChanged;
        _ = CameraPreviewService.StopAsync();
    }
    // Note: the camera keeps running if the window is just closed; End Session
    // stops it. This lets the user close the window while gestures stay active.
}

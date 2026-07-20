using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using PalmSync.Tray.Services;

namespace PalmSync.Tray.ViewModels;

public enum HomeState { Idle, ConnectInput, Hosting, Connected }

public class HomeViewModel : ViewModelBase, IDisposable
{
    private readonly DispatcherTimer _timer;

    private HomeState _state = HomeState.Idle;
    private string _code = "";
    private string _password = "";
    private string _hostIp = "";
    private string _inputCode = "";
    private string _inputPassword = "";
    private string _statusText = "";
    private Brush _statusBrush = Brushes.Gray;
    private string _flash = "";
    private int _waitingCycles;
    private bool _cameraOn;

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
    public Brush StatusBrush { get => _statusBrush; set => Set(ref _statusBrush, value); }
    public string Flash { get => _flash; set => Set(ref _flash, value); }

    public bool CameraOn { get => _cameraOn; set { if (Set(ref _cameraOn, value)) OnPropertyChanged(nameof(CameraButtonText)); } }
    public string CameraButtonText => _cameraOn ? "📷  Stop camera gestures" : "📷  Use hand gestures (camera)";
    public bool CameraAvailable => EyeManager.IsAvailable;

    public ICommand StartHostingCommand { get; }
    public ICommand ShowConnectCommand { get; }
    public ICommand JoinCommand { get; }
    public ICommand BackCommand { get; }
    public ICommand GrabCommand { get; }
    public ICommand RestoreCommand { get; }
    public ICommand StopCommand { get; }
    public ICommand ToggleCameraCommand { get; }
    public ICommand CopyCodeCommand { get; }
    public ICommand CopyPasswordCommand { get; }

    public HomeViewModel()
    {
        StartHostingCommand = new RelayCommand(StartHosting);
        ShowConnectCommand = new RelayCommand(() => State = HomeState.ConnectInput);
        JoinCommand = new RelayCommand(Join);
        BackCommand = new RelayCommand(() => State = HomeState.Idle);
        GrabCommand = new RelayCommand(() => SendGesture("grab", "Workspace captured"));
        RestoreCommand = new RelayCommand(() => SendGesture("drop", "Workspace restored"));
        StopCommand = new RelayCommand(Stop);
        ToggleCameraCommand = new RelayCommand(ToggleCamera);
        CopyCodeCommand = new RelayCommand(() => TrySetClipboard(Code));
        CopyPasswordCommand = new RelayCommand(() => TrySetClipboard(Password));

        _timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(2) };
        _timer.Tick += (_, _) => RefreshStatus();

        RestoreFromExistingSession();
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
            StatusText = "Waiting for the other device...";
            StatusBrush = Brushes.DarkOrange;
        }
        catch (Exception ex)
        {
            MessageBox.Show(ex.Message, "PalmSync", MessageBoxButton.OK, MessageBoxImage.Warning);
        }
    }

    private void Join()
    {
        try
        {
            SessionController.Join(InputCode, InputPassword);
            _waitingCycles = 0;
            State = HomeState.Connected;
            StatusText = "Connecting...";
            StatusBrush = Brushes.DarkOrange;
            InputPassword = "";
        }
        catch (Exception ex)
        {
            MessageBox.Show(ex.Message, "PalmSync", MessageBoxButton.OK, MessageBoxImage.Warning);
        }
    }

    private void SendGesture(string kind, string confirmation)
    {
        try
        {
            GestureSender.Send(kind, SessionController.CurrentIpcPort());
            Flash = confirmation;
            // Clear the flash after a moment
            var t = new DispatcherTimer { Interval = TimeSpan.FromSeconds(2) };
            t.Tick += (_, _) => { Flash = ""; t.Stop(); };
            t.Start();
        }
        catch (Exception ex)
        {
            MessageBox.Show($"Could not send: {ex.Message}", "PalmSync",
                MessageBoxButton.OK, MessageBoxImage.Warning);
        }
    }

    private void ToggleCamera()
    {
        try
        {
            if (CameraOn)
            {
                EyeManager.Stop();
                CameraOn = false;
            }
            else
            {
                EyeManager.Start(SessionController.CurrentIpcPort());
                CameraOn = true;
            }
        }
        catch (Exception ex)
        {
            MessageBox.Show(
                $"Could not start the camera: {ex.Message}\n\nMake sure a webcam is connected.",
                "PalmSync", MessageBoxButton.OK, MessageBoxImage.Warning);
            CameraOn = false;
        }
    }

    private void Stop()
    {
        EyeManager.Stop();
        CameraOn = false;
        SessionController.Stop();
        Code = "";
        Password = "";
        State = HomeState.Idle;
    }

    private void RefreshStatus()
    {
        if (State is not (HomeState.Hosting or HomeState.Connected)) return;

        var isMaster = State == HomeState.Hosting;
        var peer = LogWatcher.GetStatus(isMaster);

        if (peer == PeerStatus.Connected)
        {
            _waitingCycles = 0;
            StatusText = isMaster ? "Other device connected ✓" : "Connected ✓";
            StatusBrush = Brushes.SeaGreen;
            return;
        }

        _waitingCycles++;
        StatusBrush = Brushes.DarkOrange;

        // ~2s per cycle; after ~12s of no connection, surface a hint.
        if (_waitingCycles >= 6)
        {
            StatusText = isMaster
                ? "Still waiting. Make sure the other computer used the exact code + password, and both are on the same Wi-Fi."
                : "Can't reach the host yet. Check: same Wi-Fi/network, and the code + password are exactly right.";
        }
        else
        {
            StatusText = isMaster ? "Waiting for the other device..." : "Connecting...";
        }
    }

    private static void TrySetClipboard(string text)
    {
        try { if (!string.IsNullOrEmpty(text)) Clipboard.SetText(text); }
        catch { /* clipboard busy */ }
    }

    public void Dispose() => _timer.Stop();
    // Note: the camera keeps running if the window is just closed; End Session
    // stops it. This lets the user close the window while gestures stay active.
}

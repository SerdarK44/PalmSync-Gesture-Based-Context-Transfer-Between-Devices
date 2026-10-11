using System.Diagnostics;
using System.IO;
using System.Windows;
using System.Windows.Input;
using PalmSync.Tray.Services;
using PalmSync.Tray.ViewModels;

namespace PalmSync.Tray.Views;

public partial class SystemCheckWindow : Window
{
    public SystemCheckWindow() : this(firstRun: false) { }

    public SystemCheckWindow(bool firstRun)
    {
        InitializeComponent();
        WindowFit.Attach(this);
        var vm = new SystemCheckViewModel(firstRun);
        DataContext = vm;
        Closed += (_, _) => vm.Dispose();
    }

    /// Open (or bring forward) the system check window.
    public static void ShowWindow(bool firstRun = false)
    {
        var existing = Application.Current.Windows.OfType<SystemCheckWindow>().FirstOrDefault();
        if (existing != null) { existing.Activate(); return; }
        new SystemCheckWindow(firstRun).Show();
    }
}

public record CheckGroup(string Name, List<CheckResult> Items);

public class SystemCheckViewModel : ViewModelBase, IDisposable
{
    private List<CheckGroup> _groups = new();
    private bool _isBusy;
    private string _busyText = "";
    private string _headline = "Kontrol ediliyor…";
    private string _subLine = "";
    private string _lastAction = "";

    public bool IsFirstRun { get; }
    public List<CheckGroup> Groups { get => _groups; private set => Set(ref _groups, value); }
    public bool IsBusy { get => _isBusy; private set => Set(ref _isBusy, value); }
    private bool _hasFixable;
    public bool HasFixable { get => _hasFixable; private set => Set(ref _hasFixable, value); }
    public string BusyText { get => _busyText; private set => Set(ref _busyText, value); }
    public string Headline { get => _headline; private set => Set(ref _headline, value); }
    public string SubLine { get => _subLine; private set => Set(ref _subLine, value); }
    public string FooterText => string.IsNullOrEmpty(_lastAction)
        ? $"Raporlar: {AppPaths.HealthDir}"
        : _lastAction;

    public ICommand RunCommand { get; }
    public ICommand FixAllCommand { get; }
    public ICommand FixCommand { get; }
    public ICommand OpenCommand { get; }
    public ICommand OpenLogsCommand { get; }
    public ICommand OpenReportsCommand { get; }

    public SystemCheckViewModel(bool firstRun)
    {
        IsFirstRun = firstRun;
        RunCommand = new AsyncRelayCommand(() => RunAsync("elle"));
        FixAllCommand = new AsyncRelayCommand(() => FixAsync(null));
        FixCommand = new RelayCommand<CheckResult>(r => { if (r != null) _ = FixAsync(r); });
        OpenCommand = new RelayCommand<CheckResult>(r =>
        {
            if (r?.OpenUri == null) return;
            try { Process.Start(new ProcessStartInfo(r.OpenUri) { UseShellExecute = true }); }
            catch (Exception ex) { AppLog.Warn($"'{r.OpenUri}' açılamadı: {ex.Message}"); }
        });
        OpenLogsCommand = new RelayCommand(TrayViewModel.ShowSingleton<LogViewerWindow>);
        OpenReportsCommand = new RelayCommand(() =>
        {
            try
            {
                Directory.CreateDirectory(AppPaths.HealthDir);
                Process.Start(new ProcessStartInfo("explorer.exe", $"\"{AppPaths.HealthDir}\"") { UseShellExecute = true });
            }
            catch { }
        });

        HealthCheck.Changed += OnChanged;

        // Show the last result at once; refresh if it's old or missing.
        if (HealthCheck.Last.Count > 0) Render();
        if (HealthCheck.Last.Count == 0 || DateTime.Now - HealthCheck.LastRun > TimeSpan.FromMinutes(2))
            _ = RunAsync(firstRun ? "ilk kurulum" : "pencere açıldı");
    }

    private void OnChanged() => Application.Current.Dispatcher.BeginInvoke(Render);

    private async Task RunAsync(string reason)
    {
        if (IsBusy) return;
        IsBusy = true;
        BusyText = "Kontrol ediliyor… (kamera testi birkaç saniye sürebilir)";
        try { await HealthCheck.RunAsync(reason, probeCamera: true); }
        finally { IsBusy = false; }
    }

    private async Task FixAsync(CheckResult? only)
    {
        if (IsBusy) return;
        IsBusy = true;
        BusyText = only == null
            ? "Düzeltiliyor… Windows yönetici izni isterse 'Evet' de."
            : $"Düzeltiliyor: {only.Title}…";
        try
        {
            var summary = await HealthCheck.FixAllAsync(only == null ? null : new[] { only });
            _lastAction = "Son işlem: " + summary;
            OnPropertyChanged(nameof(FooterText));
            if (IsFirstRun) MarkSetupDone();
        }
        finally { IsBusy = false; }
    }

    private void Render()
    {
        var results = HealthCheck.Last;
        Groups = results.GroupBy(r => r.Category)
            .Select(g => new CheckGroup(g.Key, g
                .OrderBy(r => r.Status switch { CheckStatus.Fail => 0, CheckStatus.Warn => 1, CheckStatus.Ok => 2, _ => 3 })
                .ToList()))
            .ToList();

        var fail = results.Count(r => r.Status == CheckStatus.Fail);
        var warn = results.Count(r => r.Status == CheckStatus.Warn);
        var fixable = results.Count(r => r.CanFix);
        HasFixable = fixable > 0;
        Headline = fail + warn == 0 ? "✅ Her şey hazır" : $"{fail} sorun, {warn} uyarı bulundu";
        SubLine = $"{HealthCheck.LastRun:HH:mm:ss} · {results.Count} kontrol" +
                  (fixable > 0 ? $" · {fixable} tanesi otomatik düzeltilebilir" : "");
        if (IsFirstRun && fail + warn == 0) MarkSetupDone();
    }

    private static void MarkSetupDone()
    {
        var s = UISettingsManager.Load();
        if (s.SetupDone) return;
        s.SetupDone = true;
        UISettingsManager.Save(s);
    }

    public void Dispose() => HealthCheck.Changed -= OnChanged;
}

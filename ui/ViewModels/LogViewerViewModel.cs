using System.Collections;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.IO.Compression;
using System.Text;
using System.Text.RegularExpressions;
using System.Windows;
using System.Windows.Data;
using System.Windows.Input;
using System.Windows.Threading;
using Microsoft.Win32;
using PalmSync.Tray.Services;

namespace PalmSync.Tray.ViewModels;

public record Choice<T>(string Label, T Value)
{
    public override string ToString() => Label;
}

/// Live, filterable view over all PalmSync logs (daemon, network, camera,
/// UI, security audit), merged by time.
public class LogViewerViewModel : ViewModelBase, IDisposable
{
    private const int MaxEntries = 20_000;

    private readonly List<LogSource> _live = new();
    private readonly DispatcherTimer _poll;
    private readonly DispatcherTimer _searchDebounce;

    private bool _friendlyMode = true;
    private bool _follow = true;
    private bool _paused;
    private string _searchText = "";
    private string _appliedSearch = "";
    private Choice<string> _source;
    private Choice<LogLevel> _minLevel;
    private LogEntry? _selected;
    private int _errorCount, _warnCount;
    private string _statusLine = "";

    public ObservableCollection<LogEntry> Entries { get; } = new();
    public ICollectionView View { get; }

    /// Raised after new rows were appended (the window scrolls to the end).
    public event Action? Appended;

    public List<Choice<string>> Sources { get; } = new()
    {
        new("Tüm kaynaklar", ""),
        new("Daemon", "Daemon"),
        new("Ağ", "Ağ"),
        new("Kamera", "Kamera"),
        new("Arayüz", "Arayüz"),
        new("Güvenlik", "Güvenlik"),
        new("Cihaz raporları", "Cihazlar"),
    };

    public List<Choice<LogLevel>> Levels { get; } = new()
    {
        new("Sadece hatalar", LogLevel.Error),
        new("Uyarı ve hatalar", LogLevel.Warn),
        new("Bilgi ve üstü", LogLevel.Info),
        new("Hepsi (debug dahil)", LogLevel.Trace),
    };

    /// true = "Önemli olaylar" (plain-language events only),
    /// false = "Tüm kayıtlar" (every technical line).
    public bool FriendlyMode
    {
        get => _friendlyMode;
        set { if (Set(ref _friendlyMode, value)) { OnPropertyChanged(nameof(AllMode)); Refilter(); } }
    }
    public bool AllMode { get => !_friendlyMode; set => FriendlyMode = !value; }

    public bool Follow { get => _follow; set => Set(ref _follow, value); }

    public bool Paused
    {
        get => _paused;
        set { if (Set(ref _paused, value)) OnPropertyChanged(nameof(LiveText)); }
    }
    public string LiveText => _paused ? "⏸ Duraklatıldı" : "● Canlı";

    public string SearchText
    {
        get => _searchText;
        set
        {
            if (Set(ref _searchText, value))
            {
                _searchDebounce.Stop();
                _searchDebounce.Start();
            }
        }
    }

    public Choice<string> Source
    {
        get => _source;
        set { if (Set(ref _source, value)) Refilter(); }
    }

    public Choice<LogLevel> MinLevel
    {
        get => _minLevel;
        set { if (Set(ref _minLevel, value)) Refilter(); }
    }

    public LogEntry? Selected
    {
        get => _selected;
        set { if (Set(ref _selected, value)) OnPropertyChanged(nameof(DetailText)); }
    }

    public string DetailText => _selected == null
        ? "Ayrıntısını görmek için bir satır seç."
        : $"{_selected.Time:dd.MM.yyyy HH:mm:ss.fff}   {_selected.LevelText}   {_selected.Source} / {_selected.Component}\n" +
          (_selected.Summary != null && _selected.Summary != _selected.Message ? $"\n{_selected.Summary}\n" : "") +
          $"\n{_selected.Message}";

    public int ErrorCount { get => _errorCount; private set => Set(ref _errorCount, value); }
    public int WarnCount { get => _warnCount; private set => Set(ref _warnCount, value); }
    public string StatusLine { get => _statusLine; private set => Set(ref _statusLine, value); }
    public bool IsEmpty => View.IsEmpty;
    public string EmptyText => Entries.Count == 0
        ? "Henüz kayıt yok. Bir oturum başlatınca burada görünecek."
        : "Filtreye uyan kayıt yok. Arama veya filtreleri değiştir.";

    public string LogDir { get; }
    public string LevelHint { get; }

    public ICommand ShowErrorsCommand { get; }
    public ICommand ShowWarningsCommand { get; }
    public ICommand ClearFiltersCommand { get; }
    public ICommand CopyCommand { get; }
    public ICommand ExportCommand { get; }
    public ICommand OpenFolderCommand { get; }
    public ICommand ClearViewCommand { get; }
    public ICommand ReloadCommand { get; }
    public ICommand TogglePauseCommand { get; }

    public LogViewerViewModel()
    {
        LogDir = AppLog.LogDir;
        var level = ConfigManager.Load().LogLevel;
        LevelHint = level is "debug" or "trace"
            ? $"Kayıt seviyesi: {level}"
            : $"Kayıt seviyesi: {level} — daha ayrıntılı kayıt için Ayarlar › Kayıt seviyesi";

        _source = Sources[0];
        _minLevel = Levels[2];

        View = CollectionViewSource.GetDefaultView(Entries);
        View.Filter = o => o is LogEntry e && Matches(e);

        ShowErrorsCommand = new RelayCommand(() => { AllMode = true; MinLevel = Levels[0]; });
        ShowWarningsCommand = new RelayCommand(() => { AllMode = true; MinLevel = Levels[1]; });
        ClearFiltersCommand = new RelayCommand(() =>
        {
            _searchText = ""; OnPropertyChanged(nameof(SearchText)); _appliedSearch = "";
            _source = Sources[0]; OnPropertyChanged(nameof(Source));
            _minLevel = Levels[2]; OnPropertyChanged(nameof(MinLevel));
            Refilter();
        });
        CopyCommand = new RelayCommand<IList>(Copy);
        ExportCommand = new RelayCommand(Export);
        OpenFolderCommand = new RelayCommand(OpenFolder);
        ClearViewCommand = new RelayCommand(() =>
        {
            Entries.Clear();
            ErrorCount = WarnCount = 0;
            Selected = null;
            UpdateStatus();
        });
        ReloadCommand = new RelayCommand(LoadAll);
        TogglePauseCommand = new RelayCommand(() => Paused = !Paused);

        _searchDebounce = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(250) };
        _searchDebounce.Tick += (_, _) =>
        {
            _searchDebounce.Stop();
            _appliedSearch = _searchText.Trim();
            Refilter();
        };

        LoadAll();

        _poll = new DispatcherTimer { Interval = TimeSpan.FromSeconds(1) };
        _poll.Tick += (_, _) => { if (!Paused) PollNew(); };
        _poll.Start();
    }

    // ── Loading ─────────────────────────────────────────────

    private IEnumerable<(string name, string file)> Files() => new[]
    {
        ("Daemon", "palmsync-brain.log"),
        ("Kamera", "palmsync-eye.log"),
        ("Arayüz", "palmsync-ui.log"),
        ("Güvenlik", "audit.log"),
        ("Cihazlar", "cihaz-raporlari.log"),
    };

    private void LoadAll()
    {
        _live.Clear();
        Entries.Clear();
        Selected = null;

        var all = new List<LogEntry>();
        foreach (var (name, file) in Files())
        {
            // The most recent rotated file too, so history survives a rotation.
            all.AddRange(new LogSource(name, Path.Combine(LogDir, file + ".1")).ReadNew(1024 * 1024));
            var live = new LogSource(name, Path.Combine(LogDir, file));
            all.AddRange(live.ReadNew(4 * 1024 * 1024));
            _live.Add(live);
        }

        // Stable sort keeps same-millisecond lines in file order.
        foreach (var e in all.OrderBy(e => e.Time).TakeLast(MaxEntries))
            Entries.Add(e);

        Recount();
        Refilter();
        Appended?.Invoke();
    }

    private void PollNew()
    {
        var batch = new List<LogEntry>();
        foreach (var s in _live) batch.AddRange(s.ReadNew());
        if (batch.Count == 0) return;

        foreach (var e in batch.OrderBy(e => e.Time))
        {
            Entries.Add(e);
            if (e.Level == LogLevel.Error) ErrorCount++;
            else if (e.Level == LogLevel.Warn) WarnCount++;
        }
        while (Entries.Count > MaxEntries) Entries.RemoveAt(0);

        UpdateStatus();
        Appended?.Invoke();
    }

    // ── Filtering ───────────────────────────────────────────

    private bool Matches(LogEntry e)
    {
        if (_friendlyMode)
        {
            if (!e.IsEvent) return false;
        }
        else if (e.Level < _minLevel.Value) return false;

        if (_source.Value.Length > 0 && e.Source != _source.Value) return false;

        if (_appliedSearch.Length > 0)
        {
            var q = _appliedSearch;
            return e.Message.Contains(q, StringComparison.CurrentCultureIgnoreCase)
                || (e.Summary?.Contains(q, StringComparison.CurrentCultureIgnoreCase) ?? false)
                || e.Component.Contains(q, StringComparison.OrdinalIgnoreCase);
        }
        return true;
    }

    private void Refilter()
    {
        View.Refresh();
        UpdateStatus();
        Appended?.Invoke();
    }

    private void Recount()
    {
        ErrorCount = Entries.Count(e => e.Level == LogLevel.Error);
        WarnCount = Entries.Count(e => e.Level == LogLevel.Warn);
    }

    private void UpdateStatus()
    {
        var shown = View.Cast<object>().Count();
        StatusLine = $"{shown:N0} / {Entries.Count:N0} kayıt gösteriliyor";
        OnPropertyChanged(nameof(IsEmpty));
        OnPropertyChanged(nameof(EmptyText));
    }

    // ── Actions ─────────────────────────────────────────────

    private void Copy(IList? selected)
    {
        IEnumerable<LogEntry> rows = selected is { Count: > 0 }
            ? selected.OfType<LogEntry>().OrderBy(e => e.Time)
            : View.Cast<LogEntry>();
        var text = string.Join(Environment.NewLine, rows.Select(r => r.ToLine()));
        try
        {
            Clipboard.SetText(text);
            StatusLine = $"{rows.Count():N0} satır panoya kopyalandı";
        }
        catch { /* clipboard busy */ }
    }

    private void OpenFolder()
    {
        try
        {
            Directory.CreateDirectory(LogDir);
            Process.Start(new ProcessStartInfo("explorer.exe", $"\"{LogDir}\"") { UseShellExecute = true });
        }
        catch (Exception ex)
        {
            MessageBox.Show($"Klasör açılamadı: {ex.Message}", "PalmSync",
                MessageBoxButton.OK, MessageBoxImage.Warning);
        }
    }

    /// One zip with every log file, a troubleshoot report and the config
    /// (password removed) — everything needed to diagnose a bug report.
    private async void Export()
    {
        var dlg = new SaveFileDialog
        {
            FileName = $"palmsync-kayitlar-{DateTime.Now:yyyyMMdd-HHmmss}.zip",
            Filter = "Zip arşivi (*.zip)|*.zip",
        };
        if (dlg.ShowDialog() != true) return;

        StatusLine = "Dışa aktarılıyor…";
        try
        {
            var target = dlg.FileName;
            var dir = LogDir;
            var count = await Task.Run(() => WriteZip(target, dir));
            AppLog.Info($"Kayıtlar dışa aktarıldı: {target}");
            StatusLine = $"{count} dosya dışa aktarıldı";
            MessageBox.Show($"Kayıtlar kaydedildi:\n{target}\n\nHata bildirirken bu dosyayı ekleyebilirsin.",
                "PalmSync", MessageBoxButton.OK, MessageBoxImage.Information);
        }
        catch (Exception ex)
        {
            AppLog.Error("Kayıtlar dışa aktarılamadı", ex);
            MessageBox.Show($"Dışa aktarma başarısız: {ex.Message}", "PalmSync",
                MessageBoxButton.OK, MessageBoxImage.Error);
            UpdateStatus();
        }
    }

    private static int WriteZip(string target, string logDir)
    {
        int n = 0;
        using var zip = ZipFile.Open(target, ZipArchiveMode.Create);

        if (Directory.Exists(logDir))
        {
            foreach (var file in Directory.EnumerateFiles(logDir, "*", SearchOption.AllDirectories))
            {
                var rel = Path.GetRelativePath(logDir, file).Replace('\\', '/');
                var entry = zip.CreateEntry("loglar/" + rel, CompressionLevel.Optimal);
                using var src = new FileStream(file, FileMode.Open, FileAccess.Read,
                    FileShare.ReadWrite | FileShare.Delete);
                using var dst = entry.Open();
                src.CopyTo(dst);
                n++;
            }
        }

        void AddText(string name, string text)
        {
            using var w = new StreamWriter(zip.CreateEntry(name).Open(), new UTF8Encoding(false));
            w.Write(text);
            n++;
        }

        try { AddText("sorun-giderme-raporu.txt", Diagnostics.Run()); } catch { }
        try
        {
            if (File.Exists(ConfigManager.ConfigPath))
            {
                var cfg = File.ReadAllText(ConfigManager.ConfigPath);
                cfg = Regex.Replace(cfg, "(\"password\"\\s*:\\s*)\"[^\"]*\"", "$1\"***\"");
                AddText("config.json", cfg);
            }
        }
        catch { }
        return n;
    }

    public void Dispose()
    {
        _poll.Stop();
        _searchDebounce.Stop();
    }
}

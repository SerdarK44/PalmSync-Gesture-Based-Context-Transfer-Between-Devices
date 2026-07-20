using System.Collections.ObjectModel;
using System.IO;
using System.Windows;
using System.Windows.Input;
using Microsoft.Win32;
using PalmSync.Tray.Services;

namespace PalmSync.Tray.ViewModels;

public record LogEntry(string Timestamp, string Level, string Component, string Message);

public class LogViewerViewModel : ViewModelBase
{
    private const int MaxLines = 2000;

    private string _selectedLevel = "ALL";
    private List<LogEntry> _allEntries = new();

    public ObservableCollection<LogEntry> Entries { get; } = new();
    public List<string> LevelFilters { get; } = new() { "ALL", "ERROR", "WARN", "INFO", "DEBUG", "TRACE" };

    public string SelectedLevel
    {
        get => _selectedLevel;
        set
        {
            if (Set(ref _selectedLevel, value))
                ApplyFilter();
        }
    }

    public ICommand RefreshCommand { get; }
    public ICommand ExportCommand { get; }

    public LogViewerViewModel()
    {
        RefreshCommand = new RelayCommand(LoadLogs);
        ExportCommand = new RelayCommand(Export);
        LoadLogs();
    }

    private string LogFilePath
    {
        get
        {
            var config = ConfigManager.Load();
            return Path.Combine(config.LogDir, "palmsync-brain.log");
        }
    }

    private void LoadLogs()
    {
        _allEntries.Clear();

        try
        {
            if (!File.Exists(LogFilePath))
            {
                _allEntries.Add(new LogEntry("—", "INFO", "ui", $"Log file not found: {LogFilePath}"));
                ApplyFilter();
                return;
            }

            using var fs = new FileStream(LogFilePath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite);
            using var reader = new StreamReader(fs);
            var lines = new List<string>();
            string? line;
            while ((line = reader.ReadLine()) != null)
            {
                lines.Add(line);
                if (lines.Count > MaxLines) lines.RemoveAt(0);
            }

            foreach (var l in lines)
                _allEntries.Add(ParseLine(l));
        }
        catch (Exception ex)
        {
            _allEntries.Add(new LogEntry("—", "ERROR", "ui", $"Could not read log: {ex.Message}"));
        }

        ApplyFilter();
    }

    // fern format: "2026-07-20 18:45:12.345 [INFO] target: message"
    private static LogEntry ParseLine(string line)
    {
        try
        {
            var bracketStart = line.IndexOf('[');
            var bracketEnd = line.IndexOf(']');
            if (bracketStart > 0 && bracketEnd > bracketStart)
            {
                var timestamp = line[..bracketStart].Trim();
                var level = line[(bracketStart + 1)..bracketEnd];
                var rest = line[(bracketEnd + 1)..].TrimStart();

                var colonIdx = rest.IndexOf(": ");
                var component = colonIdx > 0 ? rest[..colonIdx] : "";
                var message = colonIdx > 0 ? rest[(colonIdx + 2)..] : rest;

                return new LogEntry(timestamp, level, component, message);
            }
        }
        catch
        {
            // fall through
        }
        return new LogEntry("", "", "", line);
    }

    private void ApplyFilter()
    {
        Entries.Clear();
        var filtered = _selectedLevel == "ALL"
            ? _allEntries
            : _allEntries.Where(e => e.Level.Equals(_selectedLevel, StringComparison.OrdinalIgnoreCase)).ToList();

        // Show newest first
        for (var i = filtered.Count - 1; i >= 0; i--)
            Entries.Add(filtered[i]);
    }

    private void Export()
    {
        var dlg = new SaveFileDialog
        {
            FileName = $"palmsync-logs-{DateTime.Now:yyyyMMdd-HHmmss}.txt",
            Filter = "Text files (*.txt)|*.txt|All files (*.*)|*.*",
        };

        if (dlg.ShowDialog() != true) return;

        try
        {
            File.WriteAllLines(dlg.FileName,
                Entries.Select(e => $"{e.Timestamp} [{e.Level}] {e.Component}: {e.Message}"));
            MessageBox.Show($"Exported {Entries.Count} entries.", "PalmSync",
                MessageBoxButton.OK, MessageBoxImage.Information);
        }
        catch (Exception ex)
        {
            MessageBox.Show($"Export failed: {ex.Message}", "PalmSync",
                MessageBoxButton.OK, MessageBoxImage.Error);
        }
    }
}

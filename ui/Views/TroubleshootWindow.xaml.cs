using System.Windows;
using PalmSync.Tray.Services;
using PalmSync.Tray.ViewModels;

namespace PalmSync.Tray.Views;

public partial class TroubleshootWindow : Window
{
    public TroubleshootWindow()
    {
        InitializeComponent();
        WindowFit.Attach(this);
        DataContext = new TroubleshootViewModel();
    }
}

public class TroubleshootViewModel : ViewModelBase
{
    private string _report = "Kontroller yapılıyor…";
    public string Report { get => _report; set => Set(ref _report, value); }

    public System.Windows.Input.ICommand RefreshCommand { get; }
    public System.Windows.Input.ICommand CopyCommand { get; }
    public System.Windows.Input.ICommand OpenLogsCommand { get; }
    public System.Windows.Input.ICommand OpenSystemCheckCommand { get; }

    public TroubleshootViewModel()
    {
        RefreshCommand = new AsyncRelayCommand(RunAsync);
        CopyCommand = new RelayCommand(() =>
        {
            try { Clipboard.SetText(Report); } catch { }
        });
        OpenLogsCommand = new RelayCommand(TrayViewModel.ShowSingleton<LogViewerWindow>);
        OpenSystemCheckCommand = new RelayCommand(() => SystemCheckWindow.ShowWindow());
        _ = RunAsync();
    }

    private async Task RunAsync()
    {
        Report = "Kontroller yapılıyor…";
        try
        {
            Report = await Task.Run(Diagnostics.Run);
        }
        catch (Exception ex)
        {
            AppLog.Error("Sorun giderme raporu oluşturulamadı", ex);
            Report = "Rapor oluşturulamadı: " + ex.Message;
        }
    }
}

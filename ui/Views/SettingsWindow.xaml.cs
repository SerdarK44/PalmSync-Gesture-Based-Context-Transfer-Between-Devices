using System.Windows;
using PalmSync.Tray.Services;
using PalmSync.Tray.ViewModels;

namespace PalmSync.Tray.Views;

public partial class SettingsWindow : Window
{
    public SettingsWindow()
    {
        InitializeComponent();
        WindowFit.Attach(this);
        var vm = new SettingsViewModel();
        vm.RequestClose += Close;
        Closed += (_, _) => vm.OnClosed();
        DataContext = vm;
    }
}

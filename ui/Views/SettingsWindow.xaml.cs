using System.Windows;
using PalmSync.Tray.ViewModels;

namespace PalmSync.Tray.Views;

public partial class SettingsWindow : Window
{
    public SettingsWindow()
    {
        InitializeComponent();
        var vm = new SettingsViewModel();
        vm.RequestClose += Close;
        DataContext = vm;
    }
}

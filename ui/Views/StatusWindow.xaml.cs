using System.Windows;
using PalmSync.Tray.ViewModels;

namespace PalmSync.Tray.Views;

public partial class StatusWindow : Window
{
    public StatusWindow()
    {
        InitializeComponent();
        var vm = new StatusViewModel();
        DataContext = vm;
        Closed += (_, _) => vm.Dispose();
    }
}

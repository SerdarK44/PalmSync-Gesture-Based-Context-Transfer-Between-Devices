using System.Windows;
using PalmSync.Tray.ViewModels;

namespace PalmSync.Tray.Views;

public partial class HomeWindow : Window
{
    public HomeWindow()
    {
        InitializeComponent();
        var vm = new HomeViewModel();
        DataContext = vm;
        Closed += (_, _) => vm.Dispose();
    }
}

using System.Windows;
using PalmSync.Tray.ViewModels;

namespace PalmSync.Tray.Views;

public partial class LogViewerWindow : Window
{
    public LogViewerWindow()
    {
        InitializeComponent();
        DataContext = new LogViewerViewModel();
    }
}

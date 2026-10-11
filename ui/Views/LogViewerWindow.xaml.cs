using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using PalmSync.Tray.Services;
using PalmSync.Tray.ViewModels;

namespace PalmSync.Tray.Views;

public partial class LogViewerWindow : Window
{
    private readonly LogViewerViewModel _vm;
    private ScrollViewer? _scroll;

    public ICommand FocusSearchCommand { get; }

    public LogViewerWindow()
    {
        FocusSearchCommand = new RelayCommand(() => { SearchBox.Focus(); SearchBox.SelectAll(); });
        InitializeComponent();
        WindowFit.Attach(this);

        _vm = new LogViewerViewModel();
        DataContext = _vm;
        _vm.Appended += ScrollToEndIfFollowing;

        Loaded += (_, _) =>
        {
            _scroll = FindScrollViewer(Grid);
            if (_scroll != null) _scroll.ScrollChanged += OnScrollChanged;
            ScrollToEndIfFollowing();
        };
        Closed += (_, _) =>
        {
            _vm.Appended -= ScrollToEndIfFollowing;
            _vm.Dispose();
        };
    }

    /// Follow the newest line like a terminal — but stop following as soon as
    /// the user scrolls up to read, and resume when they scroll back down.
    private void OnScrollChanged(object sender, ScrollChangedEventArgs e)
    {
        if (e.ExtentHeightChange != 0) return;   // content grew, not a user scroll
        _vm.Follow = _scroll!.VerticalOffset >= _scroll.ScrollableHeight - 1;
    }

    private void ScrollToEndIfFollowing()
    {
        if (!_vm.Follow) return;
        Dispatcher.BeginInvoke(() => _scroll?.ScrollToEnd(),
            System.Windows.Threading.DispatcherPriority.Background);
    }

    private static ScrollViewer? FindScrollViewer(DependencyObject root)
    {
        if (root is ScrollViewer sv) return sv;
        for (int i = 0; i < VisualTreeHelper.GetChildrenCount(root); i++)
        {
            var found = FindScrollViewer(VisualTreeHelper.GetChild(root, i));
            if (found != null) return found;
        }
        return null;
    }
}

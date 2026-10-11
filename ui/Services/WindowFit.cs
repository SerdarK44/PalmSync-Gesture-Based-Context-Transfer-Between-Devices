using System.Windows;

namespace PalmSync.Tray.Services;

/// Keeps a window inside the usable screen area (taskbar excluded). Laptops
/// at 1366×768 with 125% scaling only have ~580 px of height; several windows
/// were taller than that and their bottom buttons were off-screen.
public static class WindowFit
{
    private const double Margin = 16;

    public static void Attach(Window w)
    {
        Apply(w);
        // SizeToContent windows grow after layout: re-clamp once rendered.
        w.ContentRendered += (_, _) => Apply(w);
    }

    public static void Apply(Window w)
    {
        var area = SystemParameters.WorkArea;
        var maxW = Math.Max(320, area.Width - Margin * 2);
        var maxH = Math.Max(320, area.Height - Margin * 2);

        w.MaxWidth = Math.Min(double.IsInfinity(w.MaxWidth) ? maxW : w.MaxWidth, maxW);
        w.MaxHeight = Math.Min(double.IsInfinity(w.MaxHeight) ? maxH : w.MaxHeight, maxH);
        if (w.MinHeight > maxH) w.MinHeight = maxH;
        if (w.MinWidth > maxW) w.MinWidth = maxW;
        if (!double.IsNaN(w.Width) && w.Width > maxW) w.Width = maxW;
        if (!double.IsNaN(w.Height) && w.Height > maxH) w.Height = maxH;

        if (!w.IsLoaded) return;

        // Pull the window back on screen if it ended up partly outside.
        var width = w.ActualWidth > 0 ? w.ActualWidth : w.Width;
        var height = w.ActualHeight > 0 ? w.ActualHeight : w.Height;
        if (double.IsNaN(width) || double.IsNaN(height)) return;
        if (w.Left + width > area.Right) w.Left = Math.Max(area.Left, area.Right - width - Margin);
        if (w.Top + height > area.Bottom) w.Top = Math.Max(area.Top, area.Bottom - height - Margin);
        if (w.Left < area.Left) w.Left = area.Left + Margin;
        if (w.Top < area.Top) w.Top = area.Top + Margin;
    }
}

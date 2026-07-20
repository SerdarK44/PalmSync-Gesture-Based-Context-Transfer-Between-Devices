using System.Windows;
using Microsoft.Win32;

namespace PalmSync.Tray.Services;

public enum AppTheme
{
    Light,
    Dark,
}

public static class ThemeService
{
    public static AppTheme Current { get; private set; } = AppTheme.Light;

    public static void ApplySystemTheme()
    {
        Apply(DetectSystemTheme());
    }

    public static void Apply(AppTheme theme)
    {
        Current = theme;
        var uri = theme == AppTheme.Dark
            ? new Uri("Resources/Themes/DarkTheme.xaml", UriKind.Relative)
            : new Uri("Resources/Themes/LightTheme.xaml", UriKind.Relative);

        var dict = new ResourceDictionary { Source = uri };
        var merged = Application.Current.Resources.MergedDictionaries;

        // Replace the theme dictionary (first merged dictionary is always the theme)
        if (merged.Count > 0)
            merged[0] = dict;
        else
            merged.Add(dict);
    }

    public static void Toggle()
        => Apply(Current == AppTheme.Dark ? AppTheme.Light : AppTheme.Dark);

    private static AppTheme DetectSystemTheme()
    {
        try
        {
            var value = Registry.GetValue(
                @"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
                "AppsUseLightTheme", 1);
            return value is int light && light == 0 ? AppTheme.Dark : AppTheme.Light;
        }
        catch
        {
            return AppTheme.Light;
        }
    }
}

# 🎨 Faz 5: UI ve Kullanıcı Deneyimi (User Interface)

**Süre**: 2 hafta | **Öncelik**: ⭐⭐ (Yüksek) | **Bağımlılık**: Faz 1-4

---

## 📍 Hedef

PalmSync'in görünür yüzünü inşa edeceğiz. System tray uygulaması, ayarlar paneli, status indikator'lar ve kullanıcı dostu bildirimler sayesinde kontrol ve gözlemleme sağlayacağız.

---

## ✅ Başarı Kriterleri

- [ ] System tray uygulaması çalışır
- [ ] Tray menu'sü açılır/kapanır, ayarları kur
- [ ] Status indicator (connected/disconnected/error) gösterilir
- [ ] Configuration UI açılır ve ayarları kaydeder
- [ ] Toast notifications çalışır (GRAB/DROP events)
- [ ] Event log viewer mevcuttur
- [ ] Performance monitor gösterilir (CPU/Memory/Latency)
- [ ] Keyboard shortcuts konfigüre edilebilir
- [ ] Dark/Light theme toggle'ı vardır
- [ ] Language support (TR/EN) opsiyonel

---

## 🎯 Görevler (Checklist)

### 1. System Tray Application Framework

**Teknoloji Stack:**
- C#/.NET 8.0 (WPF veya WinUI 3)
- fody/PropertyChanged (MVVM)
- Hardcodet.NotifyIcon.Wpf (Tray control)

**`ui/PalmSync.Tray.csproj`:**

```xml
<Project Sdk="Microsoft.NET.Sdk.WindowsDesktop">

  <PropertyGroup>
    <OutputType>WinExe</OutputType>
    <TargetFramework>net8.0-windows</TargetFramework>
    <UseWPF>true</UseWPF>
    <Platforms>x64</Platforms>
    <RuntimeIdentifier>win-x64</RuntimeIdentifier>
  </PropertyGroup>

  <ItemGroup>
    <PackageReference Include="Hardcodet.NotifyIcon.Wpf" Version="1.1.0" />
    <PackageReference Include="PropertyChanged.Fody" Version="4.1.0" />
    <PackageReference Include="Serilog" Version="3.0.0" />
  </ItemGroup>

</Project>
```

**Yapılacaklar:**
- [ ] WPF project structure'ı oluştur
- [ ] MVVM pattern'i implement et
- [ ] Dependency injection container'ı kur
- [ ] Application lifecycle'ı manage et

---

### 2. System Tray Icon ve Context Menu

**`ui/Views/TrayIcon.xaml`:**

```xml
<Window x:Class="PalmSync.Tray.Views.TrayIcon"
        xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        xmlns:tb="http://www.hardcodet.net/taskbar"
        ShowInTaskbar="False"
        Visibility="Hidden">
    
    <tb:TaskbarIcon IconSource="/Assets/palmsync.ico"
                    ToolTipText="{Binding StatusText}">
        <tb:TaskbarIcon.ContextMenu>
            <ContextMenu>
                <MenuItem Header="Open Settings" Command="{Binding OpenSettingsCommand}" />
                <MenuItem Header="Show Status" Command="{Binding ShowStatusCommand}" />
                <MenuItem Header="Logs" Command="{Binding ShowLogsCommand}" />
                <Separator />
                <MenuItem Header="Restart Service" Command="{Binding RestartCommand}" />
                <MenuItem Header="About" Command="{Binding ShowAboutCommand}" />
                <Separator />
                <MenuItem Header="Exit" Command="{Binding ExitCommand}" />
            </ContextMenu>
        </tb:TaskbarIcon.ContextMenu>
    </tb:TaskbarIcon>
</Window>
```

**`ui/ViewModels/TrayIconViewModel.cs`:**

```csharp
using PropertyChanged;

[AddINotifyPropertyChangedInterface]
public class TrayIconViewModel {
    
    public string StatusText { get; set; } = "PalmSync Ready";
    public Status CurrentStatus { get; set; } = Status.Connected;
    
    public ICommand OpenSettingsCommand { get; }
    public ICommand ShowStatusCommand { get; }
    public ICommand ShowLogsCommand { get; }
    public ICommand RestartCommand { get; }
    public ICommand ShowAboutCommand { get; }
    public ICommand ExitCommand { get; }
    
    public TrayIconViewModel() {
        OpenSettingsCommand = new RelayCommand(OpenSettings);
        ShowStatusCommand = new RelayCommand(ShowStatus);
        ShowLogsCommand = new RelayCommand(ShowLogs);
        RestartCommand = new AsyncRelayCommand(Restart);
        ShowAboutCommand = new RelayCommand(ShowAbout);
        ExitCommand = new RelayCommand(Exit);
    }
    
    private void OpenSettings() {
        var settingsWindow = new SettingsWindow();
        settingsWindow.ShowDialog();
    }
    
    private void ShowStatus() {
        var statusWindow = new StatusWindow { DataContext = new StatusViewModel() };
        statusWindow.ShowDialog();
    }
    
    private void ShowLogs() {
        Process.Start(new ProcessStartInfo {
            FileName = "notepad",
            Arguments = @"C:\ProgramData\PalmSync\logs\palmsync.log",
            UseShellExecute = true
        });
    }
    
    private async Task Restart() {
        await RestartService();
        StatusText = "Service restarted";
    }
    
    private void ShowAbout() {
        var aboutWindow = new AboutWindow();
        aboutWindow.ShowDialog();
    }
    
    private void Exit() {
        Application.Current.Shutdown();
    }
    
    private async Task RestartService() {
        // Windows Service control komutları
        var psi = new ProcessStartInfo {
            FileName = "cmd.exe",
            Arguments = "/c net stop PalmSync & net start PalmSync",
            UseShellExecute = false,
            RedirectStandardOutput = true,
            CreateNoWindow = true
        };
        
        using (var process = Process.Start(psi)) {
            await Task.Run(() => process.WaitForExit());
        }
    }
}

public enum Status {
    Connected,
    Disconnected,
    Error
}
```

**Yapılacaklar:**
- [ ] Tray icon resource'ları ekle (ICO, PNG)
- [ ] Context menu'yü design et
- [ ] Command binding'lerini implement et
- [ ] Status indicator'ları ekle
- [ ] Hover tooltip'i konfigür et

---

### 3. Settings/Configuration UI

**`ui/Views/SettingsWindow.xaml`:**

```xml
<Window x:Class="PalmSync.Tray.Views.SettingsWindow"
        xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        Title="PalmSync Settings"
        Width="600"
        Height="500"
        ResizeMode="NoResize">
    
    <Grid Background="{DynamicResource WindowBackground}">
        <TabControl>
            <!-- Tab 1: General -->
            <TabItem Header="General">
                <StackPanel Margin="16">
                    <CheckBox Content="Auto-start on login"
                              IsChecked="{Binding AutoStartOnLogin}" 
                              Margin="0,10"/>
                    
                    <CheckBox Content="Enable notifications"
                              IsChecked="{Binding EnableNotifications}" 
                              Margin="0,10"/>
                    
                    <Label Content="Language" FontWeight="Bold" Margin="0,20,0,5"/>
                    <ComboBox ItemsSource="{Binding Languages}"
                              SelectedItem="{Binding SelectedLanguage}">
                        <ComboBoxItem>English</ComboBoxItem>
                        <ComboBoxItem>Türkçe</ComboBoxItem>
                    </ComboBox>
                </StackPanel>
            </TabItem>
            
            <!-- Tab 2: Network -->
            <TabItem Header="Network">
                <StackPanel Margin="16">
                    <Label Content="Master Node Address" FontWeight="Bold"/>
                    <TextBox Text="{Binding MasterAddress}" Margin="0,5,0,15"/>
                    
                    <Label Content="Master Port" FontWeight="Bold"/>
                    <TextBox Text="{Binding MasterPort}" Margin="0,5,0,15"/>
                    
                    <Label Content="Password" FontWeight="Bold"/>
                    <PasswordBox x:Name="PasswordBox" Margin="0,5,0,15"/>
                    
                    <Button Content="Test Connection" 
                            Command="{Binding TestConnectionCommand}"
                            Padding="10,8"
                            Background="Blue"
                            Foreground="White"/>
                </StackPanel>
            </TabItem>
            
            <!-- Tab 3: Gestures -->
            <TabItem Header="Gestures">
                <StackPanel Margin="16">
                    <Label Content="Gesture Sensitivity" FontWeight="Bold"/>
                    <Slider Value="{Binding GestureSensitivity}"
                            Minimum="0.1"
                            Maximum="1.0"
                            TickFrequency="0.1"
                            Margin="0,5,0,15"/>
                    
                    <Label Content="Debounce (ms)" FontWeight="Bold"/>
                    <TextBox Text="{Binding DebounceMs}" Margin="0,5,0,15"/>
                    
                    <Label Content="Camera" FontWeight="Bold"/>
                    <ComboBox ItemsSource="{Binding AvailableCameras}"
                              SelectedItem="{Binding SelectedCamera}"
                              Margin="0,5,0,15"/>
                </StackPanel>
            </TabItem>
            
            <!-- Tab 4: Keyboard Shortcuts -->
            <TabItem Header="Shortcuts">
                <StackPanel Margin="16">
                    <Label Content="Configure keyboard shortcuts" FontWeight="Bold"/>
                    
                    <Grid Margin="0,15">
                        <Grid.ColumnDefinitions>
                            <ColumnDefinition Width="200"/>
                            <ColumnDefinition Width="*"/>
                        </Grid.ColumnDefinitions>
                        <Grid.RowDefinitions>
                            <RowDefinition Height="40"/>
                            <RowDefinition Height="40"/>
                            <RowDefinition Height="40"/>
                        </Grid.RowDefinitions>
                        
                        <Label Content="Enable/Disable:" Grid.Row="0" Grid.Column="0"/>
                        <TextBox Grid.Row="0" Grid.Column="1" 
                                 Text="{Binding ShortcutToggle}" 
                                 IsReadOnly="True"/>
                        
                        <Label Content="Show Settings:" Grid.Row="1" Grid.Column="0"/>
                        <TextBox Grid.Row="1" Grid.Column="1" 
                                 Text="{Binding ShortcutSettings}" 
                                 IsReadOnly="True"/>
                    </Grid>
                </StackPanel>
            </TabItem>
            
            <!-- Tab 5: About -->
            <TabItem Header="About">
                <StackPanel Margin="16" VerticalAlignment="Center">
                    <TextBlock Text="PalmSync v0.1.0" FontSize="24" FontWeight="Bold"/>
                    <TextBlock Text="Workspace Synchronization System" Margin="0,10"/>
                    <TextBlock Text="© 2026 PalmSync Contributors" Margin="0,20,0,0"/>
                    <Hyperlink Margin="0,10">GitHub</Hyperlink>
                </StackPanel>
            </TabItem>
        </TabControl>
        
        <StackPanel Orientation="Horizontal" 
                    VerticalAlignment="Bottom" 
                    HorizontalAlignment="Right"
                    Margin="16">
            <Button Content="Apply" Command="{Binding ApplyCommand}" Padding="15,8" Margin="0,0,10,0"/>
            <Button Content="OK" Command="{Binding OkCommand}" Padding="15,8" Margin="0,0,10,0"/>
            <Button Content="Cancel" Command="{Binding CancelCommand}" Padding="15,8"/>
        </StackPanel>
    </Grid>
</Window>
```

**`ui/ViewModels/SettingsViewModel.cs`:**

```csharp
[AddINotifyPropertyChangedInterface]
public class SettingsViewModel {
    
    // General
    public bool AutoStartOnLogin { get; set; }
    public bool EnableNotifications { get; set; }
    public string SelectedLanguage { get; set; } = "English";
    public List<string> Languages { get; } = new() { "English", "Türkçe" };
    
    // Network
    public string MasterAddress { get; set; }
    public int MasterPort { get; set; }
    
    // Gestures
    public double GestureSensitivity { get; set; } = 0.3;
    public int DebounceMs { get; set; } = 200;
    public string SelectedCamera { get; set; }
    public List<string> AvailableCameras { get; set; } = new();
    
    // Shortcuts
    public string ShortcutToggle { get; set; } = "Ctrl+Alt+P";
    public string ShortcutSettings { get; set; } = "Ctrl+Alt+S";
    
    public ICommand ApplyCommand { get; }
    public ICommand OkCommand { get; }
    public ICommand CancelCommand { get; }
    public ICommand TestConnectionCommand { get; }
    
    public SettingsViewModel() {
        ApplyCommand = new AsyncRelayCommand(Apply);
        OkCommand = new AsyncRelayCommand(Ok);
        CancelCommand = new RelayCommand(Cancel);
        TestConnectionCommand = new AsyncRelayCommand(TestConnection);
        
        LoadSettings();
        EnumerateCameras();
    }
    
    private void LoadSettings() {
        // Registry veya config file'dan settings'i yükle
        var config = ConfigManager.LoadConfig();
        MasterAddress = config.MasterAddress;
        MasterPort = config.MasterPort;
        AutoStartOnLogin = config.AutoStartOnLogin;
        EnableNotifications = config.EnableNotifications;
        GestureSensitivity = config.GestureSensitivity;
        DebounceMs = config.DebounceMs;
    }
    
    private async Task Apply() {
        await SaveSettings();
        MessageBox.Show("Settings saved successfully");
    }
    
    private async Task Ok() {
        await SaveSettings();
        // Close window
    }
    
    private void Cancel() {
        // Close window
    }
    
    private async Task SaveSettings() {
        var config = new Config {
            MasterAddress = MasterAddress,
            MasterPort = MasterPort,
            AutoStartOnLogin = AutoStartOnLogin,
            EnableNotifications = EnableNotifications,
            GestureSensitivity = GestureSensitivity,
            DebounceMs = DebounceMs,
            SelectedLanguage = SelectedLanguage
        };
        
        ConfigManager.SaveConfig(config);
        
        // Daemon'a notification gönder
        await NotifyDaemonOfConfigChange();
    }
    
    private async Task TestConnection() {
        try {
            var client = new PalmSyncClient(MasterAddress, MasterPort);
            await client.TestConnection();
            MessageBox.Show("Connected successfully!");
        } catch (Exception ex) {
            MessageBox.Show($"Connection failed: {ex.Message}", "Error", MessageBoxButton.OK, MessageBoxImage.Error);
        }
    }
    
    private void EnumerateCameras() {
        // DirectShow veya Windows API ile kameraları listele
        var cameras = CameraEnumerator.GetAvailableCameras();
        foreach (var cam in cameras) {
            AvailableCameras.Add(cam);
        }
    }
}
```

**Yapılacaklar:**
- [ ] Settings window'u design et
- [ ] Tab control'u kur
- [ ] Data binding'lerini implement et
- [ ] Config persistence'ı implement et
- [ ] Validation logic'i ekle

---

### 4. Status Monitor Window

**`ui/Views/StatusWindow.xaml`:**

```xml
<Window x:Class="PalmSync.Tray.Views.StatusWindow"
        Title="PalmSync Status"
        Width="600"
        Height="400">
    
    <Grid Background="{DynamicResource WindowBackground}">
        <StackPanel Margin="16">
            
            <!-- Connection Status -->
            <TextBlock Text="Connection Status" FontSize="16" FontWeight="Bold" Margin="0,0,0,10"/>
            <Border Background="{Binding StatusColor}" CornerRadius="4" Padding="12" Margin="0,0,0,20">
                <StackPanel>
                    <TextBlock Text="{Binding ConnectionStatus}" 
                               Foreground="White" 
                               FontWeight="Bold" 
                               FontSize="14"/>
                    <TextBlock Text="{Binding ConnectionDetails}" 
                               Foreground="White" 
                               Margin="0,5,0,0"/>
                </StackPanel>
            </Border>
            
            <!-- Performance Metrics -->
            <TextBlock Text="Performance" FontSize="16" FontWeight="Bold" Margin="0,0,0,10"/>
            <Grid Margin="0,0,0,20">
                <Grid.ColumnDefinitions>
                    <ColumnDefinition Width="*"/>
                    <ColumnDefinition Width="*"/>
                </Grid.ColumnDefinitions>
                <Grid.RowDefinitions>
                    <RowDefinition Height="40"/>
                    <RowDefinition Height="40"/>
                </Grid.RowDefinitions>
                
                <StackPanel Grid.Row="0" Grid.Column="0" Margin="0,0,10,10">
                    <TextBlock Text="CPU Usage" FontWeight="Bold"/>
                    <ProgressBar Value="{Binding CpuUsage}" Maximum="100" Height="20"/>
                    <TextBlock Text="{Binding CpuUsage, StringFormat='{0:F1}%'}" Margin="0,5,0,0"/>
                </StackPanel>
                
                <StackPanel Grid.Row="0" Grid.Column="1" Margin="10,0,0,10">
                    <TextBlock Text="Memory Usage" FontWeight="Bold"/>
                    <ProgressBar Value="{Binding MemoryUsage}" Maximum="100" Height="20"/>
                    <TextBlock Text="{Binding MemoryUsage, StringFormat='{0:F1}%'}" Margin="0,5,0,0"/>
                </StackPanel>
                
                <StackPanel Grid.Row="1" Grid.Column="0" Margin="0,10,10,0">
                    <TextBlock Text="Network Latency" FontWeight="Bold"/>
                    <TextBlock Text="{Binding NetworkLatency, StringFormat='{0:F0}ms'}" FontSize="18"/>
                </StackPanel>
                
                <StackPanel Grid.Row="1" Grid.Column="1" Margin="10,10,0,0">
                    <TextBlock Text="Last Event" FontWeight="Bold"/>
                    <TextBlock Text="{Binding LastEventTime}" FontSize="14"/>
                </StackPanel>
            </Grid>
            
            <!-- Events Log -->
            <TextBlock Text="Recent Events" FontSize="16" FontWeight="Bold" Margin="0,0,0,10"/>
            <ListBox ItemsSource="{Binding RecentEvents}" Height="150">
                <ListBox.ItemTemplate>
                    <DataTemplate>
                        <TextBlock>
                            <Run Text="{Binding Timestamp, StringFormat='[{0:HH:mm:ss}]'}"/>
                            <Run Text="{Binding EventType}"/>
                            <Run Text="-" Margin="5,0"/>
                            <Run Text="{Binding Description}"/>
                        </TextBlock>
                    </DataTemplate>
                </ListBox.ItemTemplate>
            </ListBox>
            
            <Button Content="Refresh" Command="{Binding RefreshCommand}" Margin="0,15,0,0" Padding="10,8"/>
        </StackPanel>
    </Grid>
</Window>
```

**Yapılacaklar:**
- [ ] Real-time status updates'ı implement et
- [ ] Performance metrics'i al (ProcessAPI)
- [ ] Event log'u göster
- [ ] Auto-refresh timer'ı kur

---

### 5. Toast Notifications

**`ui/Services/NotificationService.cs`:**

```csharp
public class NotificationService {
    
    public static void ShowGrabNotification(string fileName, string filePath) {
        var notification = new ToastNotificationBuilder()
            .AddText("GRAB Event")
            .AddText($"Captured: {fileName}")
            .AddText($"Location: {filePath}")
            .Show();
    }
    
    public static void ShowDropNotification(string fileName, int lineNumber) {
        var notification = new ToastNotificationBuilder()
            .AddText("DROP Event")
            .AddText($"Restored: {fileName}:{lineNumber}")
            .Show();
    }
    
    public static void ShowErrorNotification(string error) {
        var notification = new ToastNotificationBuilder()
            .AddText("Error")
            .AddText(error)
            .SetDuration(ToastDuration.Long)
            .Show();
    }
    
    public static void ShowConnectionStatusNotification(bool connected) {
        var status = connected ? "Connected to Master" : "Disconnected from Master";
        var notification = new ToastNotificationBuilder()
            .AddText("Connection Status")
            .AddText(status)
            .Show();
    }
}
```

**Windows.UI.Notifications kullanarak:**

```csharp
using Windows.UI.Notifications;
using Windows.Data.Xml.Dom;

public class ToastNotificationBuilder {
    private string _title;
    private string _message;
    
    public ToastNotificationBuilder AddText(string text) {
        _title ??= text;
        _message = text;
        return this;
    }
    
    public void Show() {
        string xml = $@"<toast>
            <visual>
                <binding template='ToastText02'>
                    <text id='1'>{_title}</text>
                    <text id='2'>{_message}</text>
                </binding>
            </visual>
        </toast>";
        
        var xmlDoc = new XmlDocument();
        xmlDoc.LoadXml(xml);
        
        var notif = new ToastNotification(xmlDoc);
        ToastNotificationManager.CreateToastNotifier("PalmSync").Show(notif);
    }
}
```

**Yapılacaklar:**
- [ ] Toast notification'ı implement et
- [ ] Event listener'ı daemon'dan dinle
- [ ] Notification timeout'u set et
- [ ] Click action'ları ekle

---

### 6. Event Log Viewer

**`ui/Views/LogViewerWindow.xaml`:**

```xml
<Window x:Class="PalmSync.Tray.Views.LogViewerWindow"
        Title="Event Logs"
        Width="800"
        Height="600">
    
    <Grid>
        <DockPanel>
            <!-- Toolbar -->
            <StackPanel DockPanel.Dock="Top" Orientation="Horizontal" Margin="16" Height="40">
                <ComboBox ItemsSource="{Binding LogLevels}" 
                          SelectedItem="{Binding SelectedLogLevel}"
                          Width="100"
                          Margin="0,0,10,0"/>
                
                <Button Content="Clear" Command="{Binding ClearLogsCommand}" Padding="10,5" Margin="0,0,10,0"/>
                <Button Content="Export" Command="{Binding ExportLogsCommand}" Padding="10,5" Margin="0,0,10,0"/>
                <Button Content="Refresh" Command="{Binding RefreshCommand}" Padding="10,5"/>
            </StackPanel>
            
            <!-- Log List -->
            <DataGrid ItemsSource="{Binding LogEntries}" 
                      DockPanel.Dock="Bottom"
                      IsReadOnly="True"
                      AutoGenerateColumns="False"
                      Margin="16">
                
                <DataGrid.Columns>
                    <DataGridTextColumn Header="Timestamp" Binding="{Binding Timestamp}" Width="150"/>
                    <DataGridTextColumn Header="Level" Binding="{Binding Level}" Width="80"/>
                    <DataGridTextColumn Header="Component" Binding="{Binding Component}" Width="100"/>
                    <DataGridTextColumn Header="Message" Binding="{Binding Message}" Width="*"/>
                </DataGrid.Columns>
            </DataGrid>
        </DockPanel>
    </Grid>
</Window>
```

**Yapılacaklar:**
- [ ] Log file'ı parse et
- [ ] Filter options'ı implement et
- [ ] Export functionality'i ekle
- [ ] Auto-refresh feature'ı kur

---

### 7. Keyboard Shortcut Support

**`ui/Services/HotKeyManager.cs`:**

```csharp
using System.Runtime.InteropServices;

public class HotKeyManager {
    
    [DllImport("user32.dll")]
    private static extern bool RegisterHotKey(IntPtr hWnd, int id, uint fsModifiers, uint vk);
    
    [DllImport("user32.dll")]
    private static extern bool UnregisterHotKey(IntPtr hWnd, int id);
    
    private const uint MOD_CONTROL = 0x0002;
    private const uint MOD_ALT = 0x0001;
    private const uint VK_P = 0x50;
    private const uint VK_S = 0x53;
    
    public static void RegisterShortcuts(IntPtr windowHandle) {
        RegisterHotKey(windowHandle, 1, MOD_CONTROL | MOD_ALT, VK_P);  // Ctrl+Alt+P
        RegisterHotKey(windowHandle, 2, MOD_CONTROL | MOD_ALT, VK_S);  // Ctrl+Alt+S
    }
    
    public static void UnregisterShortcuts(IntPtr windowHandle) {
        UnregisterHotKey(windowHandle, 1);
        UnregisterHotKey(windowHandle, 2);
    }
}
```

**Yapılacaklar:**
- [ ] Global hotkey registration'ı implement et
- [ ] Shortcut collision detection'ı ekle
- [ ] Custom key binding support'u kur

---

### 8. Theme Support (Dark/Light)

**`ui/Resources/Themes/DarkTheme.xaml`:**

```xml
<ResourceDictionary xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation">
    
    <Color x:Key="WindowBackgroundColor">#1e1e1e</Color>
    <Color x:Key="TextForegroundColor">#ffffff</Color>
    <Color x:Key="AccentColor">#007acc</Color>
    
    <SolidColorBrush x:Key="WindowBackground" Color="{StaticResource WindowBackgroundColor}"/>
    <SolidColorBrush x:Key="TextForeground" Color="{StaticResource TextForegroundColor}"/>
    <SolidColorBrush x:Key="Accent" Color="{StaticResource AccentColor}"/>
    
</ResourceDictionary>
```

**`ui/Services/ThemeService.cs`:**

```csharp
public class ThemeService {
    
    public static void SetTheme(Theme theme) {
        var dict = new ResourceDictionary();
        dict.Source = theme == Theme.Dark 
            ? new Uri("Resources/Themes/DarkTheme.xaml", UriKind.Relative)
            : new Uri("Resources/Themes/LightTheme.xaml", UriKind.Relative);
        
        Application.Current.Resources.MergedDictionaries.Add(dict);
    }
}

public enum Theme {
    Light,
    Dark
}
```

**Yapılacaklar:**
- [ ] Dark ve Light theme resource'larını oluştur
- [ ] Theme switching'i implement et
- [ ] System theme preference'ı detect et

---

## 🧪 Testing

**`ui.tests/TrayIconViewModelTests.cs`:**

```csharp
[TestClass]
public class TrayIconViewModelTests {
    
    [TestMethod]
    public void OpenSettings_CommandExecuted_WindowShown() {
        var vm = new TrayIconViewModel();
        // Test settings window
    }
    
    [TestMethod]
    public async Task RestartService_ServiceRestarted_StatusUpdated() {
        var vm = new TrayIconViewModel();
        await vm.RestartService();
        // Assert
    }
}
```

**Yapılacaklar:**
- [ ] ViewModel tests'i yaz
- [ ] Integration testleri ekle
- [ ] UI automation testleri (optional)

---

## 📦 Deliverables

Bu faz tamamlandığında:

1. ✅ **System Tray App**: palmsync-ui.exe
2. ✅ **Settings Panel**: Configuration UI
3. ✅ **Status Monitor**: Real-time monitoring
4. ✅ **Notifications**: Toast alerts
5. ✅ **Log Viewer**: Event log browser
6. ✅ **Keyboard Shortcuts**: Global hotkeys
7. ✅ **Dark/Light Theme**: Theme support

---

## 🔄 Sonraki Faz Bağlantıları

- **Faz 6 (Security)**: Notification encryption ve secure config storage
- **Faz 7 (Testing)**: UI automation testing

---

## 📝 Uyarılar

⚠️ **UI Development Notes:**
- WPF vs WinUI 3 decision: WPF daha mature, WinUI 3 modern
- Accessibility (WCAG) standards'ı follow et
- High DPI awareness'ı handle et
- Localization framework'ü hazırla

---

**Son Güncelleme**: 2026-07-20  
**Statüsü**: 🔵 Hazır Implementation için

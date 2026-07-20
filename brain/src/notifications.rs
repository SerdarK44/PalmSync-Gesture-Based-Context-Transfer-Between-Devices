use std::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub fn show(title: &str, message: &str) {
    if let Err(e) = show_toast(title, message) {
        log::debug!("Toast notification failed: {}", e);
    }
}

pub fn notify_grab(file: &str, line: u32) {
    show("PalmSync — GRAB", &format!("Captured: {}:{}", file, line));
}

pub fn notify_drop(file: &str, line: u32) {
    show("PalmSync — DROP", &format!("Restored: {}:{}", file, line));
}

pub fn notify_connected(addr: &str) {
    show("PalmSync", &format!("Connected to master: {}", addr));
}

// Reserved for error surfacing from the service loop (Faz 7)
#[allow(dead_code)]
pub fn notify_error(msg: &str) {
    show("PalmSync Error", msg);
}

fn show_toast(title: &str, message: &str) -> anyhow::Result<()> {
    let ps_script = format!(
        r#"
$null = [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime]
$null = [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom, ContentType = WindowsRuntime]
$xml = @"
<toast>
  <visual>
    <binding template="ToastText02">
      <text id="1">{}</text>
      <text id="2">{}</text>
    </binding>
  </visual>
</toast>
"@
$xdoc = [Windows.Data.Xml.Dom.XmlDocument]::new()
$xdoc.LoadXml($xml)
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('PalmSync').Show(
    [Windows.UI.Notifications.ToastNotification]::new($xdoc)
)
"#,
        title.replace('\'', "''").replace('"', "&quot;"),
        message.replace('\'', "''").replace('"', "&quot;"),
    );

    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()?;

    Ok(())
}

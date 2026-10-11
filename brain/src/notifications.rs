#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::Command;

/// Show a toast without blocking the caller. The warm shell helper shows it
/// almost immediately; a fresh PowerShell (~0.4 s until visible) is only the
/// fallback when the helper is unavailable.
pub fn show(title: &str, message: &str) {
    let (title, message) = (title.to_string(), message.to_string());
    std::thread::spawn(move || {
        let req = serde_json::json!({ "op": "toast", "title": title, "message": message });
        let shown = crate::shell_helper::request(&req, std::time::Duration::from_millis(1500))
            .is_some_and(|r| r["ok"] == true);
        if !shown {
            if let Err(e) = show_toast(&title, &message) {
                log::debug!("Toast notification failed: {}", e);
            }
        }
    });
}

pub fn notify_grab(what: &str) {
    show("PalmSync — yakalandı ✓", what);
}

/// `outcome` is executor::restore's message ("… VS Code ile açıldı",
/// "Uygulama bulunamadı: … — ekran görüntüsü açıldı").
pub fn notify_drop(outcome: &str) {
    show("PalmSync — geri yüklendi", outcome);
}

pub fn notify_connected(addr: &str) {
    show("PalmSync", &format!("Connected to master: {}", addr));
}

// Reserved for error surfacing from the service loop (Faz 7)
#[allow(dead_code)]
pub fn notify_error(msg: &str) {
    show("PalmSync Error", msg);
}

/// Escape for the toast XML (a file name with `&` or `<` broke the toast) and
/// for the PowerShell @"..."@ here-string, which expands `$` and backticks.
fn xml_escape(s: &str) -> String {
    s.replace('`', "``")
        .replace('$', "`$")
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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
        xml_escape(title),
        xml_escape(message),
    );

    // spawn, not output(): PowerShell takes ~1 s to start and this is called
    // from the gesture loop — waiting for it delayed every GRAB/DROP.
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn()?;

    Ok(())
}

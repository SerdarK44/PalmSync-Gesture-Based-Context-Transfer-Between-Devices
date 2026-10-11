//! One long-lived, hidden PowerShell process for the few things that need
//! .NET/COM: reading a browser's address bar (UIAutomation), an Explorer
//! window's folder, Recent-files shortcuts, and showing toasts.
//!
//! Starting PowerShell and loading those assemblies costs ~250 ms; doing it
//! for every GRAB and every notification made gestures feel slow. The helper
//! is started once (and restarted if it dies or hangs). Requests and replies
//! are single JSON lines; request values are only ever parsed as data by
//! ConvertFrom-Json, never placed into script text.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine as _;
use serde_json::Value;

const SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
[Console]::InputEncoding = [Text.Encoding]::UTF8
[Console]::OutputEncoding = [Text.Encoding]::UTF8
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$null = [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime]
$null = [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom, ContentType = WindowsRuntime]
$shell = New-Object -ComObject WScript.Shell
$recentDir = Join-Path $env:APPDATA 'Microsoft\Windows\Recent'

function Get-Url([int64]$hwnd) {
  $root = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$hwnd)
  $cond = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
    [System.Windows.Automation.ControlType]::Edit)
  $edit = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
  if ($edit) { return [string]$edit.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value }
  return ''
}

while ($true) {
  $line = [Console]::In.ReadLine()
  if ($line -eq $null) { break }
  $res = @{ ok = $true }
  try {
    $req = $line | ConvertFrom-Json
    switch ([string]$req.op) {
      'url' { $res.url = Get-Url $req.hwnd }
      'folder' {
        $res.folder = ''
        foreach ($w in (New-Object -ComObject Shell.Application).Windows()) {
          if ([int64]$w.HWND -eq [int64]$req.hwnd) { $res.folder = [string]$w.Document.Folder.Self.Path }
        }
      }
      'recent' {
        $hits = @()
        foreach ($n in @($req.names)) {
          $n = [IO.Path]::GetFileName([string]$n)
          if (-not $n) { continue }
          Get-ChildItem -LiteralPath $recentDir -Filter ($n + '*.lnk') |
            Sort-Object LastWriteTime -Descending | Select-Object -First 5 |
            ForEach-Object { $t = $shell.CreateShortcut($_.FullName).TargetPath; if ($t) { $hits += [string]$t } }
        }
        $res.recent = @($hits)
      }
      'toast' {
        $e = { param($s) [Security.SecurityElement]::Escape([string]$s) }
        $xml = '<toast><visual><binding template="ToastText02"><text id="1">' + (& $e $req.title) +
               '</text><text id="2">' + (& $e $req.message) + '</text></binding></visual></toast>'
        $doc = [Windows.Data.Xml.Dom.XmlDocument]::new()
        $doc.LoadXml($xml)
        [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('PalmSync').Show(
          [Windows.UI.Notifications.ToastNotification]::new($doc))
      }
      default { }
    }
  } catch { $res.ok = $false; $res.error = [string]$_ }
  [Console]::Out.WriteLine(($res | ConvertTo-Json -Compress -Depth 4))
  [Console]::Out.Flush()
}
"#;

struct Running {
    child: Child,
    stdin: ChildStdin,
    replies: Receiver<String>,
}

static HELPER: Mutex<Option<Running>> = Mutex::new(None);

fn start() -> anyhow::Result<Running> {
    let utf16: Vec<u8> = SCRIPT.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn()?;
    let stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("no stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("no stdout"))?;
    let (tx, replies) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    Ok(Running {
        child,
        stdin,
        replies,
    })
}

/// Send one request and wait for its reply. A helper that does not answer
/// in time is killed (so a late reply can't be mistaken for the next one)
/// and restarted on the next request.
pub fn request(req: &Value, timeout: Duration) -> Option<Value> {
    let mut guard = HELPER.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        match start() {
            Ok(r) => *guard = Some(r),
            Err(e) => {
                log::warn!("Shell helper could not start: {}", e);
                return None;
            }
        }
    }
    let running = guard.as_mut()?;
    let line = format!("{}\n", req);
    let reply = running
        .stdin
        .write_all(line.as_bytes())
        .and_then(|_| running.stdin.flush())
        .ok()
        .and_then(|_| running.replies.recv_timeout(timeout).ok());
    match reply.and_then(|r| serde_json::from_str::<Value>(r.trim_start_matches('\u{feff}')).ok()) {
        Some(v) => Some(v),
        None => {
            log::warn!("Shell helper did not answer '{}' in time — restarting it", req["op"]);
            if let Some(mut r) = guard.take() {
                let _ = r.child.kill();
            }
            None
        }
    }
}

/// Start the helper in the background so the first gesture is fast too
/// (its own start-up includes loading UIAutomation and the toast API).
pub fn warm_up() {
    std::thread::spawn(|| {
        let started = std::time::Instant::now();
        if request(&serde_json::json!({ "op": "ping" }), Duration::from_secs(20)).is_some() {
            log::info!("Shell helper ready in {} ms", started.elapsed().as_millis());
        }
    });
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn helper_answers_and_treats_input_as_data() {
        // A name crafted to break out of a script string must stay data.
        let reply = request(
            &serde_json::json!({ "op": "recent", "names": ["x'; Remove-Item C:\\nope; '"] }),
            Duration::from_secs(30),
        )
        .expect("helper reply");
        assert_eq!(reply["ok"], true);
        // Warm: later requests are fast.
        let started = std::time::Instant::now();
        let reply = request(&serde_json::json!({ "op": "ping" }), Duration::from_secs(5)).unwrap();
        assert_eq!(reply["ok"], true);
        println!("warm ping {} ms", started.elapsed().as_millis());
        let started = std::time::Instant::now();
        request(&serde_json::json!({ "op": "recent", "names": ["README.md"] }), Duration::from_secs(5)).unwrap();
        println!("warm recent-files lookup {} ms", started.elapsed().as_millis());
        assert!(started.elapsed() < Duration::from_millis(1500));
    }
}

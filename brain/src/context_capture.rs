use std::process::Command;

use palmsync_shared::StateSnapshot;

pub fn capture_vscode_context(node_id: &str) -> anyhow::Result<StateSnapshot> {
    let title = get_foreground_window_title()?;

    if !title.contains("Visual Studio Code") {
        return Err(anyhow::anyhow!(
            "Active window is not VS Code: {}",
            title
        ));
    }

    let (file_name, project_path) = parse_vscode_title(&title);
    let clipboard_content = crate::clipboard::get_text().unwrap_or_default();

    Ok(StateSnapshot {
        project_path,
        file_name,
        cursor_line: 1,
        clipboard_content,
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
        source_node_id: node_id.to_string(),
    })
}

fn get_foreground_window_title() -> anyhow::Result<String> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            r#"Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public class WinAPI {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll", SetLastError=true)]
    public static extern int GetWindowText(IntPtr hWnd, StringBuilder lpString, int nMaxCount);
}
"@
$hwnd = [WinAPI]::GetForegroundWindow()
$sb = New-Object System.Text.StringBuilder 512
[void][WinAPI]::GetWindowText($hwnd, $sb, 512)
$sb.ToString()
"#,
        ])
        .output()?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn parse_vscode_title(title: &str) -> (String, String) {
    // VS Code title format: "filename.ext - ProjectFolder - Visual Studio Code"
    let parts: Vec<&str> = title.split(" - ").collect();
    match parts.len() {
        n if n >= 3 => {
            let file_name = parts[0].trim().to_string();
            let project_path = parts[1].trim().to_string();
            (file_name, project_path)
        }
        2 => {
            let file_name = parts[0].trim().to_string();
            (file_name, String::new())
        }
        _ => (title.to_string(), String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_title() {
        let title = "main.rs - palmsync - Visual Studio Code";
        let (file, project) = parse_vscode_title(title);
        assert_eq!(file, "main.rs");
        assert_eq!(project, "palmsync");
    }

    #[test]
    fn parse_title_without_project() {
        let title = "Untitled - Visual Studio Code";
        let (file, _project) = parse_vscode_title(title);
        assert_eq!(file, "Untitled");
    }
}

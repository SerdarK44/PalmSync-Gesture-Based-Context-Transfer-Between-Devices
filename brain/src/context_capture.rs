use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

use palmsync_shared::validation::MAX_FILE_CONTENT_SIZE;
use palmsync_shared::StateSnapshot;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const BROWSERS: &[&str] = &["chrome", "msedge", "brave", "opera", "firefox", "vivaldi"];

pub fn capture_vscode_context(node_id: &str) -> anyhow::Result<StateSnapshot> {
    let fg = get_foreground_info()?;
    let clipboard_content = crate::clipboard::get_text().unwrap_or_default();

    let is_browser = BROWSERS.iter().any(|b| fg.process.eq_ignore_ascii_case(b));
    let is_vscode = fg.title.contains("Visual Studio Code");

    let (file_name, mut project_path) = if is_vscode {
        parse_vscode_title(&fg.title)
    } else {
        (String::new(), String::new())
    };

    // The title only gives "file.rs - folder". Find the real file on disk so
    // its CONTENTS can travel with the snapshot — the other machine usually
    // has no such file, and opening just the name gave an empty editor.
    let mut file_content = String::new();
    let mut file_rel_path = String::new();
    if is_vscode && !file_name.is_empty() && !project_path.is_empty() {
        match locate_file(&project_path, &file_name) {
            Some((folder, file)) => {
                if is_dirty_title(&fg.title) {
                    // Unsaved edits live only in VS Code's buffer: save first
                    // so we send what the user is actually looking at.
                    save_foreground_editor();
                }
                file_content = read_text_file(&file);
                file_rel_path = file
                    .strip_prefix(&folder)
                    .map(|r| r.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                log::info!(
                    "Resolved {} -> {} ({} bytes)",
                    file_name,
                    file.display(),
                    file_content.len()
                );
                project_path = folder.to_string_lossy().to_string();
            }
            None => log::warn!(
                "Could not locate '{}' in a folder named '{}' — sending name only",
                file_name,
                project_path
            ),
        }
    }

    let browser_url = if is_browser { normalize_url(&fg.url) } else { String::new() };

    // Capture only if there is something to restore (an app context or clipboard)
    if !is_vscode && browser_url.is_empty() && clipboard_content.is_empty() {
        return Err(anyhow::anyhow!(
            "Nothing to capture (foreground: {}, no clipboard)",
            fg.process
        ));
    }

    Ok(StateSnapshot {
        project_path,
        file_name,
        cursor_line: 0,  // VS Code title bar doesn't expose cursor position
        clipboard_content,
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
        source_node_id: node_id.to_string(),
        browser_url,
        file_content,
        file_rel_path,
    })
}

// ── Locating the active VS Code file on disk ─────────────────────────────

const SKIP_DIRS: &[&str] = &[
    ".git", "node_modules", "target", "bin", "obj", "dist", "build", "out",
    ".venv", "venv", "__pycache__", ".idea", ".vs", ".next", ".gradle",
];
const MAX_SCAN_ENTRIES: usize = 50_000;
const MAX_SCAN_DEPTH: usize = 10;

fn is_dirty_title(title: &str) -> bool {
    let t = title.trim_start();
    t.starts_with('●') || t.starts_with('•')
}

/// Find `(project_folder, file)` for a VS Code title "file - folder".
fn locate_file(folder_name: &str, file_name: &str) -> Option<(PathBuf, PathBuf)> {
    for folder in candidate_folders(folder_name) {
        if let Some(file) = find_in_folder(&folder, file_name) {
            return Some((folder, file));
        }
    }
    None
}

/// Folders whose name matches `name`, most likely first: VS Code's own list
/// of open / recent folders, then common project locations.
pub(crate) fn candidate_folders(name: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf| {
        let matches = p
            .file_name()
            .map(|n| n.to_string_lossy().eq_ignore_ascii_case(name))
            .unwrap_or(false);
        if matches && p.is_dir() && !out.contains(&p) {
            out.push(p);
        }
    };

    for uri in vscode_folder_uris() {
        if let Some(p) = file_uri_to_path(&uri) {
            push(p);
        }
    }

    if let Ok(home) = std::env::var("USERPROFILE") {
        let home = PathBuf::from(home);
        for sub in [
            "", "Desktop", "Documents", "source\\repos", "Projects", "projects",
            "OneDrive\\Desktop", "OneDrive\\Documents", "OneDrive\\Masaüstü", "OneDrive\\Belgeler",
        ] {
            push(home.join(sub).join(name));
        }
    }
    out
}

/// Folder URIs VS Code (and its forks) remember, most relevant first.
fn vscode_folder_uris() -> Vec<String> {
    let mut uris = Vec::new();
    let Ok(appdata) = std::env::var("APPDATA") else { return uris };

    for product in ["Code", "Code - Insiders", "VSCodium", "Cursor"] {
        let user = Path::new(&appdata).join(product).join("User");

        // 1. globalStorage/storage.json: currently open + last active windows
        if let Ok(text) = std::fs::read_to_string(user.join("globalStorage").join("storage.json")) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(arr) = v.pointer("/backupWorkspaces/folders").and_then(|a| a.as_array()) {
                    for f in arr {
                        if let Some(u) = f.get("folderUri").and_then(|u| u.as_str()) {
                            uris.push(u.to_string());
                        }
                    }
                }
                if let Some(u) = v.pointer("/windowsState/lastActiveWindow/folder").and_then(|u| u.as_str()) {
                    uris.push(u.to_string());
                }
                if let Some(arr) = v.pointer("/windowsState/openedWindows").and_then(|a| a.as_array()) {
                    for w in arr {
                        if let Some(u) = w.get("folder").and_then(|u| u.as_str()) {
                            uris.push(u.to_string());
                        }
                    }
                }
            }
        }

        // 2. workspaceStorage/*/workspace.json, most recently used first
        if let Ok(entries) = std::fs::read_dir(user.join("workspaceStorage")) {
            let mut ws: Vec<(std::time::SystemTime, String)> = entries
                .flatten()
                .filter_map(|e| {
                    let dir = e.path();
                    let text = std::fs::read_to_string(dir.join("workspace.json")).ok()?;
                    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
                    let uri = v.get("folder")?.as_str()?.to_string();
                    let mtime = std::fs::metadata(dir.join("state.vscdb"))
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::UNIX_EPOCH);
                    Some((mtime, uri))
                })
                .collect();
            ws.sort_by(|a, b| b.0.cmp(&a.0));
            uris.extend(ws.into_iter().map(|(_, u)| u));
        }
    }
    uris
}

fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let u = url::Url::parse(uri).ok()?;
    if u.scheme() != "file" {
        return None; // remote (WSL/SSH) workspaces can't be read locally
    }
    u.to_file_path().ok()
}

/// Search `folder` for `file_name`, skipping build/dependency dirs. When
/// several match, the most recently modified wins (it's the one being edited).
fn find_in_folder(folder: &Path, file_name: &str) -> Option<PathBuf> {
    let direct = folder.join(file_name);
    if direct.is_file() {
        return Some(direct);
    }

    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    let mut stack = vec![(folder.to_path_buf(), 0usize)];
    let mut seen = 0usize;

    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            seen += 1;
            if seen > MAX_SCAN_ENTRIES {
                return best.map(|b| b.1);
            }
            let name = e.file_name();
            let name = name.to_string_lossy();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                if depth < MAX_SCAN_DEPTH && !SKIP_DIRS.iter().any(|s| name.eq_ignore_ascii_case(s)) {
                    stack.push((e.path(), depth + 1));
                }
            } else if ft.is_file() && name.eq_ignore_ascii_case(file_name) {
                let mtime = e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
                if best.as_ref().map(|b| mtime > b.0).unwrap_or(true) {
                    best = Some((mtime, e.path()));
                }
            }
        }
    }
    best.map(|b| b.1)
}

/// File contents as text, or empty if too big / not UTF-8 (binary).
fn read_text_file(path: &Path) -> String {
    match std::fs::metadata(path) {
        Ok(m) if m.len() as usize <= MAX_FILE_CONTENT_SIZE => {}
        Ok(m) => {
            log::warn!("{} is {} bytes — too large to send, sending name only", path.display(), m.len());
            return String::new();
        }
        Err(_) => return String::new(),
    }
    match std::fs::read(path) {
        Ok(bytes) => String::from_utf8(bytes).unwrap_or_else(|_| {
            log::warn!("{} is not a text file — sending name only", path.display());
            String::new()
        }),
        Err(e) => {
            log::warn!("Could not read {}: {}", path.display(), e);
            String::new()
        }
    }
}

/// Press Ctrl+S in the foreground window (VS Code) and give it a moment.
fn save_foreground_editor() {
    #[cfg(windows)]
    {
        win32::send_ctrl_s();
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
}

struct ForegroundInfo {
    process: String,
    title: String,
    url: String,
}

#[cfg(windows)]
mod win32 {
    use std::ffi::c_void;

    type HWND = *mut c_void;
    type HANDLE = *mut c_void;
    type DWORD = u32;
    type BOOL = i32;

    const PROCESS_QUERY_LIMITED_INFORMATION: DWORD = 0x1000;

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> HWND;
        fn GetWindowTextW(hwnd: HWND, lp_string: *mut u16, n_max_count: i32) -> i32;
        fn GetWindowThreadProcessId(hwnd: HWND, lpdw_process_id: *mut DWORD) -> DWORD;
        fn keybd_event(b_vk: u8, b_scan: u8, dw_flags: DWORD, dw_extra_info: usize);
    }

    pub fn send_ctrl_s() {
        const VK_CONTROL: u8 = 0x11;
        const VK_S: u8 = 0x53;
        const KEYEVENTF_KEYUP: DWORD = 0x0002;
        unsafe {
            keybd_event(VK_CONTROL, 0, 0, 0);
            keybd_event(VK_S, 0, 0, 0);
            keybd_event(VK_S, 0, KEYEVENTF_KEYUP, 0);
            keybd_event(VK_CONTROL, 0, KEYEVENTF_KEYUP, 0);
        }
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(dw_desired_access: DWORD, b_inherit_handle: BOOL, dw_process_id: DWORD) -> HANDLE;
        fn QueryFullProcessImageNameW(h_process: HANDLE, dw_flags: DWORD, lp_exe_name: *mut u16, lpdw_size: *mut DWORD) -> BOOL;
        fn CloseHandle(h_object: HANDLE) -> BOOL;
    }

    pub fn get_foreground_info_native() -> (usize, String, String) {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return (0, String::new(), String::new());
            }

            let mut title_buf = [0u16; 1024];
            let len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), 1024);
            let title = if len > 0 {
                String::from_utf16_lossy(&title_buf[..len as usize])
            } else {
                String::new()
            };

            let mut pid: DWORD = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);

            let mut proc_name = String::new();
            if pid != 0 {
                let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if !handle.is_null() {
                    let mut path_buf = [0u16; 1024];
                    let mut size: DWORD = 1024;
                    if QueryFullProcessImageNameW(handle, 0, path_buf.as_mut_ptr(), &mut size) != 0 {
                        let full_path = String::from_utf16_lossy(&path_buf[..size as usize]);
                        if let Some(stem) = std::path::Path::new(&full_path).file_stem() {
                            proc_name = stem.to_string_lossy().to_string();
                        }
                    }
                    CloseHandle(handle);
                }
            }

            (hwnd as usize, proc_name, title)
        }
    }
}

#[cfg(windows)]
fn get_browser_url(hwnd: usize) -> String {
    if hwnd == 0 {
        return String::new();
    }
    let script = format!(
        r#"
$ErrorActionPreference = 'SilentlyContinue'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$h = [IntPtr]{}
$root = [System.Windows.Automation.AutomationElement]::FromHandle($h)
$cond = New-Object System.Windows.Automation.PropertyCondition(
  [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
  [System.Windows.Automation.ControlType]::Edit)
$edit = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
if ($edit) {{
  $vp = $edit.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
  Write-Output $vp.Current.Value
}}
"#,
        hwnd
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Err(_) => String::new(),
    }
}

#[cfg(not(windows))]
fn get_browser_url(_hwnd: usize) -> String {
    String::new()
}

/// Instant native Win32 call gets process name and window title in < 0.1 ms.
/// Only queries UIAutomation via PowerShell if the active window is actually a browser.
fn get_foreground_info() -> anyhow::Result<ForegroundInfo> {
    #[cfg(windows)]
    {
        let (hwnd, process, title) = win32::get_foreground_info_native();
        let is_browser = BROWSERS.iter().any(|b| process.eq_ignore_ascii_case(b));
        let url = if is_browser && hwnd != 0 {
            get_browser_url(hwnd)
        } else {
            String::new()
        };
        Ok(ForegroundInfo { process, title, url })
    }
    #[cfg(not(windows))]
    {
        Ok(ForegroundInfo {
            process: String::new(),
            title: String::new(),
            url: String::new(),
        })
    }
}

/// Browsers' address bars often omit the scheme ("github.com/x"). Add https://
/// so the URL opens correctly. Reject anything that isn't a plausible web URL.
fn normalize_url(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    let candidate = if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.to_string()
    } else if raw.contains('.') && !raw.contains(' ') {
        format!("https://{}", raw)
    } else {
        return String::new(); // a search term, not a URL
    };

    // Basic sanity: parseable, http(s), reasonable length
    match url::Url::parse(&candidate) {
        Ok(u) if (u.scheme() == "http" || u.scheme() == "https") && candidate.len() <= 2048 => {
            candidate
        }
        _ => String::new(),
    }
}

fn clean_file_name(raw: &str) -> String {
    let trimmed = raw.trim();
    trimmed
        .trim_start_matches(|c: char| c == '●' || c == '*' || c == '•')
        .trim()
        .to_string()
}

fn parse_vscode_title(title: &str) -> (String, String) {
    // VS Code title format: "filename.ext - ProjectFolder - Visual Studio Code"
    // Unsaved files format: "● filename.ext - ProjectFolder - Visual Studio Code"
    let parts: Vec<&str> = title.split(" - ").collect();
    match parts.len() {
        n if n >= 3 => {
            let file_name = clean_file_name(parts[0]);
            let project_path = parts[1].trim().to_string();
            (file_name, project_path)
        }
        2 => {
            let file_name = clean_file_name(parts[0]);
            (file_name, String::new())
        }
        _ => (clean_file_name(title), String::new()),
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
    fn parse_full_title_with_dirty_marker() {
        let title1 = "● main.rs - palmsync - Visual Studio Code";
        let (file1, project1) = parse_vscode_title(title1);
        assert_eq!(file1, "main.rs");
        assert_eq!(project1, "palmsync");

        let title2 = "* lib.rs - palmsync - Visual Studio Code";
        let (file2, project2) = parse_vscode_title(title2);
        assert_eq!(file2, "lib.rs");
        assert_eq!(project2, "palmsync");
    }

    #[test]
    fn parse_title_without_project() {
        let title = "Untitled - Visual Studio Code";
        let (file, _project) = parse_vscode_title(title);
        assert_eq!(file, "Untitled");
    }

    #[test]
    fn finds_nested_file_and_skips_build_dirs() {
        let root = std::env::temp_dir().join(format!("palmsync-find-{}", std::process::id()));
        let proj = root.join("myproj");
        std::fs::create_dir_all(proj.join("src").join("ui")).unwrap();
        std::fs::create_dir_all(proj.join("node_modules").join("x")).unwrap();
        std::fs::write(proj.join("node_modules").join("x").join("app.js"), "junk").unwrap();
        std::fs::write(proj.join("src").join("ui").join("app.js"), "real").unwrap();

        let found = find_in_folder(&proj, "app.js").unwrap();
        assert_eq!(found, proj.join("src").join("ui").join("app.js"));
        assert_eq!(read_text_file(&found), "real");
        assert!(find_in_folder(&proj, "missing.rs").is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn dirty_marker_detected() {
        assert!(is_dirty_title("● main.rs - palmsync - Visual Studio Code"));
        assert!(!is_dirty_title("main.rs - palmsync - Visual Studio Code"));
    }

    #[test]
    fn url_normalization() {
        assert_eq!(normalize_url("https://github.com/x"), "https://github.com/x");
        assert_eq!(normalize_url("github.com/palmsync"), "https://github.com/palmsync");
        assert_eq!(normalize_url("how to sync tabs"), ""); // search term
        assert_eq!(normalize_url(""), "");
    }
}

use std::path::{Path, PathBuf};

use base64::Engine as _;
use palmsync_shared::validation::{MAX_FILE_CONTENT_SIZE, MAX_FILE_DATA_SIZE};
use palmsync_shared::{AppContext, StateSnapshot};

use crate::apps::{self, Kind};

/// Screenshots are a preview, not an archive: keep them small on the wire.
const SCREENSHOT_MAX_W: u32 = 1600;
const SCREENSHOT_MAX_H: u32 = 1000;

/// Windows whose pixels must never leave the machine, even encrypted.
const NO_SCREENSHOT_PROCESSES: &[&str] = &[
    "keepass", "keepassxc", "1password", "bitwarden", "lastpass", "dashlane", "enpass",
    "nordpass", "roboform", "credentialuibroker", "consent", "logonui", "lockapp",
];
const SHELL_SURFACES: &[&str] = &[
    "lockapp", "logonui", "searchhost", "searchapp", "shellexperiencehost",
    "startmenuexperiencehost", "textinputhost", "shellhost",
];
const PRIVATE_WINDOW_MARKERS: &[&str] = &[
    "InPrivate",
    "Incognito",
    "Gizli",
    "Private Browsing",
    "Özel Gezinti",
    "Private Window",
];

/// Capture what the user is looking at: the foreground app, its document
/// (contents included when it can be found on disk), the browser page or
/// Explorer folder, the clipboard, and a screenshot of the window.
pub fn capture_context(node_id: &str) -> anyhow::Result<StateSnapshot> {
    let fg = get_foreground_info()?;
    let clipboard_content = crate::clipboard::get_text().unwrap_or_default();
    let process = fg.process.to_ascii_lowercase();

    // The desktop, lock screen, Start menu, search... are not apps to move.
    let is_desktop = (process == "explorer" && fg.class != "CabinetWClass")
        || SHELL_SURFACES.contains(&process.as_str());
    let profile = apps::profile_for(&process);
    let kind = if is_desktop || process.is_empty() {
        None
    } else if process == "explorer" {
        Some(Kind::Files)
    } else {
        Some(profile.map(|p| p.kind).unwrap_or(Kind::Other))
    };

    let mut snap = StateSnapshot {
        clipboard_content,
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
        source_node_id: node_id.to_string(),
        ..Default::default()
    };

    if let Some(kind) = kind {
        snap.app = AppContext {
            process: process.clone(),
            exe_path: apps::to_portable(&fg.exe_path),
            display_name: apps::display_name(&process, &fg.title),
            window_title: printable(&fg.title, 512),
            kind: kind.as_str().into(),
            ..Default::default()
        };

        match kind {
            Kind::Browser => snap.browser_url = normalize_url(&fg.url),
            Kind::Files => {
                if let Some(folder) = explorer_folder(fg.hwnd) {
                    snap.app.folder_path = apps::to_portable(&folder.to_string_lossy());
                }
            }
            _ if is_vscode_like(&process, &fg.title) => capture_vscode_file(&fg, &mut snap),
            _ => capture_document(&fg, kind, &mut snap),
        }

        if !NO_SCREENSHOT_PROCESSES.contains(&process.as_str())
            && !PRIVATE_WINDOW_MARKERS.iter().any(|m| fg.title.contains(m))
        {
            if let Some(jpg) =
                crate::win::capture_window_jpeg(fg.hwnd, SCREENSHOT_MAX_W, SCREENSHOT_MAX_H)
            {
                snap.screenshot_jpeg_b64 = base64::engine::general_purpose::STANDARD.encode(jpg);
            }
        }
    }

    if snap.app.process.is_empty() && snap.clipboard_content.is_empty() {
        return Err(anyhow::anyhow!(
            "Nothing to capture (foreground: {}, no clipboard)",
            if fg.process.is_empty() { "desktop" } else { &fg.process }
        ));
    }
    Ok(snap)
}

/// One-line description for logs and notifications.
pub fn describe(snap: &StateSnapshot) -> String {
    let app = if snap.app.display_name.is_empty() {
        "Pano".to_string()
    } else {
        snap.app.display_name.clone()
    };
    let what = if !snap.file_name.is_empty() {
        snap.file_name.clone()
    } else if !snap.browser_url.is_empty() {
        snap.browser_url.clone()
    } else if !snap.app.folder_path.is_empty() {
        snap.app.folder_path.clone()
    } else {
        String::new()
    };
    if what.is_empty() {
        app
    } else {
        format!("{} — {}", app, what)
    }
}

fn printable(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect()
}

fn is_vscode_like(process: &str, title: &str) -> bool {
    matches!(process, "code" | "cursor" | "vscodium") || title.contains("Visual Studio Code")
}

/// VS Code and its forks: the title only says "file - folder", so find the
/// real file on disk and send its contents (saving unsaved edits first).
fn capture_vscode_file(fg: &ForegroundInfo, snap: &mut StateSnapshot) {
    let (file_name, project) = parse_vscode_title(&fg.title);
    snap.file_name = file_name;
    snap.project_path = project;
    if snap.file_name.is_empty() || snap.project_path.is_empty() {
        return;
    }
    match locate_file(&snap.project_path, &snap.file_name) {
        Some((folder, file)) => {
            if is_dirty_title(&fg.title) {
                // Unsaved edits live only in the editor's buffer: save first
                // so we send what the user is actually looking at.
                save_editor(fg.hwnd);
            }
            snap.file_content = read_text_file(&file);
            snap.file_rel_path = file
                .strip_prefix(&folder)
                .map(|r| r.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            snap.app.document_path = apps::to_portable(&file.to_string_lossy());
            log::info!(
                "Resolved {} -> {} ({} bytes)",
                snap.file_name,
                file.display(),
                snap.file_content.len()
            );
            snap.project_path = folder.to_string_lossy().to_string();
        }
        None => log::warn!(
            "Could not locate '{}' in a folder named '{}' — sending name only",
            snap.file_name,
            snap.project_path
        ),
    }
}

/// Any other app: find the document its window shows and send it along.
fn capture_document(fg: &ForegroundInfo, kind: Kind, snap: &mut StateSnapshot) {
    let Some(path) = resolve_document(fg) else {
        log::info!("No document found for {} ('{}')", fg.process, fg.title);
        return;
    };
    if kind == Kind::Editor && is_dirty_title(&fg.title) {
        save_editor(fg.hwnd);
    }
    let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
        return;
    };
    match read_document(&path) {
        Some(Document::Text(text)) => snap.file_content = text,
        Some(Document::Binary(bytes)) => {
            snap.file_data_b64 = base64::engine::general_purpose::STANDARD.encode(bytes)
        }
        None => {} // too large / unreadable: name and path only
    }
    log::info!(
        "Resolved document {} ({} text / {} b64 bytes)",
        path.display(),
        snap.file_content.len(),
        snap.file_data_b64.len()
    );
    snap.file_name = name;
    snap.app.document_path = apps::to_portable(&path.to_string_lossy());
}

enum Document {
    Text(String),
    Binary(Vec<u8>),
}

fn read_document(path: &Path) -> Option<Document> {
    let len = std::fs::metadata(path).ok()?.len() as usize;
    if len > MAX_FILE_DATA_SIZE {
        log::warn!("{} is {} bytes — too large to send", path.display(), len);
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() <= MAX_FILE_CONTENT_SIZE && !bytes.contains(&0) {
        if let Ok(text) = String::from_utf8(bytes.clone()) {
            return Some(Document::Text(text));
        }
    }
    Some(Document::Binary(bytes))
}

/// Which file does this window show? 1) a full path in the title,
/// 2) a file argument of the process's command line, 3) Windows' list of
/// recently opened files — each must match a file name from the title.
fn resolve_document(fg: &ForegroundInfo) -> Option<PathBuf> {
    let candidates = apps::title_candidates(&fg.title);
    let worth_asking = candidates
        .iter()
        .any(|c| Path::new(c).extension().is_some() || Path::new(c).is_absolute())
        || fg.title.contains(" - ");
    if candidates.is_empty() || !worth_asking {
        return None;
    }
    for c in &candidates {
        let p = Path::new(c);
        if p.is_absolute() && p.is_file() {
            return Some(p.to_path_buf());
        }
    }

    let matches = |p: &Path| p.is_absolute() && p.is_file() && candidates.iter().any(|c| apps::matches_candidate(p, c));

    // The window's own process first, then any process whose command line
    // names the file (launcher stubs such as Windows 11's System32\notepad.exe
    // keep the real path while the window's process only has a session blob).
    let names: Vec<String> = candidates
        .iter()
        .filter_map(|c| Path::new(c).file_name().map(|n| n.to_string_lossy().to_lowercase()))
        .collect();
    let cmdlines = crate::win::process_command_lines(fg.pid, &|cmd| {
        let cmd = cmd.to_lowercase();
        names.iter().any(|n| cmd.contains(n.as_str()))
    });
    for cmdline in &cmdlines {
        for arg in apps::command_line_args(cmdline) {
            let mut paths = vec![arg.clone()];
            paths.extend(apps::paths_in_encoded_arg(&arg));
            if let Some(p) = paths.into_iter().map(PathBuf::from).find(|p| matches(p)) {
                return Some(p);
            }
        }
    }

    // Opened through Explorer or a file dialog: Windows' Recent shortcuts.
    let reply = crate::shell_helper::request(
        &serde_json::json!({ "op": "recent", "names": candidates }),
        HELPER_TIMEOUT,
    )?;
    reply["recent"]
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str())
        .map(PathBuf::from)
        .find(|p| matches(p))
}

/// Lookups through the shell helper normally answer in well under 200 ms;
/// a GRAB must never hang on a stuck one.
const HELPER_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(2500);

fn explorer_folder(hwnd: usize) -> Option<PathBuf> {
    let reply = crate::shell_helper::request(
        &serde_json::json!({ "op": "folder", "hwnd": hwnd }),
        HELPER_TIMEOUT,
    )?;
    let p = PathBuf::from(reply["folder"].as_str()?);
    (p.is_absolute() && p.is_dir()).then_some(p)
}

// ── Locating the active VS Code file on disk ─────────────────────────────

const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "bin",
    "obj",
    "dist",
    "build",
    "out",
    ".venv",
    "venv",
    "__pycache__",
    ".idea",
    ".vs",
    ".next",
    ".gradle",
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
            "",
            "Desktop",
            "Documents",
            "source\\repos",
            "Projects",
            "projects",
            "OneDrive\\Desktop",
            "OneDrive\\Documents",
            "OneDrive\\Masaüstü",
            "OneDrive\\Belgeler",
        ] {
            push(home.join(sub).join(name));
        }
    }
    out
}

/// Folder URIs VS Code (and its forks) remember, most relevant first.
fn vscode_folder_uris() -> Vec<String> {
    let mut uris = Vec::new();
    let Ok(appdata) = std::env::var("APPDATA") else {
        return uris;
    };

    for product in ["Code", "Code - Insiders", "VSCodium", "Cursor"] {
        let user = Path::new(&appdata).join(product).join("User");

        // 1. globalStorage/storage.json: currently open + last active windows
        if let Ok(text) = std::fs::read_to_string(user.join("globalStorage").join("storage.json")) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(arr) = v
                    .pointer("/backupWorkspaces/folders")
                    .and_then(|a| a.as_array())
                {
                    for f in arr {
                        if let Some(u) = f.get("folderUri").and_then(|u| u.as_str()) {
                            uris.push(u.to_string());
                        }
                    }
                }
                if let Some(u) = v
                    .pointer("/windowsState/lastActiveWindow/folder")
                    .and_then(|u| u.as_str())
                {
                    uris.push(u.to_string());
                }
                if let Some(arr) = v
                    .pointer("/windowsState/openedWindows")
                    .and_then(|a| a.as_array())
                {
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
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            seen += 1;
            if seen > MAX_SCAN_ENTRIES {
                return best.map(|b| b.1);
            }
            let name = e.file_name();
            let name = name.to_string_lossy();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                if depth < MAX_SCAN_DEPTH && !SKIP_DIRS.iter().any(|s| name.eq_ignore_ascii_case(s))
                {
                    stack.push((e.path(), depth + 1));
                }
            } else if ft.is_file() && name.eq_ignore_ascii_case(file_name) {
                let mtime = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
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
            log::warn!(
                "{} is {} bytes — too large to send, sending name only",
                path.display(),
                m.len()
            );
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

/// Press Ctrl+S in the captured editor — only if it really is the
/// foreground window (the gesture is often made while PalmSync's own
/// preview is in front; Ctrl+S must not go there).
fn save_editor(hwnd: usize) {
    #[cfg(windows)]
    {
        if crate::win::foreground_window() != hwnd {
            log::info!("Editor has unsaved changes but is not in front — sending the saved version");
            return;
        }
        win32::send_ctrl_s();
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
    #[cfg(not(windows))]
    let _ = hwnd;
}

#[derive(Default)]
struct ForegroundInfo {
    hwnd: usize,
    pid: u32,
    /// Full path of the executable.
    exe_path: String,
    /// Executable name without extension.
    process: String,
    title: String,
    class: String,
    url: String,
}

#[cfg(windows)]
mod win32 {
    use std::ffi::c_void;

    type Hwnd = *mut c_void;
    type Handle = *mut c_void;
    type Dword = u32;
    type Bool = i32;

    const PROCESS_QUERY_LIMITED_INFORMATION: Dword = 0x1000;

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> Hwnd;
        fn GetWindow(hwnd: Hwnd, command: u32) -> Hwnd;
        fn IsWindowVisible(hwnd: Hwnd) -> Bool;
        fn GetWindowTextW(hwnd: Hwnd, lp_string: *mut u16, n_max_count: i32) -> i32;
        fn GetWindowThreadProcessId(hwnd: Hwnd, lpdw_process_id: *mut Dword) -> Dword;
        fn keybd_event(b_vk: u8, b_scan: u8, dw_flags: Dword, dw_extra_info: usize);
    }

    pub fn send_ctrl_s() {
        const VK_CONTROL: u8 = 0x11;
        const VK_S: u8 = 0x53;
        const KEYEVENTF_KEYUP: Dword = 0x0002;
        unsafe {
            keybd_event(VK_CONTROL, 0, 0, 0);
            keybd_event(VK_S, 0, 0, 0);
            keybd_event(VK_S, 0, KEYEVENTF_KEYUP, 0);
            keybd_event(VK_CONTROL, 0, KEYEVENTF_KEYUP, 0);
        }
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(
            dw_desired_access: Dword,
            b_inherit_handle: Bool,
            dw_process_id: Dword,
        ) -> Handle;
        fn QueryFullProcessImageNameW(
            h_process: Handle,
            dw_flags: Dword,
            lp_exe_name: *mut u16,
            lpdw_size: *mut Dword,
        ) -> Bool;
        fn CloseHandle(h_object: Handle) -> Bool;
    }

    /// (pid, exe full path, exe stem, title)
    unsafe fn window_info(hwnd: Hwnd) -> (u32, String, String, String) {
        let mut title_buf = [0u16; 1024];
        let len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), 1024);
        let title = if len > 0 {
            String::from_utf16_lossy(&title_buf[..len as usize])
        } else {
            String::new()
        };

        let mut pid: Dword = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);

        let mut full_path = String::new();
        let mut proc_name = String::new();
        if pid != 0 {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if !handle.is_null() {
                let mut path_buf = [0u16; 1024];
                let mut size: Dword = 1024;
                if QueryFullProcessImageNameW(handle, 0, path_buf.as_mut_ptr(), &mut size) != 0 {
                    full_path = String::from_utf16_lossy(&path_buf[..size as usize]);
                    if let Some(stem) = std::path::Path::new(&full_path).file_stem() {
                        proc_name = stem.to_string_lossy().to_string();
                    }
                }
                CloseHandle(handle);
            }
        }

        (pid, full_path, proc_name, title)
    }

    pub fn get_capture_window_info() -> super::ForegroundInfo {
        const GW_HWNDNEXT: u32 = 2;
        let make = |hwnd: Hwnd, (pid, exe_path, process, title): (u32, String, String, String)| {
            super::ForegroundInfo {
                hwnd: hwnd as usize,
                pid,
                exe_path,
                process,
                title,
                class: crate::win::window_class(hwnd as usize),
                url: String::new(),
            }
        };

        unsafe {
            let mut hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return super::ForegroundInfo::default();
            }

            let first = window_info(hwnd);
            if !first.2.eq_ignore_ascii_case("palmsync-ui") {
                return make(hwnd, first);
            }

            // A camera gesture is commonly made while PalmSync's preview is
            // open. In that case the foreground window is PalmSync itself,
            // not the app the user wants to transfer. Walk the desktop
            // Z-order and use the first visible, titled application behind it.
            let original = hwnd;
            for _ in 0..64 {
                hwnd = GetWindow(hwnd, GW_HWNDNEXT);
                if hwnd.is_null() {
                    break;
                }
                if IsWindowVisible(hwnd) == 0 {
                    continue;
                }
                let candidate = window_info(hwnd);
                if candidate.3.trim().is_empty() || candidate.2.eq_ignore_ascii_case("palmsync-ui") {
                    continue;
                }
                return make(hwnd, candidate);
            }

            make(original, first)
        }
    }
}

/// The browser's address bar via UIAutomation (in the warm shell helper:
/// ~70 ms instead of ~330 ms with a fresh PowerShell).
#[cfg(windows)]
fn get_browser_url(hwnd: usize) -> String {
    if hwnd == 0 {
        return String::new();
    }
    crate::shell_helper::request(&serde_json::json!({ "op": "url", "hwnd": hwnd }), HELPER_TIMEOUT)
        .and_then(|r| r["url"].as_str().map(|s| s.trim().to_string()))
        .unwrap_or_default()
}

/// Instant native Win32 calls give the window, process and title; the
/// browser address bar is only read (UIAutomation) when the window actually
/// is a browser.
fn get_foreground_info() -> anyhow::Result<ForegroundInfo> {
    #[cfg(windows)]
    {
        let mut fg = win32::get_capture_window_info();
        let is_browser = apps::profile_for(&fg.process)
            .map(|p| p.kind == Kind::Browser)
            .unwrap_or(false);
        if is_browser && fg.hwnd != 0 {
            fg.url = get_browser_url(fg.hwnd);
        }
        Ok(fg)
    }
    #[cfg(not(windows))]
    {
        Ok(ForegroundInfo::default())
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
        .trim_start_matches(['●', '*', '•'])
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
        assert_eq!(
            normalize_url("https://github.com/x"),
            "https://github.com/x"
        );
        assert_eq!(
            normalize_url("github.com/palmsync"),
            "https://github.com/palmsync"
        );
        assert_eq!(normalize_url("how to sync tabs"), ""); // search term
        assert_eq!(normalize_url(""), "");
    }
}

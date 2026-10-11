//! Restoring a snapshot on this machine. See `apps.rs` for the decisions;
//! this module carries them out: write received files, start programs,
//! open pages, and fall back to the screenshot when nothing else fits.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use palmsync_shared::StateSnapshot;

use crate::apps::{self, AppPlan, FilePlan, Installed, UrlPlan};

/// Where files received from the other machine are written when they don't
/// already exist here: `Documents\PalmSync\Gelen Dosyalar\<project>\<rel path>`.
fn received_root() -> PathBuf {
    PathBuf::from(palmsync_shared::Config::load().data_dir).join("Gelen Dosyalar")
}

/// Strip characters Windows doesn't allow in a file/folder name.
fn sanitize_component(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if "<>:\"|?*/\\".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').to_string();
    if cleaned.is_empty() || cleaned == ".." {
        "_".into()
    } else {
        cleaned
    }
}

/// The file's path relative to its project (falls back to the bare name).
fn rel_path(snap: &StateSnapshot) -> PathBuf {
    let rel = if snap.file_rel_path.is_empty() {
        &snap.file_name
    } else {
        &snap.file_rel_path
    };
    rel.split(['/', '\\'])
        .filter(|p| !p.is_empty() && *p != "." && *p != "..")
        .map(sanitize_component)
        .collect()
}

fn project_name(snap: &StateSnapshot) -> Option<String> {
    let name = Path::new(&snap.project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    (!name.is_empty()).then(|| sanitize_component(&name))
}

/// The bytes that travelled with the snapshot (text or binary).
fn received_bytes(snap: &StateSnapshot) -> anyhow::Result<Option<Vec<u8>>> {
    if !snap.file_content.is_empty() {
        return Ok(Some(snap.file_content.as_bytes().to_vec()));
    }
    if !snap.file_data_b64.is_empty() {
        return Ok(Some(
            base64::engine::general_purpose::STANDARD.decode(&snap.file_data_b64)?,
        ));
    }
    Ok(None)
}

/// Find the same file on THIS machine: the original path (same machine or
/// same layout, e.g. `%USERPROFILE%\Documents\x.docx`), or a folder with the
/// same project name.
fn find_local_copy(snap: &StateSnapshot) -> Option<PathBuf> {
    if !snap.app.document_path.is_empty() {
        if let Some(p) = apps::from_portable(&snap.app.document_path) {
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let rel = rel_path(snap);
    let project = Path::new(&snap.project_path);
    if project.is_absolute() {
        let p = project.join(&rel);
        if p.is_file() {
            return Some(p);
        }
    }
    let name = project
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| snap.project_path.clone());
    if name.is_empty() {
        return None;
    }
    crate::context_capture::candidate_folders(&name)
        .into_iter()
        .map(|f| f.join(&rel))
        .find(|p| p.is_file())
}

/// Tag a received file as coming from another computer (Mark of the Web):
/// Office opens it in Protected View with macros off, SmartScreen checks
/// it. Best effort — FAT/exFAT drives have no alternate data streams.
fn mark_from_other_computer(path: &Path) {
    let ads = format!("{}:Zone.Identifier", path.display());
    if let Err(e) = std::fs::write(&ads, "[ZoneTransfer]\r\nZoneId=3\r\n") {
        log::debug!("Could not mark {} as received: {}", path.display(), e);
    }
}

/// Decide which file to open for a DROP, writing the received contents to
/// disk when needed. Never returns a path that doesn't exist — opening a
/// missing path is what produced the "empty file" bug.
pub fn prepare_file(snap: &StateSnapshot) -> anyhow::Result<PathBuf> {
    let bytes = received_bytes(snap)?;
    if let Some(local) = find_local_copy(snap) {
        match &bytes {
            None => return Ok(local),
            Some(b) if std::fs::read(&local).is_ok_and(|l| &l == b) => return Ok(local),
            Some(_) => log::info!(
                "{} differs from the received version — saving received copy separately",
                local.display()
            ),
        }
    }

    let Some(bytes) = bytes else {
        anyhow::bail!(
            "'{}' does not exist on this computer and its contents were not sent \
             (file too large, or not found on the source computer)",
            snap.file_name
        );
    };

    let mut target = received_root();
    if let Some(project) = project_name(snap) {
        target.push(project);
    }
    target.push(rel_path(snap));
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&target, &bytes)?;
    mark_from_other_computer(&target);
    log::info!("Wrote received file to {}", target.display());
    Ok(target)
}

// ── What is installed here ─────────────────────────────────────────────────

/// Locate VS Code's command-line launcher. `code` is often not on PATH for
/// the daemon (or at all), so check the standard install locations too.
fn find_code_cli() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let p = dir.join("code.cmd");
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let mut candidates = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join("Programs\\Microsoft VS Code\\bin\\code.cmd"));
    }
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Ok(pf) = std::env::var(var) {
            candidates.push(PathBuf::from(pf).join("Microsoft VS Code\\bin\\code.cmd"));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// `App Paths` registrations (how Windows itself finds `winword.exe`,
/// `chrome.exe`, `notepad++.exe`... without them being on PATH).
#[cfg(windows)]
fn app_paths(exe_name: &str) -> Vec<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let key = format!(
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{}",
        exe_name
    );
    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
        .into_iter()
        .filter_map(|hive| RegKey::predef(hive).open_subkey(&key).ok())
        .filter_map(|k| k.get_value::<String, _>("").ok())
        .filter_map(|v| apps::from_portable(v.trim().trim_matches('"')))
        .collect()
}

#[cfg(not(windows))]
fn app_paths(_exe_name: &str) -> Vec<PathBuf> {
    Vec::new()
}

/// Find `process` installed on THIS machine. The other computer's path is
/// only a hint: whatever is found must be a local program in a normal
/// location whose name matches, and shells/installers are never returned.
fn find_installed(process: &str, portable_hint: &str) -> Option<PathBuf> {
    if !apps::may_launch(process) {
        return None;
    }
    let windir = std::env::var("WINDIR").ok();
    let exe_name = format!("{}.exe", process);
    let usable = |p: &Path| {
        p.is_file()
            && p.file_stem()
                .is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case(process))
            && apps::location_allowed(p, windir.as_deref())
    };

    let mut candidates: Vec<PathBuf> = Vec::new();
    if !portable_hint.is_empty() {
        candidates.extend(apps::from_portable(portable_hint));
    }
    candidates.extend(app_paths(&exe_name));
    if let Some(profile) = apps::profile_for(process) {
        candidates.extend(profile.locations.iter().filter_map(|l| apps::from_portable(l)));
    }
    if let Ok(path) = std::env::var("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|d| d.join(&exe_name)));
    }
    candidates.into_iter().find(|p| usable(p))
}

/// `%SystemRoot%\x.exe` -> `C:\Windows\x.exe` (local registry values).
fn expand_env(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(v) if !name.is_empty() => out.push_str(&v),
                    _ => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The program a registry `shell\open\command` string runs.
fn command_program(command: &str) -> Option<PathBuf> {
    let command = command.trim();
    let raw = match command.strip_prefix('"') {
        Some(rest) => rest.split('"').next()?,
        None => command.split_whitespace().next()?,
    };
    let path = PathBuf::from(expand_env(raw));
    (!path.as_os_str().is_empty()).then_some(path)
}

/// Programs registered for `.ext` besides its default ("Open with" list),
/// the user's own choices first. Each must resolve to a program PalmSync
/// may start (no shells, script hosts, rundll32, installers, network paths).
#[cfg(windows)]
fn registered_handlers(ext: &str) -> Vec<(String, String)> {
    use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER};
    use winreg::RegKey;
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let dot = format!(".{}", ext);
    let user_exts = format!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\{}", dot);

    let mut classes: Vec<String> = Vec::new();
    let mut push = |c: String| {
        if !c.is_empty() && !classes.iter().any(|x| x.eq_ignore_ascii_case(&c)) {
            classes.push(c);
        }
    };
    if let Ok(k) = hkcu.open_subkey(format!(r"{}\OpenWithProgids", user_exts)) {
        k.enum_values().flatten().for_each(|(name, _)| push(name));
    }
    if let Ok(k) = hkcr.open_subkey(format!(r"{}\OpenWithProgids", dot)) {
        k.enum_values().flatten().for_each(|(name, _)| push(name));
    }
    if let Ok(k) = hkcu.open_subkey(format!(r"{}\OpenWithList", user_exts)) {
        let order: String = k.get_value("MRUList").unwrap_or_default();
        for slot in order.chars() {
            if let Ok(exe) = k.get_value::<String, _>(slot.to_string()) {
                push(format!(r"Applications\{}", exe));
            }
        }
    }
    if let Ok(k) = hkcr.open_subkey(format!(r"{}\OpenWithList", dot)) {
        k.enum_keys().flatten().for_each(|exe| push(format!(r"Applications\{}", exe)));
    }

    let windir = std::env::var("WINDIR").ok();
    let mut out = Vec::new();
    for class in classes {
        let usable = match hkcr.open_subkey(format!(r"{}\shell\open\command", class)) {
            Ok(k) => match k.get_value::<String, _>("") {
                Ok(cmd) => command_program(&cmd).is_some_and(|exe| {
                    let stem = exe
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    apps::may_launch(&stem) && apps::location_allowed(&exe, windir.as_deref())
                }),
                // Store apps open through a COM handler instead of a command.
                Err(_) => class.starts_with("AppX") && k.get_raw_value("DelegateExecute").is_ok(),
            },
            Err(_) => false,
        };
        if !usable {
            continue;
        }
        let name = crate::win::class_display_name(&class).unwrap_or_else(|| {
            class
                .trim_start_matches(r"Applications\")
                .trim_end_matches(".exe")
                .to_string()
        });
        out.push((name, class));
        if out.len() >= 10 {
            break;
        }
    }
    out
}

#[cfg(not(windows))]
fn registered_handlers(_ext: &str) -> Vec<(String, String)> {
    Vec::new()
}

struct SystemApps;

impl Installed for SystemApps {
    fn find(&self, process: &str, portable_hint: &str) -> Option<PathBuf> {
        find_installed(process, portable_hint)
    }
    fn code_cli(&self) -> Option<PathBuf> {
        find_code_cli()
    }
    fn association(&self, ext_or_scheme: &str) -> Option<String> {
        crate::win::association(ext_or_scheme)
    }
    fn handlers(&self, ext: &str) -> Vec<(String, String)> {
        registered_handlers(ext)
    }
}

// ── Doing it ───────────────────────────────────────────────────────────────

/// Start a program with arguments directly (CreateProcess, no shell).
fn launch(exe: &Path, args: &[&OsStr]) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args);
    // The started app must not inherit the daemon's stdout/stderr: when the
    // tray app reads those through a pipe, the pipe would stay open for as
    // long as the editor runs.
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // code.cmd is a batch file; keep its console hidden.
        if exe.extension().is_some_and(|e| e.eq_ignore_ascii_case("cmd")) {
            cmd.creation_flags(0x08000000);
        }
    }
    log::info!("Executing: {} {:?}", exe.display(), args);
    cmd.spawn()?;
    Ok(())
}

/// Save the screenshot under `Gelen Dosyalar\Ekran Görüntüleri` and open it.
fn show_screenshot(snap: &StateSnapshot, app_name: &str) -> anyhow::Result<PathBuf> {
    if snap.screenshot_jpeg_b64.is_empty() {
        anyhow::bail!("no screenshot was sent");
    }
    let jpg = base64::engine::general_purpose::STANDARD.decode(&snap.screenshot_jpeg_b64)?;
    if jpg.len() < 4 || jpg[..2] != [0xFF, 0xD8] {
        anyhow::bail!("screenshot is not a JPEG");
    }
    let dir = received_root().join("Ekran Görüntüleri");
    std::fs::create_dir_all(&dir)?;
    let stamp = chrono::Local::now().format("%Y-%m-%d %H.%M.%S");
    let path = dir.join(format!("{} {}.jpg", stamp, sanitize_component(app_name)));
    std::fs::write(&path, jpg)?;
    crate::win::shell_open(&path.to_string_lossy())?;
    log::info!("Opened screenshot {}", path.display());
    Ok(path)
}

fn source_name(snap: &StateSnapshot) -> String {
    if snap.app.display_name.is_empty() {
        apps::display_name(&snap.app.process, &snap.app.window_title)
    } else {
        snap.app.display_name.clone()
    }
}

/// Reopen what the snapshot captured. Returns a short Turkish message for
/// the user (also logged); never panics, always does the best it can.
pub fn restore(snap: &StateSnapshot) -> String {
    let sys = SystemApps;
    let message = if !snap.browser_url.is_empty() {
        restore_url(snap, &sys)
    } else if !snap.file_name.is_empty() {
        restore_file(snap, &sys)
    } else if !snap.app.folder_path.is_empty() {
        restore_folder(snap)
    } else if !snap.app.process.is_empty() {
        restore_app(snap, &sys)
    } else if !snap.clipboard_content.is_empty() {
        "Pano aktarıldı".to_string()
    } else {
        "Geri yüklenecek içerik yoktu".to_string()
    };
    log::info!("Restore: {}", message);
    message
}

fn restore_url(snap: &StateSnapshot, sys: &dyn Installed) -> String {
    let url = &snap.browser_url;
    log::info!("Opening browser tab: {}", url);
    let src = source_name(snap);
    match apps::plan_url(&snap.app, sys) {
        UrlPlan::Browser { exe, name } => match launch(&exe, &[OsStr::new(url)]) {
            Ok(()) => return format!("Sayfa açıldı ({})", name),
            Err(e) => log::warn!("{} could not be started: {}", name, e),
        },
        UrlPlan::Default { name, same_app } => {
            if let Err(e) = crate::win::shell_open(url) {
                log::warn!("Browser open failed: {}", e);
                return format!("Sayfa açılamadı: {}", e);
            }
            let browser = name.unwrap_or_else(|| "varsayılan tarayıcı".into());
            return if same_app {
                format!("Sayfa açıldı ({})", browser)
            } else {
                format!("{} bu bilgisayarda yok — sayfa {} ile açıldı", src, browser)
            };
        }
    }
    match crate::win::shell_open(url) {
        Ok(()) => "Sayfa varsayılan tarayıcıda açıldı".into(),
        Err(e) => format!("Sayfa açılamadı: {}", e),
    }
}

fn restore_file(snap: &StateSnapshot, sys: &dyn Installed) -> String {
    let src = source_name(snap);
    let path = match prepare_file(snap) {
        Ok(p) => p,
        Err(e) => {
            log::warn!("File restore failed: {}", e);
            return match show_screenshot(snap, &src) {
                Ok(_) => format!("{} aktarılamadı ({}) — ekran görüntüsü açıldı", snap.file_name, e),
                Err(_) => format!("Dosya açılamadı: {}", e),
            };
        }
    };
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let is_text = !snap.file_content.is_empty();
    let file = path.as_os_str();

    let fallback = |reason: String| -> String {
        let _ = crate::win::reveal_in_explorer(&path);
        match show_screenshot(snap, &src) {
            Ok(_) => format!("{} — dosya kaydedildi, ekran görüntüsü açıldı", reason),
            Err(_) => format!("{} — dosya kaydedildi: {}", reason, path.display()),
        }
    };

    match apps::plan_file(&snap.app, &ext, is_text, sys) {
        FilePlan::VsCode { cli, same_app } => {
            let goto = format!("{}:{}", path.display(), snap.cursor_line);
            let result = if snap.cursor_line > 0 {
                launch(&cli, &[OsStr::new("--goto"), OsStr::new(&goto)])
            } else {
                launch(&cli, &[file])
            };
            match result {
                Ok(()) if same_app || src.is_empty() => format!("{} açıldı (VS Code)", snap.file_name),
                Ok(()) => format!("{} bu bilgisayarda yok — {} VS Code ile açıldı", src, snap.file_name),
                Err(e) => fallback(format!("VS Code başlatılamadı ({})", e)),
            }
        }
        FilePlan::App { exe, name, same_app } => match launch(&exe, &[file]) {
            Ok(()) if same_app => format!("{} açıldı ({})", snap.file_name, name),
            Ok(()) if src.is_empty() => format!("{} {} ile açıldı", snap.file_name, name),
            Ok(()) => format!("{} bu bilgisayarda yok — {} {} ile açıldı", src, snap.file_name, name),
            Err(e) => fallback(format!("{} başlatılamadı ({})", name, e)),
        },
        FilePlan::Default { name } => match crate::win::shell_open(&path.to_string_lossy()) {
            Ok(()) => format!(
                "{} bu bilgisayarda yok — {} varsayılan programla açıldı ({})",
                src, snap.file_name, name
            ),
            Err(e) => fallback(format!("Varsayılan program açılamadı ({})", e)),
        },
        FilePlan::Handler { name, class } => {
            match crate::win::shell_open_with_class(&path.to_string_lossy(), &class) {
                Ok(()) => format!(
                    "{} bu bilgisayarda yok — {} bu dosya türünü açabilen {} ile açıldı",
                    src, snap.file_name, name
                ),
                Err(e) => fallback(format!("{} açılamadı ({})", name, e)),
            }
        }
        FilePlan::Reveal => {
            let reason = if apps::is_dangerous_extension(&ext) {
                format!("Güvenlik nedeniyle {} açılmadı", snap.file_name)
            } else if src.is_empty() {
                format!("{} için uygulama bulunamadı", snap.file_name)
            } else {
                format!("Uygulama bulunamadı: {}", src)
            };
            fallback(reason)
        }
    }
}

fn restore_folder(snap: &StateSnapshot) -> String {
    let folder = apps::from_portable(&snap.app.folder_path).filter(|p| p.is_dir());
    if let Some(folder) = folder {
        match crate::win::shell_open(&folder.to_string_lossy()) {
            Ok(()) => return format!("Klasör açıldı: {}", folder.display()),
            Err(e) => log::warn!("Folder open failed: {}", e),
        }
    }
    match show_screenshot(snap, "Dosya Gezgini") {
        Ok(_) => "Klasör bu bilgisayarda yok — ekran görüntüsü açıldı".into(),
        Err(_) => format!("Klasör bu bilgisayarda yok: {}", snap.app.folder_path),
    }
}

fn restore_app(snap: &StateSnapshot, sys: &dyn Installed) -> String {
    match apps::plan_app(&snap.app, sys) {
        AppPlan::Launch { exe, name } => match launch(&exe, &[]) {
            Ok(()) => format!("{} açıldı (içerik bir belge değildi, uygulama başlatıldı)", name),
            Err(e) => match show_screenshot(snap, &name) {
                Ok(_) => format!("{} başlatılamadı ({}) — ekran görüntüsü açıldı", name, e),
                Err(_) => format!("{} başlatılamadı: {}", name, e),
            },
        },
        AppPlan::Screenshot { name } => match show_screenshot(snap, &name) {
            Ok(_) => format!("Uygulama bulunamadı: {} — ekran görüntüsü açıldı", name),
            Err(_) => format!("Uygulama bulunamadı: {}", name),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(project: &str, rel: &str, content: &str) -> StateSnapshot {
        StateSnapshot {
            project_path: project.into(),
            file_name: Path::new(rel).file_name().unwrap().to_string_lossy().into(),
            file_rel_path: rel.into(),
            file_content: content.into(),
            source_node_id: "n".into(),
            ..Default::default()
        }
    }

    #[test]
    fn rel_path_cannot_escape() {
        let s = snap("C:\\x\\proj", "../../evil/a.txt", "x");
        let r = rel_path(&s);
        assert!(r
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_))));
        assert_eq!(r, PathBuf::from("evil").join("a.txt"));
    }

    #[test]
    fn existing_identical_file_is_opened_in_place() {
        let dir = std::env::temp_dir().join(format!("palmsync-exec-{}", std::process::id()));
        let src = dir.join("proj").join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("main.rs"), "fn main() {}").unwrap();

        let s = snap(
            &dir.join("proj").to_string_lossy(),
            "src/main.rs",
            "fn main() {}",
        );
        assert_eq!(prepare_file(&s).unwrap(), src.join("main.rs"));

        // Name only, no content, file exists -> still opened in place
        let s = snap(&dir.join("proj").to_string_lossy(), "src/main.rs", "");
        assert_eq!(prepare_file(&s).unwrap(), src.join("main.rs"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn same_document_path_is_found_by_portable_path() {
        let dir = std::env::temp_dir().join(format!("palmsync-doc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Rapor.docx");
        std::fs::write(&file, [0x50u8, 0x4B, 3, 4, 0, 1]).unwrap();

        let mut s = StateSnapshot {
            file_name: "Rapor.docx".into(),
            file_data_b64: base64::engine::general_purpose::STANDARD.encode([0x50u8, 0x4B, 3, 4, 0, 1]),
            source_node_id: "n".into(),
            ..Default::default()
        };
        s.app.document_path = file.to_string_lossy().to_string();
        assert_eq!(prepare_file(&s).unwrap(), file);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_without_content_is_an_error_not_an_empty_file() {
        let s = snap("C:\\definitely\\not\\here\\proj-xyz", "a.rs", "");
        assert!(prepare_file(&s).is_err());
    }

    /// Manual check of this machine's registrations:
    /// `cargo test -p palmsync-brain show_handlers -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn show_handlers() {
        for ext in ["txt", "md", "pdf", "png", "psd", "docx", "mp4", "py", "html"] {
            println!(
                ".{:<5} default={:?} others={:?}",
                ext,
                crate::win::association(ext),
                registered_handlers(ext)
            );
        }
    }

    /// Manual: opens a temp .txt with an explicit handler class.
    /// `cargo test -p palmsync-brain open_with_class -- --ignored`
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn open_with_class() {
        let file = std::env::temp_dir().join("palmsync-open-with.txt");
        std::fs::write(&file, "PalmSync open-with test").unwrap();
        let class = registered_handlers("txt").first().map(|h| h.1.clone()).expect("a .txt handler");
        crate::win::shell_open_with_class(&file.to_string_lossy(), &class).unwrap();
        println!("opened with {}", class);
    }

    #[test]
    fn command_program_is_parsed() {
        assert_eq!(
            command_program(r#""C:\Program Files\GIMP 2\bin\gimp-2.10.exe" "%1""#).unwrap(),
            PathBuf::from(r"C:\Program Files\GIMP 2\bin\gimp-2.10.exe")
        );
        assert_eq!(
            command_program(r"C:\Windows\System32\rundll32.exe x.dll,Open %1").unwrap(),
            PathBuf::from(r"C:\Windows\System32\rundll32.exe")
        );
        assert!(command_program("   ").is_none());
    }

    #[test]
    fn invalid_screenshot_is_not_written() {
        let s = StateSnapshot {
            screenshot_jpeg_b64: base64::engine::general_purpose::STANDARD.encode(b"<html>"),
            ..Default::default()
        };
        assert!(show_screenshot(&s, "x").is_err());
    }
}

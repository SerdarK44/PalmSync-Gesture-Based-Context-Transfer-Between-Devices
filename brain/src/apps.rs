//! Which application a snapshot came from, and how its content is reopened on
//! this machine: the same app if it is installed, an equivalent one (any
//! browser for a page, any editor for a text file), the file's default
//! program, or — when nothing fits — the screenshot taken at capture time.
//!
//! Everything that comes from the network is only used to LOOK UP programs
//! and files on this machine. A path from the other computer is never
//! executed as-is, interpreters/shells are never launched, and file types
//! that run code are only ever opened in a text editor.

use std::path::{Path, PathBuf};

use palmsync_shared::AppContext;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Browser,
    Editor,
    Document,
    Files,
    Other,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Browser => "browser",
            Kind::Editor => "editor",
            Kind::Document => "document",
            Kind::Files => "files",
            Kind::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        Some(match s {
            "browser" => Kind::Browser,
            "editor" => Kind::Editor,
            "document" => Kind::Document,
            "files" => Kind::Files,
            "other" => Kind::Other,
            _ => return None,
        })
    }
}

pub struct Profile {
    pub display: &'static str,
    pub kind: Kind,
    /// Lower-case executable names without `.exe`.
    pub processes: &'static [&'static str],
    /// Usual install locations, portable (`%VAR%\...`).
    pub locations: &'static [&'static str],
}

/// Known applications. Editors are listed in fallback order: a text file
/// whose own editor is missing opens in the first one installed here, and
/// Notepad (always present) is the last resort.
pub const PROFILES: &[Profile] = &[
    // ── Browsers ──
    Profile {
        display: "Google Chrome",
        kind: Kind::Browser,
        processes: &["chrome"],
        locations: &[
            r"%ProgramFiles%\Google\Chrome\Application\chrome.exe",
            r"%ProgramFiles(x86)%\Google\Chrome\Application\chrome.exe",
            r"%LOCALAPPDATA%\Google\Chrome\Application\chrome.exe",
        ],
    },
    Profile {
        display: "Microsoft Edge",
        kind: Kind::Browser,
        processes: &["msedge"],
        locations: &[
            r"%ProgramFiles(x86)%\Microsoft\Edge\Application\msedge.exe",
            r"%ProgramFiles%\Microsoft\Edge\Application\msedge.exe",
        ],
    },
    Profile {
        display: "Brave",
        kind: Kind::Browser,
        processes: &["brave"],
        locations: &[
            r"%ProgramFiles%\BraveSoftware\Brave-Browser\Application\brave.exe",
            r"%LOCALAPPDATA%\BraveSoftware\Brave-Browser\Application\brave.exe",
        ],
    },
    Profile {
        display: "Firefox",
        kind: Kind::Browser,
        processes: &["firefox"],
        locations: &[
            r"%ProgramFiles%\Mozilla Firefox\firefox.exe",
            r"%ProgramFiles(x86)%\Mozilla Firefox\firefox.exe",
        ],
    },
    Profile {
        display: "Opera",
        kind: Kind::Browser,
        processes: &["opera"],
        locations: &[
            r"%LOCALAPPDATA%\Programs\Opera\opera.exe",
            r"%LOCALAPPDATA%\Programs\Opera GX\opera.exe",
        ],
    },
    Profile {
        display: "Vivaldi",
        kind: Kind::Browser,
        processes: &["vivaldi"],
        locations: &[
            r"%LOCALAPPDATA%\Vivaldi\Application\vivaldi.exe",
            r"%ProgramFiles%\Vivaldi\Application\vivaldi.exe",
        ],
    },
    // ── Editors (fallback order) ──
    Profile {
        display: "VS Code",
        kind: Kind::Editor,
        processes: &["code"],
        locations: &[
            r"%LOCALAPPDATA%\Programs\Microsoft VS Code\Code.exe",
            r"%ProgramFiles%\Microsoft VS Code\Code.exe",
        ],
    },
    Profile {
        display: "Cursor",
        kind: Kind::Editor,
        processes: &["cursor"],
        locations: &[r"%LOCALAPPDATA%\Programs\cursor\Cursor.exe"],
    },
    Profile {
        display: "VSCodium",
        kind: Kind::Editor,
        processes: &["vscodium"],
        locations: &[
            r"%LOCALAPPDATA%\Programs\VSCodium\VSCodium.exe",
            r"%ProgramFiles%\VSCodium\VSCodium.exe",
        ],
    },
    Profile {
        display: "Sublime Text",
        kind: Kind::Editor,
        processes: &["sublime_text"],
        locations: &[
            r"%ProgramFiles%\Sublime Text\sublime_text.exe",
            r"%ProgramFiles%\Sublime Text 3\sublime_text.exe",
        ],
    },
    Profile {
        display: "Notepad++",
        kind: Kind::Editor,
        processes: &["notepad++"],
        locations: &[
            r"%ProgramFiles%\Notepad++\notepad++.exe",
            r"%ProgramFiles(x86)%\Notepad++\notepad++.exe",
        ],
    },
    Profile {
        display: "Not Defteri",
        kind: Kind::Editor,
        processes: &["notepad"],
        locations: &[r"%WINDIR%\System32\notepad.exe"],
    },
    // ── Documents ──
    Profile {
        display: "Microsoft Word",
        kind: Kind::Document,
        processes: &["winword"],
        locations: &[
            r"%ProgramFiles%\Microsoft Office\root\Office16\WINWORD.EXE",
            r"%ProgramFiles(x86)%\Microsoft Office\root\Office16\WINWORD.EXE",
        ],
    },
    Profile {
        display: "Microsoft Excel",
        kind: Kind::Document,
        processes: &["excel"],
        locations: &[
            r"%ProgramFiles%\Microsoft Office\root\Office16\EXCEL.EXE",
            r"%ProgramFiles(x86)%\Microsoft Office\root\Office16\EXCEL.EXE",
        ],
    },
    Profile {
        display: "Microsoft PowerPoint",
        kind: Kind::Document,
        processes: &["powerpnt"],
        locations: &[
            r"%ProgramFiles%\Microsoft Office\root\Office16\POWERPNT.EXE",
            r"%ProgramFiles(x86)%\Microsoft Office\root\Office16\POWERPNT.EXE",
        ],
    },
    Profile {
        display: "LibreOffice",
        kind: Kind::Document,
        processes: &["soffice", "swriter", "scalc", "simpress"],
        locations: &[r"%ProgramFiles%\LibreOffice\program\soffice.exe"],
    },
    Profile {
        display: "Adobe Acrobat",
        kind: Kind::Document,
        processes: &["acrobat", "acrord32"],
        locations: &[
            r"%ProgramFiles%\Adobe\Acrobat DC\Acrobat\Acrobat.exe",
            r"%ProgramFiles(x86)%\Adobe\Acrobat Reader DC\Reader\AcroRd32.exe",
        ],
    },
    Profile {
        display: "SumatraPDF",
        kind: Kind::Document,
        processes: &["sumatrapdf"],
        locations: &[
            r"%LOCALAPPDATA%\SumatraPDF\SumatraPDF.exe",
            r"%ProgramFiles%\SumatraPDF\SumatraPDF.exe",
        ],
    },
    Profile {
        display: "Paint",
        kind: Kind::Document,
        processes: &["mspaint"],
        locations: &[r"%WINDIR%\System32\mspaint.exe"],
    },
    Profile {
        display: "VLC",
        kind: Kind::Document,
        processes: &["vlc"],
        locations: &[r"%ProgramFiles%\VideoLAN\VLC\vlc.exe"],
    },
    // ── Files ──
    Profile {
        display: "Dosya Gezgini",
        kind: Kind::Files,
        processes: &["explorer"],
        locations: &[],
    },
];

pub fn profile_for(process: &str) -> Option<&'static Profile> {
    let p = process.to_ascii_lowercase();
    PROFILES.iter().find(|pr| pr.processes.contains(&p.as_str()))
}

pub fn editors() -> impl Iterator<Item = &'static Profile> {
    PROFILES.iter().filter(|p| p.kind == Kind::Editor)
}

/// Programs that must never be started on behalf of the other computer:
/// shells and script hosts run their file argument as code, installers and
/// system tools have no place in a "reopen my work" action.
const NEVER_LAUNCH: &[&str] = &[
    "cmd", "powershell", "pwsh", "powershell_ise", "wscript", "cscript", "mshta",
    "rundll32", "regsvr32", "msiexec", "python", "pythonw", "py", "pyw", "node", "java",
    "javaw", "bash", "sh", "wsl", "wslhost", "conhost", "openconsole", "windowsterminal",
    "wt", "git-bash", "mintty", "msbuild", "installutil", "regasm", "regsvcs", "certutil",
    "bitsadmin", "schtasks", "sc", "reg", "regedit", "taskkill", "shutdown", "format",
    "diskpart", "bcdedit", "vssadmin", "wmic", "cmstp", "control", "mmc", "dllhost",
    "svchost", "explorer", "applicationframehost", "palmsync-ui", "palmsync-daemon",
    "palmsync-eye",
];

/// Windows' own folder only holds system tools; just these may be started.
const WINDOWS_DIR_ALLOWED: &[&str] = &["notepad", "mspaint", "write", "charmap", "snippingtool"];

pub fn may_launch(process: &str) -> bool {
    let p = process.to_ascii_lowercase();
    !p.is_empty()
        && !NEVER_LAUNCH.contains(&p.as_str())
        && !["setup", "install", "uninst", "update"]
            .iter()
            .any(|w| p.contains(w))
}

/// File types that execute code or change the system when opened with
/// their default program. They are saved, but only ever opened in a text
/// editor (where they are just text).
const DANGEROUS_EXTENSIONS: &[&str] = &[
    "exe", "com", "scr", "pif", "bat", "cmd", "btm", "ps1", "ps1xml", "ps2", "psc1", "psm1",
    "psd1", "vb", "vbs", "vbe", "js", "jse", "mjs", "ws", "wsf", "wsc", "wsh", "hta", "msi",
    "msp", "mst", "msc", "lnk", "url", "scf", "reg", "inf", "cpl", "jar", "py", "pyw", "pyc",
    "pyz", "pl", "rb", "php", "sh", "application", "appref-ms", "gadget", "sct", "chm", "hlp",
    "iso", "img", "vhd", "vhdx", "appx", "appxbundle", "msix", "msixbundle", "diagcab",
    "settingcontent-ms", "library-ms", "search-ms", "searchconnector-ms", "xll", "xlam",
    "ppam", "ade", "adp", "mda", "mdb", "mde", "accde", "website", "xbap", "ocx", "dll",
    "sys", "drv", "rdp", "theme", "themepack", "desktopthemepack", "ins", "isp", "job",
];

pub fn is_dangerous_extension(ext: &str) -> bool {
    DANGEROUS_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
}

// ── Portable paths ─────────────────────────────────────────────────────────

/// Most specific first: LOCALAPPDATA/APPDATA live under USERPROFILE, and
/// "ProgramFiles(x86)" starts with "ProgramFiles".
const PORTABLE_VARS: &[&str] = &[
    "LOCALAPPDATA",
    "APPDATA",
    "ProgramFiles(x86)",
    "ProgramFiles",
    "ProgramData",
    "USERPROFILE",
    "WINDIR",
];

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// `C:\Users\alice\AppData\Local\x` -> `%LOCALAPPDATA%\x`, so the other
/// machine (another user name, maybe another drive) can resolve it.
pub fn to_portable(path: &str) -> String {
    to_portable_with(path, &env_var)
}

pub fn to_portable_with(path: &str, env: &dyn Fn(&str) -> Option<String>) -> String {
    for var in PORTABLE_VARS {
        if let Some(value) = env(var) {
            let value = value.trim_end_matches('\\');
            if path.len() > value.len()
                && path[..value.len()].eq_ignore_ascii_case(value)
                && path.as_bytes()[value.len()] == b'\\'
            {
                return format!("%{}%{}", var, &path[value.len()..]);
            }
        }
    }
    path.to_string()
}

/// Reverse of [`to_portable`] with THIS machine's folders. Only the known
/// variables are expanded; anything else is returned as-is.
pub fn from_portable(path: &str) -> Option<PathBuf> {
    from_portable_with(path, &env_var)
}

pub fn from_portable_with(path: &str, env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(rest) = path.strip_prefix('%') {
        let end = rest.find('%')?;
        let var = &rest[..end];
        if !PORTABLE_VARS.iter().any(|v| v.eq_ignore_ascii_case(var)) {
            return None;
        }
        let base = env(var)?;
        return Some(PathBuf::from(format!(
            "{}{}",
            base.trim_end_matches('\\'),
            &rest[end + 1..]
        )));
    }
    Some(PathBuf::from(path))
}

/// A program file may be started only from a normal local location: not a
/// network share, and inside the Windows folder only a few known tools.
pub fn location_allowed(exe: &Path, windir: Option<&str>) -> bool {
    let s = exe.to_string_lossy();
    // Store apps (Windows 11 Notepad, Photos...) cannot be started by their
    // WindowsApps path; their classic stub (System32\notepad.exe) is used.
    if s.starts_with(r"\\") || s.contains("..") || s.to_ascii_lowercase().contains(r"\windowsapps\") {
        return false;
    }
    if let Some(win) = windir {
        let win = win.trim_end_matches('\\');
        if s.len() > win.len() && s[..win.len()].eq_ignore_ascii_case(win) {
            let stem = exe
                .file_stem()
                .map(|n| n.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            return WINDOWS_DIR_ALLOWED.contains(&stem.as_str());
        }
    }
    true
}

// ── Display names ──────────────────────────────────────────────────────────

/// Name for messages: the known profile, else the app name most windows
/// put after the last " - " in their title, else the executable name.
pub fn display_name(process: &str, title: &str) -> String {
    if let Some(p) = profile_for(process) {
        return p.display.to_string();
    }
    if let Some(tail) = title.rsplit(" - ").next().filter(|_| title.contains(" - ")) {
        let tail = tail.trim();
        if !tail.is_empty() && tail.chars().count() <= 40 {
            return tail.to_string();
        }
    }
    process.to_string()
}

// ── Decisions (pure, so they can be unit-tested) ───────────────────────────

/// What THIS machine has installed. The real implementation lives in the
/// executor; tests use a fake.
pub trait Installed {
    /// Executable for `process`, looked up locally (the portable path from
    /// the other machine is only a hint). None if missing or not allowed.
    fn find(&self, process: &str, portable_hint: &str) -> Option<PathBuf>;
    /// VS Code's `code.cmd` launcher (handles `--goto file:line`).
    fn code_cli(&self) -> Option<PathBuf>;
    /// Friendly name of the default program for an extension (`docx`) or a
    /// URL scheme (`https`), if one is registered.
    fn association(&self, ext_or_scheme: &str) -> Option<String>;
    /// Other programs registered for the extension ("Open with" list),
    /// best first, as (display name, shell class to open the file with).
    /// Shells, script hosts and installers are already filtered out.
    fn handlers(&self, ext: &str) -> Vec<(String, String)>;
}

#[derive(Debug, PartialEq)]
pub enum FilePlan {
    /// VS Code through its CLI.
    VsCode { cli: PathBuf, same_app: bool },
    App { exe: PathBuf, name: String, same_app: bool },
    /// The file's default program on this machine.
    Default { name: String },
    /// Another program registered for the file type (no default is set).
    Handler { name: String, class: String },
    /// Nothing suitable: show the file in Explorer (and the screenshot).
    Reveal,
}

/// First editor installed here (Notepad is always there on Windows).
fn editor_plan(sys: &dyn Installed) -> Option<FilePlan> {
    for editor in editors() {
        let process = editor.processes[0];
        if process == "code" {
            if let Some(cli) = sys.code_cli() {
                return Some(FilePlan::VsCode { cli, same_app: false });
            }
        } else if let Some(exe) = sys.find(process, "") {
            return Some(FilePlan::App {
                exe,
                name: editor.display.to_string(),
                same_app: false,
            });
        }
    }
    None
}

/// How to open a received file.
pub fn plan_file(src: &AppContext, ext: &str, is_text: bool, sys: &dyn Installed) -> FilePlan {
    let dangerous = is_dangerous_extension(ext);
    let src_profile = profile_for(&src.process);
    // A newer sender may know editors this catalog doesn't.
    let src_is_editor = src_profile.map(|p| p.kind == Kind::Editor).unwrap_or(false)
        || Kind::parse(&src.kind) == Some(Kind::Editor);

    // 1. The same application.
    if may_launch(&src.process) {
        if src.process.eq_ignore_ascii_case("code") {
            if let Some(cli) = sys.code_cli() {
                return FilePlan::VsCode { cli, same_app: true };
            }
        } else if !dangerous || src_is_editor {
            if let Some(exe) = sys.find(&src.process, &src.exe_path) {
                return FilePlan::App {
                    exe,
                    name: display_name(&src.process, &src.window_title),
                    same_app: true,
                };
            }
        }
    }

    // 2. Code being edited stays in an editor (an .html from VS Code must not
    //    land in a browser), and files that run code are only safe there.
    //    Snapshots from versions before 1.0 carry no app: they only ever
    //    came from VS Code.
    let legacy_editor = src.process.is_empty() && is_text;
    let editor_first = src_is_editor || dangerous || legacy_editor;
    if editor_first {
        if let Some(plan) = editor_plan(sys) {
            return plan;
        }
    }

    // 3. Anything here that supports the file type: its default program,
    //    otherwise another program registered for that extension.
    if !dangerous && !ext.is_empty() {
        if let Some(name) = sys.association(ext) {
            return FilePlan::Default { name };
        }
        if let Some((name, class)) = sys.handlers(ext).into_iter().next() {
            return FilePlan::Handler { name, class };
        }
    }

    // 4. Text nobody here claims (.log, .cfg, unknown extensions): an editor.
    if is_text && !editor_first {
        if let Some(plan) = editor_plan(sys) {
            return plan;
        }
    }
    FilePlan::Reveal
}

#[derive(Debug, PartialEq)]
pub enum UrlPlan {
    Browser { exe: PathBuf, name: String },
    /// This machine's default browser (named when known).
    Default { name: Option<String>, same_app: bool },
}

/// How to open a received web page: the same browser if installed,
/// otherwise the default browser — Chrome on one machine, Brave on another.
pub fn plan_url(src: &AppContext, sys: &dyn Installed) -> UrlPlan {
    let src_is_browser = profile_for(&src.process)
        .map(|p| p.kind == Kind::Browser)
        .unwrap_or(false);
    if src_is_browser {
        if let Some(exe) = sys.find(&src.process, &src.exe_path) {
            return UrlPlan::Browser {
                exe,
                name: display_name(&src.process, ""),
            };
        }
    }
    UrlPlan::Default {
        name: sys.association("https"),
        same_app: !src_is_browser,
    }
}

#[derive(Debug, PartialEq)]
pub enum AppPlan {
    /// Start the same program (no document could be transferred).
    Launch { exe: PathBuf, name: String },
    /// Not installed here: show the screenshot instead.
    Screenshot { name: String },
}

/// What to do for an app whose content could not be transferred (a game, a
/// chat app, an unsaved document...).
pub fn plan_app(src: &AppContext, sys: &dyn Installed) -> AppPlan {
    let name = display_name(&src.process, &src.window_title);
    if may_launch(&src.process) {
        if let Some(exe) = sys.find(&src.process, &src.exe_path) {
            return AppPlan::Launch { exe, name };
        }
    }
    AppPlan::Screenshot { name }
}

// ── Window titles ──────────────────────────────────────────────────────────

/// File names (or full paths) a window title may refer to, most likely
/// first. Handles the common shapes:
///   `notes.txt - Notepad`, `*notes.txt - Notepad`,
///   `C:\x\notes.txt - Notepad++`, `notes.txt (C:\x) - Sublime Text`,
///   `report - Word` (extension hidden by Explorer settings).
pub fn title_candidates(title: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut parts: Vec<&str> = vec![title];
    for sep in [" - ", " — ", " – ", " | "] {
        parts = parts.iter().flat_map(|p| p.split(sep)).collect();
    }

    // The last part is usually the application name; skip it when there is
    // more than one part.
    let n = if parts.len() > 1 { parts.len() - 1 } else { parts.len() };
    for raw in &parts[..n] {
        let mut s = raw
            .trim()
            .trim_start_matches(['●', '•', '*'])
            .trim_end_matches('*')
            .trim()
            .to_string();
        // "notes.txt (C:\folder)" / "Document1 [Compatibility Mode]"
        for open in [" (", " ["] {
            if let Some(i) = s.find(open) {
                s.truncate(i);
            }
        }
        let s = s.trim().to_string();
        // Localised Windows 11 Notepad: "notes.txt, Not Defteri". Names can
        // contain ", " too, so keep both; candidates are only ever matched
        // against real files.
        let mut variants = vec![s.clone()];
        if let Some(i) = s.rfind(", ") {
            variants.push(s[..i].trim().to_string());
        }
        for v in variants {
            if v.is_empty() || v.len() > 260 || v.chars().any(|c| "<>\"|?*".contains(c)) {
                continue;
            }
            if !out.contains(&v) {
                out.push(v);
            }
        }
    }
    out
}

/// Tokens of a Windows command line (quoted or bare) after the program.
pub fn command_line_args(cmdline: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut any = false;
    for c in cmdline.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            ' ' | '\t' if !quoted => {
                if any {
                    args.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            _ => {
                cur.push(c);
                any = true;
            }
        }
    }
    if any {
        args.push(cur);
    }
    if !args.is_empty() {
        args.remove(0); // the program itself
    }
    args
}

/// Some apps pass the document inside an encoded argument instead of a
/// plain path — Windows 11 Notepad's window process gets
/// `/SESSION:<base64>` whose bytes hold the path as UTF-16. Returns any
/// `X:\...` paths found in such a base64 blob.
pub fn paths_in_encoded_arg(arg: &str) -> Vec<String> {
    use base64::Engine as _;
    let blob = arg.rsplit(':').next().unwrap_or(arg);
    if blob.len() < 40 || !blob.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b)) {
        return Vec::new();
    }
    let trimmed = blob.trim_end_matches('=');
    let Ok(bytes) = base64::engine::general_purpose::STANDARD_NO_PAD.decode(trimmed) else {
        return Vec::new();
    };
    // Scan both byte alignments for UTF-16LE runs of printable characters.
    let mut out = Vec::new();
    for start in 0..2 {
        let units: Vec<u16> = bytes[start..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        for run in units.split(|&u| u < 0x20) {
            // Header bytes may run straight into the path: start at "X:\".
            let Some(i) = run.windows(3).position(|w| {
                (w[0] as u8 as char).is_ascii_alphabetic() && w[0] < 0x80 && w[1] == ':' as u16 && w[2] == '\\' as u16
            }) else {
                continue;
            };
            let s = String::from_utf16_lossy(&run[i..]);
            if s.len() > 3 && !s.contains('\u{FFFD}') {
                out.push(s);
            }
        }
    }
    out
}

/// Does `path` match a title candidate (full path, file name, or file name
/// without its extension when Explorer hides extensions)?
pub fn matches_candidate(path: &Path, candidate: &str) -> bool {
    if Path::new(candidate).is_absolute() {
        return path.to_string_lossy().eq_ignore_ascii_case(candidate);
    }
    let name = path.file_name().map(|n| n.to_string_lossy().to_string());
    let stem = path.file_stem().map(|n| n.to_string_lossy().to_string());
    name.is_some_and(|n| n.eq_ignore_ascii_case(candidate))
        || stem.is_some_and(|n| n.eq_ignore_ascii_case(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct Fake {
        exes: HashMap<&'static str, &'static str>,
        code: bool,
        assoc: HashMap<&'static str, &'static str>,
        handlers: HashMap<&'static str, (&'static str, &'static str)>,
    }

    impl Fake {
        fn new(exes: &[(&'static str, &'static str)], code: bool) -> Self {
            Fake {
                exes: exes.iter().cloned().collect(),
                code,
                assoc: [
                    ("https", "Brave"),
                    ("docx", "LibreOffice Writer"),
                    ("pdf", "Edge"),
                    ("md", "Obsidian"),
                    ("html", "Brave"),
                ]
                .into_iter()
                .collect(),
                handlers: [("psd", ("GIMP", "GIMP.psd")), ("ora", ("Krita", "Krita.ora"))]
                    .into_iter()
                    .collect(),
            }
        }
    }

    impl Installed for Fake {
        fn find(&self, process: &str, _hint: &str) -> Option<PathBuf> {
            if !may_launch(process) {
                return None;
            }
            self.exes.get(process).map(PathBuf::from)
        }
        fn code_cli(&self) -> Option<PathBuf> {
            self.code.then(|| PathBuf::from(r"C:\VSCode\bin\code.cmd"))
        }
        fn association(&self, ext: &str) -> Option<String> {
            self.assoc.get(ext).map(|s| s.to_string())
        }
        fn handlers(&self, ext: &str) -> Vec<(String, String)> {
            self.handlers
                .get(ext)
                .map(|(n, c)| vec![(n.to_string(), c.to_string())])
                .unwrap_or_default()
        }
    }

    #[test]
    fn missing_app_uses_a_program_that_supports_the_extension() {
        let sys = Fake::new(&[NOTEPAD], true);
        // Typora -> no Typora here; .md is Obsidian's on this machine.
        assert_eq!(
            plan_file(&app("typora", "other"), "md", true, &sys),
            FilePlan::Default {
                name: "Obsidian".into()
            }
        );
        // No default program for .psd, but GIMP registered itself for it.
        assert_eq!(
            plan_file(&app("photoshop", "other"), "psd", false, &sys),
            FilePlan::Handler {
                name: "GIMP".into(),
                class: "GIMP.psd".into()
            }
        );
        // A PDF tab from Chrome (no Chrome here) opens in the PDF program.
        assert_eq!(
            plan_file(&app("chrome", "browser"), "pdf", false, &sys),
            FilePlan::Default { name: "Edge".into() }
        );
    }

    #[test]
    fn code_from_an_editor_stays_in_an_editor() {
        // index.html from Sublime: an editor, not the browser that owns .html.
        let sys = Fake::new(&[NOTEPAD], true);
        assert!(matches!(
            plan_file(&app("sublime_text", "editor"), "html", true, &sys),
            FilePlan::VsCode { .. }
        ));
    }

    #[test]
    fn unclaimed_text_falls_back_to_an_editor() {
        let sys = Fake::new(&[NOTEPAD], false);
        assert!(matches!(
            plan_file(&app("someapp", "other"), "cfg", true, &sys),
            FilePlan::App { ref name, .. } if name == "Not Defteri"
        ));
    }

    fn app(process: &str, kind: &str) -> AppContext {
        AppContext {
            process: process.into(),
            kind: kind.into(),
            ..Default::default()
        }
    }

    const NOTEPAD: (&str, &str) = ("notepad", r"C:\Windows\System32\notepad.exe");

    #[test]
    fn same_editor_is_preferred() {
        let sys = Fake::new(&[("sublime_text", r"C:\ST\sublime_text.exe"), NOTEPAD], true);
        assert_eq!(
            plan_file(&app("sublime_text", "editor"), "txt", true, &sys),
            FilePlan::App {
                exe: PathBuf::from(r"C:\ST\sublime_text.exe"),
                name: "Sublime Text".into(),
                same_app: true
            }
        );
    }

    #[test]
    fn missing_sublime_falls_back_to_vscode_then_notepad() {
        let with_code = Fake::new(&[NOTEPAD], true);
        assert!(matches!(
            plan_file(&app("sublime_text", "editor"), "txt", true, &with_code),
            FilePlan::VsCode { same_app: false, .. }
        ));
        let bare = Fake::new(&[NOTEPAD], false);
        assert_eq!(
            plan_file(&app("sublime_text", "editor"), "txt", true, &bare),
            FilePlan::App {
                exe: PathBuf::from(r"C:\Windows\System32\notepad.exe"),
                name: "Not Defteri".into(),
                same_app: false
            }
        );
    }

    #[test]
    fn missing_word_uses_the_default_program() {
        let sys = Fake::new(&[NOTEPAD], false);
        assert_eq!(
            plan_file(&app("winword", "document"), "docx", false, &sys),
            FilePlan::Default {
                name: "LibreOffice Writer".into()
            }
        );
    }

    #[test]
    fn unknown_binary_without_program_is_revealed() {
        let sys = Fake::new(&[NOTEPAD], false);
        assert_eq!(
            plan_file(&app("adobexd", "other"), "xd", false, &sys),
            FilePlan::Reveal
        );
    }

    #[test]
    fn script_files_never_go_to_their_default_program() {
        // A .py/.ps1/.bat sent as text opens in an editor, never in Python /
        // PowerShell / cmd (which would RUN it).
        let sys = Fake::new(&[NOTEPAD, ("python", r"C:\Python\python.exe")], false);
        for ext in ["py", "ps1", "bat", "js", "vbs", "hta"] {
            let plan = plan_file(&app("python", "other"), ext, true, &sys);
            assert!(
                matches!(plan, FilePlan::App { ref name, .. } if name == "Not Defteri"),
                "{} -> {:?}",
                ext,
                plan
            );
        }
        // Even as binary, a dangerous type never reaches its default program.
        let sys = Fake::new(&[], false);
        assert_eq!(plan_file(&app("", ""), "exe", false, &sys), FilePlan::Reveal);
    }

    #[test]
    fn legacy_snapshot_without_app_opens_in_an_editor() {
        let sys = Fake::new(&[NOTEPAD], true);
        assert!(matches!(
            plan_file(&AppContext::default(), "rs", true, &sys),
            FilePlan::VsCode { .. }
        ));
    }

    #[test]
    fn page_opens_in_same_browser_or_the_default_one() {
        let sys = Fake::new(&[("chrome", r"C:\Chrome\chrome.exe")], false);
        assert_eq!(
            plan_url(&app("chrome", "browser"), &sys),
            UrlPlan::Browser {
                exe: PathBuf::from(r"C:\Chrome\chrome.exe"),
                name: "Google Chrome".into()
            }
        );
        let sys = Fake::new(&[], false);
        assert_eq!(
            plan_url(&app("chrome", "browser"), &sys),
            UrlPlan::Default {
                name: Some("Brave".into()),
                same_app: false
            }
        );
    }

    #[test]
    fn missing_app_shows_the_screenshot() {
        let sys = Fake::new(&[("spotify", r"C:\Spotify\Spotify.exe")], false);
        assert!(matches!(
            plan_app(&app("spotify", "other"), &sys),
            AppPlan::Launch { .. }
        ));
        let mut figma = app("figma", "other");
        figma.window_title = "Untitled - Figma".into();
        assert_eq!(
            plan_app(&figma, &sys),
            AppPlan::Screenshot {
                name: "Figma".into()
            }
        );
    }

    #[test]
    fn shells_and_installers_are_never_launched() {
        for p in ["cmd", "PowerShell", "mshta", "rundll32", "explorer", "VSCodeUserSetup-x64"] {
            assert!(!may_launch(p), "{}", p);
        }
        assert!(may_launch("sublime_text"));
        let sys = Fake::new(&[("cmd", r"C:\Windows\System32\cmd.exe")], false);
        assert!(matches!(
            plan_app(&app("cmd", "other"), &sys),
            AppPlan::Screenshot { .. }
        ));
    }

    #[test]
    fn windows_folder_only_allows_known_tools() {
        let win = Some(r"C:\Windows");
        assert!(location_allowed(Path::new(r"C:\Windows\System32\notepad.exe"), win));
        assert!(!location_allowed(Path::new(r"C:\Windows\System32\certutil.exe"), win));
        assert!(location_allowed(Path::new(r"C:\Program Files\App\app.exe"), win));
        assert!(!location_allowed(Path::new(r"\\server\share\app.exe"), win));
        assert!(!location_allowed(
            Path::new(r"C:\Program Files\WindowsApps\Microsoft.WindowsNotepad_1\Notepad\Notepad.exe"),
            win
        ));
    }

    #[test]
    fn portable_paths_round_trip_between_users() {
        let alice = |v: &str| match v {
            "LOCALAPPDATA" => Some(r"C:\Users\alice\AppData\Local".to_string()),
            "USERPROFILE" => Some(r"C:\Users\alice".to_string()),
            "ProgramFiles" => Some(r"C:\Program Files".to_string()),
            "ProgramFiles(x86)" => Some(r"C:\Program Files (x86)".to_string()),
            _ => None,
        };
        let bob = |v: &str| match v {
            "LOCALAPPDATA" => Some(r"D:\Users\bob\AppData\Local".to_string()),
            _ => None,
        };
        let p = to_portable_with(r"C:\Users\alice\AppData\Local\Programs\x\x.exe", &alice);
        assert_eq!(p, r"%LOCALAPPDATA%\Programs\x\x.exe");
        assert_eq!(
            from_portable_with(&p, &bob).unwrap(),
            PathBuf::from(r"D:\Users\bob\AppData\Local\Programs\x\x.exe")
        );
        assert_eq!(
            to_portable_with(r"C:\Program Files (x86)\y.exe", &alice),
            r"%ProgramFiles(x86)%\y.exe"
        );
        // Unknown variables are not expanded.
        assert!(from_portable_with(r"%COMSPEC%\x", &bob).is_none());
    }

    #[test]
    fn titles_yield_file_candidates() {
        assert_eq!(title_candidates("notes.txt - Notepad"), vec!["notes.txt"]);
        assert!(title_candidates("notlar.txt, Not Defteri").contains(&"notlar.txt".to_string()));
        assert_eq!(title_candidates("*notes.txt - Not Defteri"), vec!["notes.txt"]);
        assert_eq!(
            title_candidates(r"*C:\work\todo.md - Notepad++"),
            vec![r"C:\work\todo.md"]
        );
        assert_eq!(
            title_candidates(r"main.py (C:\proj) - Sublime Text"),
            vec!["main.py"]
        );
        assert_eq!(
            title_candidates("Rapor [Uyumluluk Modu] - Word"),
            vec!["Rapor"]
        );
        assert!(title_candidates("Spotify Premium").len() == 1);
    }

    #[test]
    fn command_line_arguments_are_split() {
        assert_eq!(
            command_line_args(r#""C:\Program Files\App\app.exe" "C:\a b\c.txt" -n"#),
            vec![r"C:\a b\c.txt", "-n"]
        );
        assert_eq!(
            command_line_args(r"notepad.exe C:\x.txt"),
            vec![r"C:\x.txt"]
        );
    }

    #[test]
    fn paths_are_found_in_notepad_session_blobs() {
        use base64::Engine as _;
        // Header bytes + UTF-16LE path + padding, as Windows 11 Notepad does.
        let mut bytes = vec![0x27u8, 0x40, 0xEB, 0x79, 0x22, 0x1F, 0x1F, 0x4D, 0x01, 0x94, 0x00];
        for u in r"C:\Users\x\notlar.txt".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        bytes.extend_from_slice(&[0; 12]);
        let arg = format!(
            "/SESSION:{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        );
        assert!(paths_in_encoded_arg(&arg).contains(&r"C:\Users\x\notlar.txt".to_string()));
        assert!(paths_in_encoded_arg(r"C:\plain\path.txt").is_empty());
        assert!(paths_in_encoded_arg("-n").is_empty());
    }

    #[test]
    fn candidate_matching_handles_hidden_extensions() {
        let p = Path::new(r"C:\Docs\Rapor.docx");
        assert!(matches_candidate(p, "Rapor.docx"));
        assert!(matches_candidate(p, "rapor"));
        assert!(matches_candidate(p, r"C:\Docs\Rapor.docx"));
        assert!(!matches_candidate(p, "Other.docx"));
    }

    #[test]
    fn display_names() {
        assert_eq!(display_name("sublime_text", "x - Sublime Text"), "Sublime Text");
        assert_eq!(display_name("figma", "Design - Figma"), "Figma");
        assert_eq!(display_name("foo", "Foo"), "foo");
    }
}

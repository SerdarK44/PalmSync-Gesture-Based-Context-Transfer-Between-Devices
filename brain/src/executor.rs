use std::path::{Path, PathBuf};

use tokio::process::Command;

use palmsync_shared::StateSnapshot;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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

fn project_name(snap: &StateSnapshot) -> String {
    let name = Path::new(&snap.project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if name.is_empty() {
        "Untitled".into()
    } else {
        sanitize_component(&name)
    }
}

/// Find the same file on THIS machine: the exact source path (same machine
/// or identical layout), or a folder with the same project name.
fn find_local_copy(snap: &StateSnapshot) -> Option<PathBuf> {
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

/// Decide which file to open for a DROP, writing the received contents to
/// disk when needed. Never returns a path that doesn't exist — opening a
/// missing path is what produced the "empty file" bug.
pub fn prepare_file(snap: &StateSnapshot) -> anyhow::Result<PathBuf> {
    if let Some(local) = find_local_copy(snap) {
        if snap.file_content.is_empty() {
            return Ok(local);
        }
        match std::fs::read(&local) {
            Ok(bytes) if bytes == snap.file_content.as_bytes() => return Ok(local),
            _ => log::info!(
                "{} differs from the received version — saving received copy separately",
                local.display()
            ),
        }
    }

    if snap.file_content.is_empty() {
        anyhow::bail!(
            "'{}' does not exist on this computer and its contents were not sent \
             (file too large, not text, or not found on the source computer)",
            snap.file_name
        );
    }

    let target = received_root()
        .join(project_name(snap))
        .join(rel_path(snap));
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&target, snap.file_content.as_bytes())?;
    log::info!("Wrote received file to {}", target.display());
    Ok(target)
}

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

pub async fn open_vscode(file_path: &Path, line: u32) -> anyhow::Result<()> {
    let Some(code) = find_code_cli() else {
        log::warn!(
            "VS Code not found — opening {} with the default app",
            file_path.display()
        );
        return open_default(&file_path.to_string_lossy()).await;
    };

    let mut cmd = Command::new(&code);
    // Use --goto only when we have a real line number (0 means unknown).
    if line > 0 {
        cmd.arg("--goto")
            .arg(format!("{}:{}", file_path.display(), line));
    } else {
        cmd.arg(file_path);
    }
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    log::info!("Executing: {} {:?}", code.display(), file_path);
    let output = cmd.output().await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("VS Code open failed: {}", stderr));
    }
    Ok(())
}

async fn open_default(target: &str) -> anyhow::Result<()> {
    // `cmd /C start "" <target>` launches the default handler; the empty ""
    // is the window title arg that `start` requires before the target.
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "start", "", target]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn()?;
    Ok(())
}

/// Open a URL in the default browser on this machine.
pub async fn open_url(url: &str) -> anyhow::Result<()> {
    log::info!("Opening browser tab: {}", url);
    open_default(url).await
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
    fn missing_file_without_content_is_an_error_not_an_empty_file() {
        let s = snap("C:\\definitely\\not\\here\\proj-xyz", "a.rs", "");
        assert!(prepare_file(&s).is_err());
    }
}

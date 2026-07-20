use tokio::process::Command;

pub async fn open_vscode(project_path: &str, file_name: &str, line: u32) -> anyhow::Result<()> {
    let file_path = if project_path.is_empty() {
        file_name.to_string()
    } else {
        format!("{}\\{}", project_path, file_name)
    };

    let goto_arg = format!("{}:{}", file_path, line);
    log::info!("Executing: code --goto \"{}\"", goto_arg);

    let output = Command::new("cmd")
        .args(["/C", "code", "--goto", &goto_arg])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("code --goto failed: {}", stderr));
    }

    Ok(())
}

use std::process::Command;

const SERVICE_NAME: &str = "PalmSync";
const DISPLAY_NAME: &str = "PalmSync Workspace Synchronizer";

pub fn install() -> anyhow::Result<()> {
    let exe_path = std::env::current_exe()?;
    let exe = exe_path.to_string_lossy();

    run_sc(&[
        "create",
        SERVICE_NAME,
        &format!("displayname={}", DISPLAY_NAME),
        &format!("binPath={}", exe),
        "start=auto",
    ])?;

    run_sc(&[
        "description",
        SERVICE_NAME,
        "Cloudless P2P workspace synchronization via hand gestures",
    ])?;

    run_sc(&[
        "failure",
        SERVICE_NAME,
        "reset=86400",
        "actions=restart/5000/restart/10000/restart/30000",
    ])?;

    log::info!("Service '{}' registered for auto-start", SERVICE_NAME);
    Ok(())
}

pub fn uninstall() -> anyhow::Result<()> {
    let _ = Command::new("net").args(["stop", SERVICE_NAME]).output();

    run_sc(&["delete", SERVICE_NAME])?;
    log::info!("Service '{}' removed", SERVICE_NAME);
    Ok(())
}

fn run_sc(args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("sc.exe").args(args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(
            "sc.exe {} failed: {} {}",
            args.join(" "),
            stdout.trim(),
            stderr.trim()
        );
    }
    Ok(())
}

#![allow(dead_code)]
// jesse/sandbox.rs -- Execution sandboxing utilities

use anyhow::Result;
use std::path::PathBuf;

/// Validates a script before execution — blocks obviously dangerous patterns.
pub fn validate_script(script: &str) -> Result<()> {
    let blocked = [
        "rm -rf /",
        "mkfs",
        "dd if=/dev/zero of=/dev/",
        ":(){:|:&};:",
    ];
    for pattern in &blocked {
        if script.contains(pattern) {
            anyhow::bail!("Blocked dangerous pattern: '{}'", pattern);
        }
    }
    Ok(())
}

/// Create a temp directory for isolated script execution.
pub fn temp_work_dir() -> Result<tempfile::TempDir> {
    Ok(tempfile::tempdir()?)
}

/// Write script content to a temp file and return its path.
pub fn write_temp_script(content: &str) -> Result<(tempfile::TempDir, PathBuf)> {
    let dir = temp_work_dir()?;
    let path = dir.path().join("heisenberg_task.sh");
    std::fs::write(&path, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok((dir, path))
}

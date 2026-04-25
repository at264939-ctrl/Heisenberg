#![allow(dead_code)]
// the_rv/driver.rs -- Browser automation delegate

use crate::jesse::JesseRunner;
use crate::saul::policy::PolicyEngine;
use anyhow::Result;
use tracing::info;

pub struct BrowserDriver {
    policy: PolicyEngine,
    runner: JesseRunner,
}

impl BrowserDriver {
    pub fn new(policy: PolicyEngine, runner: JesseRunner) -> Self {
        Self { policy, runner }
    }

    /// Open a URL. Relies on the external 'drive.sh' script.
    pub async fn open(&self, url: &str) -> Result<()> {
        self.policy.check_browser()?;
        info!("The RV is driving to: {}", url);

        // Execute drive.sh via Jesse
        let script = format!("bash scripts/drive.sh open '{}'", url);
        let res = self.runner.run_inline(&script).await?;

        if !res.success() {
            anyhow::bail!("Browser driver failed: {}", res.stderr);
        }
        Ok(())
    }

    pub async fn click(&self, selector: &str) -> Result<()> {
        self.policy.check_browser()?;
        info!("The RV clicking: {}", selector);

        let script = format!("bash scripts/drive.sh click '{}'", selector);
        let res = self.runner.run_inline(&script).await?;

        if !res.success() {
            anyhow::bail!("Browser click failed: {}", res.stderr);
        }
        Ok(())
    }
}

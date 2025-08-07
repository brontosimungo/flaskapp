use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

const GITHUB_API_URL: &str = "https://api.github.com/repos/swpsco/nockpool-miner/releases/latest";
const USER_AGENT: &str = "nockpool-miner-updater";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: String,
    pub published_at: String,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OperatingSystem {
    MacOS,
    Linux,
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub download_url: String,
    pub asset_name: String,
    pub needs_update: bool,
}

pub struct AutoUpdater {
    client: reqwest::Client,
    current_exe_path: PathBuf,
    os: OperatingSystem,
}

impl AutoUpdater {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let current_exe_path = std::env::current_exe()?;
        let os = detect_os()?;

        Ok(Self {
            client,
            current_exe_path,
            os,
        })
    }

    /// Check if an update is available
    pub async fn check_for_updates(&self, current_version: &str) -> Result<UpdateInfo> {
        info!("Checking for updates...");
        
        let release = self.fetch_latest_release().await?;
        let asset = self.find_compatible_asset(&release.assets)?;
        
        let needs_update = current_version != release.tag_name;
        
        Ok(UpdateInfo {
            current_version: current_version.to_string(),
            latest_version: release.tag_name.clone(),
            download_url: asset.browser_download_url.clone(),
            asset_name: asset.name.clone(),
            needs_update,
        })
    }

    /// Perform the update if one is available and restart the process
    pub async fn update_and_restart(&self, current_version: &str) -> Result<bool> {
        let update_info = self.check_for_updates(current_version).await?;
        
        if !update_info.needs_update {
            info!("Already up to date (version: {})", current_version);
            return Ok(false);
        }

        info!(
            "Update available: {} -> {}, updating...",
            update_info.current_version,
            update_info.latest_version
        );

        match self.os {
            OperatingSystem::Linux => {
                self.update_linux_binary(&update_info).await?;
                self.restart_process()?;
            }
            OperatingSystem::MacOS => {
                self.update_macos_package(&update_info).await?;
                self.restart_process()?;
            }
        }

        Ok(true)
    }

    /// Perform the update if one is available
    pub async fn update(&self, current_version: &str) -> Result<bool> {
        let update_info = self.check_for_updates(current_version).await?;
        
        if !update_info.needs_update {
            info!("Already up to date (version: {})", current_version);
            return Ok(false);
        }

        info!(
            "Update available: {} -> {}",
            update_info.current_version,
            update_info.latest_version
        );

        match self.os {
            OperatingSystem::Linux => {
                self.update_linux_binary(&update_info).await?;
            }
            OperatingSystem::MacOS => {
                self.update_macos_package(&update_info).await?;
            }
        }

        info!("Update completed successfully!");
        Ok(true)
    }

    async fn fetch_latest_release(&self) -> Result<GitHubRelease> {
        let response = self.client
            .get(GITHUB_API_URL)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to fetch release info: HTTP {}",
                response.status()
            ));
        }

        let release: GitHubRelease = response.json().await?;
        Ok(release)
    }

    fn find_compatible_asset<'a>(&self, assets: &'a [ReleaseAsset]) -> Result<&'a ReleaseAsset> {
        let expected_name = match self.os {
            OperatingSystem::Linux => "nockpool-miner-linux-x86_64",
            OperatingSystem::MacOS => "nockpool-miner-macos-aarch64.pkg",
        };

        assets
            .iter()
            .find(|asset| asset.name == expected_name)
            .ok_or_else(|| anyhow!("No compatible asset found for {}", expected_name))
    }

    async fn update_linux_binary(&self, update_info: &UpdateInfo) -> Result<()> {
        info!("Downloading Linux binary: {}", update_info.asset_name);
        
        // Download the new binary
        let response = self.client
            .get(&update_info.download_url)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to download binary: HTTP {}",
                response.status()
            ));
        }

        let binary_data = response.bytes().await?;
        
        // Create backup of current binary
        let backup_path = self.current_exe_path.with_extension("backup");
        if backup_path.exists() {
            std::fs::remove_file(&backup_path)?;
        }
        std::fs::copy(&self.current_exe_path, &backup_path)?;
        
        // Write new binary
        std::fs::write(&self.current_exe_path, binary_data)?;
        
        // Make executable on Linux
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&self.current_exe_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&self.current_exe_path, perms)?;
        }
        
        info!("Binary updated successfully. Backup saved to: {:?}", backup_path);
        Ok(())
    }

    async fn update_macos_package(&self, update_info: &UpdateInfo) -> Result<()> {
        info!("Downloading macOS package: {}", update_info.asset_name);
        
        // Download the package
        let response = self.client
            .get(&update_info.download_url)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to download package: HTTP {}",
                response.status()
            ));
        }

        let package_data = response.bytes().await?;
        
        // Save to temporary file
        let temp_pkg = tempfile::NamedTempFile::with_suffix(".pkg")?;
        std::fs::write(temp_pkg.path(), package_data)?;
        
        info!("Installing macOS package...");
        
        // Install the package using the installer command
        let output = std::process::Command::new("sudo")
            .arg("installer")
            .arg("-pkg")
            .arg(temp_pkg.path())
            .arg("-target")
            .arg("/")
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Package installation failed: {}", stderr));
        }

        info!("macOS package installed successfully!");
        Ok(())
    }

    fn restart_process(&self) -> Result<()> {
        info!("Restarting process with updated binary...");
        
        // Get current process arguments
        let args: Vec<String> = std::env::args().collect();
        let mut cmd_args = args.clone();
        
        // Remove the first argument (the executable path) and use current_exe_path instead
        cmd_args.remove(0);
        
        // Start the new process with same arguments
        let mut cmd = std::process::Command::new(&self.current_exe_path);
        cmd.args(&cmd_args);
        
        match cmd.spawn() {
            Ok(_) => {
                info!("New process started successfully. Exiting current process...");
                std::process::exit(0);
            }
            Err(e) => {
                return Err(anyhow!("Failed to restart process: {}", e));
            }
        }
    }
}

fn detect_os() -> Result<OperatingSystem> {
    match std::env::consts::OS {
        "linux" => Ok(OperatingSystem::Linux),
        "macos" => Ok(OperatingSystem::MacOS),
        os => Err(anyhow!("Unsupported operating system: {}", os)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_os() {
        let os = detect_os().unwrap();
        // This will pass on the systems we support
        assert!(matches!(os, OperatingSystem::Linux | OperatingSystem::MacOS));
    }

    #[tokio::test]
    async fn test_fetch_latest_release() {
        let updater = AutoUpdater::new().unwrap();
        let release = updater.fetch_latest_release().await;
        
        // This test requires internet connectivity
        if release.is_ok() {
            let release = release.unwrap();
            assert!(!release.tag_name.is_empty());
            assert!(!release.assets.is_empty());
        }
    }
}
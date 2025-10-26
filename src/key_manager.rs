use anyhow::{anyhow, Result};
use tracing::info;

use crate::auth::SupabaseAuth;
use crate::config::Config;
use crate::key_storage::KeyStorage;

pub struct KeyManager {
    storage: KeyStorage,
    auth: SupabaseAuth,
}

impl KeyManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            storage: KeyStorage::new()?,
            auth: SupabaseAuth::new(),
        })
    }

    pub async fn get_mining_key(&self, config: &Config) -> Result<String> {
        // If a key is explicitly provided via CLI, use it directly
        if let Some(key) = &config.key {
            info!("Using node key provided via --key argument");
            return Ok(key.clone());
        }

        // Check if we have a stored key
        if let Some(stored_key) = self.storage.load_key()? {
            info!("Using stored node key");
            return Ok(stored_key);
        }

        // No stored key, need to create one using account token
        let account_token = config.account_token.as_ref()
            .ok_or_else(|| anyhow!("Account token is required for authentication"))?;

        info!("No stored node key found, creating new one using account token...");
        
        // Generate a nickname for the device (optional)
        let device_nickname = self.generate_device_nickname();
        
        // Create new mining token via account token
        let new_key = self.auth.get_or_create_mining_token(account_token, device_nickname, &config.api_url).await?;
        
        // Store the new key locally
        self.storage.save_key(&new_key)?;
        
        info!("Successfully created and stored new node key");
        Ok(new_key)
    }

    pub fn clear_stored_key(&self) -> Result<()> {
        self.storage.delete_key()
    }


    pub fn get_key_storage_path(&self) -> String {
        self.storage.get_key_file_path().display().to_string()
    }

    fn generate_device_nickname(&self) -> Option<String> {
        use uuid::Uuid;
        
        // Try to get hostname as nickname
        let base_name = match std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .or_else(|_| {
                // Try reading from /etc/hostname on Unix systems
                std::fs::read_to_string("/etc/hostname")
                    .map(|s| s.trim().to_string())
            }) {
            Ok(hostname) if !hostname.is_empty() => {
                format!("miner-{}", hostname)
            }
            _ => {
                // Fallback to a generic name
                "miner".to_string()
            }
        };
        
        // Append UUID to ensure uniqueness
        let uuid = Uuid::new_v4().to_string();
        let short_uuid = &uuid[0..8]; // Use first 8 characters of UUID
        Some(format!("{}-{}", base_name, short_uuid))
    }
}

pub async fn resolve_mining_key(config: &Config) -> Result<String> {
    // Validate authentication configuration
    if let Err(e) = config.validate_auth() {
        return Err(anyhow!("Authentication configuration error: {}", e));
    }

    let key_manager = KeyManager::new()?;
    key_manager.get_mining_key(config).await
}

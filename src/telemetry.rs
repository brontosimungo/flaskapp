use anyhow::Result;
use reqwest::Client;
use serde::Serialize;
use std::time::Duration;
use tokio::time::{interval, sleep};
use tracing::{error, info, warn};

use crate::device::get_device_info_with_proof_rate;

#[derive(Debug, Serialize)]
struct TelemetryData {
    device_os: String,
    device_cpu: String,
    device_ram_capacity_gb: u64,
    device_proof_rate_per_sec: f64,
}

pub struct TelemetryClient {
    client: Client,
    api_key: String,
    api_base_url: String,
}

impl TelemetryClient {
    pub fn new(api_key: String, api_base_url: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            api_base_url,
        }
    }

    pub async fn send_telemetry(&self) -> Result<()> {
        let (device_info, proof_rate) = get_device_info_with_proof_rate();
        
        let telemetry = TelemetryData {
            device_os: device_info.os,
            device_cpu: device_info.cpu_model,
            device_ram_capacity_gb: device_info.ram_capacity_gb,
            device_proof_rate_per_sec: proof_rate,
        };

        let api_url = format!("{}/api/v1/telemetry", self.api_base_url);
        
        let response = self
            .client
            .post(&api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&telemetry)
            .timeout(Duration::from_secs(30))
            .send()
            .await?;

        if response.status().is_success() {
            info!("Telemetry sent successfully: {:.4} proofs/sec", proof_rate);
        } else {
            let status = response.status();
            let error_text = response.text().await?;
            warn!("Failed to send telemetry ({}): {}", status, error_text);
        }

        Ok(())
    }

    pub async fn start_telemetry_loop(&self) -> Result<()> {
        let mut interval = interval(Duration::from_secs(300)); // Send telemetry every 5 minutes
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.send_telemetry().await {
                error!("Error sending telemetry: {}", e);
                // Don't break the loop on error, just wait and try again
                sleep(Duration::from_secs(10)).await;
            }
        }
    }
}
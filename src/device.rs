use sysinfo::System;
use quiver::device_info::DeviceInfo;
use crate::miner::get_current_proof_rate;
use std::process::Command;

pub fn get_device_info() -> DeviceInfo {
    let mut sys = System::new();
    sys.refresh_cpu();
    sys.refresh_memory();
    let os = System::long_os_version().unwrap_or_else(|| "Unknown OS".to_string());
    let cpu_model = sys.cpus().first().map_or("Unknown CPU".to_string(), |cpu| cpu.brand().to_string());
    // Convert from bytes to gigabytes
    let ram_capacity_gb = sys.total_memory() / (1024 * 1024 * 1024);

    DeviceInfo {
        os: os.clone().trim().to_string(),
        cpu_model: cpu_model.clone().trim().to_string(),
        ram_capacity_gb,
    }
}

pub fn get_device_info_with_proof_rate() -> (DeviceInfo, f64) {
    let device_info = get_device_info();
    let proof_rate = get_current_proof_rate();
    (device_info, proof_rate)
}

pub fn get_gpu_info() -> Option<String> {
    let mut gpu_models = Vec::new();

    // Only detect GPUs on Linux
    #[cfg(target_os = "linux")]
    {
        // Try to detect NVIDIA GPUs
        gpu_models.extend(detect_nvidia_gpus());

        // Try to detect AMD GPUs
        gpu_models.extend(detect_amd_gpus());

        // Try to detect Intel GPUs
        gpu_models.extend(detect_intel_gpus());
    }

    if gpu_models.is_empty() {
        None
    } else {
        Some(gpu_models.join(", "))
    }
}

#[cfg(target_os = "linux")]
fn detect_nvidia_gpus() -> Vec<String> {
    let mut gpu_models = Vec::new();

    // Check if nvidia-smi is available and working
    if let Ok(output) = Command::new("nvidia-smi")
        .args(&["--query-gpu=name", "--format=csv,noheader,nounits"])
        .output()
    {
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                let name = line.trim();
                if !name.is_empty() {
                    gpu_models.push(name.to_string());
                }
            }
        }
    }

    gpu_models
}

#[cfg(target_os = "linux")]
fn detect_amd_gpus() -> Vec<String> {
    let mut gpu_models = Vec::new();

    // Check lspci for AMD GPUs
    if let Ok(output) = Command::new("lspci")
        .args(&["-nn"])
        .output()
    {
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                let line_lower = line.to_lowercase();
                if (line_lower.contains("amd") || line_lower.contains("ati")) &&
                   (line_lower.contains("vga") || line_lower.contains("display") || line_lower.contains("3d")) {

                    // Extract GPU name from lspci output
                    let parts: Vec<&str> = line.split(": ").collect();
                    let model = if parts.len() > 1 {
                        parts[1].to_string()
                    } else {
                        "AMD GPU".to_string()
                    };

                    gpu_models.push(model);
                }
            }
        }
    }

    gpu_models
}

#[cfg(target_os = "linux")]
fn detect_intel_gpus() -> Vec<String> {
    let mut gpu_models = Vec::new();

    // Check lspci for Intel GPUs
    if let Ok(output) = Command::new("lspci")
        .args(&["-nn"])
        .output()
    {
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                let line_lower = line.to_lowercase();
                if line_lower.contains("intel") &&
                   (line_lower.contains("vga") || line_lower.contains("display") || line_lower.contains("graphics")) {

                    // Extract GPU name from lspci output
                    let parts: Vec<&str> = line.split(": ").collect();
                    let model = if parts.len() > 1 {
                        parts[1].to_string()
                    } else {
                        "Intel GPU".to_string()
                    };

                    gpu_models.push(model);
                }
            }
        }
    }

    gpu_models
}
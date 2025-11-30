//! Deterministic client ID generation for ACP TUI
//!
//! Generates a stable, unique client ID based on hardware characteristics
//! that will be the same every time on the same machine.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Generate a deterministic client ID for this machine
/// 
/// The ID is based on hardware characteristics and will always be the same
/// for the same machine, enabling subscription resumption.
///
/// Format: `tui_{8_hex_chars}` (e.g., "tui_4a5b2c8d")
pub fn generate_client_id() -> Result<String> {
    let hardware_fingerprint = get_hardware_fingerprint()
        .context("Failed to generate hardware fingerprint")?;
    
    let hash = Sha256::digest(hardware_fingerprint.as_bytes());
    let client_id = format!("tui_{}", hex::encode(&hash[..8]));
    
    tracing::debug!(client_id = %client_id, "Generated deterministic client ID");
    Ok(client_id)
}

/// Generate a hardware fingerprint for this machine
/// 
/// Combines multiple hardware identifiers to create a stable fingerprint:
/// - Primary network interface MAC address
/// - Hostname
/// - CPU information (if available)
fn get_hardware_fingerprint() -> Result<String> {
    let mut components = Vec::new();
    
    // Primary MAC address
    if let Some(mac) = get_primary_mac_address() {
        components.push(format!("mac:{}", mac));
    }
    
    // Hostname
    if let Some(hostname) = get_hostname() {
        components.push(format!("host:{}", hostname));
    }
    
    // CPU info as fallback
    if let Some(cpu_info) = get_cpu_info() {
        components.push(format!("cpu:{}", cpu_info));
    }
    
    // If we couldn't get any hardware info, use a static fallback
    if components.is_empty() {
        components.push("fallback:unknown-machine".to_string());
        tracing::warn!("No hardware identifiers found, using fallback client ID");
    }
    
    let fingerprint = components.join("-");
    tracing::trace!(fingerprint = %fingerprint, "Hardware fingerprint created");
    
    Ok(fingerprint)
}

/// Get the primary MAC address from the system
fn get_primary_mac_address() -> Option<String> {
    match mac_address::get_mac_address() {
        Ok(Some(mac)) => {
            let mac_string = format!("{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                mac.bytes()[0], mac.bytes()[1], mac.bytes()[2],
                mac.bytes()[3], mac.bytes()[4], mac.bytes()[5]);
            tracing::trace!(mac = %mac_string, "Found primary MAC address");
            Some(mac_string)
        }
        Ok(None) => {
            tracing::warn!("No MAC address found");
            None
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get MAC address");
            None
        }
    }
}

/// Get the system hostname
fn get_hostname() -> Option<String> {
    match std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .or_else(|_| {
            std::process::Command::new("hostname")
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .map_err(|e| std::env::VarError::NotPresent)
        }) {
        Ok(hostname) if !hostname.is_empty() => {
            tracing::trace!(hostname = %hostname, "Found hostname");
            Some(hostname)
        }
        _ => {
            tracing::warn!("No hostname found");
            None
        }
    }
}

/// Get CPU information as an additional identifier
fn get_cpu_info() -> Option<String> {
    // Try to read CPU info from /proc/cpuinfo on Linux
    if let Ok(cpu_info) = std::fs::read_to_string("/proc/cpuinfo") {
        // Extract model name
        for line in cpu_info.lines() {
            if line.starts_with("model name") {
                if let Some(model) = line.split(':').nth(1) {
                    let model = model.trim();
                    tracing::trace!(cpu_model = %model, "Found CPU model");
                    return Some(model.to_string());
                }
            }
        }
    }
    
    // Fallback: try environment variables or system commands
    if let Ok(output) = std::process::Command::new("uname").arg("-m").output() {
        if output.status.success() {
            let arch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !arch.is_empty() {
                tracing::trace!(arch = %arch, "Found system architecture");
                return Some(format!("arch:{}", arch));
            }
        }
    }
    
    tracing::trace!("No CPU info found");
    None
}

/// Format client ID for display (abbreviated)
pub fn format_client_id_short(client_id: &str) -> String {
    if client_id.len() > 12 {
        format!("{}..{}", &client_id[..8], &client_id[client_id.len()-4..])
    } else {
        client_id.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_client_id_format() {
        let client_id = generate_client_id().unwrap();
        assert!(client_id.starts_with("tui_"));
        assert_eq!(client_id.len(), 20); // "tui_" + 16 hex chars
    }

    #[test]
    fn test_client_id_deterministic() {
        let id1 = generate_client_id().unwrap();
        let id2 = generate_client_id().unwrap();
        assert_eq!(id1, id2, "Client ID should be deterministic");
    }

    #[test]
    fn test_format_client_id_short() {
        let long_id = "tui_1234567890abcdef";
        let short = format_client_id_short(long_id);
        assert_eq!(short, "tui_1234..cdef");
        
        let short_id = "tui_1234";
        let unchanged = format_client_id_short(short_id);
        assert_eq!(unchanged, "tui_1234");
    }

    #[test]
    fn test_hardware_fingerprint_generation() {
        let fingerprint = get_hardware_fingerprint().unwrap();
        assert!(!fingerprint.is_empty());
        
        // Should be consistent
        let fingerprint2 = get_hardware_fingerprint().unwrap();
        assert_eq!(fingerprint, fingerprint2);
    }
}
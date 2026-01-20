//! Node command implementations
//!
//! Commands for starting, stopping, and managing blockchain nodes.

use super::RpcClient;
use crate::ui;
use common::{Result, VotingError};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tracing::{error, info};

/// Start a blockchain node
pub async fn start(
    node_type: String,
    genesis: Option<PathBuf>,
    validator_key: Option<PathBuf>,
    data_dir: Option<PathBuf>,
    enable_rpc: bool,
    rpc_addr: String,
    enable_metrics: bool,
    metrics_addr: String,
) -> Result<()> {
    info!("Starting {} node", node_type);

    // Validate node type
    let node_type_lower = node_type.to_lowercase();
    match node_type_lower.as_str() {
        "validator" | "full" | "light" => {}
        _ => {
            return Err(VotingError::ConfigError(format!(
                "Invalid node type: {}. Must be 'validator', 'full', or 'light'",
                node_type
            )));
        }
    }

    // Validator nodes require a validator key
    if node_type_lower == "validator" && validator_key.is_none() {
        return Err(VotingError::ConfigError(
            "Validator nodes require --validator-key".to_string(),
        ));
    }

    // Build node binary name based on type
    let binary_name = match node_type_lower.as_str() {
        "validator" => "validator-node",
        "full" => "full-node",
        "light" => "light-client",
        _ => unreachable!(),
    };

    // Build command arguments
    let mut args = Vec::new();

    if let Some(genesis_path) = genesis {
        args.push("--genesis".to_string());
        args.push(genesis_path.to_string_lossy().to_string());
    }

    if let Some(key_path) = validator_key {
        args.push("--validator-key".to_string());
        args.push(key_path.to_string_lossy().to_string());
    }

    if let Some(data_path) = data_dir {
        args.push("--data-dir".to_string());
        args.push(data_path.to_string_lossy().to_string());
    }

    if enable_rpc {
        args.push("--enable-rpc".to_string());
        args.push("--rpc-addr".to_string());
        args.push(rpc_addr.clone());
    }

    if enable_metrics {
        args.push("--enable-metrics".to_string());
        args.push("--metrics-addr".to_string());
        args.push(metrics_addr.clone());
    }

    info!("Launching {} with args: {:?}", binary_name, args);

    // Check if binary exists
    let binary_path = find_binary(binary_name)?;

    // Start the node process
    let child = Command::new(binary_path)
        .args(&args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| VotingError::ConfigError(format!("Failed to start node: {}", e)))?;

    // Save PID for later stopping
    let pid = child.id();
    save_pid(pid)?;

    info!("Node started with PID: {}", pid);
    ui::display_success(&format!(
        "{} node started successfully (PID: {})",
        node_type, pid
    ));

    if enable_rpc {
        println!("\nRPC endpoint: {}", rpc_addr);
    }

    if enable_metrics {
        println!("Metrics endpoint: {}", metrics_addr);
    }

    println!("\nNode is running in the background.");
    println!("Use 'voting-cli node status' to check node status.");
    println!("Use 'voting-cli node stop' to stop the node.");

    Ok(())
}

/// Stop a running node
pub async fn stop() -> Result<()> {
    info!("Stopping node");

    let pid = load_pid()?;

    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        let pid = Pid::from_raw(pid as i32);
        kill(pid, Signal::SIGTERM)
            .map_err(|e| VotingError::ConfigError(format!("Failed to stop node: {}", e)))?;

        info!("Sent SIGTERM to process {}", pid);
    }

    #[cfg(windows)]
    {
        Command::new("taskkill")
            .args(&["/PID", &pid.to_string(), "/F"])
            .output()
            .map_err(|e| VotingError::ConfigError(format!("Failed to stop node: {}", e)))?;

        info!("Killed process {}", pid);
    }

    remove_pid()?;

    ui::display_success("Node stopped successfully");
    Ok(())
}

/// Get node status
pub async fn status(rpc_endpoint: &str) -> Result<()> {
    info!("Checking node status at {}", rpc_endpoint);

    let client = RpcClient::new(rpc_endpoint);

    // Try to get node info
    match client.call_no_params("node_info").await {
        Ok(info) => {
            ui::display_node_info(&info, ui::DisplayFormat::Table)?;

            // Also get sync status
            if let Ok(sync_status) = client.call_no_params("sync_status").await {
                println!("\n╔══════════════════════════════════════════════════════════════╗");
                println!("║                       SYNC STATUS                            ║");
                println!("╠══════════════════════════════════════════════════════════════╣");
                
                let syncing = sync_status["syncing"].as_bool().unwrap_or(false);
                let current = sync_status["current_height"].as_u64().unwrap_or(0);
                let target = sync_status["target_height"].as_u64().unwrap_or(0);
                let progress = sync_status["progress"].as_f64().unwrap_or(0.0);

                println!("║  Syncing            │  {:<39}║", if syncing { "Yes" } else { "No" });
                println!("║  Current Height     │  {:<39}║", current);
                println!("║  Target Height      │  {:<39}║", target);
                println!("║  Progress           │  {:<37.2}%  ║", progress);
                
                if let Some(eta) = sync_status["estimated_time_remaining"].as_u64() {
                    let eta_str = format_eta(eta);
                    println!("║  ETA                │  {:<39}║", eta_str);
                }
                
                println!("╚══════════════════════════════════════════════════════════════╝");
            }

            Ok(())
        }
        Err(e) => {
            error!("Node is not responding: {}", e);
            ui::display_error(&e);
            Err(e)
        }
    }
}

/// Get detailed node information
pub async fn info(rpc_endpoint: &str) -> Result<()> {
    info!("Getting node information from {}", rpc_endpoint);

    let client = RpcClient::new(rpc_endpoint);

    // Get comprehensive node info
    let node_info = client.call_no_params("node_info").await?;
    let chain_info = client.call_no_params("chain_info").await?;
    let health = client.call_no_params("health").await?;

    // Display node info
    ui::display_node_info(&node_info, ui::DisplayFormat::Table)?;

    // Display chain info
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                     BLOCKCHAIN INFORMATION                   ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║  Height             │  {:<39}║",
        chain_info["height"]
    );
    println!(
        "║  Total Transactions │  {:<39}║",
        chain_info["total_transactions"]
    );
    println!(
        "║  Total Votes        │  {:<39}║",
        chain_info["total_votes"]
    );
    println!(
        "║  Active Elections   │  {:<39}║",
        chain_info["active_elections"]
    );
    println!(
        "║  Active Validators  │  {:<39}║",
        chain_info["active_validators"]
    );
    println!("╚══════════════════════════════════════════════════════════════╝");

    // Display health
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                       HEALTH STATUS                          ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║  Overall Status     │  {:<39}║",
        health["status"].as_str().unwrap_or("unknown")
    );
    println!(
        "║  Database           │  {:<39}║",
        health["database"].as_str().unwrap_or("unknown")
    );
    println!(
        "║  Network            │  {:<39}║",
        health["network"].as_str().unwrap_or("unknown")
    );
    println!(
        "║  Consensus          │  {:<39}║",
        health["consensus"].as_str().unwrap_or("unknown")
    );
    println!("╚══════════════════════════════════════════════════════════════╝");

    Ok(())
}

// Helper functions

fn find_binary(name: &str) -> Result<PathBuf> {
    // Try to find binary in cargo target directory
    let cargo_target = PathBuf::from("target/release").join(name);
    if cargo_target.exists() {
        return Ok(cargo_target);
    }

    let cargo_target_debug = PathBuf::from("target/debug").join(name);
    if cargo_target_debug.exists() {
        return Ok(cargo_target_debug);
    }

    // Try system PATH
    if which::which(name).is_ok() {
        return Ok(PathBuf::from(name));
    }

    Err(VotingError::ConfigError(format!(
        "Binary '{}' not found. Please build the project first.",
        name
    )))
}

fn get_pid_file() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push("voting-node.pid");
    path
}

fn save_pid(pid: u32) -> Result<()> {
    let pid_file = get_pid_file();
    std::fs::write(&pid_file, pid.to_string())
        .map_err(|e| VotingError::ConfigError(format!("Failed to save PID: {}", e)))?;
    Ok(())
}

fn load_pid() -> Result<u32> {
    let pid_file = get_pid_file();
    if !pid_file.exists() {
        return Err(VotingError::ConfigError(
            "No running node found (PID file not found)".to_string(),
        ));
    }

    let pid_str = std::fs::read_to_string(&pid_file)
        .map_err(|e| VotingError::ConfigError(format!("Failed to read PID: {}", e)))?;

    pid_str
        .trim()
        .parse()
        .map_err(|e| VotingError::ConfigError(format!("Invalid PID: {}", e)))
}

fn remove_pid() -> Result<()> {
    let pid_file = get_pid_file();
    if pid_file.exists() {
        std::fs::remove_file(&pid_file)
            .map_err(|e| VotingError::ConfigError(format!("Failed to remove PID file: {}", e)))?;
    }
    Ok(())
}

fn format_eta(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_eta() {
        assert_eq!(format_eta(3661), "1h 1m");
        assert_eq!(format_eta(125), "2m 5s");
        assert_eq!(format_eta(45), "45s");
    }

    #[test]
    fn test_pid_file_operations() {
        let test_pid = 12345u32;
        save_pid(test_pid).unwrap();
        let loaded_pid = load_pid().unwrap();
        assert_eq!(loaded_pid, test_pid);
        remove_pid().unwrap();
    }
}

//! User interface utilities for the CLI
//!
//! Provides formatted output, tables, progress indicators, and interactive displays
//! for vote verification, tallies, analytics, and anomaly detection.

use common::{Result, VotingError};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, Write};

/// Display formats for output
#[derive(Debug, Clone, Copy)]
pub enum DisplayFormat {
    /// Human-readable table format
    Table,
    /// JSON format
    Json,
    /// Compact single-line format
    Compact,
}

/// Display node information
pub fn display_node_info(info: &Value, format: DisplayFormat) -> Result<()> {
    match format {
        DisplayFormat::Json => {
            println!("{}", serde_json::to_string_pretty(info).unwrap());
        }
        DisplayFormat::Table => {
            println!("╔══════════════════════════════════════════════════════════════╗");
            println!("║                        NODE INFORMATION                      ║");
            println!("╠══════════════════════════════════════════════════════════════╣");
            print_field("Version", info["version"].as_str().unwrap_or("N/A"));
            print_field("Node Type", info["node_type"].as_str().unwrap_or("N/A"));
            print_field("Network ID", info["network_id"].as_str().unwrap_or("N/A"));
            print_field("Height", &info["height"].to_string());
            print_field("Peers", &info["peer_count"].to_string());
            print_field(
                "Syncing",
                if info["syncing"].as_bool().unwrap_or(false) {
                    "Yes"
                } else {
                    "No"
                },
            );
            print_field("Uptime", &format_uptime(info["uptime"].as_u64().unwrap_or(0)));
            println!("╚══════════════════════════════════════════════════════════════╝");
        }
        DisplayFormat::Compact => {
            println!(
                "Node: {} | Height: {} | Peers: {} | Syncing: {}",
                info["node_type"].as_str().unwrap_or("N/A"),
                info["height"],
                info["peer_count"],
                info["syncing"]
            );
        }
    }
    Ok(())
}

/// Display block information
pub fn display_block(block: &Value, format: DisplayFormat) -> Result<()> {
    match format {
        DisplayFormat::Json => {
            println!("{}", serde_json::to_string_pretty(block).unwrap());
        }
        DisplayFormat::Table => {
            println!("╔══════════════════════════════════════════════════════════════╗");
            println!("║                       BLOCK INFORMATION                      ║");
            println!("╠══════════════════════════════════════════════════════════════╣");
            print_field("Height", &block["height"].to_string());
            print_field("Hash", truncate(block["hash"].as_str().unwrap_or("N/A"), 60));
            print_field(
                "Previous",
                truncate(block["previous_hash"].as_str().unwrap_or("N/A"), 60),
            );
            print_field("Timestamp", &format_timestamp(block["timestamp"].as_u64().unwrap_or(0)));
            print_field(
                "Validator",
                truncate(block["validator"].as_str().unwrap_or("N/A"), 60),
            );
            print_field("Transactions", &block["transaction_count"].to_string());
            println!("╚══════════════════════════════════════════════════════════════╝");
        }
        DisplayFormat::Compact => {
            println!(
                "Block {} | Txs: {} | Validator: {}",
                block["height"],
                block["transaction_count"],
                truncate(block["validator"].as_str().unwrap_or("N/A"), 16)
            );
        }
    }
    Ok(())
}

/// Display election results with real-time refresh capability
pub fn display_election_results(results: &Value, format: DisplayFormat) -> Result<()> {
    match format {
        DisplayFormat::Json => {
            println!("{}", serde_json::to_string_pretty(results).unwrap());
        }
        DisplayFormat::Table => {
            let election_id = results["election_id"].as_str().unwrap_or("N/A");
            let total_votes = results["total_votes"].as_u64().unwrap_or(0);
            let finalized = results["finalized"].as_bool().unwrap_or(false);

            println!("╔══════════════════════════════════════════════════════════════╗");
            println!("║                      ELECTION RESULTS                        ║");
            println!("╠══════════════════════════════════════════════════════════════╣");
            print_field("Election ID", truncate(election_id, 60));
            print_field("Total Votes", &total_votes.to_string());
            print_field("Finalized", if finalized { "Yes" } else { "No" });
            println!("╠══════════════════════════════════════════════════════════════╣");
            println!("║  Rank  │  Candidate               │  Votes    │  Percentage  ║");
            println!("╠══════════════════════════════════════════════════════════════╣");

            if let Some(candidate_results) = results["results"].as_array() {
                let mut sorted: Vec<_> = candidate_results.iter().collect();
                sorted.sort_by(|a, b| {
                    b["votes"]
                        .as_u64()
                        .unwrap_or(0)
                        .cmp(&a["votes"].as_u64().unwrap_or(0))
                });

                for (i, candidate) in sorted.iter().enumerate() {
                    let name = candidate["candidate_name"].as_str().unwrap_or("Unknown");
                    let votes = candidate["votes"].as_u64().unwrap_or(0);
                    let percentage = candidate["percentage"].as_f64().unwrap_or(0.0);

                    println!(
                        "║  {:>4}  │  {:<23} │  {:>8} │  {:>6.2}%     ║",
                        i + 1,
                        truncate(name, 23),
                        votes,
                        percentage
                    );
                }
            }

            println!("╚══════════════════════════════════════════════════════════════╝");
        }
        DisplayFormat::Compact => {
            if let Some(candidate_results) = results["results"].as_array() {
                for candidate in candidate_results {
                    println!(
                        "{}: {} votes ({:.2}%)",
                        candidate["candidate_name"].as_str().unwrap_or("Unknown"),
                        candidate["votes"],
                        candidate["percentage"].as_f64().unwrap_or(0.0)
                    );
                }
            }
        }
    }
    Ok(())
}

/// Display jurisdiction-filtered results
pub fn display_jurisdiction_results(
    jurisdiction: &str,
    results: &Value,
    format: DisplayFormat,
) -> Result<()> {
    match format {
        DisplayFormat::Json => {
            println!("{}", serde_json::to_string_pretty(results).unwrap());
        }
        DisplayFormat::Table => {
            println!("╔══════════════════════════════════════════════════════════════╗");
            println!("║               JURISDICTION RESULTS: {:<26}║", truncate(jurisdiction, 26));
            println!("╠══════════════════════════════════════════════════════════════╣");
            display_election_results(results, format)?;
        }
        DisplayFormat::Compact => {
            println!("Jurisdiction: {}", jurisdiction);
            display_election_results(results, DisplayFormat::Compact)?;
        }
    }
    Ok(())
}

/// Display vote verification result
pub fn display_vote_verification(verification: &Value, format: DisplayFormat) -> Result<()> {
    match format {
        DisplayFormat::Json => {
            println!("{}", serde_json::to_string_pretty(verification).unwrap());
        }
        DisplayFormat::Table => {
            let valid = verification["valid"].as_bool().unwrap_or(false);
            let details = verification["details"].as_str().unwrap_or("N/A");

            println!("╔══════════════════════════════════════════════════════════════╗");
            println!("║                    VOTE VERIFICATION                         ║");
            println!("╠══════════════════════════════════════════════════════════════╣");
            print_field(
                "Status",
                if valid {
                    "✓ VALID"
                } else {
                    "✗ INVALID"
                },
            );
            print_field("Details", details);
            if let Some(height) = verification["block_height"].as_u64() {
                print_field("Block Height", &height.to_string());
            }
            if let Some(election_id) = verification["election_id"].as_str() {
                print_field("Election ID", truncate(election_id, 60));
            }
            println!("╚══════════════════════════════════════════════════════════════╝");
        }
        DisplayFormat::Compact => {
            println!(
                "Vote: {} | {}",
                if verification["valid"].as_bool().unwrap_or(false) {
                    "VALID"
                } else {
                    "INVALID"
                },
                verification["details"].as_str().unwrap_or("N/A")
            );
        }
    }
    Ok(())
}

/// Display anomaly detection results
pub fn display_anomalies(anomalies: &[AnomalyReport], format: DisplayFormat) -> Result<()> {
    match format {
        DisplayFormat::Json => {
            println!("{}", serde_json::to_string_pretty(anomalies).unwrap());
        }
        DisplayFormat::Table => {
            println!("╔══════════════════════════════════════════════════════════════╗");
            println!("║                     ANOMALY DETECTION                        ║");
            println!("╠══════════════════════════════════════════════════════════════╣");
            println!("║  Type        │  Location            │  Severity  │  Score    ║");
            println!("╠══════════════════════════════════════════════════════════════╣");

            for anomaly in anomalies {
                println!(
                    "║  {:<11} │  {:<19} │  {:<9} │  {:>6.2}    ║",
                    truncate(&anomaly.anomaly_type, 11),
                    truncate(&anomaly.location, 19),
                    anomaly.severity,
                    anomaly.score
                );
            }

            println!("╠══════════════════════════════════════════════════════════════╣");
            println!("║  Total Anomalies: {:<43}║", anomalies.len());
            println!("╚══════════════════════════════════════════════════════════════╝");
        }
        DisplayFormat::Compact => {
            for anomaly in anomalies {
                println!(
                    "{} at {} | Severity: {} | Score: {:.2}",
                    anomaly.anomaly_type, anomaly.location, anomaly.severity, anomaly.score
                );
            }
        }
    }
    Ok(())
}

/// Display real-time vote statistics
pub fn display_vote_stats(stats: &VoteStatistics, refresh_mode: bool) -> Result<()> {
    if refresh_mode {
        // Clear screen for refresh mode
        print!("\x1B[2J\x1B[1;1H");
    }

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    REAL-TIME VOTE STATISTICS                 ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    print_field("Total Votes Cast", &stats.total_votes.to_string());
    print_field("Votes Per Second", &format!("{:.2}", stats.votes_per_second));
    print_field("Active Elections", &stats.active_elections.to_string());
    print_field(
        "Last Update",
        &format_timestamp(stats.last_update_timestamp),
    );
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Jurisdiction Breakdown:                                     ║");
    println!("╠══════════════════════════════════════════════════════════════╣");

    for (jurisdiction, count) in &stats.votes_by_jurisdiction {
        let percentage = if stats.total_votes > 0 {
            (*count as f64 / stats.total_votes as f64) * 100.0
        } else {
            0.0
        };
        println!(
            "║  {:<30} │  {:>8} ({:>5.2}%)           ║",
            truncate(jurisdiction, 30),
            count,
            percentage
        );
    }

    println!("╚══════════════════════════════════════════════════════════════╝");

    if refresh_mode {
        println!("\nPress Ctrl+C to exit refresh mode");
    }

    Ok(())
}

/// Display progress bar for blockchain verification
pub fn display_verification_progress(current: u64, total: u64, start_time: std::time::Instant) {
    let percentage = if total > 0 {
        (current as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let elapsed = start_time.elapsed().as_secs_f64();
    let rate = if elapsed > 0.0 {
        current as f64 / elapsed
    } else {
        0.0
    };

    let eta = if rate > 0.0 {
        ((total - current) as f64 / rate) as u64
    } else {
        0
    };

    let bar_width = 50;
    let filled = ((percentage / 100.0) * bar_width as f64) as usize;
    let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);

    print!(
        "\r[{}] {:.1}% | Block {}/{} | {:.1} blocks/s | ETA: {}s  ",
        bar, percentage, current, total, rate, eta
    );
    io::stdout().flush().unwrap();
}

/// Display validator list
pub fn display_validators(validators: &Value, format: DisplayFormat) -> Result<()> {
    match format {
        DisplayFormat::Json => {
            println!("{}", serde_json::to_string_pretty(validators).unwrap());
        }
        DisplayFormat::Table => {
            let total = validators["total_validators"].as_u64().unwrap_or(0);
            let height = validators["height"].as_u64().unwrap_or(0);

            println!("╔══════════════════════════════════════════════════════════════╗");
            println!("║                      VALIDATOR SET                           ║");
            println!("╠══════════════════════════════════════════════════════════════╣");
            print_field("Height", &height.to_string());
            print_field("Total Validators", &total.to_string());
            println!("╠══════════════════════════════════════════════════════════════╣");
            println!("║  Address (truncated)      │  Stake      │  Blocks  │  Active ║");
            println!("╠══════════════════════════════════════════════════════════════╣");

            if let Some(validator_list) = validators["validators"].as_array() {
                for validator in validator_list {
                    let addr = validator["address"].as_str().unwrap_or("N/A");
                    let stake = validator["stake"].as_u64().unwrap_or(0);
                    let blocks = validator["blocks_produced"].as_u64().unwrap_or(0);
                    let active = validator["is_active"].as_bool().unwrap_or(false);

                    println!(
                        "║  {:<24} │  {:>10} │  {:>7} │  {:<6} ║",
                        truncate(addr, 24),
                        stake,
                        blocks,
                        if active { "Yes" } else { "No" }
                    );
                }
            }

            println!("╚══════════════════════════════════════════════════════════════╝");
        }
        DisplayFormat::Compact => {
            if let Some(validator_list) = validators["validators"].as_array() {
                for validator in validator_list {
                    println!(
                        "{} | Stake: {} | Blocks: {}",
                        truncate(validator["address"].as_str().unwrap_or("N/A"), 20),
                        validator["stake"],
                        validator["blocks_produced"]
                    );
                }
            }
        }
    }
    Ok(())
}

/// Anomaly report structure
#[derive(Debug, Clone)]
pub struct AnomalyReport {
    pub anomaly_type: String,
    pub location: String,
    pub severity: String,
    pub score: f64,
    pub description: String,
}

/// Vote statistics for real-time display
#[derive(Debug, Clone)]
pub struct VoteStatistics {
    pub total_votes: u64,
    pub votes_per_second: f64,
    pub active_elections: usize,
    pub votes_by_jurisdiction: HashMap<String, u64>,
    pub last_update_timestamp: u64,
}

// Helper functions

fn print_field(label: &str, value: &str) {
    println!("║  {:<28} │  {:<31}║", label, value);
}

fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[..max_len]
    }
}

fn format_timestamp(timestamp: u64) -> String {
    use chrono::{DateTime, Utc};
    let dt = DateTime::<Utc>::from_timestamp(timestamp as i64, 0).unwrap_or_default();
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{}d {}h {}m {}s", days, hours, minutes, secs)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

/// Display error message
pub fn display_error(error: &VotingError) {
    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║                          ERROR                               ║");
    eprintln!("╠══════════════════════════════════════════════════════════════╣");
    eprintln!("║  {:<58}  ║", truncate(&error.to_string(), 58));
    eprintln!("╚══════════════════════════════════════════════════════════════╝");
}

/// Display success message
pub fn display_success(message: &str) {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                         SUCCESS                              ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  {:<58}  ║", truncate(message, 58));
    println!("╚══════════════════════════════════════════════════════════════╝");
}

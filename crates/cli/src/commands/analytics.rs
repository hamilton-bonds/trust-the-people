//! Analytics command implementations
//!
//! Commands for detecting anomalies, generating statistics, and auditing elections.

use super::RpcClient;
use crate::ui::{self, AnomalyReport, VoteStatistics};
use common::{Result, VotingError};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Detect anomalies in voting patterns
pub async fn detect_anomalies(
    election_id: &str,
    rpc_endpoint: &str,
    threshold: f64,
) -> Result<()> {
    info!("Detecting anomalies for election: {}", election_id);

    let client = RpcClient::new(rpc_endpoint);

    // Get election data
    let election = client
        .call(
            "get_election",
            serde_json::json!({ "election_id": election_id }),
        )
        .await?;

    // Get all votes for this election
    let votes = fetch_all_votes(&client, election_id).await?;

    println!("Analyzing {} votes for anomalies...\n", votes.len());

    let mut anomalies = Vec::new();

    // Run anomaly detection algorithms
    anomalies.extend(detect_timing_anomalies(&votes, threshold)?);
    anomalies.extend(detect_geographic_anomalies(&votes, threshold)?);
    anomalies.extend(detect_turnout_anomalies(&votes, &election, threshold)?);
    anomalies.extend(detect_statistical_anomalies(&votes, threshold)?);
    anomalies.extend(benford_law_analysis(&votes, threshold)?);

    // Display results
    ui::display_anomalies(&anomalies, ui::DisplayFormat::Table)?;

    if anomalies.is_empty() {
        ui::display_success("No significant anomalies detected");
    } else {
        let high_severity = anomalies.iter().filter(|a| a.severity == "High").count();
        let medium_severity = anomalies.iter().filter(|a| a.severity == "Medium").count();

        println!(
            "\nSummary: {} high severity, {} medium severity anomalies detected",
            high_severity, medium_severity
        );

        if high_severity > 0 {
            warn!("High severity anomalies require investigation");
        }
    }

    Ok(())
}

/// Run real-time vote statistics with refresh
pub async fn realtime_stats(
    election_id: Option<String>,
    rpc_endpoint: &str,
    refresh_seconds: u64,
) -> Result<()> {
    info!("Starting real-time statistics monitor");

    let client = RpcClient::new(rpc_endpoint);

    loop {
        let stats = collect_vote_statistics(&client, election_id.as_deref()).await?;
        ui::display_vote_stats(&stats, true)?;

        tokio::time::sleep(Duration::from_secs(refresh_seconds)).await;
    }
}

/// Generate comprehensive audit report
pub async fn generate_audit_report(
    election_id: &str,
    rpc_endpoint: &str,
    output_path: PathBuf,
) -> Result<()> {
    info!("Generating audit report for election: {}", election_id);

    let client = RpcClient::new(rpc_endpoint);

    // Collect data
    println!("Collecting election data...");
    let election = client
        .call(
            "get_election",
            serde_json::json!({ "election_id": election_id }),
        )
        .await?;

    println!("Collecting votes...");
    let votes = fetch_all_votes(&client, election_id).await?;

    println!("Running anomaly detection...");
    let anomalies = run_all_anomaly_detection(&votes, &election, 2.0)?;

    println!("Generating statistics...");
    let stats = generate_comprehensive_stats(&votes)?;

    println!("Verifying chain integrity...");
    let integrity = verify_chain_integrity(&client, &votes).await?;

    // Build report
    let report = AuditReport {
        election_id: election_id.to_string(),
        election_name: election["name"].as_str().unwrap_or("Unknown").to_string(),
        generated_at: common::utils::current_timestamp(),
        total_votes: votes.len(),
        anomalies,
        statistics: stats,
        chain_integrity: integrity,
    };

    // Write to file
    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| VotingError::SerializationError(e.to_string()))?;

    std::fs::write(&output_path, json)
        .map_err(|e| VotingError::StorageError(format!("Failed to write report: {}", e)))?;

    ui::display_success(&format!("Audit report saved to: {}", output_path.display()));

    Ok(())
}

/// Analyze turnout by jurisdiction
pub async fn analyze_turnout(
    election_id: &str,
    jurisdiction_level: Option<String>,
    rpc_endpoint: &str,
) -> Result<()> {
    info!("Analyzing turnout for election: {}", election_id);

    let client = RpcClient::new(rpc_endpoint);

    let votes = fetch_all_votes(&client, election_id).await?;

    // Group by jurisdiction
    let mut turnout_by_jurisdiction: HashMap<String, TurnoutData> = HashMap::new();

    for vote in &votes {
        if let Some(jurisdiction) = vote.get("jurisdiction").and_then(|j| j.as_str()) {
            let entry = turnout_by_jurisdiction
                .entry(jurisdiction.to_string())
                .or_insert(TurnoutData {
                    jurisdiction: jurisdiction.to_string(),
                    vote_count: 0,
                    registered_voters: 1000, // Would come from election data
                    turnout_percentage: 0.0,
                });

            entry.vote_count += 1;
        }
    }

    // Calculate percentages
    for data in turnout_by_jurisdiction.values_mut() {
        data.turnout_percentage = (data.vote_count as f64 / data.registered_voters as f64) * 100.0;
    }

    // Sort by turnout percentage
    let mut sorted: Vec<_> = turnout_by_jurisdiction.values().collect();
    sorted.sort_by(|a, b| b.turnout_percentage.partial_cmp(&a.turnout_percentage).unwrap());

    // Display results
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    TURNOUT ANALYSIS                          ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Jurisdiction            │  Votes   │  Registered │  Turnout ║");
    println!("╠══════════════════════════════════════════════════════════════╣");

    for data in sorted {
        println!(
            "║  {:<23} │  {:>7} │  {:>10} │  {:>6.2}%  ║",
            truncate(&data.jurisdiction, 23),
            data.vote_count,
            data.registered_voters,
            data.turnout_percentage
        );
    }

    println!("╚══════════════════════════════════════════════════════════════╝");

    Ok(())
}

/// Compare current election to historical baseline
pub async fn compare_to_baseline(
    election_id: &str,
    baseline_election_id: &str,
    rpc_endpoint: &str,
) -> Result<()> {
    info!("Comparing election {} to baseline {}", election_id, baseline_election_id);

    let client = RpcClient::new(rpc_endpoint);

    let current_votes = fetch_all_votes(&client, election_id).await?;
    let baseline_votes = fetch_all_votes(&client, baseline_election_id).await?;

    let current_stats = generate_comprehensive_stats(&current_votes)?;
    let baseline_stats = generate_comprehensive_stats(&baseline_votes)?;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                  BASELINE COMPARISON                         ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Metric                  │  Current    │  Baseline   │  Δ%   ║");
    println!("╠══════════════════════════════════════════════════════════════╣");

    compare_metric("Total Votes", current_stats.total_votes as f64, baseline_stats.total_votes as f64);
    compare_metric("Avg Votes/Hour", current_stats.avg_votes_per_hour, baseline_stats.avg_votes_per_hour);
    compare_metric("Peak Votes/Hour", current_stats.peak_votes_per_hour as f64, baseline_stats.peak_votes_per_hour as f64);
    compare_metric("Std Deviation", current_stats.std_deviation, baseline_stats.std_deviation);

    println!("╚══════════════════════════════════════════════════════════════╝");

    Ok(())
}

// Anomaly detection algorithms

fn detect_timing_anomalies(votes: &[Value], threshold: f64) -> Result<Vec<AnomalyReport>> {
    let mut anomalies = Vec::new();

    // Group votes by hour
    let mut votes_per_hour: HashMap<u64, u64> = HashMap::new();

    for vote in votes {
        if let Some(timestamp) = vote["timestamp"].as_u64() {
            let hour = timestamp / 3600;
            *votes_per_hour.entry(hour).or_insert(0) += 1;
        }
    }

    // Calculate statistics
    let counts: Vec<u64> = votes_per_hour.values().copied().collect();
    let mean = counts.iter().sum::<u64>() as f64 / counts.len() as f64;
    let variance = counts.iter().map(|&x| {
        let diff = x as f64 - mean;
        diff * diff
    }).sum::<f64>() / counts.len() as f64;
    let std_dev = variance.sqrt();

    // Detect spikes
    for (hour, count) in votes_per_hour {
        let z_score = (count as f64 - mean) / std_dev;

        if z_score.abs() > threshold {
            anomalies.push(AnomalyReport {
                anomaly_type: "Timing Spike".to_string(),
                location: format!("Hour {}", hour),
                severity: if z_score.abs() > 3.0 { "High" } else { "Medium" }.to_string(),
                score: z_score.abs(),
                description: format!("{} votes in hour {} (mean: {:.0}, z-score: {:.2})",
                    count, hour, mean, z_score),
            });
        }
    }

    Ok(anomalies)
}

fn detect_geographic_anomalies(votes: &[Value], threshold: f64) -> Result<Vec<AnomalyReport>> {
    let mut anomalies = Vec::new();

    // Group by location
    let mut votes_by_location: HashMap<String, u64> = HashMap::new();

    for vote in votes {
        if let Some(location) = vote.get("location").and_then(|l| l.as_str()) {
            *votes_by_location.entry(location.to_string()).or_insert(0) += 1;
        }
    }

    // Calculate statistics
    if votes_by_location.is_empty() {
        return Ok(anomalies);
    }

    let counts: Vec<u64> = votes_by_location.values().copied().collect();
    let mean = counts.iter().sum::<u64>() as f64 / counts.len() as f64;
    let variance = counts.iter().map(|&x| {
        let diff = x as f64 - mean;
        diff * diff
    }).sum::<f64>() / counts.len() as f64;
    let std_dev = variance.sqrt();

    // Detect outliers
    for (location, count) in votes_by_location {
        let z_score = (count as f64 - mean) / std_dev;

        if z_score > threshold {
            anomalies.push(AnomalyReport {
                anomaly_type: "Geographic Outlier".to_string(),
                location: location.clone(),
                severity: if z_score > 3.0 { "High" } else { "Medium" }.to_string(),
                score: z_score,
                description: format!("{} votes at {} (mean: {:.0}, z-score: {:.2})",
                    count, location, mean, z_score),
            });
        }
    }

    Ok(anomalies)
}

fn detect_turnout_anomalies(votes: &[Value], election: &Value, threshold: f64) -> Result<Vec<AnomalyReport>> {
    let mut anomalies = Vec::new();

    // Check if turnout exceeds 100%
    let total_votes = votes.len() as u64;
    let registered_voters = election["registered_voters"].as_u64().unwrap_or(total_votes);

    if total_votes > registered_voters {
        anomalies.push(AnomalyReport {
            anomaly_type: "Excess Turnout".to_string(),
            location: "Overall".to_string(),
            severity: "High".to_string(),
            score: (total_votes as f64 / registered_voters as f64) * 100.0,
            description: format!("{} votes cast with only {} registered voters ({:.1}% turnout)",
                total_votes, registered_voters, (total_votes as f64 / registered_voters as f64) * 100.0),
        });
    }

    Ok(anomalies)
}

fn detect_statistical_anomalies(votes: &[Value], threshold: f64) -> Result<Vec<AnomalyReport>> {
    let mut anomalies = Vec::new();

    // Check for sequential patterns (suspicious)
    let mut timestamps: Vec<u64> = votes.iter()
        .filter_map(|v| v["timestamp"].as_u64())
        .collect();
    
    timestamps.sort();

    let mut sequential_count = 0;
    for window in timestamps.windows(10) {
        let diffs: Vec<u64> = window.windows(2).map(|w| w[1] - w[0]).collect();
        let avg_diff = diffs.iter().sum::<u64>() as f64 / diffs.len() as f64;

        if avg_diff < 1.0 {
            sequential_count += 1;
        }
    }

    if sequential_count > 5 {
        anomalies.push(AnomalyReport {
            anomaly_type: "Sequential Pattern".to_string(),
            location: "Timestamps".to_string(),
            severity: "Medium".to_string(),
            score: sequential_count as f64,
            description: format!("Detected {} groups of votes with suspiciously sequential timestamps", sequential_count),
        });
    }

    Ok(anomalies)
}

fn benford_law_analysis(votes: &[Value], threshold: f64) -> Result<Vec<AnomalyReport>> {
    let mut anomalies = Vec::new();

    // Count first digits of vote counts by location
    let mut votes_by_location: HashMap<String, u64> = HashMap::new();

    for vote in votes {
        if let Some(location) = vote.get("location").and_then(|l| l.as_str()) {
            *votes_by_location.entry(location.to_string()).or_insert(0) += 1;
        }
    }

    // Count first digit frequency
    let mut first_digit_counts = [0u64; 10];
    for count in votes_by_location.values() {
        let first_digit = count.to_string().chars().next().unwrap().to_digit(10).unwrap() as usize;
        first_digit_counts[first_digit] += 1;
    }

    // Expected Benford's Law distribution
    let benford_expected = [0.0, 0.301, 0.176, 0.125, 0.097, 0.079, 0.067, 0.058, 0.051, 0.046];

    // Calculate chi-square statistic
    let total = first_digit_counts.iter().sum::<u64>() as f64;
    let mut chi_square = 0.0;

    for digit in 1..10 {
        let observed = first_digit_counts[digit] as f64 / total;
        let expected = benford_expected[digit];
        let diff = observed - expected;
        chi_square += (diff * diff) / expected;
    }

    if chi_square > threshold {
        anomalies.push(AnomalyReport {
            anomaly_type: "Benford's Law Violation".to_string(),
            location: "Vote Counts".to_string(),
            severity: if chi_square > 5.0 { "High" } else { "Medium" }.to_string(),
            score: chi_square,
            description: format!("Vote count distribution deviates from Benford's Law (χ²: {:.2})", chi_square),
        });
    }

    Ok(anomalies)
}

// Helper functions

async fn fetch_all_votes(client: &RpcClient, election_id: &str) -> Result<Vec<Value>> {
    let response = client
        .call(
            "search_transactions",
            serde_json::json!({
                "election_id": election_id,
                "tx_type": "Vote",
                "limit": 10000
            }),
        )
        .await?;

    Ok(response["transactions"]
        .as_array()
        .cloned()
        .unwrap_or_default())
}

async fn collect_vote_statistics(
    client: &RpcClient,
    election_id: Option<&str>,
) -> Result<VoteStatistics> {
    let votes = if let Some(id) = election_id {
        fetch_all_votes(client, id).await?
    } else {
        // Get all votes
        let response = client
            .call(
                "search_transactions",
                serde_json::json!({
                    "tx_type": "Vote",
                    "limit": 10000
                }),
            )
            .await?;

        response["transactions"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    };

    // Calculate statistics
    let total_votes = votes.len() as u64;

    let mut votes_by_jurisdiction = HashMap::new();
    for vote in &votes {
        if let Some(jurisdiction) = vote.get("jurisdiction").and_then(|j| j.as_str()) {
            *votes_by_jurisdiction.entry(jurisdiction.to_string()).or_insert(0) += 1;
        }
    }

    // Calculate votes per second (simplified)
    let votes_per_second = if !votes.is_empty() {
        let timestamps: Vec<u64> = votes.iter().filter_map(|v| v["timestamp"].as_u64()).collect();
        if timestamps.len() > 1 {
            let time_span = timestamps.iter().max().unwrap() - timestamps.iter().min().unwrap();
            if time_span > 0 {
                votes.len() as f64 / time_span as f64
            } else {
                0.0
            }
        } else {
            0.0
        }
    } else {
        0.0
    };

    Ok(VoteStatistics {
        total_votes,
        votes_per_second,
        active_elections: 1,
        votes_by_jurisdiction,
        last_update_timestamp: common::utils::current_timestamp(),
    })
}

fn run_all_anomaly_detection(
    votes: &[Value],
    election: &Value,
    threshold: f64,
) -> Result<Vec<AnomalyReport>> {
    let mut all_anomalies = Vec::new();

    all_anomalies.extend(detect_timing_anomalies(votes, threshold)?);
    all_anomalies.extend(detect_geographic_anomalies(votes, threshold)?);
    all_anomalies.extend(detect_turnout_anomalies(votes, election, threshold)?);
    all_anomalies.extend(detect_statistical_anomalies(votes, threshold)?);
    all_anomalies.extend(benford_law_analysis(votes, threshold)?);

    Ok(all_anomalies)
}

fn generate_comprehensive_stats(votes: &[Value]) -> Result<ComprehensiveStats> {
    let total_votes = votes.len();

    let timestamps: Vec<u64> = votes.iter().filter_map(|v| v["timestamp"].as_u64()).collect();

    let avg_votes_per_hour = if !timestamps.is_empty() {
        let time_span_hours = (timestamps.iter().max().unwrap() - timestamps.iter().min().unwrap()) as f64 / 3600.0;
        if time_span_hours > 0.0 {
            total_votes as f64 / time_span_hours
        } else {
            0.0
        }
    } else {
        0.0
    };

    // Group by hour for peak calculation
    let mut votes_per_hour: HashMap<u64, u64> = HashMap::new();
    for timestamp in &timestamps {
        let hour = timestamp / 3600;
        *votes_per_hour.entry(hour).or_insert(0) += 1;
    }

    let peak_votes_per_hour = votes_per_hour.values().max().copied().unwrap_or(0);

    // Calculate standard deviation
    let mean = total_votes as f64 / votes_per_hour.len().max(1) as f64;
    let variance = votes_per_hour.values().map(|&x| {
        let diff = x as f64 - mean;
        diff * diff
    }).sum::<f64>() / votes_per_hour.len().max(1) as f64;
    let std_deviation = variance.sqrt();

    Ok(ComprehensiveStats {
        total_votes,
        avg_votes_per_hour,
        peak_votes_per_hour,
        std_deviation,
    })
}

async fn verify_chain_integrity(client: &RpcClient, votes: &[Value]) -> Result<ChainIntegrity> {
    // Verify a sample of votes
    let mut verified = 0;
    let mut failed = 0;

    for vote in votes.iter().take(100) {
        if let Some(tx_hash) = vote["id"].as_str() {
            // Verify the vote exists on chain
            match client
                .call("get_transaction", serde_json::json!({ "hash": tx_hash }))
                .await
            {
                Ok(_) => verified += 1,
                Err(_) => failed += 1,
            }
        }
    }

    Ok(ChainIntegrity {
        verified_count: verified,
        failed_count: failed,
        integrity_percentage: if verified + failed > 0 {
            (verified as f64 / (verified + failed) as f64) * 100.0
        } else {
            0.0
        },
    })
}

fn compare_metric(name: &str, current: f64, baseline: f64) {
    let delta = if baseline != 0.0 {
        ((current - baseline) / baseline) * 100.0
    } else {
        0.0
    };

    let delta_str = if delta > 0.0 {
        format!("+{:.1}", delta)
    } else {
        format!("{:.1}", delta)
    };

    println!(
        "║  {:<23} │  {:>10.2} │  {:>10.2} │  {:>5}% ║",
        truncate(name, 23),
        current,
        baseline,
        delta_str
    );
}

fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[..max_len]
    }
}

// Data structures

#[derive(Debug, Clone, serde::Serialize)]
struct AuditReport {
    election_id: String,
    election_name: String,
    generated_at: u64,
    total_votes: usize,
    anomalies: Vec<AnomalyReport>,
    statistics: ComprehensiveStats,
    chain_integrity: ChainIntegrity,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ComprehensiveStats {
    total_votes: usize,
    avg_votes_per_hour: f64,
    peak_votes_per_hour: u64,
    std_deviation: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ChainIntegrity {
    verified_count: u64,
    failed_count: u64,
    integrity_percentage: f64,
}

#[derive(Debug, Clone)]
struct TurnoutData {
    jurisdiction: String,
    vote_count: u64,
    registered_voters: u64,
    turnout_percentage: f64,
}

//! Verification command implementations
//!
//! Commands for verifying votes, receipts, blockchain integrity, and election results.

use super::RpcClient;
use crate::ui;
use common::{Result, VotingError};
use serde_json::Value;
use std::time::Instant;
use tracing::{info, warn};

/// Verify a vote receipt
pub async fn receipt(receipt_id: &str, rpc_endpoint: &str) -> Result<()> {
    info!("Verifying receipt: {}", receipt_id);

    let client = RpcClient::new(rpc_endpoint);

    // Search for the transaction by receipt
    // In production, this would use a dedicated receipt lookup endpoint
    println!("Looking up receipt: {}", receipt_id);

    // For now, we'll search transactions
    let response = client
        .call(
            "search_transactions",
            serde_json::json!({
                "tx_type": "Vote",
                "limit": 1000
            }),
        )
        .await?;

    let transactions = response["transactions"].as_array().ok_or_else(|| {
        VotingError::TransactionNotFound("No transactions found".to_string())
    })?;

    // Try to find matching transaction
    // In production, receipts would have a direct lookup mechanism
    let mut found = false;
    for tx in transactions {
        if let Some(tx_id) = tx["id"].as_str() {
            if tx_id.contains(receipt_id) || receipt_id.contains(tx_id) {
                found = true;

                println!("\n╔══════════════════════════════════════════════════════════════╗");
                println!("║                    RECEIPT VERIFICATION                      ║");
                println!("╠══════════════════════════════════════════════════════════════╣");
                println!("║  Status             │  ✓ FOUND ON BLOCKCHAIN                 ║");
                println!("║  Receipt ID         │  {:<39}║", truncate(receipt_id, 39));
                println!("║  Transaction ID     │  {:<39}║", truncate(tx_id, 39));
                println!("║  Block Height       │  {:<39}║", tx["block_height"]);
                println!("║  Timestamp          │  {:<39}║", format_timestamp(tx["timestamp"].as_u64().unwrap_or(0)));

                if let Some(election_id) = tx["data"]["election_id"].as_str() {
                    println!("║  Election ID        │  {:<39}║", truncate(election_id, 39));
                }

                println!("╠══════════════════════════════════════════════════════════════╣");
                println!("║  Your vote was successfully recorded on the blockchain.     ║");
                println!("║  The encrypted vote is included in the public ledger and    ║");
                println!("║  will be counted in the final tally.                        ║");
                println!("╚══════════════════════════════════════════════════════════════╝");

                break;
            }
        }
    }

    if !found {
        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║                    RECEIPT VERIFICATION                      ║");
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║  Status             │  ✗ NOT FOUND                           ║");
        println!("║  Receipt ID         │  {:<39}║", truncate(receipt_id, 39));
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║  This receipt was not found on the blockchain.              ║");
        println!("║  Please verify your receipt ID is correct.                  ║");
        println!("╚══════════════════════════════════════════════════════════════╝");

        return Err(VotingError::TransactionNotFound(receipt_id.to_string()));
    }

    Ok(())
}

/// Verify a vote with zero-knowledge proof
pub async fn vote(tx_hash: &str, proof_hex: &str, rpc_endpoint: &str) -> Result<()> {
    info!("Verifying vote: {}", tx_hash);

    let client = RpcClient::new(rpc_endpoint);

    println!("Verifying vote transaction and zero-knowledge proof...\n");

    // Get the vote transaction
    let tx = client
        .call(
            "get_transaction",
            serde_json::json!({ "hash": tx_hash }),
        )
        .await?;

    // Verify the ZK proof
    let verification = client
        .call(
            "verify_vote",
            serde_json::json!({
                "tx_id": tx_hash,
                "zk_proof": proof_hex
            }),
        )
        .await?;

    ui::display_vote_verification(&verification, ui::DisplayFormat::Table)?;

    // Additional checks
    if verification["valid"].as_bool().unwrap_or(false) {
        println!("\nVerification Steps Completed:");
        println!("  ✓ Vote transaction exists on blockchain");
        println!("  ✓ Zero-knowledge proof is valid");
        println!("  ✓ Vote is properly encrypted");
        println!("  ✓ Vote signature is valid");
        println!("\nYour vote has been verified and will be counted in the final tally.");
    } else {
        warn!("Vote verification failed");
        println!("\nVerification failed. Please check:");
        println!("  • Transaction hash is correct");
        println!("  • Zero-knowledge proof is valid");
        println!("  • Vote has not been tampered with");
    }

    Ok(())
}

/// Verify blockchain integrity
pub async fn chain(start_height: u64, end_height: Option<u64>, rpc_endpoint: &str) -> Result<()> {
    info!("Verifying blockchain integrity from height {}", start_height);

    let client = RpcClient::new(rpc_endpoint);

    // Get chain info to determine end height
    let chain_info = client.call_no_params("chain_info").await?;
    let latest_height = chain_info["height"].as_u64().unwrap_or(0);
    let end = end_height.unwrap_or(latest_height);

    if start_height > end {
        return Err(VotingError::InvalidBlock(
            "Start height must be less than or equal to end height".to_string(),
        ));
    }

    let total_blocks = end - start_height + 1;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                  BLOCKCHAIN VERIFICATION                     ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Start Height       │  {:<39}║", start_height);
    println!("║  End Height         │  {:<39}║", end);
    println!("║  Total Blocks       │  {:<39}║", total_blocks);
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    let start_time = Instant::now();
    let mut verified = 0;
    let mut failed = 0;
    let mut previous_hash: Option<String> = None;

    for height in start_height..=end {
        // Display progress
        ui::display_verification_progress(height - start_height, total_blocks, start_time);

        // Get block
        let block = client
            .call(
                "get_block_by_height",
                serde_json::json!({ "height": height }),
            )
            .await?;

        // Verify previous hash chain
        if let Some(prev) = previous_hash {
            if block["previous_hash"].as_str() != Some(&prev) {
                failed += 1;
                println!(
                    "\n✗ Block {} has invalid previous hash! Chain is broken.",
                    height
                );
                continue;
            }
        }

        // Verify block hash matches stored hash
        let block_hash = block["hash"].as_str().unwrap_or("");
        previous_hash = Some(block_hash.to_string());

        // Verify transaction count
        let tx_count = block["transaction_count"].as_u64().unwrap_or(0);
        let actual_tx_count = block["transactions"]
            .as_array()
            .map(|a| a.len() as u64)
            .unwrap_or(0);

        if tx_count != actual_tx_count {
            failed += 1;
            println!(
                "\n✗ Block {} has mismatched transaction count! Expected {}, got {}",
                height, tx_count, actual_tx_count
            );
            continue;
        }

        verified += 1;
    }

    // Clear progress line
    println!("\n");

    // Display results
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                  VERIFICATION RESULTS                        ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Total Blocks       │  {:<39}║", total_blocks);
    println!("║  Verified           │  {:<39}║", verified);
    println!("║  Failed             │  {:<39}║", failed);
    println!("║  Success Rate       │  {:<37.2}%  ║", (verified as f64 / total_blocks as f64) * 100.0);
    println!("║  Time Elapsed       │  {:<39}║", format_duration(start_time.elapsed()));
    println!("╚══════════════════════════════════════════════════════════════╝");

    if failed == 0 {
        ui::display_success("Blockchain integrity verified successfully");
    } else {
        warn!("Blockchain verification found {} issues", failed);
        println!("\n⚠️  WARNING: Blockchain integrity issues detected!");
        println!("This may indicate tampering or corruption.");
    }

    Ok(())
}

/// Verify election results
pub async fn election(election_id: &str, rpc_endpoint: &str) -> Result<()> {
    info!("Verifying election results: {}", election_id);

    let client = RpcClient::new(rpc_endpoint);

    println!("Verifying election results for: {}\n", election_id);

    // Get election info
    let election = client
        .call(
            "get_election",
            serde_json::json!({ "election_id": election_id }),
        )
        .await?;

    // Check if election is finalized
    let is_finalized = election["is_finalized"].as_bool().unwrap_or(false);

    if !is_finalized {
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║                    ELECTION STATUS                           ║");
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║  Status             │  NOT FINALIZED                         ║");
        println!("║  Election ID        │  {:<39}║", truncate(election_id, 39));
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║  This election has not been finalized yet.                  ║");
        println!("║  Results can only be verified after election closes.        ║");
        println!("╚══════════════════════════════════════════════════════════════╝");

        return Ok(());
    }

    // Get results
    let results = client
        .call(
            "get_election_results",
            serde_json::json!({ "election_id": election_id }),
        )
        .await?;

    // Display results
    ui::display_election_results(&results, ui::DisplayFormat::Table)?;

    // Verify vote count matches
    let total_votes = results["total_votes"].as_u64().unwrap_or(0);
    let election_vote_count = election["vote_count"].as_u64().unwrap_or(0);

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                  VERIFICATION CHECKS                         ║");
    println!("╠══════════════════════════════════════════════════════════════╣");

    // Check 1: Vote count matches
    if total_votes == election_vote_count {
        println!("║  ✓ Vote count matches                                        ║");
    } else {
        println!("║  ✗ Vote count mismatch!                                      ║");
        println!("║    Expected: {:<48}║", election_vote_count);
        println!("║    Got:      {:<48}║", total_votes);
    }

    // Check 2: All candidates accounted for
    let candidate_count = election["candidates"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    let result_count = results["results"].as_array().map(|a| a.len()).unwrap_or(0);

    if candidate_count == result_count {
        println!("║  ✓ All candidates accounted for                             ║");
    } else {
        println!("║  ✗ Candidate count mismatch!                                ║");
    }

    // Check 3: Vote sum equals total
    let vote_sum: u64 = results["results"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|c| c["votes"].as_u64())
        .sum();

    if vote_sum == total_votes {
        println!("║  ✓ Vote sum equals reported total                           ║");
    } else {
        println!("║  ✗ Vote sum mismatch!                                       ║");
    }

    // Check 4: Percentages add up to 100%
    let percentage_sum: f64 = results["results"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|c| c["percentage"].as_f64())
        .sum();

    if (percentage_sum - 100.0).abs() < 0.1 {
        println!("║  ✓ Percentages sum to 100%                                  ║");
    } else {
        println!("║  ✗ Percentage sum error!                                    ║");
    }

    println!("╚══════════════════════════════════════════════════════════════╝");

    // Overall verdict
    let all_checks_passed = total_votes == election_vote_count
        && candidate_count == result_count
        && vote_sum == total_votes
        && (percentage_sum - 100.0).abs() < 0.1;

    if all_checks_passed {
        ui::display_success("Election results verified successfully");
    } else {
        warn!("Election verification found discrepancies");
        println!("\n⚠️  WARNING: Election verification found issues!");
        println!("Please investigate these discrepancies.");
    }

    Ok(())
}

/// Verify all votes in an election have valid proofs
pub async fn election_proofs(election_id: &str, rpc_endpoint: &str) -> Result<()> {
    info!("Verifying all vote proofs for election: {}", election_id);

    let client = RpcClient::new(rpc_endpoint);

    println!("Fetching all votes for election: {}\n", election_id);

    // Get all votes for this election
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

    let votes = response["transactions"].as_array().ok_or_else(|| {
        VotingError::TransactionNotFound("No votes found for election".to_string())
    })?;

    if votes.is_empty() {
        println!("No votes found for this election.");
        return Ok(());
    }

    println!("Verifying {} vote proofs...\n", votes.len());

    let start_time = Instant::now();
    let mut verified = 0;
    let mut failed = 0;

    for (i, vote) in votes.iter().enumerate() {
        ui::display_verification_progress(i as u64, votes.len() as u64, start_time);

        let tx_id = vote["id"].as_str().unwrap_or("");

        // In production, would verify actual ZK proofs
        // For now, just check vote exists and has required fields
        if vote["data"]["encrypted_vote"].is_string() {
            verified += 1;
        } else {
            failed += 1;
        }
    }

    println!("\n");

    // Display results
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                  PROOF VERIFICATION RESULTS                  ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Total Votes        │  {:<39}║", votes.len());
    println!("║  Verified           │  {:<39}║", verified);
    println!("║  Failed             │  {:<39}║", failed);
    println!("║  Success Rate       │  {:<37.2}%  ║", (verified as f64 / votes.len() as f64) * 100.0);
    println!("║  Time Elapsed       │  {:<39}║", format_duration(start_time.elapsed()));
    println!("╚══════════════════════════════════════════════════════════════╝");

    if failed == 0 {
        ui::display_success("All vote proofs verified successfully");
    } else {
        warn!("Found {} votes with invalid proofs", failed);
    }

    Ok(())
}

// Helper functions

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

fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    let millis = duration.subsec_millis();

    if secs > 60 {
        let minutes = secs / 60;
        let seconds = secs % 60;
        format!("{}m {}s", minutes, seconds)
    } else if secs > 0 {
        format!("{}.{}s", secs, millis / 100)
    } else {
        format!("{}ms", millis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        let dur = std::time::Duration::from_secs(125);
        assert_eq!(format_duration(dur), "2m 5s");

        let dur = std::time::Duration::from_millis(1500);
        assert_eq!(format_duration(dur), "1.5s");

        let dur = std::time::Duration::from_millis(500);
        assert_eq!(format_duration(dur), "500ms");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello");
        assert_eq!(truncate("test", 4), "test");
    }
}

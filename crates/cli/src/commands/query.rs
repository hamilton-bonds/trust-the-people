//! Query command implementations
//!
//! Commands for querying blockchain data, elections, validators, and results.

use super::RpcClient;
use crate::ui;
use common::{Result, VotingError};
use serde_json::Value;
use tracing::info;

/// Query block by hash or height
pub async fn block(identifier: &str, rpc_endpoint: &str) -> Result<()> {
    info!("Querying block: {}", identifier);

    let client = RpcClient::new(rpc_endpoint);

    // Try to parse as height first
    let block = if let Ok(height) = identifier.parse::<u64>() {
        client
            .call(
                "get_block_by_height",
                serde_json::json!({ "height": height }),
            )
            .await?
    } else {
        // Assume it's a hash
        client
            .call("get_block", serde_json::json!({ "hash": identifier }))
            .await?
    };

    ui::display_block(&block, ui::DisplayFormat::Table)?;

    // Display transactions if present
    if let Some(transactions) = block["transactions"].as_array() {
        if !transactions.is_empty() {
            println!("\n╔══════════════════════════════════════════════════════════════╗");
            println!("║                        TRANSACTIONS                          ║");
            println!("╠══════════════════════════════════════════════════════════════╣");
            println!("║  #  │  Type            │  Transaction ID                     ║");
            println!("╠══════════════════════════════════════════════════════════════╣");

            for (i, tx) in transactions.iter().enumerate() {
                let tx_type = tx["tx_type"].as_str().unwrap_or("Unknown");
                let tx_id = tx["id"].as_str().unwrap_or("N/A");

                println!(
                    "║  {:>2} │  {:<15} │  {:<33} ║",
                    i + 1,
                    truncate(tx_type, 15),
                    truncate(tx_id, 33)
                );
            }

            println!("╚══════════════════════════════════════════════════════════════╝");
        }
    }

    Ok(())
}

/// Query transaction by hash
pub async fn transaction(hash: &str, rpc_endpoint: &str) -> Result<()> {
    info!("Querying transaction: {}", hash);

    let client = RpcClient::new(rpc_endpoint);

    let tx = client
        .call("get_transaction", serde_json::json!({ "hash": hash }))
        .await?;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                   TRANSACTION INFORMATION                    ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║  Transaction ID     │  {:<39}║",
        truncate(tx["id"].as_str().unwrap_or("N/A"), 39)
    );
    println!(
        "║  Type               │  {:<39}║",
        tx["tx_type"].as_str().unwrap_or("N/A")
    );
    println!(
        "║  Block Height       │  {:<39}║",
        tx["block_height"]
    );
    println!(
        "║  Block Hash         │  {:<39}║",
        truncate(tx["block_hash"].as_str().unwrap_or("N/A"), 39)
    );
    println!(
        "║  Timestamp          │  {:<39}║",
        format_timestamp(tx["timestamp"].as_u64().unwrap_or(0))
    );
    println!("╠══════════════════════════════════════════════════════════════╣");

    // Display type-specific data
    if let Some(data) = tx["data"].as_object() {
        println!("║  Transaction Data:                                           ║");
        println!("╠══════════════════════════════════════════════════════════════╣");

        match tx["tx_type"].as_str().unwrap_or("") {
            "Vote" => {
                if let Some(election_id) = data.get("election_id") {
                    println!(
                        "║  Election ID        │  {:<39}║",
                        truncate(election_id.as_str().unwrap_or("N/A"), 39)
                    );
                }
                if let Some(encrypted) = data.get("encrypted_vote") {
                    println!(
                        "║  Encrypted Vote     │  {:<39}║",
                        truncate(encrypted.as_str().unwrap_or("N/A"), 39)
                    );
                }
                println!("║  ZK Proof           │  Present                               ║");
            }
            "CreateElection" => {
                if let Some(election) = data.get("election") {
                    if let Some(name) = election.get("name") {
                        println!(
                            "║  Election Name      │  {:<39}║",
                            truncate(name.as_str().unwrap_or("N/A"), 39)
                        );
                    }
                    if let Some(start) = election.get("start_time") {
                        println!(
                            "║  Start Time         │  {:<39}║",
                            format_timestamp(start.as_u64().unwrap_or(0))
                        );
                    }
                    if let Some(end) = election.get("end_time") {
                        println!(
                            "║  End Time           │  {:<39}║",
                            format_timestamp(end.as_u64().unwrap_or(0))
                        );
                    }
                }
            }
            "RegisterValidator" => {
                if let Some(validator) = data.get("validator") {
                    if let Some(address) = validator.get("address") {
                        println!(
                            "║  Validator Address  │  {:<39}║",
                            truncate(address.as_str().unwrap_or("N/A"), 39)
                        );
                    }
                    if let Some(stake) = validator.get("stake") {
                        println!("║  Stake              │  {:<39}║", stake);
                    }
                }
            }
            _ => {}
        }
    }

    println!("╚══════════════════════════════════════════════════════════════╝");

    Ok(())
}

/// Query election information
pub async fn election(election_id: &str, rpc_endpoint: &str) -> Result<()> {
    info!("Querying election: {}", election_id);

    let client = RpcClient::new(rpc_endpoint);

    let election = client
        .call(
            "get_election",
            serde_json::json!({ "election_id": election_id }),
        )
        .await?;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    ELECTION INFORMATION                      ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║  Election ID        │  {:<39}║",
        truncate(election["id"].as_str().unwrap_or("N/A"), 39)
    );
    println!(
        "║  Name               │  {:<39}║",
        truncate(election["name"].as_str().unwrap_or("N/A"), 39)
    );

    if let Some(description) = election["description"].as_str() {
        println!(
            "║  Description        │  {:<39}║",
            truncate(description, 39)
        );
    }

    println!(
        "║  Start Time         │  {:<39}║",
        format_timestamp(election["start_time"].as_u64().unwrap_or(0))
    );
    println!(
        "║  End Time           │  {:<39}║",
        format_timestamp(election["end_time"].as_u64().unwrap_or(0))
    );
    println!(
        "║  Vote Count         │  {:<39}║",
        election["vote_count"]
    );

    let is_active = election["is_active"].as_bool().unwrap_or(false);
    let is_finalized = election["is_finalized"].as_bool().unwrap_or(false);

    println!(
        "║  Active             │  {:<39}║",
        if is_active { "Yes" } else { "No" }
    );
    println!(
        "║  Finalized          │  {:<39}║",
        if is_finalized { "Yes" } else { "No" }
    );
    println!(
        "║  Created at Height  │  {:<39}║",
        election["created_at_height"]
    );

    if let Some(closed) = election["closed_at_height"].as_u64() {
        println!("║  Closed at Height   │  {:<39}║", closed);
    }

    println!("╠══════════════════════════════════════════════════════════════╣");

    // Display jurisdiction if present
    if let Some(jurisdiction) = election["jurisdiction"].as_object() {
        println!("║  Jurisdiction:                                               ║");
        println!(
            "║    Level            │  {:<39}║",
            jurisdiction["level"].as_str().unwrap_or("N/A")
        );
        println!(
            "║    Name             │  {:<39}║",
            truncate(jurisdiction["name"].as_str().unwrap_or("N/A"), 39)
        );
        println!("╠══════════════════════════════════════════════════════════════╣");
    }

    // Display candidates
    if let Some(candidates) = election["candidates"].as_array() {
        println!("║  Candidates:                                                 ║");
        println!("╠══════════════════════════════════════════════════════════════╣");

        for (i, candidate) in candidates.iter().enumerate() {
            let name = candidate["name"].as_str().unwrap_or("Unknown");
            let party = candidate["party"]
                .as_str()
                .map(|p| format!(" ({})", p))
                .unwrap_or_default();

            println!(
                "║  {}. {:<54} ║",
                i + 1,
                truncate(&format!("{}{}", name, party), 54)
            );
        }
    }

    println!("╚══════════════════════════════════════════════════════════════╝");

    Ok(())
}

/// Query election results
pub async fn results(election_id: &str, rpc_endpoint: &str) -> Result<()> {
    info!("Querying election results: {}", election_id);

    let client = RpcClient::new(rpc_endpoint);

    let results = client
        .call(
            "get_election_results",
            serde_json::json!({ "election_id": election_id }),
        )
        .await?;

    ui::display_election_results(&results, ui::DisplayFormat::Table)?;

    Ok(())
}

/// Query validator information
pub async fn validator(address: &str, rpc_endpoint: &str) -> Result<()> {
    info!("Querying validator: {}", address);

    let client = RpcClient::new(rpc_endpoint);

    let validator = client
        .call(
            "get_validator",
            serde_json::json!({ "address": address }),
        )
        .await?;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                   VALIDATOR INFORMATION                      ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║  Address            │  {:<39}║",
        truncate(validator["address"].as_str().unwrap_or("N/A"), 39)
    );
    println!(
        "║  Public Key         │  {:<39}║",
        truncate(validator["public_key"].as_str().unwrap_or("N/A"), 39)
    );
    println!("║  Stake              │  {:<39}║", validator["stake"]);

    let is_active = validator["is_active"].as_bool().unwrap_or(false);
    println!(
        "║  Active             │  {:<39}║",
        if is_active { "Yes" } else { "No" }
    );

    println!(
        "║  Registered Height  │  {:<39}║",
        validator["registered_at_height"]
    );
    println!(
        "║  Blocks Produced    │  {:<39}║",
        validator["blocks_produced"]
    );

    if let Some(last_block) = validator["last_block_height"].as_u64() {
        println!("║  Last Block Height  │  {:<39}║", last_block);
    }

    println!("╚══════════════════════════════════════════════════════════════╝");

    Ok(())
}

/// Query all validators
pub async fn validators(height: Option<u64>, rpc_endpoint: &str) -> Result<()> {
    info!("Querying validators");

    let client = RpcClient::new(rpc_endpoint);

    let params = if let Some(h) = height {
        serde_json::json!({ "height": h })
    } else {
        serde_json::json!({})
    };

    let validators = client.call("get_validators", params).await?;

    ui::display_validators(&validators, ui::DisplayFormat::Table)?;

    Ok(())
}

/// Query chain information
pub async fn chain(rpc_endpoint: &str) -> Result<()> {
    info!("Querying chain information");

    let client = RpcClient::new(rpc_endpoint);

    let chain_info = client.call_no_params("chain_info").await?;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    BLOCKCHAIN INFORMATION                    ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Height             │  {:<39}║", chain_info["height"]);
    println!(
        "║  Genesis Hash       │  {:<39}║",
        truncate(chain_info["genesis_hash"].as_str().unwrap_or("N/A"), 39)
    );
    println!(
        "║  Latest Block Hash  │  {:<39}║",
        truncate(
            chain_info["latest_block_hash"].as_str().unwrap_or("N/A"),
            39
        )
    );
    println!(
        "║  Latest Block Time  │  {:<39}║",
        format_timestamp(chain_info["latest_block_time"].as_u64().unwrap_or(0))
    );
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!(
        "║  Total Transactions │  {:<39}║",
        chain_info["total_transactions"]
    );
    println!(
        "║  Total Votes        │  {:<39}║",
        chain_info["total_votes"]
    );
    println!(
        "║  Total Validators   │  {:<39}║",
        chain_info["total_validators"]
    );
    println!(
        "║  Active Validators  │  {:<39}║",
        chain_info["active_validators"]
    );
    println!(
        "║  Active Elections   │  {:<39}║",
        chain_info["active_elections"]
    );
    println!("╚══════════════════════════════════════════════════════════════╝");

    Ok(())
}

/// Search blocks with filters
pub async fn search_blocks(
    start_height: Option<u64>,
    end_height: Option<u64>,
    validator: Option<String>,
    limit: Option<usize>,
    rpc_endpoint: &str,
) -> Result<()> {
    info!("Searching blocks");

    let client = RpcClient::new(rpc_endpoint);

    let mut query = serde_json::Map::new();
    if let Some(start) = start_height {
        query.insert("start_height".to_string(), serde_json::json!(start));
    }
    if let Some(end) = end_height {
        query.insert("end_height".to_string(), serde_json::json!(end));
    }
    if let Some(val) = validator {
        query.insert("validator".to_string(), serde_json::json!(val));
    }
    if let Some(lim) = limit {
        query.insert("limit".to_string(), serde_json::json!(lim));
    }

    let results = client
        .call("search_blocks", serde_json::Value::Object(query))
        .await?;

    let blocks = results["blocks"].as_array().ok_or_else(|| {
        VotingError::BlockNotFound("No blocks found".to_string())
    })?;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                       BLOCK SEARCH RESULTS                   ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Total Results      │  {:<39}║", results["total"]);
    println!("║  Returned           │  {:<39}║", blocks.len());
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Height │  Hash (truncated)       │  Txs  │  Timestamp       ║");
    println!("╠══════════════════════════════════════════════════════════════╣");

    for block in blocks {
        println!(
            "║  {:>6} │  {:<21} │  {:>4} │  {:<15} ║",
            block["height"],
            truncate(block["hash"].as_str().unwrap_or("N/A"), 21),
            block["transaction_count"],
            format_timestamp_short(block["timestamp"].as_u64().unwrap_or(0))
        );
    }

    println!("╚══════════════════════════════════════════════════════════════╝");

    Ok(())
}

/// Search transactions with filters
pub async fn search_transactions(
    tx_type: Option<String>,
    election_id: Option<String>,
    start_height: Option<u64>,
    limit: Option<usize>,
    rpc_endpoint: &str,
) -> Result<()> {
    info!("Searching transactions");

    let client = RpcClient::new(rpc_endpoint);

    let mut query = serde_json::Map::new();
    if let Some(t) = tx_type {
        query.insert("tx_type".to_string(), serde_json::json!(t));
    }
    if let Some(e) = election_id {
        query.insert("election_id".to_string(), serde_json::json!(e));
    }
    if let Some(start) = start_height {
        query.insert("start_height".to_string(), serde_json::json!(start));
    }
    if let Some(lim) = limit {
        query.insert("limit".to_string(), serde_json::json!(lim));
    }

    let results = client
        .call("search_transactions", serde_json::Value::Object(query))
        .await?;

    let transactions = results["transactions"].as_array().ok_or_else(|| {
        VotingError::TransactionNotFound("No transactions found".to_string())
    })?;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                  TRANSACTION SEARCH RESULTS                  ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Total Results      │  {:<39}║", results["total"]);
    println!("║  Returned           │  {:<39}║", transactions.len());
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Type       │  Transaction ID (truncated)  │  Block Height   ║");
    println!("╠══════════════════════════════════════════════════════════════╣");

    for tx in transactions {
        println!(
            "║  {:<10} │  {:<27} │  {:>14} ║",
            truncate(tx["tx_type"].as_str().unwrap_or("N/A"), 10),
            truncate(tx["id"].as_str().unwrap_or("N/A"), 27),
            tx["block_height"]
        );
    }

    println!("╚══════════════════════════════════════════════════════════════╝");

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

fn format_timestamp_short(timestamp: u64) -> String {
    use chrono::{DateTime, Utc};
    let dt = DateTime::<Utc>::from_timestamp(timestamp as i64, 0).unwrap_or_default();
    dt.format("%Y-%m-%d %H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello");
    }
}

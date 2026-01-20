//! Command implementations for the CLI
//!
//! This module organizes all CLI commands into logical categories:
//! - node: Node operations (start, stop, status, info)
//! - query: Blockchain queries (blocks, transactions, elections)
//! - verify: Verification operations (receipts, votes, chain integrity)
//! - wallet: Wallet operations (key generation, inspection)

pub mod node;
pub mod query;
pub mod verify;
pub mod wallet;

use common::{Result, VotingError};
use serde_json::Value;

/// RPC client for making requests to the node
pub struct RpcClient {
    endpoint: String,
    client: reqwest::Client,
}

impl RpcClient {
    /// Create a new RPC client
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Make a JSON-RPC request
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });

        let response = self
            .client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await
            .map_err(|e| VotingError::NetworkError(format!("RPC request failed: {}", e)))?;

        let body: Value = response
            .json()
            .await
            .map_err(|e| VotingError::NetworkError(format!("Failed to parse response: {}", e)))?;

        if let Some(error) = body.get("error") {
            return Err(VotingError::NetworkError(format!(
                "RPC error: {}",
                error["message"].as_str().unwrap_or("Unknown error")
            )));
        }

        body.get("result")
            .cloned()
            .ok_or_else(|| VotingError::NetworkError("No result in response".to_string()))
    }

    /// Call with no parameters
    pub async fn call_no_params(&self, method: &str) -> Result<Value> {
        self.call(method, serde_json::json!({})).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_client_creation() {
        let client = RpcClient::new("http://localhost:8545");
        assert_eq!(client.endpoint, "http://localhost:8545");
    }
}

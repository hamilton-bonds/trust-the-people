//! RPC method implementations organized by category
//!
//! This module contains the actual implementation of all RPC methods,
//! organized into logical categories for maintainability.

use crate::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, RequestId, RpcService};
use std::sync::Arc;
use tracing::{debug, error};

pub mod blockchain;
pub mod network;
pub mod node;
pub mod validation;

/// Main RPC method handler that routes requests to appropriate handlers
pub struct RpcMethodHandler {
    service: Arc<dyn RpcService>,
}

impl RpcMethodHandler {
    /// Create a new RPC method handler
    pub fn new(service: Arc<dyn RpcService>) -> Self {
        Self { service }
    }

    /// Handle an incoming JSON-RPC request
    pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        debug!("Handling RPC method: {}", request.method);

        let result = match request.method.as_str() {
            // Node methods
            "node_info" => self.handle_node_info().await,
            "health" => self.handle_health().await,
            "sync_status" => self.handle_sync_status().await,

            // Blockchain methods
            "chain_info" => self.handle_chain_info().await,
            "get_block" => self.handle_get_block(&request.params).await,
            "get_block_by_height" => self.handle_get_block_by_height(&request.params).await,
            "get_transaction" => self.handle_get_transaction(&request.params).await,
            "search_blocks" => self.handle_search_blocks(&request.params).await,
            "search_transactions" => self.handle_search_transactions(&request.params).await,

            // Voting methods
            "get_vote" => self.handle_get_vote(&request.params).await,
            "get_election" => self.handle_get_election(&request.params).await,
            "get_election_results" => self.handle_get_election_results(&request.params).await,
            "submit_transaction" => self.handle_submit_transaction(&request.params).await,

            // Validation methods
            "verify_vote" => self.handle_verify_vote(&request.params).await,
            "get_validators" => self.handle_get_validators(&request.params).await,
            "get_validator" => self.handle_get_validator(&request.params).await,

            // Network methods
            "get_peers" => self.handle_get_peers().await,
            "get_mempool" => self.handle_get_mempool().await,

            // Unknown method
            _ => Err(JsonRpcError::method_not_found(&request.method)),
        };

        match result {
            Ok(value) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(value),
                error: None,
                id: request.id,
            },
            Err(error) => {
                error!("RPC method '{}' failed: {}", request.method, error.message);
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(error),
                    id: request.id,
                }
            }
        }
    }

    // Node methods
    async fn handle_node_info(&self) -> Result<serde_json::Value, JsonRpcError> {
        let info = self.service.node_info().await?;
        Ok(serde_json::to_value(info)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    async fn handle_health(&self) -> Result<serde_json::Value, JsonRpcError> {
        let health = self.service.health().await?;
        Ok(serde_json::to_value(health)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    async fn handle_sync_status(&self) -> Result<serde_json::Value, JsonRpcError> {
        let status = self.service.sync_status().await?;
        Ok(serde_json::to_value(status)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    // Blockchain methods
    async fn handle_chain_info(&self) -> Result<serde_json::Value, JsonRpcError> {
        let info = self.service.chain_info().await?;
        Ok(serde_json::to_value(info)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    async fn handle_get_block(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let hash = extract_string_param(params, "hash")?;
        let block = self.service.get_block(hash).await?;
        Ok(serde_json::to_value(block)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    async fn handle_get_block_by_height(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let height = extract_u64_param(params, "height")?;
        let block = self.service.get_block_by_height(height).await?;
        Ok(serde_json::to_value(block)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    async fn handle_get_transaction(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let hash = extract_string_param(params, "hash")?;
        let tx = self.service.get_transaction(hash).await?;
        Ok(serde_json::to_value(tx)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    async fn handle_search_blocks(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let query = serde_json::from_value(params.clone())
            .map_err(|e| JsonRpcError::invalid_params(format!("Invalid query: {}", e)))?;
        let results = self.service.search_blocks(query).await?;
        Ok(serde_json::to_value(results)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    async fn handle_search_transactions(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let query = serde_json::from_value(params.clone())
            .map_err(|e| JsonRpcError::invalid_params(format!("Invalid query: {}", e)))?;
        let results = self.service.search_transactions(query).await?;
        Ok(serde_json::to_value(results)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    // Voting methods
    async fn handle_get_vote(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let tx_hash = extract_string_param(params, "tx_hash")?;
        let vote = self.service.get_vote(tx_hash).await?;
        Ok(serde_json::to_value(vote)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    async fn handle_get_election(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let election_id = extract_string_param(params, "election_id")?;
        let election = self.service.get_election(election_id).await?;
        Ok(serde_json::to_value(election)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    async fn handle_get_election_results(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let election_id = extract_string_param(params, "election_id")?;
        let results = self.service.get_election_results(election_id).await?;
        Ok(serde_json::to_value(results)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    async fn handle_submit_transaction(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let tx_hex = extract_string_param(params, "tx_hex")?;
        let response = self.service.submit_transaction(tx_hex).await?;
        Ok(serde_json::to_value(response)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    // Validation methods
    async fn handle_verify_vote(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let request = serde_json::from_value(params.clone())
            .map_err(|e| JsonRpcError::invalid_params(format!("Invalid request: {}", e)))?;
        let response = self.service.verify_vote(request).await?;
        Ok(serde_json::to_value(response)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    async fn handle_get_validators(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let height = if params.is_null() || params.as_object().map_or(true, |o| o.is_empty()) {
            None
        } else {
            Some(extract_u64_param(params, "height")?)
        };
        let validators = self.service.get_validators(height).await?;
        Ok(serde_json::to_value(validators)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    async fn handle_get_validator(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        let address = extract_string_param(params, "address")?;
        let validator = self.service.get_validator(address).await?;
        Ok(serde_json::to_value(validator)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    // Network methods
    async fn handle_get_peers(&self) -> Result<serde_json::Value, JsonRpcError> {
        let peers = self.service.get_peers().await?;
        Ok(serde_json::to_value(peers)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }

    async fn handle_get_mempool(&self) -> Result<serde_json::Value, JsonRpcError> {
        let mempool = self.service.get_mempool().await?;
        Ok(serde_json::to_value(mempool)
            .map_err(|e| JsonRpcError::internal_error(format!("Serialization error: {}", e)))?)
    }
}

/// Extract string parameter from JSON params
fn extract_string_param(params: &serde_json::Value, key: &str) -> Result<String, JsonRpcError> {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| JsonRpcError::invalid_params(format!("Missing or invalid parameter: {}", key)))
}

/// Extract u64 parameter from JSON params
fn extract_u64_param(params: &serde_json::Value, key: &str) -> Result<u64, JsonRpcError> {
    params
        .get(key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| JsonRpcError::invalid_params(format!("Missing or invalid parameter: {}", key)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_string_param() {
        let params = json!({"hash": "abc123"});
        let result = extract_string_param(&params, "hash").unwrap();
        assert_eq!(result, "abc123");
    }

    #[test]
    fn test_extract_string_param_missing() {
        let params = json!({});
        let result = extract_string_param(&params, "hash");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_u64_param() {
        let params = json!({"height": 1000});
        let result = extract_u64_param(&params, "height").unwrap();
        assert_eq!(result, 1000);
    }

    #[test]
    fn test_extract_u64_param_invalid() {
        let params = json!({"height": "not_a_number"});
        let result = extract_u64_param(&params, "height");
        assert!(result.is_err());
    }
}

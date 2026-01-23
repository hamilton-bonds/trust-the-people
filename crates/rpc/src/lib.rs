//! RPC server implementation for the voting node.
//!
//! This module provides JSON-RPC 2.0 API endpoints for interacting with the voting node.
//! It supports queries for blocks, transactions, votes, validators, and chain state.

use common::errors::VotingError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod methods;
pub mod server;
pub mod types;
pub mod service;

pub use server::RpcServer;
pub use types::*;

/// RPC API version
pub const RPC_VERSION: &str = "1.0";

/// Maximum number of items to return in a single query
pub const MAX_QUERY_LIMIT: usize = 100;

/// Default query limit if none specified
pub const DEFAULT_QUERY_LIMIT: usize = 20;

/// RPC service trait that defines all available RPC methods
#[async_trait::async_trait]
pub trait RpcService: Send + Sync {
    /// Get node information
    async fn node_info(&self) -> Result<NodeInfo, VotingError>;

    /// Get blockchain information
    async fn chain_info(&self) -> Result<ChainInfo, VotingError>;

    /// Get block by hash
    async fn get_block(&self, hash: String) -> Result<BlockResponse, VotingError>;
    
    /// Get block by height
    async fn get_block_by_height(&self, height: u64) -> Result<BlockResponse, VotingError>;

    /// Get transaction by hash
    async fn get_transaction(&self, hash: String) -> Result<TransactionResponse, VotingError>;

    /// Get vote by transaction hash
    async fn get_vote(&self, tx_hash: String) -> Result<VoteResponse, VotingError>;

    /// Get election information
    async fn get_election(&self, election_id: String) -> Result<ElectionResponse, VotingError>;

    /// Get election results
    async fn get_election_results(
        &self,
        election_id: String,
    ) -> Result<ElectionResultsResponse, VotingError>;

    /// Get validator set at specific height
    async fn get_validators(
        &self,
        height: Option<u64>,
    ) -> Result<ValidatorSetResponse, VotingError>;

    /// Get validator by address
    async fn get_validator(
        &self,
        address: String,
    ) -> Result<ValidatorResponse, VotingError>;

    /// Verify vote proof
    async fn verify_vote(&self, request: VerifyVoteRequest) -> Result<VerifyVoteResponse, VotingError>;

    /// Submit transaction (broadcast to network)
    async fn submit_transaction(&self, tx_hex: String) -> Result<SubmitResponse, VotingError>;

    /// Get pending transactions in mempool
    async fn get_mempool(&self) -> Result<MempoolResponse, VotingError>;

    /// Get network peers
    async fn get_peers(&self) -> Result<PeersResponse, VotingError>;

    /// Get node sync status
    async fn sync_status(&self) -> Result<SyncStatusResponse, VotingError>;

    /// Search blocks by criteria
    async fn search_blocks(&self, query: BlockQuery) -> Result<BlockSearchResponse, VotingError>;

    /// Search transactions by criteria
    async fn search_transactions(
        &self,
        query: TransactionQuery,
    ) -> Result<TransactionSearchResponse, VotingError>;

    /// Get health check
    async fn health(&self) -> Result<HealthResponse, VotingError>;
}

/// RPC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    /// Listen address
    pub listen_addr: String,

    /// Port to bind to
    pub port: u16,

    /// Enable CORS
    pub enable_cors: bool,

    /// Allowed CORS origins
    pub cors_origins: Vec<String>,

    /// Request timeout in seconds
    pub request_timeout: u64,

    /// Maximum request size in bytes
    pub max_request_size: usize,

    /// Enable rate limiting
    pub enable_rate_limit: bool,

    /// Rate limit: requests per second per IP
    pub rate_limit_per_second: u32,

    /// Enable authentication
    pub enable_auth: bool,

    /// API key for authentication
    pub api_key: Option<String>,

    /// Enable metrics endpoint
    pub enable_metrics: bool,

    /// TLS configuration
    pub tls: Option<TlsConfig>,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1".to_string(),
            port: 8545,
            enable_cors: true,
            cors_origins: vec!["*".to_string()],
            request_timeout: 30,
            max_request_size: 1024 * 1024, // 1MB
            enable_rate_limit: true,
            rate_limit_per_second: 100,
            enable_auth: false,
            api_key: None,
            enable_metrics: true,
            tls: None,
        }
    }
}

/// TLS configuration for RPC server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Path to certificate file
    pub cert_path: String,

    /// Path to private key file
    pub key_path: String,
}

/// JSON-RPC 2.0 request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC version (must be "2.0")
    pub jsonrpc: String,

    /// Method name
    pub method: String,

    /// Parameters (can be array or object)
    #[serde(default)]
    pub params: serde_json::Value,

    /// Request ID
    pub id: RequestId,
}

/// JSON-RPC 2.0 response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC version
    pub jsonrpc: String,

    /// Result (only present on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,

    /// Error (only present on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,

    /// Request ID
    pub id: RequestId,
}

/// JSON-RPC request ID (can be string, number, or null)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum RequestId {
    String(String),
    Number(i64),
    Null,
}

/// JSON-RPC error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code
    pub code: i32,

    /// Error message
    pub message: String,

    /// Additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    /// Create parse error
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self {
            code: -32700,
            message: message.into(),
            data: None,
        }
    }

    /// Create invalid request error
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            code: -32600,
            message: message.into(),
            data: None,
        }
    }

    /// Create method not found error
    pub fn method_not_found(method: impl Into<String>) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {}", method.into()),
            data: None,
        }
    }

    /// Create invalid params error
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
            data: None,
        }
    }

    /// Create internal error
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: message.into(),
            data: None,
        }
    }

    /// Create custom error
    pub fn custom(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
}

impl From<VotingError> for JsonRpcError {
    fn from(err: VotingError) -> Self {
        match err {
            VotingError::InvalidTransaction(_) => {
                Self::invalid_params(err.to_string())
            }
            VotingError::BlockNotFound(_) => {
                Self::custom(-32001, err.to_string())
            }
            VotingError::TransactionNotFound(_) => {
                Self::custom(-32002, err.to_string())
            }
            VotingError::InvalidBlock(_) => {
                Self::invalid_params(err.to_string())
            }
            VotingError::ConsensusError(_) => {
                Self::internal_error(err.to_string())
            }
            _ => Self::internal_error(err.to_string()),
        }
    }
}

/// Rate limiter for RPC requests
pub struct RateLimiter {
    requests: Arc<RwLock<std::collections::HashMap<String, (u32, std::time::Instant)>>>,
    limit_per_second: u32,
}

impl RateLimiter {
    /// Create new rate limiter
    pub fn new(limit_per_second: u32) -> Self {
        Self {
            requests: Arc::new(RwLock::new(std::collections::HashMap::new())),
            limit_per_second,
        }
    }

    /// Check if request is allowed for given IP
    pub async fn is_allowed(&self, ip: &str) -> bool {
        let mut requests = self.requests.write().await;
        let now = std::time::Instant::now();

        if let Some((count, timestamp)) = requests.get_mut(ip) {
            // Reset counter if more than 1 second has passed
            if now.duration_since(*timestamp).as_secs() >= 1 {
                *count = 1;
                *timestamp = now;
                true
            } else if *count < self.limit_per_second {
                *count += 1;
                true
            } else {
                false
            }
        } else {
            requests.insert(ip.to_string(), (1, now));
            true
        }
    }

    /// Clean up old entries periodically
    pub async fn cleanup(&self) {
        let mut requests = self.requests.write().await;
        let now = std::time::Instant::now();
        
        requests.retain(|_, (_, timestamp)| {
            now.duration_since(*timestamp).as_secs() < 60
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_config_default() {
        let config = RpcConfig::default();
        assert_eq!(config.listen_addr, "127.0.0.1");
        assert_eq!(config.port, 8545);
        assert!(config.enable_cors);
        assert!(config.enable_rate_limit);
        assert!(!config.enable_auth);
    }

    #[test]
    fn test_json_rpc_errors() {
        let err = JsonRpcError::parse_error("Invalid JSON");
        assert_eq!(err.code, -32700);

        let err = JsonRpcError::method_not_found("unknown_method");
        assert_eq!(err.code, -32601);

        let err = JsonRpcError::invalid_params("Missing parameter");
        assert_eq!(err.code, -32602);
    }

    #[test]
    fn test_request_id_serialization() {
        let id = RequestId::String("test-123".to_string());
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#""test-123""#);

        let id = RequestId::Number(42);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "42");

        let id = RequestId::Null;
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "null");
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new(2);
        
        // First two requests should succeed
        assert!(limiter.is_allowed("127.0.0.1").await);
        assert!(limiter.is_allowed("127.0.0.1").await);
        
        // Third request should be blocked
        assert!(!limiter.is_allowed("127.0.0.1").await);
        
        // Different IP should be allowed
        assert!(limiter.is_allowed("127.0.0.2").await);
        
        // Wait for reset
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        assert!(limiter.is_allowed("127.0.0.1").await);
    }
}

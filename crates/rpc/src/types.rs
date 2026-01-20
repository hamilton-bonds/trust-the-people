//! RPC response types and query structures
//!
//! This module defines all the response types returned by RPC methods
//! and query structures used for filtering and searching.

use common::{Address, BlockHash, ElectionId, PublicKey, Signature, Timestamp, TxId};
use serde::{Deserialize, Serialize};

/// Node information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Node version
    pub version: String,

    /// Node type (validator, full, light)
    pub node_type: String,

    /// Node public key
    pub public_key: String,

    /// Network ID
    pub network_id: String,

    /// Current blockchain height
    pub height: u64,

    /// Latest block hash
    pub latest_block_hash: String,

    /// Number of connected peers
    pub peer_count: usize,

    /// Whether node is syncing
    pub syncing: bool,

    /// Uptime in seconds
    pub uptime: u64,
}

/// Blockchain information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainInfo {
    /// Current blockchain height
    pub height: u64,

    /// Genesis block hash
    pub genesis_hash: String,

    /// Latest block hash
    pub latest_block_hash: String,

    /// Latest block timestamp
    pub latest_block_time: Timestamp,

    /// Total number of transactions
    pub total_transactions: u64,

    /// Total number of votes
    pub total_votes: u64,

    /// Total number of validators
    pub total_validators: usize,

    /// Active validator count
    pub active_validators: usize,

    /// Number of active elections
    pub active_elections: usize,
}

/// Block response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockResponse {
    /// Block height
    pub height: u64,

    /// Block hash
    pub hash: String,

    /// Previous block hash
    pub previous_hash: String,

    /// Block timestamp
    pub timestamp: Timestamp,

    /// Validator who produced the block
    pub validator: String,

    /// Merkle root of transactions
    pub transactions_root: String,

    /// Number of transactions in block
    pub transaction_count: u32,

    /// List of transactions
    pub transactions: Vec<TransactionSummary>,

    /// Block signature
    pub signature: String,
}

/// Transaction summary (lightweight)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSummary {
    /// Transaction ID
    pub id: String,

    /// Transaction type
    pub tx_type: String,

    /// Block height where transaction is included
    pub block_height: u64,

    /// Transaction timestamp
    pub timestamp: Timestamp,
}

/// Full transaction response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResponse {
    /// Transaction ID
    pub id: String,

    /// Transaction type
    pub tx_type: String,

    /// Block height
    pub block_height: u64,

    /// Block hash
    pub block_hash: String,

    /// Transaction timestamp
    pub timestamp: Timestamp,

    /// Transaction data (type-specific)
    pub data: serde_json::Value,

    /// Transaction signature
    pub signature: String,
}

/// Vote response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteResponse {
    /// Transaction ID
    pub tx_id: String,

    /// Election ID
    pub election_id: String,

    /// Encrypted vote data
    pub encrypted_vote: String,

    /// Vote timestamp
    pub timestamp: Timestamp,

    /// Block height where vote was recorded
    pub block_height: u64,

    /// Zero-knowledge proof (if applicable)
    pub zk_proof: Option<String>,

    /// Verification status
    pub verified: bool,
}

/// Election response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectionResponse {
    /// Election ID
    pub id: String,

    /// Election name
    pub name: String,

    /// Election description
    pub description: Option<String>,

    /// Start timestamp
    pub start_time: Timestamp,

    /// End timestamp
    pub end_time: Timestamp,

    /// Current vote count
    pub vote_count: u64,

    /// Whether election is active
    pub is_active: bool,

    /// Whether election is finalized
    pub is_finalized: bool,

    /// Block height when election was created
    pub created_at_height: u64,

    /// Block height when election was closed
    pub closed_at_height: Option<u64>,

    /// Candidates
    pub candidates: Vec<Candidate>,

    /// Jurisdiction
    pub jurisdiction: Option<JurisdictionInfo>,
}

/// Candidate information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    /// Candidate ID
    pub id: String,

    /// Candidate name
    pub name: String,

    /// Party affiliation
    pub party: Option<String>,

    /// Current vote count (if available)
    pub vote_count: Option<u64>,
}

/// Jurisdiction information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JurisdictionInfo {
    /// Jurisdiction level
    pub level: String,

    /// Jurisdiction name
    pub name: String,

    /// Parent jurisdiction
    pub parent: Option<String>,
}

/// Election results response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectionResultsResponse {
    /// Election ID
    pub election_id: String,

    /// Total votes cast
    pub total_votes: u64,

    /// Results by candidate
    pub results: Vec<CandidateResult>,

    /// Whether results are finalized
    pub finalized: bool,

    /// Verification proof
    pub verification_proof: Option<String>,
}

/// Candidate result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateResult {
    /// Candidate ID
    pub candidate_id: String,

    /// Candidate name
    pub candidate_name: String,

    /// Vote count
    pub votes: u64,

    /// Percentage of total votes
    pub percentage: f64,
}

/// Validator response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorResponse {
    /// Validator address
    pub address: String,

    /// Validator public key
    pub public_key: String,

    /// Validator stake
    pub stake: u64,

    /// Whether validator is active
    pub is_active: bool,

    /// Block height when validator was registered
    pub registered_at_height: u64,

    /// Total blocks produced
    pub blocks_produced: u64,

    /// Last block height produced
    pub last_block_height: Option<u64>,
}

/// Validator set response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorSetResponse {
    /// Block height for this validator set
    pub height: u64,

    /// Total number of validators
    pub total_validators: usize,

    /// Active validators
    pub validators: Vec<ValidatorResponse>,
}

/// Verify vote request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyVoteRequest {
    /// Transaction ID of the vote
    pub tx_id: String,

    /// Zero-knowledge proof
    pub zk_proof: String,

    /// Optional voter signature for verification
    pub voter_signature: Option<String>,
}

/// Verify vote response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyVoteResponse {
    /// Whether vote is valid
    pub valid: bool,

    /// Verification details
    pub details: Option<String>,

    /// Block height where vote was recorded
    pub block_height: Option<u64>,

    /// Election ID
    pub election_id: Option<String>,
}

/// Submit transaction response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitResponse {
    /// Transaction ID
    pub tx_id: String,

    /// Whether transaction was accepted
    pub accepted: bool,

    /// Error message if rejected
    pub error: Option<String>,
}

/// Mempool response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolResponse {
    /// Number of pending transactions
    pub pending_count: usize,

    /// Pending transactions
    pub transactions: Vec<TransactionSummary>,
}

/// Network peer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer ID
    pub peer_id: String,

    /// Peer address
    pub address: String,

    /// Connection direction (inbound or outbound)
    pub direction: String,

    /// Peer's current height
    pub height: u64,

    /// Connection duration in seconds
    pub connected_duration: u64,
}

/// Peers response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeersResponse {
    /// Total peer count
    pub total_peers: usize,

    /// Connected peers
    pub peers: Vec<PeerInfo>,
}

/// Sync status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatusResponse {
    /// Whether node is syncing
    pub syncing: bool,

    /// Current height
    pub current_height: u64,

    /// Target height (highest known)
    pub target_height: u64,

    /// Sync progress percentage
    pub progress: f64,

    /// Estimated time remaining in seconds
    pub estimated_time_remaining: Option<u64>,
}

/// Block query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockQuery {
    /// Start height (inclusive)
    pub start_height: Option<u64>,

    /// End height (inclusive)
    pub end_height: Option<u64>,

    /// Validator address filter
    pub validator: Option<String>,

    /// Minimum timestamp
    pub min_timestamp: Option<Timestamp>,

    /// Maximum timestamp
    pub max_timestamp: Option<Timestamp>,

    /// Maximum number of results
    pub limit: Option<usize>,

    /// Offset for pagination
    pub offset: Option<usize>,
}

/// Block search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockSearchResponse {
    /// Total number of matching blocks
    pub total: usize,

    /// Returned blocks
    pub blocks: Vec<BlockResponse>,

    /// Whether there are more results
    pub has_more: bool,
}

/// Transaction query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionQuery {
    /// Transaction type filter
    pub tx_type: Option<String>,

    /// Start height (inclusive)
    pub start_height: Option<u64>,

    /// End height (inclusive)
    pub end_height: Option<u64>,

    /// Election ID filter
    pub election_id: Option<String>,

    /// Minimum timestamp
    pub min_timestamp: Option<Timestamp>,

    /// Maximum timestamp
    pub max_timestamp: Option<Timestamp>,

    /// Maximum number of results
    pub limit: Option<usize>,

    /// Offset for pagination
    pub offset: Option<usize>,
}

/// Transaction search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSearchResponse {
    /// Total number of matching transactions
    pub total: usize,

    /// Returned transactions
    pub transactions: Vec<TransactionResponse>,

    /// Whether there are more results
    pub has_more: bool,
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Overall health status
    pub status: String,

    /// Database health
    pub database: String,

    /// Network health
    pub network: String,

    /// Consensus health
    pub consensus: String,

    /// Timestamp of check
    pub timestamp: Timestamp,
}

impl Default for BlockQuery {
    fn default() -> Self {
        Self {
            start_height: None,
            end_height: None,
            validator: None,
            min_timestamp: None,
            max_timestamp: None,
            limit: Some(crate::DEFAULT_QUERY_LIMIT),
            offset: None,
        }
    }
}

impl Default for TransactionQuery {
    fn default() -> Self {
        Self {
            tx_type: None,
            start_height: None,
            end_height: None,
            election_id: None,
            min_timestamp: None,
            max_timestamp: None,
            limit: Some(crate::DEFAULT_QUERY_LIMIT),
            offset: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_info_serialization() {
        let info = NodeInfo {
            version: "1.0.0".to_string(),
            node_type: "validator".to_string(),
            public_key: "abc123".to_string(),
            network_id: "mainnet".to_string(),
            height: 1000,
            latest_block_hash: "hash123".to_string(),
            peer_count: 5,
            syncing: false,
            uptime: 3600,
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: NodeInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info.version, deserialized.version);
        assert_eq!(info.height, deserialized.height);
    }

    #[test]
    fn test_block_query_default() {
        let query = BlockQuery::default();
        assert_eq!(query.limit, Some(crate::DEFAULT_QUERY_LIMIT));
        assert!(query.start_height.is_none());
        assert!(query.validator.is_none());
    }

    #[test]
    fn test_transaction_query_default() {
        let query = TransactionQuery::default();
        assert_eq!(query.limit, Some(crate::DEFAULT_QUERY_LIMIT));
        assert!(query.tx_type.is_none());
        assert!(query.election_id.is_none());
    }

    #[test]
    fn test_health_response() {
        let health = HealthResponse {
            status: "healthy".to_string(),
            database: "ok".to_string(),
            network: "ok".to_string(),
            consensus: "ok".to_string(),
            timestamp: 1000000,
        };

        assert_eq!(health.status, "healthy");
        assert_eq!(health.database, "ok");
    }

    #[test]
    fn test_candidate_result_percentage() {
        let result = CandidateResult {
            candidate_id: "c1".to_string(),
            candidate_name: "Candidate 1".to_string(),
            votes: 75,
            percentage: 75.0,
        };

        assert_eq!(result.votes, 75);
        assert_eq!(result.percentage, 75.0);
    }
}

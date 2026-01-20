use thiserror::Error;

/// Core error type for the voting system
#[derive(Error, Debug)]
pub enum VotingError {
    // Blockchain errors
    #[error("Invalid block: {0}")]
    InvalidBlock(String),

    #[error("Invalid block hash: expected {expected}, got {actual}")]
    InvalidBlockHash { expected: String, actual: String },

    #[error("Invalid previous hash: {0}")]
    InvalidPreviousHash(String),

    #[error("Block height mismatch: expected {expected}, got {actual}")]
    BlockHeightMismatch { expected: u64, actual: u64 },

    #[error("Genesis block error: {0}")]
    GenesisError(String),

    // Transaction errors
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),

    #[error("Invalid transaction signature")]
    InvalidSignature,

    #[error("Transaction already exists: {0}")]
    DuplicateTransaction(String),

    #[error("Invalid vote: {0}")]
    InvalidVote(String),

    // Cryptography errors
    #[error("Cryptographic error: {0}")]
    CryptoError(String),

    #[error("Key generation failed: {0}")]
    KeyGenerationError(String),

    #[error("Signature verification failed")]
    SignatureVerificationFailed,

    #[error("Hash computation failed: {0}")]
    HashError(String),

    #[error("Invalid public key")]
    InvalidPublicKey,

    #[error("Invalid private key")]
    InvalidPrivateKey,

    // Storage errors
    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Block not found: {0}")]
    BlockNotFound(String),

    #[error("Transaction not found: {0}")]
    TransactionNotFound(String),

    // Network errors
    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Peer connection failed: {0}")]
    PeerConnectionError(String),

    #[error("Failed to sync blockchain: {0}")]
    SyncError(String),

    #[error("Invalid peer message: {0}")]
    InvalidMessage(String),

    #[error("Peer timeout: {0}")]
    PeerTimeout(String),

    // Consensus errors
    #[error("Consensus error: {0}")]
    ConsensusError(String),

    #[error("Invalid validator: {0}")]
    InvalidValidator(String),

    #[error("Insufficient validator signatures")]
    InsufficientValidators,

    #[error("Block finality not reached")]
    FinalityNotReached,

    // Node errors
    #[error("Node not initialized")]
    NodeNotInitialized,

    #[error("Node configuration error: {0}")]
    ConfigError(String),

    #[error("Node already running")]
    NodeAlreadyRunning,

    #[error("Node not running")]
    NodeNotRunning,

    // Voting specific errors
    #[error("Voter not eligible: {0}")]
    VoterNotEligible(String),

    #[error("Election not found: {0}")]
    ElectionNotFound(String),

    #[error("Election not active")]
    ElectionNotActive,

    #[error("Election already closed")]
    ElectionClosed,

    #[error("Voter already voted")]
    AlreadyVoted,

    #[error("Invalid ballot: {0}")]
    InvalidBallot(String),

    #[error("Invalid candidate: {0}")]
    InvalidCandidate(String),

    // RPC errors
    #[error("RPC error: {0}")]
    RpcError(String),

    #[error("Invalid RPC request: {0}")]
    InvalidRpcRequest(String),

    #[error("RPC timeout")]
    RpcTimeout,

    // General errors
    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Operation failed: {0}")]
    OperationFailed(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    // IO errors
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    // Parsing errors
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid hex string")]
    InvalidHex(#[from] hex::FromHexError),
}

/// Result type alias for voting system operations
pub type Result<T> = std::result::Result<T, VotingError>;

// Convenience implementations for common error conversions
impl From<serde_json::Error> for VotingError {
    fn from(err: serde_json::Error) -> Self {
        VotingError::SerializationError(err.to_string())
    }
}

impl From<bincode::Error> for VotingError {
    fn from(err: bincode::Error) -> Self {
        VotingError::SerializationError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = VotingError::InvalidBlock("test error".to_string());
        assert_eq!(err.to_string(), "Invalid block: test error");
    }

    #[test]
    fn test_block_height_mismatch() {
        let err = VotingError::BlockHeightMismatch {
            expected: 10,
            actual: 5,
        };
        assert!(err.to_string().contains("expected 10"));
        assert!(err.to_string().contains("got 5"));
    }

    #[test]
    fn test_result_type() {
        let success: Result<i32> = Ok(42);
        assert!(success.is_ok());

        let failure: Result<i32> = Err(VotingError::NodeNotInitialized);
        assert!(failure.is_err());
    }
}

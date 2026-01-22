use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

/// Main configuration for the voting node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Node identity and network settings
    pub network: NetworkConfig,
    
    /// Storage configuration
    pub storage: StorageConfig,
    
    /// Consensus configuration
    pub consensus: ConsensusConfig,
    
    /// RPC server configuration
    pub rpc: RpcConfig,
    
    /// Logging configuration
    pub logging: LoggingConfig,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            network: NetworkConfig::default(),
            storage: StorageConfig::default(),
            consensus: ConsensusConfig::default(),
            rpc: RpcConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl NodeConfig {
    /// Load configuration from TOML file
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        let config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Save configuration to TOML file
    pub fn to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// P2P listen address
    pub listen_addr: SocketAddr,
    
    /// List of bootstrap peer addresses
    pub bootstrap_peers: Vec<SocketAddr>,
    
    /// Maximum number of peers
    pub max_peers: usize,
    
    /// Connection timeout in seconds
    pub connection_timeout: u64,
    
    /// Enable peer discovery
    pub enable_discovery: bool,
    
    /// Network ID (mainnet, testnet, etc.)
    pub network_id: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:9000".parse().unwrap(),
            bootstrap_peers: vec![],
            max_peers: 50,
            connection_timeout: 30,
            enable_discovery: true,
            network_id: "voting-mainnet".to_string(),
        }
    }
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Data directory path
    pub data_dir: PathBuf,
    
    /// Blockchain database path
    pub blockchain_db_path: PathBuf,
    
    /// State database path
    pub state_db_path: PathBuf,
    
    /// Enable database compression
    pub enable_compression: bool,
    
    /// Cache size in MB
    pub cache_size_mb: usize,
    
    /// Enable pruning of old data
    pub enable_pruning: bool,
    
    /// Keep blocks for N days before pruning
    pub pruning_days: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
            blockchain_db_path: PathBuf::from("./data/blockchain"),
            state_db_path: PathBuf::from("./data/state"),
            enable_compression: true,
            cache_size_mb: 128,
            enable_pruning: false,
            pruning_days: 365,
        }
    }
}

/// Consensus configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Consensus algorithm (proof_of_authority, etc.)
    pub algorithm: ConsensusAlgorithm,
    
    /// Block time in seconds
    pub block_time: u64,
    
    /// Minimum number of validators required
    pub min_validators: usize,
    
    /// Threshold for finality (e.g., 2/3 for BFT)
    pub finality_threshold: f64,
    
    /// Is this node a validator
    pub is_validator: bool,
    
    /// Validator key path (if validator)
    pub validator_key_path: Option<PathBuf>,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            algorithm: ConsensusAlgorithm::ProofOfAuthority,
            block_time: 5,
            min_validators: 3,
            finality_threshold: 0.67,
            is_validator: false,
            validator_key_path: None,
        }
    }
}

/// Consensus algorithm type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConsensusAlgorithm {
    /// Proof of Authority
    ProofOfAuthority,
    /// Proof of Stake (future)
    ProofOfStake,
}

/// RPC server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    /// Enable RPC server
    pub enabled: bool,
    
    /// RPC listen address
    pub listen_addr: SocketAddr,
    
    /// Enable CORS
    pub enable_cors: bool,
    
    /// Allowed CORS origins
    pub cors_origins: Vec<String>,
    
    /// Request timeout in seconds
    pub request_timeout: u64,
    
    /// Maximum request size in bytes
    pub max_request_size: usize,
    
    /// Rate limiting: requests per minute
    pub rate_limit: usize,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            listen_addr: "127.0.0.1:8545".parse().unwrap(),
            enable_cors: true,
            cors_origins: vec!["*".to_string()],
            request_timeout: 30,
            max_request_size: 10 * 1024 * 1024, // 10 MB
            rate_limit: 100,
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: LogLevel,
    
    /// Log output format (json, pretty)
    pub format: LogFormat,
    
    /// Log to file
    pub log_to_file: bool,
    
    /// Log file path
    pub log_file_path: PathBuf,
    
    /// Log to console
    pub log_to_console: bool,
    
    /// Enable metrics collection
    pub enable_metrics: bool,
    
    /// Metrics endpoint
    pub metrics_addr: Option<SocketAddr>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            format: LogFormat::Pretty,
            log_to_file: true,
            log_file_path: PathBuf::from("./data/logs/node.log"),
            log_to_console: true,
            enable_metrics: true,
            metrics_addr: Some("127.0.0.1:9090".parse().unwrap()),
        }
    }
}

/// Log level
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

/// Log format
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Pretty,
}

/// Genesis configuration for blockchain initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisConfig {
    /// Genesis timestamp
    pub timestamp: u64,
    
    /// Initial validators
    pub validators: Vec<String>,
    
    /// Chain ID
    pub chain_id: String,
    
    /// Network name
    pub network_name: String,
}

impl Default for GenesisConfig {
    fn default() -> Self {
        Self {
            timestamp: 0,
            validators: vec![],
            chain_id: "voting-chain-1".to_string(),
            network_name: "Voting Network Mainnet".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = NodeConfig::default();
        assert_eq!(config.network.max_peers, 50);
        assert_eq!(config.consensus.block_time, 5);
        assert!(config.rpc.enabled);
    }

    #[test]
    fn test_consensus_algorithm() {
        let algo = ConsensusAlgorithm::ProofOfAuthority;
        assert_eq!(algo, ConsensusAlgorithm::ProofOfAuthority);
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(LogLevel::Info.to_string(), "info");
        assert_eq!(LogLevel::Error.to_string(), "error");
    }

    #[test]
    fn test_genesis_config() {
        let genesis = GenesisConfig::default();
        assert_eq!(genesis.chain_id, "voting-chain-1");
        assert!(genesis.validators.is_empty());
    }
}

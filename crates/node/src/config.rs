use common::{ConsensusConfig, NetworkConfig, StorageConfig};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Node type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    /// Validator node - produces and validates blocks
    Validator,
    /// Full node - maintains complete blockchain, validates but does not produce
    Full,
    /// Light node - only maintains block headers and verifies specific transactions
    Light,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeType::Validator => write!(f, "validator"),
            NodeType::Full => write!(f, "full"),
            NodeType::Light => write!(f, "light"),
        }
    }
}

impl std::str::FromStr for NodeType {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "validator" => Ok(NodeType::Validator),
            "full" => Ok(NodeType::Full),
            "light" => Ok(NodeType::Light),
            _ => Err(format!("Unknown node type: {}", s)),
        }
    }
}

/// Node configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Node type
    pub node_type: NodeType,
    
    /// Network configuration
    pub network: NetworkConfig,
    
    /// Storage configuration
    pub storage: StorageConfig,
    
    /// Consensus configuration
    pub consensus: ConsensusConfig,
    
    /// Path to genesis block file
    pub genesis_path: Option<PathBuf>,
    
    /// Path to validator key (for validator nodes)
    pub validator_key_path: Option<PathBuf>,
    
    /// Data directory
    pub data_dir: PathBuf,
    
    /// Enable metrics collection
    pub enable_metrics: bool,
    
    /// Metrics server address
    pub metrics_addr: Option<String>,
    
    /// Enable JSON-RPC server
    pub enable_rpc: bool,
    
    /// RPC server address
    pub rpc_addr: Option<String>,
    
    /// Log level
    pub log_level: String,
    
    /// Log to file
    pub log_to_file: bool,
    
    /// Log file path
    pub log_file_path: Option<PathBuf>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_type: NodeType::Full,
            network: NetworkConfig::default(),
            storage: StorageConfig::default(),
            consensus: ConsensusConfig::default(),
            genesis_path: None,
            validator_key_path: None,
            data_dir: PathBuf::from("./data"),
            enable_metrics: true,
            metrics_addr: Some("127.0.0.1:9090".to_string()),
            enable_rpc: true,
            rpc_addr: Some("127.0.0.1:8545".to_string()),
            log_level: "info".to_string(),
            log_to_file: false,
            log_file_path: None,
        }
    }
}

impl NodeConfig {
    /// Create configuration for a validator node
    pub fn validator() -> Self {
        Self {
            node_type: NodeType::Validator,
            ..Default::default()
        }
    }
    
    /// Create configuration for a full node
    pub fn full_node() -> Self {
        Self {
            node_type: NodeType::Full,
            ..Default::default()
        }
    }
    
    /// Create configuration for a light node
    pub fn light_node() -> Self {
        let mut config = Self {
            node_type: NodeType::Light,
            ..Default::default()
        };
        config.storage.enable_pruning = true;
        config
    }
    
    /// Set data directory
    pub fn with_data_dir(mut self, path: PathBuf) -> Self {
        self.data_dir = path;
        self
    }
    
    /// Set genesis file path
    pub fn with_genesis(mut self, path: PathBuf) -> Self {
        self.genesis_path = Some(path);
        self
    }
    
    /// Set validator key path
    pub fn with_validator_key(mut self, path: PathBuf) -> Self {
        self.validator_key_path = Some(path);
        self
    }
    
    /// Enable or disable metrics
    pub fn with_metrics(mut self, enable: bool) -> Self {
        self.enable_metrics = enable;
        self
    }
    
    /// Enable or disable RPC
    pub fn with_rpc(mut self, enable: bool) -> Self {
        self.enable_rpc = enable;
        self
    }
    
    /// Set log level
    pub fn with_log_level(mut self, level: String) -> Self {
        self.log_level = level;
        self
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validator nodes must have a validator key
        if self.node_type == NodeType::Validator && self.validator_key_path.is_none() {
            return Err("Validator nodes require a validator_key_path".to_string());
        }
        
        // All nodes need a genesis path unless they're joining an existing network
        if self.genesis_path.is_none() {
            return Err("genesis_path is required".to_string());
        }
        
        // Validate log level
        if !["trace", "debug", "info", "warn", "error"].contains(&self.log_level.as_str()) {
            return Err(format!("Invalid log level: {}", self.log_level));
        }
        
        Ok(())
    }
}

/// Node configuration builder
pub struct NodeConfigBuilder {
    config: NodeConfig,
}

impl NodeConfigBuilder {
    pub fn new(node_type: NodeType) -> Self {
        let config = match node_type {
            NodeType::Validator => NodeConfig::validator(),
            NodeType::Full => NodeConfig::full_node(),
            NodeType::Light => NodeConfig::light_node(),
        };
        
        Self { config }
    }
    
    pub fn data_dir(mut self, path: PathBuf) -> Self {
        self.config.data_dir = path;
        self
    }
    
    pub fn genesis(mut self, path: PathBuf) -> Self {
        self.config.genesis_path = Some(path);
        self
    }
    
    pub fn validator_key(mut self, path: PathBuf) -> Self {
        self.config.validator_key_path = Some(path);
        self
    }
    
    pub fn network(mut self, network: NetworkConfig) -> Self {
        self.config.network = network;
        self
    }
    
    pub fn storage(mut self, storage: StorageConfig) -> Self {
        self.config.storage = storage;
        self
    }
    
    pub fn consensus(mut self, consensus: ConsensusConfig) -> Self {
        self.config.consensus = consensus;
        self
    }
    
    pub fn metrics(mut self, enable: bool) -> Self {
        self.config.enable_metrics = enable;
        self
    }
    
    pub fn metrics_addr(mut self, addr: String) -> Self {
        self.config.metrics_addr = Some(addr);
        self
    }
    
    pub fn rpc(mut self, enable: bool) -> Self {
        self.config.enable_rpc = enable;
        self
    }
    
    pub fn rpc_addr(mut self, addr: String) -> Self {
        self.config.rpc_addr = Some(addr);
        self
    }
    
    pub fn log_level(mut self, level: String) -> Self {
        self.config.log_level = level;
        self
    }
    
    pub fn build(self) -> Result<NodeConfig, String> {
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for NodeConfigBuilder {
    fn default() -> Self {
        Self::new(NodeType::Full)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_type_display() {
        assert_eq!(NodeType::Validator.to_string(), "validator");
        assert_eq!(NodeType::Full.to_string(), "full");
        assert_eq!(NodeType::Light.to_string(), "light");
    }

    #[test]
    fn test_node_type_from_str() {
        assert_eq!("validator".parse::<NodeType>().unwrap(), NodeType::Validator);
        assert_eq!("full".parse::<NodeType>().unwrap(), NodeType::Full);
        assert_eq!("light".parse::<NodeType>().unwrap(), NodeType::Light);
        assert_eq!("VALIDATOR".parse::<NodeType>().unwrap(), NodeType::Validator);
        
        assert!("invalid".parse::<NodeType>().is_err());
    }

    #[test]
    fn test_node_config_default() {
        let config = NodeConfig::default();
        assert_eq!(config.node_type, NodeType::Full);
        assert!(config.enable_metrics);
        assert!(config.enable_rpc);
    }

    #[test]
    fn test_node_config_validator() {
        let config = NodeConfig::validator();
        assert_eq!(config.node_type, NodeType::Validator);
    }

    #[test]
    fn test_node_config_full_node() {
        let config = NodeConfig::full_node();
        assert_eq!(config.node_type, NodeType::Full);
    }

    #[test]
    fn test_node_config_light_node() {
        let config = NodeConfig::light_node();
        assert_eq!(config.node_type, NodeType::Light);
        assert!(config.storage.enable_pruning);
    }

    #[test]
    fn test_node_config_validation() {
        let config = NodeConfig::validator();
        assert!(config.validate().is_err());
        
        let config = NodeConfig::validator()
            .with_validator_key(PathBuf::from("/tmp/key.pem"))
            .with_genesis(PathBuf::from("/tmp/genesis.json"));
        assert!(config.validate().is_ok());
    }
}

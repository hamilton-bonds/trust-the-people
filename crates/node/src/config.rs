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
            log_to_file: true,
            log_file_path: Some(PathBuf::from("./data/logs/node.log")),
        }
    }
}

impl NodeConfig {
    /// Create a validator node configuration
    pub fn validator() -> Self {
        Self {
            node_type: NodeType::Validator,
            ..Default::default()
        }
    }
    
    /// Create a full node configuration
    pub fn full_node() -> Self {
        Self {
            node_type: NodeType::Full,
            ..Default::default()
        }
    }
    
    /// Create a light node configuration
    pub fn light_node() -> Self {
        Self {
            node_type: NodeType::Light,
            storage: StorageConfig {
                enable_pruning: true,
                pruning_retention_days: 7,
                ..Default::default()
            },
            ..Default::default()
        }
    }
    
    /// Set data directory
    pub fn with_data_dir(mut self, path: PathBuf) -> Self {
        self.data_dir = path;
        self
    }
    
    /// Set genesis path
    pub fn with_genesis(mut self, path: PathBuf) -> Self {
        self.genesis_path = Some(path);
        self
    }
    
    /// Set validator key path
    pub fn with_validator_key(mut self, path: PathBuf) -> Self {
        self.validator_key_path = Some(path);
        self
    }
    
    /// Set network configuration
    pub fn with_network(mut self, network: NetworkConfig) -> Self {
        self.network = network;
        self
    }
    
    /// Set storage configuration
    pub fn with_storage(mut self, storage: StorageConfig) -> Self {
        self.storage = storage;
        self
    }
    
    /// Set consensus configuration
    pub fn with_consensus(mut self, consensus: ConsensusConfig) -> Self {
        self.consensus = consensus;
        self
    }
    
    /// Enable or disable metrics
    pub fn with_metrics(mut self, enable: bool) -> Self {
        self.enable_metrics = enable;
        self
    }
    
    /// Set metrics address
    pub fn with_metrics_addr(mut self, addr: String) -> Self {
        self.metrics_addr = Some(addr);
        self
    }
    
    /// Enable or disable RPC
    pub fn with_rpc(mut self, enable: bool) -> Self {
        self.enable_rpc = enable;
        self
    }
    
    /// Set RPC address
    pub fn with_rpc_addr(mut self, addr: String) -> Self {
        self.rpc_addr = Some(addr);
        self
    }
    
    /// Set log level
    pub fn with_log_level(mut self, level: String) -> Self {
        self.log_level = level;
        self
    }
    
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
    
    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.node_type == NodeType::Validator && self.validator_key_path.is_none() {
            return Err("Validator node requires validator_key_path".to_string());
        }
        
        if self.genesis_path.is_none() {
            return Err("Genesis path is required".to_string());
        }
        
        if self.enable_metrics && self.metrics_addr.is_none() {
            return Err("Metrics enabled but no address specified".to_string());
        }
        
        if self.enable_rpc && self.rpc_addr.is_none() {
            return Err("RPC enabled but no address specified".to_string());
        }
        
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
    fn test_node_config_builder() {
        let config = NodeConfigBuilder::new(NodeType::Validator)
            .data_dir(PathBuf::from("/tmp/data"))
            .genesis(PathBuf::from("/tmp/genesis.json"))
            .validator_key(PathBuf::from("/tmp/validator.pem"))
            .metrics(true)
            .rpc(false)
            .log_level("debug".to_string())
            .build()
            .unwrap();
        
        assert_eq!(config.node_type, NodeType::Validator);
        assert_eq!(config.data_dir, PathBuf::from("/tmp/data"));
        assert!(config.enable_metrics);
        assert!(!config.enable_rpc);
        assert_eq!(config.log_level, "debug");
    }

    #[test]
    fn test_node_config_with_methods() {
        let config = NodeConfig::validator()
            .with_data_dir(PathBuf::from("/tmp/data"))
            .with_genesis(PathBuf::from("/tmp/genesis.json"))
            .with_validator_key(PathBuf::from("/tmp/validator.pem"))
            .with_metrics(false)
            .with_rpc(true)
            .with_log_level("trace".to_string());
        
        assert_eq!(config.data_dir, PathBuf::from("/tmp/data"));
        assert!(!config.enable_metrics);
        assert!(config.enable_rpc);
        assert_eq!(config.log_level, "trace");
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

    #[test]
    fn test_node_config_validation_invalid_log_level() {
        let config = NodeConfig::full_node()
            .with_genesis(PathBuf::from("/tmp/genesis.json"))
            .with_log_level("invalid".to_string());
        
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_node_config_validation_full_node() {
        let config = NodeConfig::full_node()
            .with_genesis(PathBuf::from("/tmp/genesis.json"));
        
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_node_config_validation_light_node() {
        let config = NodeConfig::light_node()
            .with_genesis(PathBuf::from("/tmp/genesis.json"));
        
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_node_config_builder_default() {
        let builder = NodeConfigBuilder::default();
        assert_eq!(builder.config.node_type, NodeType::Full);
    }

    #[test]
    fn test_node_config_builder_validation_fails() {
        let result = NodeConfigBuilder::new(NodeType::Validator).build();
        assert!(result.is_err());
    }

    #[test]
    fn test_node_config_with_rpc_addr() {
        let config = NodeConfig::default()
            .with_rpc_addr("0.0.0.0:8080".to_string());
        
        assert_eq!(config.rpc_addr, Some("0.0.0.0:8080".to_string()));
    }

    #[test]
    fn test_node_config_with_metrics_addr() {
        let config = NodeConfig::default()
            .with_metrics_addr("0.0.0.0:9090".to_string());
        
        assert_eq!(config.metrics_addr, Some("0.0.0.0:9090".to_string()));
    }
}

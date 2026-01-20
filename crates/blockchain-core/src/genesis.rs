use common::{BlockHash, PublicKey, Result, Timestamp, VotingError};
use crate::block::Block;
use crate::transaction::{Transaction, TransactionType, ValidatorRegistration, ValidatorMetadata};
use serde::{Deserialize, Serialize};

/// Genesis block configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisBlock {
    /// Initial set of validators
    pub validators: Vec<PublicKey>,
    
    /// Genesis timestamp
    pub timestamp: Timestamp,
    
    /// Chain ID
    pub chain_id: String,
    
    /// Network name
    pub network_name: String,
    
    /// Additional configuration
    pub config: GenesisConfig,
}

/// Additional genesis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisConfig {
    /// Block time in seconds
    pub block_time: u64,
    
    /// Consensus threshold (e.g., 0.67 for 2/3)
    pub consensus_threshold: f64,
    
    /// Maximum block size in bytes
    pub max_block_size: usize,
    
    /// Maximum transactions per block
    pub max_transactions_per_block: u32,
}

impl Default for GenesisConfig {
    fn default() -> Self {
        Self {
            block_time: 5,
            consensus_threshold: 0.67,
            max_block_size: 1024 * 1024, // 1 MB
            max_transactions_per_block: 1000,
        }
    }
}

impl GenesisBlock {
    /// Create a new genesis block configuration
    pub fn new(validators: Vec<PublicKey>, timestamp: Timestamp) -> Self {
        Self {
            validators,
            timestamp,
            chain_id: "voting-chain-1".to_string(),
            network_name: "Voting Network Mainnet".to_string(),
            config: GenesisConfig::default(),
        }
    }
    
    /// Create genesis with custom configuration
    pub fn with_config(
        validators: Vec<PublicKey>,
        timestamp: Timestamp,
        chain_id: String,
        network_name: String,
        config: GenesisConfig,
    ) -> Self {
        Self {
            validators,
            timestamp,
            chain_id,
            network_name,
            config,
        }
    }
    
    /// Create the genesis block
    pub fn create_block(&self) -> Result<Block> {
        if self.validators.is_empty() {
            return Err(VotingError::GenesisError(
                "Genesis must have at least one validator".to_string(),
            ));
        }
        
        // Create validator registration transactions
        let mut transactions = Vec::new();
        
        for (i, validator_key) in self.validators.iter().enumerate() {
            let metadata = ValidatorMetadata {
                name: format!("Genesis Validator {}", i + 1),
                organization: Some("Genesis".to_string()),
                contact: None,
            };
            
            let registration = ValidatorRegistration {
                validator_key: *validator_key,
                metadata,
                authorizing_signature: common::Signature::new([0u8; 64]), // Genesis signatures are zero
            };
            
            let tx = Transaction::new_with_params(
                TransactionType::ValidatorRegistration(registration),
                self.timestamp,
                i as u64,
            );
            
            transactions.push(tx);
        }
        
        // Genesis block is created by the first validator
        let genesis_validator = self.validators[0];
        
        // Create genesis block with height 0 and zero previous hash
        let mut block = Block::new(
            0,
            BlockHash::zero(),
            transactions,
            genesis_validator,
        );
        
        // Override timestamp to use genesis timestamp
        block.header.timestamp = self.timestamp;
        block.header.hash = block.header.calculate_hash();
        
        Ok(block)
    }
    
    /// Validate genesis configuration
    pub fn validate(&self) -> Result<()> {
        if self.validators.is_empty() {
            return Err(VotingError::GenesisError(
                "Must have at least one validator".to_string(),
            ));
        }
        
        if self.chain_id.is_empty() {
            return Err(VotingError::GenesisError(
                "Chain ID cannot be empty".to_string(),
            ));
        }
        
        if self.network_name.is_empty() {
            return Err(VotingError::GenesisError(
                "Network name cannot be empty".to_string(),
            ));
        }
        
        if self.config.block_time == 0 {
            return Err(VotingError::GenesisError(
                "Block time must be greater than 0".to_string(),
            ));
        }
        
        if self.config.consensus_threshold <= 0.0 || self.config.consensus_threshold > 1.0 {
            return Err(VotingError::GenesisError(
                "Consensus threshold must be between 0 and 1".to_string(),
            ));
        }
        
        if self.config.max_block_size == 0 {
            return Err(VotingError::GenesisError(
                "Max block size must be greater than 0".to_string(),
            ));
        }
        
        if self.config.max_transactions_per_block == 0 {
            return Err(VotingError::GenesisError(
                "Max transactions per block must be greater than 0".to_string(),
            ));
        }
        
        Ok(())
    }
    
    /// Load genesis from JSON file
    pub fn from_file(path: &str) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| VotingError::GenesisError(format!("Failed to read file: {}", e)))?;
        
        let genesis: Self = serde_json::from_str(&contents)
            .map_err(|e| VotingError::GenesisError(format!("Failed to parse JSON: {}", e)))?;
        
        genesis.validate()?;
        Ok(genesis)
    }
    
    /// Save genesis to JSON file
    pub fn to_file(&self, path: &str) -> Result<()> {
        self.validate()?;
        
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| VotingError::GenesisError(format!("Failed to serialize: {}", e)))?;
        
        std::fs::write(path, json)
            .map_err(|e| VotingError::GenesisError(format!("Failed to write file: {}", e)))?;
        
        Ok(())
    }
    
    /// Get the hash that this genesis block will have
    pub fn calculate_hash(&self) -> Result<BlockHash> {
        let block = self.create_block()?;
        Ok(block.hash())
    }
}

/// Builder for creating genesis block configurations
pub struct GenesisBuilder {
    validators: Vec<PublicKey>,
    timestamp: Option<Timestamp>,
    chain_id: Option<String>,
    network_name: Option<String>,
    config: GenesisConfig,
}

impl GenesisBuilder {
    /// Create a new genesis builder
    pub fn new() -> Self {
        Self {
            validators: Vec::new(),
            timestamp: None,
            chain_id: None,
            network_name: None,
            config: GenesisConfig::default(),
        }
    }
    
    /// Add a validator
    pub fn add_validator(mut self, validator: PublicKey) -> Self {
        self.validators.push(validator);
        self
    }
    
    /// Add multiple validators
    pub fn add_validators(mut self, validators: Vec<PublicKey>) -> Self {
        self.validators.extend(validators);
        self
    }
    
    /// Set timestamp
    pub fn timestamp(mut self, timestamp: Timestamp) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
    
    /// Set chain ID
    pub fn chain_id(mut self, chain_id: String) -> Self {
        self.chain_id = Some(chain_id);
        self
    }
    
    /// Set network name
    pub fn network_name(mut self, network_name: String) -> Self {
        self.network_name = Some(network_name);
        self
    }
    
    /// Set block time
    pub fn block_time(mut self, seconds: u64) -> Self {
        self.config.block_time = seconds;
        self
    }
    
    /// Set consensus threshold
    pub fn consensus_threshold(mut self, threshold: f64) -> Self {
        self.config.consensus_threshold = threshold;
        self
    }
    
    /// Set max block size
    pub fn max_block_size(mut self, bytes: usize) -> Self {
        self.config.max_block_size = bytes;
        self
    }
    
    /// Set max transactions per block
    pub fn max_transactions_per_block(mut self, count: u32) -> Self {
        self.config.max_transactions_per_block = count;
        self
    }
    
    /// Build the genesis block configuration
    pub fn build(self) -> GenesisBlock {
        let timestamp = self.timestamp.unwrap_or_else(common::utils::current_timestamp);
        let chain_id = self.chain_id.unwrap_or_else(|| "voting-chain-1".to_string());
        let network_name = self.network_name.unwrap_or_else(|| "Voting Network".to_string());
        
        GenesisBlock {
            validators: self.validators,
            timestamp,
            chain_id,
            network_name,
            config: self.config,
        }
    }
}

impl Default for GenesisBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_validators(count: usize) -> Vec<PublicKey> {
        (0..count)
            .map(|i| {
                let mut key = [0u8; 32];
                key[0] = i as u8;
                PublicKey::new(key)
            })
            .collect()
    }

    #[test]
    fn test_genesis_creation() {
        let validators = create_test_validators(3);
        let genesis = GenesisBlock::new(validators, 1000000000);
        
        assert_eq!(genesis.validators.len(), 3);
        assert_eq!(genesis.timestamp, 1000000000);
    }

    #[test]
    fn test_genesis_block_creation() {
        let validators = create_test_validators(3);
        let genesis = GenesisBlock::new(validators, 1000000000);
        
        let block = genesis.create_block().unwrap();
        
        assert_eq!(block.height(), 0);
        assert_eq!(block.previous_hash(), BlockHash::zero());
        assert_eq!(block.transactions.len(), 3); // One tx per validator
    }

    #[test]
    fn test_genesis_validation() {
        let validators = create_test_validators(1);
        let genesis = GenesisBlock::new(validators, 1000000000);
        
        assert!(genesis.validate().is_ok());
    }

    #[test]
    fn test_genesis_no_validators() {
        let genesis = GenesisBlock::new(vec![], 1000000000);
        
        assert!(genesis.validate().is_err());
        assert!(genesis.create_block().is_err());
    }

    #[test]
    fn test_genesis_builder() {
        let validators = create_test_validators(2);
        let genesis = GenesisBuilder::new()
            .add_validators(validators)
            .timestamp(1000000000)
            .chain_id("test-chain".to_string())
            .network_name("Test Network".to_string())
            .block_time(10)
            .consensus_threshold(0.75)
            .build();
        
        assert_eq!(genesis.validators.len(), 2);
        assert_eq!(genesis.timestamp, 1000000000);
        assert_eq!(genesis.chain_id, "test-chain");
        assert_eq!(genesis.network_name, "Test Network");
        assert_eq!(genesis.config.block_time, 10);
        assert_eq!(genesis.config.consensus_threshold, 0.75);
    }

    #[test]
    fn test_genesis_builder_defaults() {
        let validators = create_test_validators(1);
        let genesis = GenesisBuilder::new()
            .add_validators(validators)
            .build();
        
        assert!(!genesis.chain_id.is_empty());
        assert!(!genesis.network_name.is_empty());
        assert!(genesis.timestamp > 0);
    }

    #[test]
    fn test_genesis_block_hash_deterministic() {
        let validators = create_test_validators(2);
        let genesis = GenesisBlock::new(validators, 1000000000);
        
        let block1 = genesis.create_block().unwrap();
        let block2 = genesis.create_block().unwrap();
        
        assert_eq!(block1.hash(), block2.hash());
    }

    #[test]
    fn test_invalid_consensus_threshold() {
        let mut genesis = GenesisBlock::new(create_test_validators(1), 1000000000);
        genesis.config.consensus_threshold = 1.5;
        
        assert!(genesis.validate().is_err());
    }

    #[test]
    fn test_invalid_block_time() {
        let mut genesis = GenesisBlock::new(create_test_validators(1), 1000000000);
        genesis.config.block_time = 0;
        
        assert!(genesis.validate().is_err());
    }

    #[test]
    fn test_calculate_genesis_hash() {
        let validators = create_test_validators(2);
        let genesis = GenesisBlock::new(validators, 1000000000);
        
        let hash = genesis.calculate_hash().unwrap();
        assert_ne!(hash, BlockHash::zero());
    }

    #[test]
    fn test_genesis_config_defaults() {
        let config = GenesisConfig::default();
        
        assert_eq!(config.block_time, 5);
        assert_eq!(config.consensus_threshold, 0.67);
        assert_eq!(config.max_block_size, 1024 * 1024);
        assert_eq!(config.max_transactions_per_block, 1000);
    }
}

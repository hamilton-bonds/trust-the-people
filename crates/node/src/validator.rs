use crate::config::NodeConfig;
use crate::{Node, NodeComponents, NodeEvent, NodeEventHandler, NodeStats, NodeStatus, NodeType};
use blockchain_core::consensus::{ConsensusEngine, ProofOfAuthority};
use blockchain_core::{Block, Blockchain, GenesisBlock, Transaction};
use common::{BlockHash, BlockHeight, PublicKey, Result, Timestamp, VotingError};
use crypto::KeyPair;
use network::{GossipMessage, NetworkManager};
use storage::StorageManager;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// Validator node - produces and validates blocks
pub struct ValidatorNode {
    config: NodeConfig,
    components: Arc<RwLock<Option<NodeComponents>>>,
    validator_key: KeyPair,
    consensus: Arc<RwLock<Option<ProofOfAuthority>>>,
    running: Arc<RwLock<bool>>,
    started_at: Arc<RwLock<Option<Timestamp>>>,
    stats: Arc<RwLock<NodeStats>>,
}

impl ValidatorNode {
    pub fn new(config: NodeConfig) -> Result<Self> {
        let validator_key = if let Some(key_path) = &config.validator_key_path {
            KeyPair::load_from_file(key_path)?
        } else {
            return Err(VotingError::ConfigError(
                "Validator key path not configured".to_string(),
            ));
        };
        
        Ok(Self {
            config,
            components: Arc::new(RwLock::new(None)),
            validator_key,
            consensus: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
            started_at: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(NodeStats::default())),
        })
    }
    
    async fn initialize(&self) -> Result<()> {
        let genesis = if let Some(genesis_path) = &self.config.genesis_path {
            GenesisBlock::from_file(genesis_path)?
        } else {
            return Err(VotingError::ConfigError(
                "Genesis path not configured".to_string(),
            ));
        };
        
        let blockchain = Blockchain::new(genesis)?;
        
        let storage = StorageManager::new(self.config.storage.clone())?;
        
        let network = NetworkManager::new(self.config.network.clone())?;
        
        let components = NodeComponents::new(blockchain, storage, network).await;
        
        let validators = vec![self.validator_key.public_key()];
        let consensus = ProofOfAuthority::new(validators, self.config.consensus.clone());
        
        *self.components.write().await = Some(components);
        *self.consensus.write().await = Some(consensus);
        
        Ok(())
    }
    
    async fn block_production_loop(&self) {
        let mut ticker = interval(Duration::from_secs(self.config.consensus.block_time));
        
        loop {
            ticker.tick().await;
            
            let running = *self.running.read().await;
            if !running {
                break;
            }
            
            if let Err(e) = self.produce_block().await {
                tracing::error!("Failed to produce block: {}", e);
            }
        }
    }
    
    async fn produce_block(&self) -> Result<()> {
        let components = self.components.read().await;
        let Some(ref components) = *components else {
            return Err(VotingError::NodeNotInitialized);
        };
        
        let consensus = self.consensus.read().await;
        let Some(ref consensus) = *consensus else {
            return Err(VotingError::NodeNotInitialized);
        };
        
        let validator_key = self.validator_key.public_key();
        if !consensus.is_validator(&validator_key).await {
            return Ok(());
        }
        
        let previous_block = {
            let blockchain = components.blockchain.read().await;
            blockchain.latest_block().clone()
        };
        
        let height = previous_block.height() + 1;
        let previous_hash = previous_block.hash();
        
        let pending_txs = components.storage.get_pending_transactions(100).await;
        
        let transactions: Vec<Transaction> = pending_txs
            .into_iter()
            .filter_map(|ptx| common::utils::deserialize(&ptx.data).ok())
            .collect();
        
        let mut block = Block::new(height, previous_hash, transactions, validator_key);
        
        let signature = self.validator_key.sign(&block.hash().0)?;
        block.add_signature(validator_key, signature);
        
        block.validate()?;
        
        {
            let mut blockchain = components.blockchain.write().await;
            blockchain.add_block(block.clone())?;
        }
        
        components.storage.store_block(&block).await?;
        
        let gossip_msg = GossipMessage::NewBlock(block.clone());
        components.network.gossip(gossip_msg).await?;
        
        let mut stats = self.stats.write().await;
        stats.blocks_produced += 1;
        stats.blocks_processed += 1;
        
        tracing::info!("Produced block {} at height {}", block.hash().to_hex(), height);
        
        Ok(())
    }
    
    async fn message_processing_loop(&self) {
        let mut ticker = interval(Duration::from_millis(100));
        
        loop {
            ticker.tick().await;
            
            let running = *self.running.read().await;
            if !running {
                break;
            }
            
            if let Err(e) = self.process_messages().await {
                tracing::warn!("Message processing error: {}", e);
            }
        }
    }
    
    async fn process_messages(&self) -> Result<()> {
        Ok(())
    }
    
    pub async fn validate_block(&self, block: &Block) -> Result<bool> {
        let consensus = self.consensus.read().await;
        let Some(ref consensus) = *consensus else {
            return Err(VotingError::NodeNotInitialized);
        };
        
        block.validate()?;
        
        if !consensus.is_validator(&block.header.validator).await {
            return Ok(false);
        }
        
        let components = self.components.read().await;
        let Some(ref components) = *components else {
            return Err(VotingError::NodeNotInitialized);
        };
        
        let blockchain = components.blockchain.read().await;
        let previous_block = blockchain.get_block_by_height(block.height() - 1);
        
        if let Some(prev) = previous_block {
            block.validate_chain_link(prev)?;
        }
        
        let mut stats = self.stats.write().await;
        stats.blocks_validated += 1;
        
        Ok(true)
    }
    
    pub fn validator_public_key(&self) -> PublicKey {
        self.validator_key.public_key()
    }
    
    pub async fn is_active_validator(&self) -> bool {
        let consensus = self.consensus.read().await;
        if let Some(ref consensus) = *consensus {
            consensus.is_validator(&self.validator_key.public_key()).await
        } else {
            false
        }
    }
}

impl Node for ValidatorNode {
    async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Err(VotingError::NodeAlreadyRunning);
        }
        
        self.initialize().await?;
        
        let components = self.components.read().await;
        let Some(ref components) = *components else {
            return Err(VotingError::NodeNotInitialized);
        };
        
        components.network.start().await?;
        
        *running = true;
        *self.started_at.write().await = Some(common::utils::current_timestamp());
        
        drop(running);
        drop(components);
        
        let node = Arc::new(self);
        let node_clone = Arc::clone(&node);
        tokio::spawn(async move {
            node_clone.block_production_loop().await;
        });
        
        let node_clone = Arc::clone(&node);
        tokio::spawn(async move {
            node_clone.message_processing_loop().await;
        });
        
        tracing::info!("Validator node started");
        
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if !*running {
            return Err(VotingError::NodeNotRunning);
        }
        
        *running = false;
        
        let components = self.components.read().await;
        if let Some(ref components) = *components {
            components.network.stop().await?;
        }
        
        tracing::info!("Validator node stopped");
        
        Ok(())
    }
    
    async fn status(&self) -> NodeStatus {
        let components = self.components.read().await;
        
        let (current_height, best_hash, peer_count, is_syncing) = if let Some(ref comp) = *components {
            (
                comp.get_current_height().await,
                comp.get_best_hash().await,
                comp.network.peer_count().await,
                comp.network.sync_status().await.is_syncing(),
            )
        } else {
            (0, BlockHash::zero(), 0, false)
        };
        
        let started_at = *self.started_at.read().await;
        let uptime = if let Some(start) = started_at {
            common::utils::current_timestamp().saturating_sub(start)
        } else {
            0
        };
        
        NodeStatus {
            node_type: NodeType::Validator,
            is_running: *self.running.read().await,
            is_syncing,
            current_height,
            best_hash,
            peer_count,
            uptime,
            started_at,
        }
    }
    
    async fn stats(&self) -> NodeStats {
        self.stats.read().await.clone()
    }
    
    async fn is_running(&self) -> bool {
        *self.running.read().await
    }
    
    async fn current_height(&self) -> BlockHeight {
        let components = self.components.read().await;
        if let Some(ref comp) = *components {
            comp.get_current_height().await
        } else {
            0
        }
    }
    
    async fn best_hash(&self) -> BlockHash {
        let components = self.components.read().await;
        if let Some(ref comp) = *components {
            comp.get_best_hash().await
        } else {
            BlockHash::zero()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::PublicKey;

    fn create_test_config() -> NodeConfig {
        NodeConfig {
            node_type: NodeType::Validator,
            validator_key_path: None,
            genesis_path: None,
            ..Default::default()
        }
    }

    #[test]
    fn test_validator_node_creation_without_key() {
        let config = create_test_config();
        let result = ValidatorNode::new(config);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validator_public_key() {
        let keypair = KeyPair::generate();
        let expected_pubkey = keypair.public_key();
        
        let mut config = create_test_config();
        let temp_dir = std::env::temp_dir();
        let key_path = temp_dir.join(format!("test_key_{}.pem", rand::random::<u64>()));
        keypair.save_to_file(&key_path).unwrap();
        config.validator_key_path = Some(key_path.clone());
        
        let node = ValidatorNode::new(config).unwrap();
        assert_eq!(node.validator_public_key(), expected_pubkey);
        
        let _ = std::fs::remove_file(key_path);
    }

    #[tokio::test]
    async fn test_validator_stats() {
        let keypair = KeyPair::generate();
        let mut config = create_test_config();
        let temp_dir = std::env::temp_dir();
        let key_path = temp_dir.join(format!("test_key_{}.pem", rand::random::<u64>()));
        keypair.save_to_file(&key_path).unwrap();
        config.validator_key_path = Some(key_path.clone());
        
        let node = ValidatorNode::new(config).unwrap();
        let stats = node.stats().await;
        
        assert_eq!(stats.blocks_produced, 0);
        assert_eq!(stats.blocks_validated, 0);
        
        let _ = std::fs::remove_file(key_path);
    }

    #[tokio::test]
    async fn test_validator_not_running() {
        let keypair = KeyPair::generate();
        let mut config = create_test_config();
        let temp_dir = std::env::temp_dir();
        let key_path = temp_dir.join(format!("test_key_{}.pem", rand::random::<u64>()));
        keypair.save_to_file(&key_path).unwrap();
        config.validator_key_path = Some(key_path.clone());
        
        let node = ValidatorNode::new(config).unwrap();
        assert!(!node.is_running().await);
        
        let _ = std::fs::remove_file(key_path);
    }

    #[tokio::test]
    async fn test_validator_status() {
        let keypair = KeyPair::generate();
        let mut config = create_test_config();
        let temp_dir = std::env::temp_dir();
        let key_path = temp_dir.join(format!("test_key_{}.pem", rand::random::<u64>()));
        keypair.save_to_file(&key_path).unwrap();
        config.validator_key_path = Some(key_path.clone());
        
        let node = ValidatorNode::new(config).unwrap();
        let status = node.status().await;
        
        assert_eq!(status.node_type, NodeType::Validator);
        assert!(!status.is_running);
        assert_eq!(status.current_height, 0);
        
        let _ = std::fs::remove_file(key_path);
    }
}

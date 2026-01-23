use crate::config::NodeConfig;
use crate::{Node, NodeComponents, NodeStats, NodeStatus, NodeType};
use blockchain_core::consensus::{Consensus, ProofOfAuthority, ValidatorSet};
use blockchain_core::{Block, Blockchain, GenesisBlock};
use common::{BlockHash, BlockHeight, PublicKey, Result, Timestamp, VotingError};
use crypto::keys::KeyPair;
use network::{Message, MessageType, NetworkManager};
use storage::StorageManager;
use std::sync::Arc;
use std::time::Duration;
use std::net::SocketAddr;
use tokio::sync::RwLock;
use tokio::time::interval;

/// Validator node - produces and validates blocks
pub struct ValidatorNode {
    config: NodeConfig,
    validator_key: Arc<KeyPair>,
    components: Arc<RwLock<Option<NodeComponents>>>,
    consensus: Arc<RwLock<Option<ProofOfAuthority>>>,
    running: Arc<RwLock<bool>>,
    started_at: Arc<RwLock<Option<Timestamp>>>,
    stats: Arc<RwLock<NodeStats>>,
}

impl ValidatorNode {
    pub fn new(config: NodeConfig) -> Result<Self> {
        // Validator nodes require a validator key
        let key_path = config
            .validator_key_path
            .as_ref()
            .ok_or_else(|| VotingError::ConfigError("Validator key path required".to_string()))?;

        let validator_key = KeyPair::load_from_file(key_path, "")?;

        Ok(Self {
            config,
            validator_key: Arc::new(validator_key),
            components: Arc::new(RwLock::new(None)),
            consensus: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
            started_at: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(NodeStats::default())),
        })
    }

    async fn initialize(&self) -> Result<()> {
        let genesis_path = self
            .config
            .genesis_path
            .as_ref()
            .ok_or_else(|| VotingError::ConfigError("Genesis path required".to_string()))?;

        let genesis = GenesisBlock::from_file(genesis_path.to_str().unwrap())?;

        // Initialize blockchain from genesis
        let blockchain = Arc::new(RwLock::new(Blockchain::new(genesis.clone())?));

        // Convert storage config from common to storage crate
        let storage_config = storage::StorageConfig {
            data_dir: self.config.storage.data_dir.clone(),
            blockchain_path: self.config.storage.blockchain_db_path.clone(),
            state_path: self.config.storage.state_db_path.clone(),
            enable_compression: self.config.storage.enable_compression,
            block_cache_capacity: 1000,
            tx_cache_capacity: 10000,
            state_cache_capacity: 5000,
            enable_pruning: self.config.storage.enable_pruning,
            pruning_retention_days: self.config.storage.pruning_days,
            max_db_size: Some(1024 * 1024 * 1024 * 100), // 100GB
            sync_mode: storage::SyncMode::Normal,
        };
        let storage = Arc::new(StorageManager::new(storage_config)?);

        // Convert network config from common to network crate
        let network_config = network::NetworkConfig {
            bootstrap_peers: self.config.network.bootstrap_peers.clone(),
            max_inbound: self.config.network.max_peers / 2,
            max_outbound: self.config.network.max_peers / 2,
            connection_timeout: self.config.network.connection_timeout,
            enable_discovery: self.config.network.enable_discovery,
            listen_addr: self.config.network.listen_addr,
            network_id: self.config.network.network_id.clone(),
            ..Default::default()
        };
        let network = Arc::new(NetworkManager::new(network_config)?);

        // Get validators from genesis
        let validators: Vec<PublicKey> = genesis
            .validators
            .iter()
            .map(|v| *v)
            .collect();

        let validator_set = ValidatorSet::new(validators.clone());
        let consensus = ProofOfAuthority::new(validator_set);

        // Verify our validator key is in the validator set
        let our_pubkey = PublicKey::new(*self.validator_key.public_key().as_bytes());
        if !consensus.is_validator(&our_pubkey) {
            return Err(VotingError::InvalidValidator("Unauthorized validator".to_string()));
        }

        // Log startup information
        tracing::info!("================================================");
        tracing::info!("Validator Node Initializing");
        tracing::info!("================================================");
        tracing::info!("Our validator address: {}", hex::encode(our_pubkey.as_bytes()));
        tracing::info!("Genesis validators ({} total):", validators.len());
        
        // Sort validators the same way consensus does for turn calculation
        let mut sorted_validators = validators.clone();
        sorted_validators.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        
        for (i, addr) in sorted_validators.iter().enumerate() {
            let is_us = addr.as_bytes() == our_pubkey.as_bytes();
            tracing::info!(
                "  [{}] {} {}",
                i,
                hex::encode(addr.as_bytes()),
                if is_us { "<-- THIS IS US" } else { "" }
            );
        }
        
        tracing::info!("================================================");
        tracing::info!("Block production schedule (first 10 blocks):");
        
        for height in 1..=10 {
            let producer = consensus.get_block_producer(height)?;
            let is_us = producer.as_bytes() == our_pubkey.as_bytes();
            tracing::info!(
                "  Block {}: {} {}",
                height,
                hex::encode(producer.as_bytes()),
                if is_us { "<-- OUR TURN" } else { "" }
            );
        }
        
        tracing::info!("================================================");

        *self.components.write().await = Some(NodeComponents {
            blockchain,
            storage,
            network,
        });

        *self.consensus.write().await = Some(consensus);

        Ok(())
    }

    async fn block_production_loop(&self) {
        // Add random startup delay to prevent all validators from trying at once
        let startup_delay = rand::random::<u64>() % 1000;
        tokio::time::sleep(Duration::from_millis(startup_delay)).await;
        
        let mut interval = interval(Duration::from_secs(10));

        loop {
            interval.tick().await;

            if !*self.running.read().await {
                break;
            }

            if let Err(e) = self.produce_block().await {
                // Only log real errors, not "not our turn" messages
                if !matches!(&e, VotingError::ConsensusError(msg) if msg.contains("Wrong validator turn")) {
                    tracing::error!("Failed to produce block: {:?}", e);
                }
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

        // Get our validator public key
        let our_pubkey = PublicKey::new(*self.validator_key.public_key().as_bytes());
        
        // Verify we're still a valid validator
        if !consensus.is_validator(&our_pubkey) {
            return Err(VotingError::InvalidValidator("Unauthorized validator".to_string()));
        }

        // Get current blockchain state
        let blockchain = components.blockchain.read().await;
        let current_height = blockchain.height();
        let next_height = current_height + 1;
        let previous_hash = blockchain.get_best_block()?.header.hash;
        drop(blockchain);

        // CHECK: Is it our turn to produce this block?
        let expected_producer = consensus.get_block_producer(next_height)?;
        
        if expected_producer.as_bytes() != our_pubkey.as_bytes() {
            // NOT our turn - skip silently
            tracing::trace!(
                "Not our turn at height {}. Expected: {}, We are: {}",
                next_height,
                hex::encode(expected_producer.as_bytes()),
                hex::encode(our_pubkey.as_bytes())
            );
            return Ok(());
        }

        tracing::info!(
            "Our turn to produce block at height {} (current: {})",
            next_height,
            current_height
        );

        // For now, create empty blocks (transaction pool will be added later)
        let transactions = vec![];

        // Create new block
        let mut block = Block::new(next_height, previous_hash, transactions, our_pubkey);

        // Sign the block
        let signature = self.validator_key.sign_message(&block.hash().0)?;
        let sig = common::Signature::new(signature.to_bytes());
        block.add_signature(our_pubkey, sig);

        // Validate block with consensus (this will check turn again)
        consensus.validate_block(&block)?;

        // Add block to blockchain
        let mut blockchain = components.blockchain.write().await;
        blockchain.add_block(block.clone())?;
        drop(blockchain);

        // Broadcast block to network
        let message = network::Message::new_block(block.clone())?;
        components.network.broadcast(message).await?;

        let mut stats = self.stats.write().await;
        stats.blocks_produced += 1;
        stats.blocks_validated += 1;

        tracing::info!(
            "Successfully produced and broadcast block at height {}",
            next_height
        );

        Ok(())
    }

    async fn handle_new_block(&self, block: &Block) -> Result<()> {
        let components = self.components.read().await;
        let Some(ref components) = *components else {
            return Err(VotingError::NodeNotInitialized);
        };

        let consensus = self.consensus.read().await;
        let Some(ref consensus) = *consensus else {
            return Err(VotingError::NodeNotInitialized);
        };

        // Validate block signatures
        if !consensus.is_validator(&block.header.validator) {
            return Err(VotingError::InvalidValidator("Unauthorized validator".to_string()));
        }

        // Validate block (includes turn checking)
        consensus.validate_block(&block)?;

        // Add to blockchain
        let mut blockchain = components.blockchain.write().await;
        blockchain.add_block(block.clone())?;

        let mut stats = self.stats.write().await;
        stats.blocks_validated += 1;

        tracing::info!(
            "Received and validated block at height {} from validator {}",
            block.header.height,
            hex::encode(block.header.validator.as_bytes())
        );

        Ok(())
    }

    pub fn validator_public_key(&self) -> PublicKey {
        PublicKey::new(*self.validator_key.public_key().as_bytes())
    }

    pub async fn is_validator(&self) -> bool {
        let consensus = self.consensus.read().await;
        if let Some(ref consensus) = *consensus {
            let our_pubkey = PublicKey::new(*self.validator_key.public_key().as_bytes());
            consensus.is_validator(&our_pubkey)
        } else {
            false
        }
    }
}

#[async_trait::async_trait]
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

        if self.config.enable_rpc {
            let rpc_addr = self.config.rpc_addr.clone()
                .ok_or_else(|| VotingError::ConfigError("RPC address required".to_string()))?;
            
            // Parse the address string to SocketAddr
            let rpc_addr_parsed: SocketAddr = rpc_addr.parse()
                .map_err(|e| VotingError::ConfigError(format!("Invalid RPC address: {}", e)))?;
            
            tracing::info!("Starting RPC server on {}", rpc_addr_parsed);
            
            let blockchain = Arc::clone(&components.blockchain);
            let network = Arc::clone(&components.network);
            
            tokio::spawn(async move {
                let service = Arc::new(rpc::service::NodeRpcService::new(blockchain, network));
                
                let rpc_config = rpc::RpcConfig {
                    listen_addr: rpc_addr_parsed.ip().to_string(),
                    port: rpc_addr_parsed.port(),
                    enable_cors: true,
                    enable_rate_limit: false,
                    ..Default::default()
                };
                
                let rpc_server = rpc::RpcServer::new(rpc_config, service);
                
                if let Err(e) = rpc_server.start().await {
                    tracing::error!("RPC server failed: {}", e);
                }
            });
        }

        *running = true;
        *self.started_at.write().await = Some(common::utils::current_timestamp());

        drop(running);
        drop(components);

        // Spawn block production loop
        let self_arc = Arc::new(self.clone_for_task());
        tokio::spawn(async move {
            self_arc.block_production_loop().await;
        });

        tracing::info!("Validator node started successfully");

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

        let (current_height, best_hash, peer_count, is_syncing) = if let Some(ref comp) = *components
        {
            let blockchain = comp.blockchain.read().await;
            let height = blockchain.height();
            let hash = blockchain.get_best_block()
                .map(|b| b.header.hash)
                .unwrap_or_else(|_| BlockHash::zero());
            drop(blockchain);

            (
                height,
                hash,
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

impl ValidatorNode {
    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            validator_key: Arc::clone(&self.validator_key),
            components: Arc::clone(&self.components),
            consensus: Arc::clone(&self.consensus),
            running: Arc::clone(&self.running),
            started_at: Arc::clone(&self.started_at),
            stats: Arc::clone(&self.stats),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    async fn test_validator_node_not_running() {
        // This test would require a valid key file
        // For now, we skip it as it would fail during creation
    }
}

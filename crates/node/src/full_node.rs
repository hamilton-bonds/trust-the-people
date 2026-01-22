use crate::config::NodeConfig;
use crate::{Node, NodeComponents, NodeStats, NodeStatus, NodeType};
use blockchain_core::{Block, Blockchain, GenesisBlock, Transaction};
use common::{BlockHash, BlockHeight, Result, Timestamp, TxId, VotingError};
use network::{Message, MessageType, NetworkManager};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use storage::StorageManager;

/// Full node - maintains complete blockchain and validates all blocks
pub struct FullNode {
    config: NodeConfig,
    components: Arc<RwLock<Option<NodeComponents>>>,
    running: Arc<RwLock<bool>>,
    started_at: Arc<RwLock<Option<Timestamp>>>,
    stats: Arc<RwLock<NodeStats>>,
    pending_transactions: Arc<RwLock<HashSet<TxId>>>,
}

impl FullNode {
    pub fn new(config: NodeConfig) -> Result<Self> {
        Ok(Self {
            config,
            components: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
            started_at: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(NodeStats::default())),
            pending_transactions: Arc::new(RwLock::new(HashSet::new())),
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
        let blockchain = Arc::new(RwLock::new(Blockchain::new(genesis)?));

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

        *self.components.write().await = Some(NodeComponents {
            blockchain,
            storage,
            network,
        });

        Ok(())
    }

    async fn sync_loop(&self) {
        let mut interval = interval(Duration::from_secs(5));

        loop {
            interval.tick().await;

            if !*self.running.read().await {
                break;
            }

            if let Err(e) = self.sync_with_network().await {
                tracing::error!("Sync error: {:?}", e);
            }
        }
    }

    async fn sync_with_network(&self) -> Result<()> {
        let components = self.components.read().await;
        let Some(ref components) = *components else {
            return Ok(());
        };

        let sync_status = components.network.sync_status().await;

        if sync_status.is_syncing() {
            tracing::debug!("Already syncing...");
            return Ok(());
        }

        // Check if we need to sync
        let current_height = components.get_current_height().await;
        
        // For now, we'll skip network height check until we implement it
        // In a real implementation, we'd query peers for their heights
        tracing::debug!("Current height: {}", current_height);

        Ok(())
    }

    async fn process_block(&self, block: &Block) -> Result<()> {
        let components = self.components.read().await;
        let Some(ref components) = *components else {
            return Err(VotingError::NodeNotInitialized);
        };

        // Add to blockchain (validation happens inside add_block)
        let mut blockchain = components.blockchain.write().await;
        blockchain.add_block(block.clone())?;
        drop(blockchain);

        // Remove processed transactions from pending
        let mut pending = self.pending_transactions.write().await;
        for tx in &block.transactions {
            pending.remove(&tx.id);
        }
        drop(pending);

        // Update stats
        let mut stats = self.stats.write().await;
        stats.blocks_processed += 1;
        stats.transactions_processed += block.transactions.len() as u64;

        tracing::debug!("Processed block at height {}", block.header.height);

        Ok(())
    }

    pub async fn submit_transaction(&self, tx: Transaction) -> Result<()> {
        let components = self.components.read().await;
        let Some(ref components) = *components else {
            return Err(VotingError::NodeNotInitialized);
        };

        // Validate transaction
        tx.validate()?;

        // Add to pending set
        self.pending_transactions.write().await.insert(tx.id);

        // Broadcast to network
        let message = network::Message::new_transaction(tx)?;
        components.network.broadcast(message).await?;

        Ok(())
    }

    pub async fn get_transaction(&self, tx_id: &TxId) -> Result<Option<Transaction>> {
        let components = self.components.read().await;
        let Some(ref components) = *components else {
            return Ok(None);
        };

        // Check blockchain
        let blockchain = components.blockchain.read().await;
        
        // Iterate through blocks to find transaction
        for height in 0..=blockchain.height() {
            if let Ok(block) = blockchain.get_block_at_height(height) {
                for tx in &block.transactions {
                    if tx.id == *tx_id {
                        return Ok(Some(tx.clone()));
                    }
                }
            }
        }

        Ok(None)
    }

    pub async fn get_block(&self, height: BlockHeight) -> Result<Option<Block>> {
        let components = self.components.read().await;
        let Some(ref components) = *components else {
            return Ok(None);
        };

        let blockchain = components.blockchain.read().await;
        match blockchain.get_block_at_height(height) {
            Ok(block) => Ok(Some(block.clone())),
            Err(_) => Ok(None),
        }
    }

    pub async fn pending_transaction_count(&self) -> usize {
        self.pending_transactions.read().await.len()
    }
}

#[async_trait::async_trait]
impl Node for FullNode {
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

        // Spawn sync loop
        let self_arc = Arc::new(self.clone_for_task());
        tokio::spawn(async move {
            self_arc.sync_loop().await;
        });

        tracing::info!("Full node started");

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

        tracing::info!("Full node stopped");

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
            node_type: NodeType::Full,
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

impl FullNode {
    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            components: Arc::clone(&self.components),
            running: Arc::clone(&self.running),
            started_at: Arc::clone(&self.started_at),
            stats: Arc::clone(&self.stats),
            pending_transactions: Arc::clone(&self.pending_transactions),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> NodeConfig {
        NodeConfig {
            node_type: NodeType::Full,
            genesis_path: None,
            ..Default::default()
        }
    }

    #[test]
    fn test_full_node_creation() {
        let config = create_test_config();
        let result = FullNode::new(config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_full_node_not_running() {
        let config = create_test_config();
        let node = FullNode::new(config).unwrap();
        assert!(!node.is_running().await);
    }

    #[tokio::test]
    async fn test_pending_transaction_count() {
        let config = create_test_config();
        let node = FullNode::new(config).unwrap();
        assert_eq!(node.pending_transaction_count().await, 0);
    }
}

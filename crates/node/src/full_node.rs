use crate::config::NodeConfig;
use crate::{Node, NodeComponents, NodeStats, NodeStatus, NodeType};
use blockchain_core::{Block, Blockchain, GenesisBlock, Transaction};
use common::{BlockHash, BlockHeight, Result, Timestamp, TxId, VotingError};
use network::{GossipMessage, Message, NetworkManager};
use storage::StorageManager;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// Full node - maintains complete blockchain and validates all blocks
pub struct FullNode {
    config: NodeConfig,
    components: Arc<RwLock<Option<NodeComponents>>>,
    running: Arc<RwLock<bool>>,
    started_at: Arc<RwLock<Option<Timestamp>>>,
    stats: Arc<RwLock<NodeStats>>,
    pending_blocks: Arc<RwLock<Vec<Block>>>,
    pending_transactions: Arc<RwLock<Vec<Transaction>>>,
}

impl FullNode {
    pub fn new(config: NodeConfig) -> Result<Self> {
        Ok(Self {
            config,
            components: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
            started_at: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(NodeStats::default())),
            pending_blocks: Arc::new(RwLock::new(Vec::new())),
            pending_transactions: Arc::new(RwLock::new(Vec::new())),
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
        
        *self.components.write().await = Some(components);
        
        Ok(())
    }
    
    async fn sync_loop(&self) {
        let mut ticker = interval(Duration::from_secs(30));
        
        loop {
            ticker.tick().await;
            
            let running = *self.running.read().await;
            if !running {
                break;
            }
            
            if let Err(e) = self.check_sync().await {
                tracing::warn!("Sync check error: {}", e);
            }
        }
    }
    
    async fn check_sync(&self) -> Result<()> {
        let components = self.components.read().await;
        let Some(ref components) = *components else {
            return Ok(());
        };
        
        let sync_status = components.network.sync_status().await;
        
        if !sync_status.is_syncing() {
            let current_height = components.get_current_height().await;
            let peers = components.network.get_peers().await;
            
            let max_peer_height = peers
                .iter()
                .map(|p| p.best_height)
                .max()
                .unwrap_or(0);
            
            if max_peer_height > current_height + 10 {
                tracing::info!(
                    "Starting sync: current={}, target={}",
                    current_height,
                    max_peer_height
                );
                
                components
                    .network
                    .sync_blocks(current_height + 1, max_peer_height)
                    .await?;
            }
        }
        
        Ok(())
    }
    
    async fn block_processing_loop(&self) {
        let mut ticker = interval(Duration::from_millis(100));
        
        loop {
            ticker.tick().await;
            
            let running = *self.running.read().await;
            if !running {
                break;
            }
            
            if let Err(e) = self.process_pending_blocks().await {
                tracing::warn!("Block processing error: {}", e);
            }
        }
    }
    
    async fn process_pending_blocks(&self) -> Result<()> {
        let mut pending = self.pending_blocks.write().await;
        
        if pending.is_empty() {
            return Ok(());
        }
        
        pending.sort_by_key(|b| b.height());
        
        let components = self.components.read().await;
        let Some(ref components) = *components else {
            return Ok(());
        };
        
        let mut processed = Vec::new();
        
        for (idx, block) in pending.iter().enumerate() {
            match self.validate_and_add_block(block, components).await {
                Ok(()) => {
                    processed.push(idx);
                    
                    let mut stats = self.stats.write().await;
                    stats.blocks_processed += 1;
                    
                    tracing::info!("Processed block {} at height {}", block.hash().to_hex(), block.height());
                }
                Err(e) => {
                    tracing::warn!("Failed to process block {}: {}", block.hash().to_hex(), e);
                }
            }
        }
        
        for idx in processed.iter().rev() {
            pending.remove(*idx);
        }
        
        Ok(())
    }
    
    async fn validate_and_add_block(
        &self,
        block: &Block,
        components: &NodeComponents,
    ) -> Result<()> {
        block.validate()?;
        
        let current_height = components.get_current_height().await;
        
        if block.height() != current_height + 1 {
            return Err(VotingError::BlockHeightMismatch {
                expected: current_height + 1,
                actual: block.height(),
            });
        }
        
        components.add_block(block.clone()).await?;
        
        components.storage.store_block(block).await?;
        
        for tx in &block.transactions {
            components.storage.remove_pending_transaction(&tx.id).await?;
        }
        
        Ok(())
    }
    
    async fn transaction_processing_loop(&self) {
        let mut ticker = interval(Duration::from_millis(500));
        
        loop {
            ticker.tick().await;
            
            let running = *self.running.read().await;
            if !running {
                break;
            }
            
            if let Err(e) = self.process_pending_transactions().await {
                tracing::warn!("Transaction processing error: {}", e);
            }
        }
    }
    
    async fn process_pending_transactions(&self) -> Result<()> {
        let mut pending = self.pending_transactions.write().await;
        
        if pending.is_empty() {
            return Ok(());
        }
        
        let components = self.components.read().await;
        let Some(ref components) = *components else {
            return Ok(());
        };
        
        let mut processed = Vec::new();
        
        for (idx, tx) in pending.iter().enumerate() {
            match self.validate_and_store_transaction(tx, components).await {
                Ok(()) => {
                    processed.push(idx);
                    
                    let mut stats = self.stats.write().await;
                    stats.transactions_processed += 1;
                }
                Err(e) => {
                    tracing::debug!("Failed to process transaction {}: {}", tx.id.to_hex(), e);
                }
            }
        }
        
        for idx in processed.iter().rev() {
            pending.remove(*idx);
        }
        
        Ok(())
    }
    
    async fn validate_and_store_transaction(
        &self,
        tx: &Transaction,
        components: &NodeComponents,
    ) -> Result<()> {
        tx.validate()?;
        
        let blockchain = components.blockchain.read().await;
        if blockchain.contains_transaction(&tx.id) {
            return Err(VotingError::DuplicateTransaction(tx.id.to_hex()));
        }
        drop(blockchain);
        
        components.storage.add_pending_transaction(tx).await?;
        
        Ok(())
    }
    
    pub async fn handle_new_block(&self, block: Block) -> Result<()> {
        let mut pending = self.pending_blocks.write().await;
        pending.push(block);
        Ok(())
    }
    
    pub async fn handle_new_transaction(&self, tx: Transaction) -> Result<()> {
        let mut pending = self.pending_transactions.write().await;
        pending.push(tx);
        Ok(())
    }
    
    pub async fn get_block(&self, height: BlockHeight) -> Result<Option<Block>> {
        let components = self.components.read().await;
        let Some(ref components) = *components else {
            return Err(VotingError::NodeNotInitialized);
        };
        
        Ok(components.get_block(height).await)
    }
    
    pub async fn get_transaction(&self, tx_id: &TxId) -> Result<Option<Transaction>> {
        let components = self.components.read().await;
        let Some(ref components) = *components else {
            return Err(VotingError::NodeNotInitialized);
        };
        
        components.storage.get_transaction(tx_id).await
    }
    
    pub async fn broadcast_transaction(&self, tx: Transaction) -> Result<()> {
        self.validate_and_store_transaction(
            &tx,
            self.components.read().await.as_ref().ok_or(VotingError::NodeNotInitialized)?,
        ).await?;
        
        let components = self.components.read().await;
        let Some(ref components) = *components else {
            return Err(VotingError::NodeNotInitialized);
        };
        
        let gossip_msg = GossipMessage::NewTransaction(tx);
        components.network.gossip(gossip_msg).await?;
        
        Ok(())
    }
    
    pub async fn pending_block_count(&self) -> usize {
        self.pending_blocks.read().await.len()
    }
    
    pub async fn pending_transaction_count(&self) -> usize {
        self.pending_transactions.read().await.len()
    }
}

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
        
        let node = Arc::new(self);
        
        let node_clone = Arc::clone(&node);
        tokio::spawn(async move {
            node_clone.sync_loop().await;
        });
        
        let node_clone = Arc::clone(&node);
        tokio::spawn(async move {
            node_clone.block_processing_loop().await;
        });
        
        let node_clone = Arc::clone(&node);
        tokio::spawn(async move {
            node_clone.transaction_processing_loop().await;
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
    async fn test_full_node_status() {
        let config = create_test_config();
        let node = FullNode::new(config).unwrap();
        let status = node.status().await;
        
        assert_eq!(status.node_type, NodeType::Full);
        assert!(!status.is_running);
        assert_eq!(status.current_height, 0);
    }

    #[tokio::test]
    async fn test_full_node_stats() {
        let config = create_test_config();
        let node = FullNode::new(config).unwrap();
        let stats = node.stats().await;
        
        assert_eq!(stats.blocks_processed, 0);
        assert_eq!(stats.transactions_processed, 0);
    }

    #[tokio::test]
    async fn test_pending_counts() {
        let config = create_test_config();
        let node = FullNode::new(config).unwrap();
        
        assert_eq!(node.pending_block_count().await, 0);
        assert_eq!(node.pending_transaction_count().await, 0);
    }

    #[tokio::test]
    async fn test_current_height_uninitialized() {
        let config = create_test_config();
        let node = FullNode::new(config).unwrap();
        
        assert_eq!(node.current_height().await, 0);
    }

    #[tokio::test]
    async fn test_best_hash_uninitialized() {
        let config = create_test_config();
        let node = FullNode::new(config).unwrap();
        
        assert_eq!(node.best_hash().await, BlockHash::zero());
    }
}

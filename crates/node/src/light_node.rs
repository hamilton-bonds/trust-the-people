use crate::config::NodeConfig;
use crate::{Node, NodeStats, NodeStatus, NodeType};
use blockchain_core::{Block, MerkleProof, Transaction};
use common::{BlockHash, BlockHeight, Result, Timestamp, TxId, VotingError};
use network::{Message, NetworkManager, PeerId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// Light node - verifies block headers and specific transactions without full blockchain
pub struct LightNode {
    config: NodeConfig,
    network: Arc<RwLock<Option<NetworkManager>>>,
    block_headers: Arc<RwLock<HashMap<BlockHeight, BlockHeader>>>,
    verified_transactions: Arc<RwLock<HashMap<TxId, VerifiedTransaction>>>,
    trusted_validators: Arc<RwLock<Vec<common::PublicKey>>>,
    running: Arc<RwLock<bool>>,
    started_at: Arc<RwLock<Option<Timestamp>>>,
    stats: Arc<RwLock<NodeStats>>,
    current_height: Arc<RwLock<BlockHeight>>,
}

/// Lightweight block header for verification
#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub height: BlockHeight,
    pub hash: BlockHash,
    pub previous_hash: BlockHash,
    pub transactions_root: BlockHash,
    pub timestamp: Timestamp,
    pub validator: common::PublicKey,
}

impl BlockHeader {
    pub fn from_block(block: &Block) -> Self {
        Self {
            height: block.header.height,
            hash: block.header.hash,
            previous_hash: block.header.previous_hash,
            transactions_root: block.header.transactions_root,
            timestamp: block.header.timestamp,
            validator: block.header.validator,
        }
    }
    
    pub fn validate(&self, previous: Option<&BlockHeader>) -> Result<()> {
        if let Some(prev) = previous {
            if self.height != prev.height + 1 {
                return Err(VotingError::BlockHeightMismatch {
                    expected: prev.height + 1,
                    actual: self.height,
                });
            }
            
            if self.previous_hash != prev.hash {
                return Err(VotingError::InvalidPreviousHash(format!(
                    "Expected {}, got {}",
                    prev.hash.to_hex(),
                    self.previous_hash.to_hex()
                )));
            }
            
            if self.timestamp < prev.timestamp {
                return Err(VotingError::InvalidBlock(
                    "Block timestamp is before previous block".to_string(),
                ));
            }
        }
        
        Ok(())
    }
}

/// Verified transaction with proof
#[derive(Debug, Clone)]
pub struct VerifiedTransaction {
    pub transaction: Transaction,
    pub block_height: BlockHeight,
    pub block_hash: BlockHash,
    pub merkle_proof: Vec<u8>,
    pub verified_at: Timestamp,
}

impl LightNode {
    pub fn new(config: NodeConfig) -> Result<Self> {
        Ok(Self {
            config,
            network: Arc::new(RwLock::new(None)),
            block_headers: Arc::new(RwLock::new(HashMap::new())),
            verified_transactions: Arc::new(RwLock::new(HashMap::new())),
            trusted_validators: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(RwLock::new(false)),
            started_at: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(NodeStats::default())),
            current_height: Arc::new(RwLock::new(0)),
        })
    }
    
    async fn initialize(&self) -> Result<()> {
        let network = NetworkManager::new(self.config.network.clone())?;
        *self.network.write().await = Some(network);
        
        if let Some(genesis_path) = &self.config.genesis_path {
            let genesis = blockchain_core::GenesisBlock::from_file(genesis_path)?;
            
            let validators = genesis.validators.clone();
            *self.trusted_validators.write().await = validators;
        }
        
        Ok(())
    }
    
    async fn header_sync_loop(&self) {
        let mut ticker = interval(Duration::from_secs(10));
        
        loop {
            ticker.tick().await;
            
            let running = *self.running.read().await;
            if !running {
                break;
            }
            
            if let Err(e) = self.sync_headers().await {
                tracing::warn!("Header sync error: {}", e);
            }
        }
    }
    
    async fn sync_headers(&self) -> Result<()> {
        let network = self.network.read().await;
        let Some(ref network) = *network else {
            return Ok(());
        };
        
        let peers = network.get_peers().await;
        if peers.is_empty() {
            return Ok(());
        }
        
        let current_height = *self.current_height.read().await;
        
        let max_peer_height = peers.iter().map(|p| p.best_height).max().unwrap_or(0);
        
        if max_peer_height > current_height {
            self.request_headers(current_height + 1, max_peer_height, network)
                .await?;
        }
        
        Ok(())
    }
    
    async fn request_headers(
        &self,
        start: BlockHeight,
        end: BlockHeight,
        network: &NetworkManager,
    ) -> Result<()> {
        let batch_size = 100;
        let mut current = start;
        
        while current <= end {
            let batch_end = (current + batch_size).min(end);
            
            let request = network::BlockRequest {
                start_height: current,
                end_height: batch_end,
                max_blocks: batch_size as usize,
            };
            
            let message = Message::get_blocks(request)?;
            network.broadcast(message).await?;
            
            current = batch_end + 1;
        }
        
        Ok(())
    }
    
    pub async fn handle_block_headers(&self, blocks: Vec<Block>) -> Result<()> {
        let mut headers = self.block_headers.write().await;
        let validators = self.trusted_validators.read().await;
        
        for block in blocks {
            let header = BlockHeader::from_block(&block);
            
            if !validators.contains(&header.validator) {
                tracing::warn!("Block from untrusted validator: {}", header.hash.to_hex());
                continue;
            }
            
            let previous = if header.height > 0 {
                headers.get(&(header.height - 1))
            } else {
                None
            };
            
            header.validate(previous)?;
            
            headers.insert(header.height, header.clone());
            
            let mut current_height = self.current_height.write().await;
            if header.height > *current_height {
                *current_height = header.height;
            }
            
            let mut stats = self.stats.write().await;
            stats.blocks_processed += 1;
        }
        
        Ok(())
    }
    
    pub async fn verify_transaction(
        &self,
        tx: Transaction,
        block_height: BlockHeight,
        merkle_proof: MerkleProof,
    ) -> Result<bool> {
        let headers = self.block_headers.read().await;
        let header = headers
            .get(&block_height)
            .ok_or_else(|| VotingError::BlockNotFound(block_height.to_string()))?;
        
        if !merkle_proof.verify() {
            return Ok(false);
        }
        
        if merkle_proof.root != header.transactions_root {
            return Ok(false);
        }
        
        let verified = VerifiedTransaction {
            transaction: tx.clone(),
            block_height,
            block_hash: header.hash,
            merkle_proof: vec![],
            verified_at: common::utils::current_timestamp(),
        };
        
        self.verified_transactions
            .write()
            .await
            .insert(tx.id, verified);
        
        let mut stats = self.stats.write().await;
        stats.transactions_processed += 1;
        
        Ok(true)
    }
    
    pub async fn get_header(&self, height: BlockHeight) -> Option<BlockHeader> {
        self.block_headers.read().await.get(&height).cloned()
    }
    
    pub async fn get_verified_transaction(&self, tx_id: &TxId) -> Option<VerifiedTransaction> {
        self.verified_transactions.read().await.get(tx_id).cloned()
    }
    
    pub async fn header_count(&self) -> usize {
        self.block_headers.read().await.len()
    }
    
    pub async fn verified_transaction_count(&self) -> usize {
        self.verified_transactions.read().await.len()
    }
    
    pub async fn add_trusted_validator(&self, validator: common::PublicKey) {
        self.trusted_validators.write().await.push(validator);
    }
    
    pub async fn remove_trusted_validator(&self, validator: &common::PublicKey) {
        let mut validators = self.trusted_validators.write().await;
        validators.retain(|v| v != validator);
    }
    
    pub async fn trusted_validator_count(&self) -> usize {
        self.trusted_validators.read().await.len()
    }
    
    pub async fn is_validator_trusted(&self, validator: &common::PublicKey) -> bool {
        self.trusted_validators.read().await.contains(validator)
    }
}

impl Node for LightNode {
    async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Err(VotingError::NodeAlreadyRunning);
        }
        
        self.initialize().await?;
        
        let network = self.network.read().await;
        let Some(ref network) = *network else {
            return Err(VotingError::NodeNotInitialized);
        };
        
        network.start().await?;
        
        *running = true;
        *self.started_at.write().await = Some(common::utils::current_timestamp());
        
        drop(running);
        drop(network);
        
        let node = Arc::new(self);
        
        let node_clone = Arc::clone(&node);
        tokio::spawn(async move {
            node_clone.header_sync_loop().await;
        });
        
        tracing::info!("Light node started");
        
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if !*running {
            return Err(VotingError::NodeNotRunning);
        }
        
        *running = false;
        
        let network = self.network.read().await;
        if let Some(ref network) = *network {
            network.stop().await?;
        }
        
        tracing::info!("Light node stopped");
        
        Ok(())
    }
    
    async fn status(&self) -> NodeStatus {
        let network = self.network.read().await;
        
        let (peer_count, is_syncing) = if let Some(ref net) = *network {
            (
                net.peer_count().await,
                net.sync_status().await.is_syncing(),
            )
        } else {
            (0, false)
        };
        
        let current_height = *self.current_height.read().await;
        let best_hash = self
            .block_headers
            .read()
            .await
            .get(&current_height)
            .map(|h| h.hash)
            .unwrap_or(BlockHash::zero());
        
        let started_at = *self.started_at.read().await;
        let uptime = if let Some(start) = started_at {
            common::utils::current_timestamp().saturating_sub(start)
        } else {
            0
        };
        
        NodeStatus {
            node_type: NodeType::Light,
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
        *self.current_height.read().await
    }
    
    async fn best_hash(&self) -> BlockHash {
        let current_height = *self.current_height.read().await;
        self.block_headers
            .read()
            .await
            .get(&current_height)
            .map(|h| h.hash)
            .unwrap_or(BlockHash::zero())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::PublicKey;

    fn create_test_config() -> NodeConfig {
        NodeConfig {
            node_type: NodeType::Light,
            genesis_path: None,
            ..Default::default()
        }
    }

    #[test]
    fn test_light_node_creation() {
        let config = create_test_config();
        let result = LightNode::new(config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_light_node_not_running() {
        let config = create_test_config();
        let node = LightNode::new(config).unwrap();
        assert!(!node.is_running().await);
    }

    #[tokio::test]
    async fn test_light_node_status() {
        let config = create_test_config();
        let node = LightNode::new(config).unwrap();
        let status = node.status().await;
        
        assert_eq!(status.node_type, NodeType::Light);
        assert!(!status.is_running);
        assert_eq!(status.current_height, 0);
    }

    #[tokio::test]
    async fn test_light_node_stats() {
        let config = create_test_config();
        let node = LightNode::new(config).unwrap();
        let stats = node.stats().await;
        
        assert_eq!(stats.blocks_processed, 0);
        assert_eq!(stats.transactions_processed, 0);
    }

    #[tokio::test]
    async fn test_header_count() {
        let config = create_test_config();
        let node = LightNode::new(config).unwrap();
        
        assert_eq!(node.header_count().await, 0);
    }

    #[tokio::test]
    async fn test_verified_transaction_count() {
        let config = create_test_config();
        let node = LightNode::new(config).unwrap();
        
        assert_eq!(node.verified_transaction_count().await, 0);
    }

    #[tokio::test]
    async fn test_trusted_validators() {
        let config = create_test_config();
        let node = LightNode::new(config).unwrap();
        
        let validator = PublicKey::new([1u8; 32]);
        
        assert!(!node.is_validator_trusted(&validator).await);
        assert_eq!(node.trusted_validator_count().await, 0);
        
        node.add_trusted_validator(validator).await;
        
        assert!(node.is_validator_trusted(&validator).await);
        assert_eq!(node.trusted_validator_count().await, 1);
        
        node.remove_trusted_validator(&validator).await;
        
        assert!(!node.is_validator_trusted(&validator).await);
        assert_eq!(node.trusted_validator_count().await, 0);
    }

    #[test]
    fn test_block_header_from_block() {
        let block = Block::new(
            1,
            BlockHash::zero(),
            vec![],
            PublicKey::new([1u8; 32]),
        );
        
        let header = BlockHeader::from_block(&block);
        
        assert_eq!(header.height, 1);
        assert_eq!(header.hash, block.hash());
    }

    #[test]
    fn test_block_header_validation() {
        let header1 = BlockHeader {
            height: 0,
            hash: BlockHash::new([1u8; 32]),
            previous_hash: BlockHash::zero(),
            transactions_root: BlockHash::zero(),
            timestamp: 1000,
            validator: PublicKey::new([1u8; 32]),
        };
        
        let header2 = BlockHeader {
            height: 1,
            hash: BlockHash::new([2u8; 32]),
            previous_hash: header1.hash,
            transactions_root: BlockHash::zero(),
            timestamp: 2000,
            validator: PublicKey::new([1u8; 32]),
        };
        
        assert!(header1.validate(None).is_ok());
        assert!(header2.validate(Some(&header1)).is_ok());
    }

    #[test]
    fn test_block_header_invalid_height() {
        let header1 = BlockHeader {
            height: 0,
            hash: BlockHash::new([1u8; 32]),
            previous_hash: BlockHash::zero(),
            transactions_root: BlockHash::zero(),
            timestamp: 1000,
            validator: PublicKey::new([1u8; 32]),
        };
        
        let header2 = BlockHeader {
            height: 5,
            hash: BlockHash::new([2u8; 32]),
            previous_hash: header1.hash,
            transactions_root: BlockHash::zero(),
            timestamp: 2000,
            validator: PublicKey::new([1u8; 32]),
        };
        
        assert!(header2.validate(Some(&header1)).is_err());
    }

    #[tokio::test]
    async fn test_get_header() {
        let config = create_test_config();
        let node = LightNode::new(config).unwrap();
        
        let header = node.get_header(0).await;
        assert!(header.is_none());
    }

    #[tokio::test]
    async fn test_current_height() {
        let config = create_test_config();
        let node = LightNode::new(config).unwrap();
        
        assert_eq!(node.current_height().await, 0);
    }

    #[tokio::test]
    async fn test_best_hash() {
        let config = create_test_config();
        let node = LightNode::new(config).unwrap();
        
        assert_eq!(node.best_hash().await, BlockHash::zero());
    }
}

use crate::config::NodeConfig;
use crate::{Node, NodeStats, NodeStatus, NodeType};
use blockchain_core::merkle::MerkleProof;
use blockchain_core::{Block, Transaction};
use common::{BlockHash, BlockHeight, PublicKey, Result, Timestamp, TxId, VotingError};
use network::NetworkManager;
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
    trusted_validators: Arc<RwLock<Vec<PublicKey>>>,
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
    pub validator: PublicKey,
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
                    prev.hash, self.previous_hash
                )));
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
    pub merkle_proof: Vec<BlockHash>,
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

        let network = NetworkManager::new(network_config)?;

        // Load genesis if provided
        if let Some(ref genesis_path) = self.config.genesis_path {
            let genesis = blockchain_core::GenesisBlock::from_file(genesis_path.to_str().unwrap())?;

            // Add genesis validators as trusted
            let mut trusted = self.trusted_validators.write().await;
            for validator in &genesis.validators {
                trusted.push(*validator);
            }
            drop(trusted);

            // Create genesis block manually
            let genesis_block = Block::new(
                0,
                BlockHash::zero(),
                vec![],
                genesis.validators.first()
                    .map(|v| *v)
                    .unwrap_or_else(|| PublicKey::new([0u8; 32])),
            );
            
            let header = BlockHeader::from_block(&genesis_block);
            self.block_headers.write().await.insert(0, header);
        }

        *self.network.write().await = Some(network);

        Ok(())
    }

    async fn header_sync_loop(&self) {
        let mut interval = interval(Duration::from_secs(10));

        loop {
            interval.tick().await;

            if !*self.running.read().await {
                break;
            }

            if let Err(e) = self.sync_headers().await {
                tracing::error!("Header sync error: {:?}", e);
            }
        }
    }

    async fn sync_headers(&self) -> Result<()> {
        let _network = self.network.read().await;
        let Some(ref _network) = *_network else {
            return Ok(());
        };

        // Network height check will be implemented when we add the method
        // For now, just log that we're syncing
        let current_height = *self.current_height.read().await;
        tracing::debug!("Light node syncing headers from height {}", current_height);

        Ok(())
    }

    pub async fn verify_transaction(
        &self,
        tx: &Transaction,
        block_height: BlockHeight,
        merkle_proof: MerkleProof,
    ) -> Result<bool> {
        let headers = self.block_headers.read().await;
        let Some(header) = headers.get(&block_height) else {
            return Err(VotingError::BlockNotFound(block_height.to_string()));
        };

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

    pub async fn add_trusted_validator(&self, validator: PublicKey) {
        self.trusted_validators.write().await.push(validator);
    }

    pub async fn remove_trusted_validator(&self, validator: &PublicKey) {
        let mut validators = self.trusted_validators.write().await;
        validators.retain(|v| v != validator);
    }

    pub async fn trusted_validator_count(&self) -> usize {
        self.trusted_validators.read().await.len()
    }

    pub async fn is_validator_trusted(&self, validator: &PublicKey) -> bool {
        self.trusted_validators.read().await.contains(validator)
    }
}

#[async_trait::async_trait]
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

        let self_arc = Arc::new(self.clone_for_task());
        tokio::spawn(async move {
            self_arc.header_sync_loop().await;
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
            .unwrap_or_else(BlockHash::zero);

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
            .unwrap_or_else(BlockHash::zero)
    }
}

impl LightNode {
    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            network: Arc::clone(&self.network),
            block_headers: Arc::clone(&self.block_headers),
            verified_transactions: Arc::clone(&self.verified_transactions),
            trusted_validators: Arc::clone(&self.trusted_validators),
            running: Arc::clone(&self.running),
            started_at: Arc::clone(&self.started_at),
            stats: Arc::clone(&self.stats),
            current_height: Arc::clone(&self.current_height),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    async fn test_header_count() {
        let config = create_test_config();
        let node = LightNode::new(config).unwrap();
        assert_eq!(node.header_count().await, 0);
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
}

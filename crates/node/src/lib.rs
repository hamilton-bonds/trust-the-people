pub mod validator;
pub mod full_node;
pub mod light_node;
pub mod config;

pub use validator::ValidatorNode;
pub use full_node::FullNode;
pub use light_node::LightNode;
pub use config::{NodeConfig, NodeType};

use blockchain_core::{Block, Blockchain, Transaction};
use common::{BlockHash, BlockHeight, Result, Timestamp, VotingError};
use network::NetworkManager;
use storage::StorageManager;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Node status information
#[derive(Debug, Clone)]
pub struct NodeStatus {
    pub node_type: NodeType,
    pub is_running: bool,
    pub is_syncing: bool,
    pub current_height: BlockHeight,
    pub best_hash: BlockHash,
    pub peer_count: usize,
    pub uptime: u64,
    pub started_at: Option<Timestamp>,
}

/// Node statistics
#[derive(Debug, Clone, Default)]
pub struct NodeStats {
    pub blocks_processed: u64,
    pub transactions_processed: u64,
    pub votes_verified: u64,
    pub blocks_produced: u64,
    pub blocks_validated: u64,
    pub network_messages_sent: u64,
    pub network_messages_received: u64,
}

/// Core node trait implemented by all node types
pub trait Node: Send + Sync {
    /// Start the node
    fn start(&self) -> impl std::future::Future<Output = Result<()>> + Send;
    
    /// Stop the node
    fn stop(&self) -> impl std::future::Future<Output = Result<()>> + Send;
    
    /// Get node status
    fn status(&self) -> impl std::future::Future<Output = NodeStatus> + Send;
    
    /// Get node statistics
    fn stats(&self) -> impl std::future::Future<Output = NodeStats> + Send;
    
    /// Check if node is running
    fn is_running(&self) -> impl std::future::Future<Output = bool> + Send;
    
    /// Get current blockchain height
    fn current_height(&self) -> impl std::future::Future<Output = BlockHeight> + Send;
    
    /// Get best block hash
    fn best_hash(&self) -> impl std::future::Future<Output = BlockHash> + Send;
}

/// Node builder for creating different node types
pub struct NodeBuilder {
    config: NodeConfig,
}

impl NodeBuilder {
    pub fn new(config: NodeConfig) -> Self {
        Self { config }
    }
    
    pub fn build(self) -> Result<Box<dyn Node>> {
        match self.config.node_type {
            NodeType::Validator => {
                let node = ValidatorNode::new(self.config)?;
                Ok(Box::new(node))
            }
            NodeType::Full => {
                let node = FullNode::new(self.config)?;
                Ok(Box::new(node))
            }
            NodeType::Light => {
                let node = LightNode::new(self.config)?;
                Ok(Box::new(node))
            }
        }
    }
}

/// Shared node components used by all node types
pub struct NodeComponents {
    pub blockchain: Arc<RwLock<Blockchain>>,
    pub storage: Arc<StorageManager>,
    pub network: Arc<NetworkManager>,
}

impl NodeComponents {
    pub async fn new(
        blockchain: Blockchain,
        storage: StorageManager,
        network: NetworkManager,
    ) -> Self {
        Self {
            blockchain: Arc::new(RwLock::new(blockchain)),
            storage: Arc::new(storage),
            network: Arc::new(network),
        }
    }
    
    pub async fn add_block(&self, block: Block) -> Result<()> {
        let mut blockchain = self.blockchain.write().await;
        blockchain.add_block(block)?;
        Ok(())
    }
    
    pub async fn get_block(&self, height: BlockHeight) -> Option<Block> {
        let blockchain = self.blockchain.read().await;
        blockchain.get_block_by_height(height).cloned()
    }
    
    pub async fn get_current_height(&self) -> BlockHeight {
        self.blockchain.read().await.height()
    }
    
    pub async fn get_best_hash(&self) -> BlockHash {
        self.blockchain.read().await.latest_block().hash()
    }
    
    pub async fn verify_blockchain(&self) -> Result<()> {
        self.blockchain.read().await.verify()
    }
}

/// Node event types
#[derive(Debug, Clone)]
pub enum NodeEvent {
    Started,
    Stopped,
    BlockReceived(BlockHash),
    BlockProcessed(BlockHash),
    BlockProduced(BlockHash),
    TransactionReceived(common::TxId),
    PeerConnected(network::PeerId),
    PeerDisconnected(network::PeerId),
    SyncStarted,
    SyncCompleted,
    Error(String),
}

/// Node event handler
pub trait NodeEventHandler: Send + Sync {
    fn handle_event(&self, event: NodeEvent) -> impl std::future::Future<Output = ()> + Send;
}

/// Default event handler that logs events
pub struct DefaultEventHandler;

impl NodeEventHandler for DefaultEventHandler {
    async fn handle_event(&self, event: NodeEvent) {
        match event {
            NodeEvent::Started => tracing::info!("Node started"),
            NodeEvent::Stopped => tracing::info!("Node stopped"),
            NodeEvent::BlockReceived(hash) => {
                tracing::debug!("Block received: {}", hash.to_hex())
            }
            NodeEvent::BlockProcessed(hash) => {
                tracing::info!("Block processed: {}", hash.to_hex())
            }
            NodeEvent::BlockProduced(hash) => {
                tracing::info!("Block produced: {}", hash.to_hex())
            }
            NodeEvent::TransactionReceived(tx_id) => {
                tracing::debug!("Transaction received: {}", tx_id.to_hex())
            }
            NodeEvent::PeerConnected(peer_id) => {
                tracing::info!("Peer connected: {}", peer_id)
            }
            NodeEvent::PeerDisconnected(peer_id) => {
                tracing::info!("Peer disconnected: {}", peer_id)
            }
            NodeEvent::SyncStarted => tracing::info!("Blockchain sync started"),
            NodeEvent::SyncCompleted => tracing::info!("Blockchain sync completed"),
            NodeEvent::Error(msg) => tracing::error!("Node error: {}", msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_type() {
        let types = vec![NodeType::Validator, NodeType::Full, NodeType::Light];
        assert_eq!(types.len(), 3);
    }

    #[test]
    fn test_node_stats_default() {
        let stats = NodeStats::default();
        assert_eq!(stats.blocks_processed, 0);
        assert_eq!(stats.transactions_processed, 0);
    }

    #[test]
    fn test_node_event() {
        let event = NodeEvent::Started;
        assert!(matches!(event, NodeEvent::Started));
        
        let event = NodeEvent::Error("test".to_string());
        assert!(matches!(event, NodeEvent::Error(_)));
    }

    #[tokio::test]
    async fn test_default_event_handler() {
        let handler = DefaultEventHandler;
        handler.handle_event(NodeEvent::Started).await;
        handler.handle_event(NodeEvent::Stopped).await;
    }

    #[test]
    fn test_node_builder() {
        let config = NodeConfig::default();
        let builder = NodeBuilder::new(config);
        
        let result = builder.build();
        assert!(result.is_ok());
    }
}

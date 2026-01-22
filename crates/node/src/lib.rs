pub mod validator;
pub mod full_node;
pub mod light_node;
pub mod config;

pub use validator::ValidatorNode;
pub use full_node::FullNode;
pub use light_node::LightNode;
pub use config::{NodeConfig, NodeType};

use blockchain_core::Block;
use common::{BlockHash, BlockHeight, Result, Timestamp};
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
#[async_trait::async_trait]
pub trait Node: Send + Sync {
    /// Start the node
    async fn start(&self) -> Result<()>;
    
    /// Stop the node
    async fn stop(&self) -> Result<()>;
    
    /// Get node status
    async fn status(&self) -> NodeStatus;
    
    /// Get node statistics
    async fn stats(&self) -> NodeStats;
    
    /// Check if node is running
    async fn is_running(&self) -> bool;
    
    /// Get current blockchain height
    async fn current_height(&self) -> BlockHeight;
    
    /// Get best block hash
    async fn best_hash(&self) -> BlockHash;
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
                Ok(Box::new(node) as Box<dyn Node>)
            }
            NodeType::Full => {
                let node = FullNode::new(self.config)?;
                Ok(Box::new(node) as Box<dyn Node>)
            }
            NodeType::Light => {
                let node = LightNode::new(self.config)?;
                Ok(Box::new(node) as Box<dyn Node>)
            }
        }
    }
}

/// Shared components used by all node types
pub struct NodeComponents {
    pub blockchain: Arc<RwLock<blockchain_core::Blockchain>>,
    pub storage: Arc<StorageManager>,
    pub network: Arc<NetworkManager>,
}

impl NodeComponents {
    pub async fn get_current_height(&self) -> BlockHeight {
        self.blockchain.read().await.height()
    }
    
    pub async fn get_best_hash(&self) -> BlockHash {
        self.blockchain.read().await
            .get_best_block()
            .map(|b| b.header.hash)
            .unwrap_or_else(|_| BlockHash::zero())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_stats_default() {
        let stats = NodeStats::default();
        assert_eq!(stats.blocks_processed, 0);
        assert_eq!(stats.transactions_processed, 0);
        assert_eq!(stats.votes_verified, 0);
    }

    #[test]
    fn test_node_status_creation() {
        let status = NodeStatus {
            node_type: NodeType::Full,
            is_running: true,
            is_syncing: false,
            current_height: 10,
            best_hash: BlockHash::zero(),
            peer_count: 5,
            uptime: 3600,
            started_at: Some(1234567890),
        };
        
        assert_eq!(status.node_type, NodeType::Full);
        assert!(status.is_running);
        assert_eq!(status.current_height, 10);
    }
}

//! Node-related RPC methods
//!
//! This module implements RPC methods for querying node state:
//! - Node information and status
//! - Sync status
//! - Health checks

use crate::{HealthResponse, NodeInfo, SyncStatusResponse};
use blockchain_core::Chain;
use common::{Result, VotingError};
use network::{NetworkManager, SyncManager};
use storage::Database;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Node query service
pub struct NodeMethods {
    node_type: String,
    version: String,
    network_id: String,
    public_key: Option<String>,
    chain: Arc<RwLock<Chain>>,
    network_manager: Arc<RwLock<NetworkManager>>,
    sync_manager: Arc<RwLock<SyncManager>>,
    database: Arc<dyn Database>,
    start_time: Instant,
}

impl NodeMethods {
    /// Create new node methods handler
    pub fn new(
        node_type: String,
        version: String,
        network_id: String,
        public_key: Option<String>,
        chain: Arc<RwLock<Chain>>,
        network_manager: Arc<RwLock<NetworkManager>>,
        sync_manager: Arc<RwLock<SyncManager>>,
        database: Arc<dyn Database>,
    ) -> Self {
        Self {
            node_type,
            version,
            network_id,
            public_key,
            chain,
            network_manager,
            sync_manager,
            database,
            start_time: Instant::now(),
        }
    }

    /// Get node information
    pub async fn node_info(&self) -> Result<NodeInfo> {
        let chain = self.chain.read().await;
        let network_manager = self.network_manager.read().await;
        let sync_manager = self.sync_manager.read().await;

        let latest_block = chain.latest_block();

        Ok(NodeInfo {
            version: self.version.clone(),
            node_type: self.node_type.clone(),
            public_key: self.public_key.clone().unwrap_or_else(|| "none".to_string()),
            network_id: self.network_id.clone(),
            height: chain.height(),
            latest_block_hash: latest_block.hash().to_hex(),
            peer_count: network_manager.peer_count(),
            syncing: sync_manager.is_syncing(),
            uptime: self.start_time.elapsed().as_secs(),
        })
    }

    /// Get node sync status
    pub async fn sync_status(&self) -> Result<SyncStatusResponse> {
        let chain = self.chain.read().await;
        let sync_manager = self.sync_manager.read().await;

        let current_height = chain.height();
        let target_height = sync_manager.target_height().unwrap_or(current_height);
        let syncing = sync_manager.is_syncing();

        let progress = if target_height > 0 {
            (current_height as f64 / target_height as f64) * 100.0
        } else {
            100.0
        };

        let estimated_time_remaining = if syncing && target_height > current_height {
            let blocks_remaining = target_height - current_height;
            let sync_rate = sync_manager.blocks_per_second();
            
            if sync_rate > 0.0 {
                Some((blocks_remaining as f64 / sync_rate) as u64)
            } else {
                None
            }
        } else {
            None
        };

        Ok(SyncStatusResponse {
            syncing,
            current_height,
            target_height,
            progress,
            estimated_time_remaining,
        })
    }

    /// Get health status
    pub async fn health(&self) -> Result<HealthResponse> {
        let database_status = self.check_database_health().await;
        let network_status = self.check_network_health().await;
        let consensus_status = self.check_consensus_health().await;

        let overall_status = if database_status == "ok"
            && network_status == "ok"
            && consensus_status == "ok"
        {
            "healthy"
        } else if database_status == "error" {
            "critical"
        } else {
            "degraded"
        };

        Ok(HealthResponse {
            status: overall_status.to_string(),
            database: database_status,
            network: network_status,
            consensus: consensus_status,
            timestamp: common::utils::current_timestamp(),
        })
    }

    // Helper methods

    async fn check_database_health(&self) -> String {
        // Try a simple database operation
        match self.database.get(b"health_check") {
            Ok(_) => "ok".to_string(),
            Err(e) => {
                tracing::error!("Database health check failed: {}", e);
                "error".to_string()
            }
        }
    }

    async fn check_network_health(&self) -> String {
        let network_manager = self.network_manager.read().await;
        let peer_count = network_manager.peer_count();

        if peer_count == 0 {
            "warning".to_string()
        } else if peer_count < 3 {
            "degraded".to_string()
        } else {
            "ok".to_string()
        }
    }

    async fn check_consensus_health(&self) -> String {
        let chain = self.chain.read().await;
        let sync_manager = self.sync_manager.read().await;

        // Check if we're severely out of sync
        if let Some(target_height) = sync_manager.target_height() {
            let current_height = chain.height();
            let blocks_behind = target_height.saturating_sub(current_height);

            if blocks_behind > 1000 {
                return "warning".to_string();
            } else if blocks_behind > 100 {
                return "degraded".to_string();
            }
        }

        "ok".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockchain_core::GenesisBlock;
    use network::{NetworkConfig, SyncConfig};
    use storage::MemoryDatabase;

    #[tokio::test]
    async fn test_node_info() {
        let genesis = GenesisBlock::default();
        let chain = Arc::new(RwLock::new(Chain::new(genesis).unwrap()));
        let network_config = NetworkConfig::default();
        let network_manager = Arc::new(RwLock::new(NetworkManager::new(network_config)));
        let sync_config = SyncConfig::default();
        let sync_manager = Arc::new(RwLock::new(SyncManager::new(sync_config)));
        let database = Arc::new(MemoryDatabase::new());

        let methods = NodeMethods::new(
            "full".to_string(),
            "1.0.0".to_string(),
            "testnet".to_string(),
            None,
            chain,
            network_manager,
            sync_manager,
            database,
        );

        let info = methods.node_info().await.unwrap();
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.node_type, "full");
        assert_eq!(info.height, 0);
    }

    #[tokio::test]
    async fn test_sync_status_not_syncing() {
        let genesis = GenesisBlock::default();
        let chain = Arc::new(RwLock::new(Chain::new(genesis).unwrap()));
        let network_config = NetworkConfig::default();
        let network_manager = Arc::new(RwLock::new(NetworkManager::new(network_config)));
        let sync_config = SyncConfig::default();
        let sync_manager = Arc::new(RwLock::new(SyncManager::new(sync_config)));
        let database = Arc::new(MemoryDatabase::new());

        let methods = NodeMethods::new(
            "full".to_string(),
            "1.0.0".to_string(),
            "testnet".to_string(),
            None,
            chain,
            network_manager,
            sync_manager,
            database,
        );

        let status = methods.sync_status().await.unwrap();
        assert!(!status.syncing);
        assert_eq!(status.current_height, 0);
    }

    #[tokio::test]
    async fn test_health_check() {
        let genesis = GenesisBlock::default();
        let chain = Arc::new(RwLock::new(Chain::new(genesis).unwrap()));
        let network_config = NetworkConfig::default();
        let network_manager = Arc::new(RwLock::new(NetworkManager::new(network_config)));
        let sync_config = SyncConfig::default();
        let sync_manager = Arc::new(RwLock::new(SyncManager::new(sync_config)));
        let database = Arc::new(MemoryDatabase::new());

        let methods = NodeMethods::new(
            "full".to_string(),
            "1.0.0".to_string(),
            "testnet".to_string(),
            None,
            chain,
            network_manager,
            sync_manager,
            database,
        );

        let health = methods.health().await.unwrap();
        assert_eq!(health.database, "ok");
        assert!(health.timestamp > 0);
    }
}

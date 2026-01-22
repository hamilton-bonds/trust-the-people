//! Node-related RPC methods
//!
//! This module implements RPC methods for querying node state:
//! - Node information and status
//! - Sync status
//! - Health checks

use crate::{NodeInfo, SyncStatusResponse};
use blockchain_core::chain::Blockchain;
use common::Result;
use network::NetworkManager;
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
    chain: Arc<RwLock<Blockchain>>,
    network_manager: Arc<RwLock<NetworkManager>>,
    database: Arc<dyn Database>,
    start_time: Instant,
}

impl NodeMethods {
    pub fn new(
        node_type: String,
        version: String,
        network_id: String,
        public_key: Option<String>,
        chain: Arc<RwLock<Blockchain>>,
        network_manager: Arc<RwLock<NetworkManager>>,
        database: Arc<dyn Database>,
    ) -> Self {
        Self {
            node_type,
            version,
            network_id,
            public_key,
            chain,
            network_manager,
            database,
            start_time: Instant::now(),
        }
    }

    pub async fn node_info(&self) -> Result<NodeInfo> {
        let chain = self.chain.read().await;
        let network_manager = self.network_manager.read().await;
        let sync_status = network_manager.sync_status().await;

        let latest_block = chain.latest_block();

        Ok(NodeInfo {
            version: self.version.clone(),
            node_type: self.node_type.clone(),
            public_key: self.public_key.clone().unwrap_or_else(|| "none".to_string()),
            network_id: self.network_id.clone(),
            height: chain.height(),
            latest_block_hash: latest_block.hash().to_hex(),
            peer_count: network_manager.peer_count().await,
            syncing: sync_status.is_syncing(),
            uptime: self.start_time.elapsed().as_secs(),
        })
    }

    pub async fn sync_status(&self) -> Result<SyncStatusResponse> {
        let chain = self.chain.read().await;
        let network_manager = self.network_manager.read().await;
        
        let sync_status = network_manager.sync_status().await;
        let current_height = chain.height();

        let progress = sync_status.progress_percent();
        let estimated_time_remaining = sync_status.estimated_completion.map(|completion| {
            completion.saturating_sub(common::utils::current_timestamp())
        });

        Ok(SyncStatusResponse {
            syncing: sync_status.is_syncing(),
            current_height,
            target_height: sync_status.target_height,
            progress,
            estimated_time_remaining,
        })
    }

    pub async fn health(&self) -> Result<crate::HealthResponse> {
        let database_health = self.check_database_health().await;
        let network_health = self.check_network_health().await;
        let consensus_health = self.check_consensus_health().await;

        let overall_status = if database_health == "ok" && network_health == "ok" && consensus_health == "ok" {
            "ok".to_string()
        } else if database_health == "error" || network_health == "error" {
            "error".to_string()
        } else {
            "degraded".to_string()
        };

        Ok(crate::HealthResponse {
            status: overall_status,
            database: database_health,
            network: network_health,
            consensus: consensus_health,
            timestamp: common::utils::current_timestamp(),
        })
    }

    async fn check_database_health(&self) -> String {
        // Database trait has `get()` method, not `health_check()`
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
        let peer_count = network_manager.peer_count().await;
        
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
        let network_manager = self.network_manager.read().await;
        let sync_status = network_manager.sync_status().await;

        let current_height = chain.height();
        let blocks_behind = sync_status.target_height.saturating_sub(current_height);

        if blocks_behind > 1000 {
            "warning".to_string()
        } else if blocks_behind > 100 {
            "degraded".to_string()
        } else {
            "ok".to_string()
        }
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

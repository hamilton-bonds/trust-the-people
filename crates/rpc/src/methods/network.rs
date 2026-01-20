//! Network-related RPC methods
//!
//! This module implements RPC methods for querying network state:
//! - Peer information
//! - Mempool status
//! - Transaction submission

use crate::{MempoolResponse, PeerInfo, PeersResponse, SubmitResponse, TransactionSummary};
use blockchain_core::Transaction;
use common::{Result, VotingError};
use network::{NetworkManager, PeerManager};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Network query service
pub struct NetworkMethods {
    network_manager: Arc<RwLock<NetworkManager>>,
    peer_manager: Arc<RwLock<PeerManager>>,
}

impl NetworkMethods {
    /// Create new network methods handler
    pub fn new(
        network_manager: Arc<RwLock<NetworkManager>>,
        peer_manager: Arc<RwLock<PeerManager>>,
    ) -> Self {
        Self {
            network_manager,
            peer_manager,
        }
    }

    /// Get connected peers
    pub async fn get_peers(&self) -> Result<PeersResponse> {
        let peer_manager = self.peer_manager.read().await;
        let peers = peer_manager.get_all_peers();

        let peer_infos = peers
            .iter()
            .map(|peer| PeerInfo {
                peer_id: peer.id.to_string(),
                address: peer.addr.to_string(),
                direction: if peer.is_inbound {
                    "inbound".to_string()
                } else {
                    "outbound".to_string()
                },
                height: peer.best_height,
                connected_duration: peer.connected_duration_secs(),
            })
            .collect();

        Ok(PeersResponse {
            total_peers: peers.len(),
            peers: peer_infos,
        })
    }

    /// Get mempool status
    pub async fn get_mempool(&self) -> Result<MempoolResponse> {
        let network_manager = self.network_manager.read().await;
        let pending_txs = network_manager.get_pending_transactions();

        let transactions = pending_txs
            .iter()
            .map(|tx| TransactionSummary {
                id: tx.id.to_hex(),
                tx_type: tx.transaction_type_name().to_string(),
                block_height: 0, // Not yet in a block
                timestamp: tx.timestamp,
            })
            .collect();

        Ok(MempoolResponse {
            pending_count: pending_txs.len(),
            transactions,
        })
    }

    /// Submit transaction to network
    pub async fn submit_transaction(&self, tx_hex: String) -> Result<SubmitResponse> {
        // Decode transaction from hex
        let tx_bytes = hex::decode(&tx_hex).map_err(|e| {
            VotingError::InvalidTransaction(format!("Invalid hex encoding: {}", e))
        })?;

        let tx: Transaction = common::utils::deserialize(&tx_bytes)?;

        // Validate transaction
        tx.validate()?;

        // Submit to network
        let mut network_manager = self.network_manager.write().await;
        match network_manager.broadcast_transaction(tx.clone()).await {
            Ok(_) => Ok(SubmitResponse {
                tx_id: tx.id.to_hex(),
                accepted: true,
                error: None,
            }),
            Err(e) => Ok(SubmitResponse {
                tx_id: tx.id.to_hex(),
                accepted: false,
                error: Some(e.to_string()),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use network::{NetworkConfig, PeerManagerConfig};

    #[tokio::test]
    async fn test_get_peers_empty() {
        let config = NetworkConfig::default();
        let peer_config = PeerManagerConfig::default();
        let network_manager = Arc::new(RwLock::new(NetworkManager::new(config)));
        let peer_manager = Arc::new(RwLock::new(PeerManager::new(peer_config)));

        let methods = NetworkMethods::new(network_manager, peer_manager);
        let response = methods.get_peers().await.unwrap();

        assert_eq!(response.total_peers, 0);
        assert!(response.peers.is_empty());
    }

    #[tokio::test]
    async fn test_get_mempool_empty() {
        let config = NetworkConfig::default();
        let peer_config = PeerManagerConfig::default();
        let network_manager = Arc::new(RwLock::new(NetworkManager::new(config)));
        let peer_manager = Arc::new(RwLock::new(PeerManager::new(peer_config)));

        let methods = NetworkMethods::new(network_manager, peer_manager);
        let response = methods.get_mempool().await.unwrap();

        assert_eq!(response.pending_count, 0);
        assert!(response.transactions.is_empty());
    }

    #[tokio::test]
    async fn test_submit_transaction_invalid_hex() {
        let config = NetworkConfig::default();
        let peer_config = PeerManagerConfig::default();
        let network_manager = Arc::new(RwLock::new(NetworkManager::new(config)));
        let peer_manager = Arc::new(RwLock::new(PeerManager::new(peer_config)));

        let methods = NetworkMethods::new(network_manager, peer_manager);
        let result = methods.submit_transaction("invalid_hex".to_string()).await;

        assert!(result.is_err());
    }
}

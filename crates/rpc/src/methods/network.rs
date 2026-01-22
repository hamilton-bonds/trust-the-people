//! Network-related RPC methods
//!
//! This module implements RPC methods for querying network state:
//! - Peer information
//! - Mempool status
//! - Transaction submission

use crate::{MempoolResponse, PeerInfo, PeersResponse, SubmitResponse, TransactionSummary};
use blockchain_core::Transaction;
use common::Result;
use network::{NetworkManager, peer::PeerManager, Message};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Network query service
pub struct NetworkMethods {
    network_manager: Arc<RwLock<NetworkManager>>,
    peer_manager: Arc<RwLock<PeerManager>>,
}

impl NetworkMethods {
    pub fn new(
        network_manager: Arc<RwLock<NetworkManager>>,
        peer_manager: Arc<RwLock<PeerManager>>,
    ) -> Self {
        Self {
            network_manager,
            peer_manager,
        }
    }

    pub async fn get_peers(&self) -> Result<PeersResponse> {
        let peer_manager = self.peer_manager.read().await;
        // Use get_all_peers() from PeerManager
        let peers = peer_manager.get_all_peers().await;
        
        let peer_infos = peers
            .iter()
            .map(|peer| PeerInfo {
                peer_id: peer.id.to_string(),
                address: peer.addr.to_string(),
                direction: match peer.direction {
                    network::peer::ConnectionDirection::Inbound => "inbound".to_string(),
                    network::peer::ConnectionDirection::Outbound => "outbound".to_string(),
                },
                height: peer.best_height,
                connected_duration: peer.uptime(),
            })
            .collect();

        Ok(PeersResponse {
            total_peers: peers.len(),
            peers: peer_infos,
        })
    }

    pub async fn get_mempool(&self) -> Result<MempoolResponse> {
        let _network_manager = self.network_manager.read().await;
        let pending_txs: Vec<Transaction> = Vec::new(); // TODO: implement get_pending_transactions

        let transactions = pending_txs
            .iter()
            .map(|tx| TransactionSummary {
                id: tx.id.to_hex(),
                tx_type: tx.transaction_type_name().to_string(),
                block_height: 0,
                timestamp: tx.timestamp,
            })
            .collect();

        Ok(MempoolResponse {
            pending_count: pending_txs.len(),
            transactions,
        })
    }

    pub async fn submit_transaction(&self, tx_hex: String) -> Result<SubmitResponse> {
        let tx_bytes = hex::decode(&tx_hex).map_err(|e| {
            common::VotingError::InvalidTransaction(format!("Invalid hex encoding: {}", e))
        })?;

        let tx: Transaction = common::utils::deserialize(&tx_bytes)?;
        tx.validate()?;

        // Broadcast using NetworkManager
        let network_manager = self.network_manager.write().await;
        let message = Message::new_transaction(tx.clone())?;
        network_manager.broadcast(message).await?;
        
        Ok(SubmitResponse {
            tx_id: tx.id.to_hex(),
            accepted: true,
            error: None,
        })
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

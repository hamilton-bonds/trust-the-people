pub mod peer;
pub mod discovery;
pub mod protocol;
pub mod gossip;
pub mod sync;
pub mod transport;

pub use peer::{Peer, PeerId, PeerInfo, PeerStatus};
pub use discovery::{PeerDiscovery, DiscoveryConfig};
pub use protocol::{Message, MessageType, ProtocolHandler};
pub use gossip::{GossipProtocol, GossipMessage};
pub use sync::{BlockSync, SyncState, SyncStatus};
pub use transport::{Transport, TransportConfig};

use common::{BlockHash, BlockHeight, Result, VotingError};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Network configuration
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Local node listen address
    pub listen_addr: SocketAddr,
    
    /// Bootstrap peers to connect to on startup
    pub bootstrap_peers: Vec<SocketAddr>,
    
    /// Maximum number of peer connections
    pub max_peers: usize,
    
    /// Maximum inbound connections
    pub max_inbound: usize,
    
    /// Maximum outbound connections
    pub max_outbound: usize,
    
    /// Connection timeout in seconds
    pub connection_timeout: u64,
    
    /// Enable peer discovery
    pub enable_discovery: bool,
    
    /// Discovery interval in seconds
    pub discovery_interval: u64,
    
    /// Network ID (mainnet, testnet, etc)
    pub network_id: String,
    
    /// Protocol version
    pub protocol_version: u32,
    
    /// Enable gossip protocol
    pub enable_gossip: bool,
    
    /// Gossip fanout (number of peers to gossip to)
    pub gossip_fanout: usize,
    
    /// Sync batch size (blocks per request)
    pub sync_batch_size: usize,
    
    /// Maximum message size in bytes
    pub max_message_size: usize,
    
    /// Rate limit: messages per second
    pub rate_limit: u32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:9000".parse().unwrap(),
            bootstrap_peers: vec![],
            max_peers: 50,
            max_inbound: 25,
            max_outbound: 25,
            connection_timeout: 30,
            enable_discovery: true,
            discovery_interval: 60,
            network_id: "voting-mainnet".to_string(),
            protocol_version: 1,
            enable_gossip: true,
            gossip_fanout: 4,
            sync_batch_size: 100,
            max_message_size: 10 * 1024 * 1024,
            rate_limit: 100,
        }
    }
}

/// Network manager coordinating all network components
pub struct NetworkManager {
    config: NetworkConfig,
    transport: Arc<Transport>,
    peers: Arc<RwLock<Vec<Peer>>>,
    discovery: Arc<PeerDiscovery>,
    gossip: Arc<GossipProtocol>,
    sync: Arc<BlockSync>,
    protocol: Arc<ProtocolHandler>,
}

impl NetworkManager {
    /// Create a new network manager
    pub fn new(config: NetworkConfig) -> Result<Self> {
        let transport_config = TransportConfig {
            listen_addr: config.listen_addr,
            connection_timeout: config.connection_timeout,
            max_message_size: config.max_message_size,
            rate_limit: config.rate_limit,
        };
        
        let transport = Arc::new(Transport::new(transport_config)?);
        let peers = Arc::new(RwLock::new(Vec::new()));
        
        let discovery_config = DiscoveryConfig {
            bootstrap_peers: config.bootstrap_peers.clone(),
            discovery_interval: config.discovery_interval,
            max_peers: config.max_peers,
        };
        let discovery = Arc::new(PeerDiscovery::new(discovery_config));
        
        let gossip = Arc::new(GossipProtocol::new(config.gossip_fanout));
        let sync = Arc::new(BlockSync::new(config.sync_batch_size));
        let protocol = Arc::new(ProtocolHandler::new(
            config.network_id.clone(),
            config.protocol_version,
        ));
        
        Ok(Self {
            config,
            transport,
            peers,
            discovery,
            gossip,
            sync,
            protocol,
        })
    }
    
    /// Start the network manager
    pub async fn start(&self) -> Result<()> {
        self.transport.start().await?;
        
        if self.config.enable_discovery {
            self.discovery.start().await?;
        }
        
        if self.config.enable_gossip {
            self.gossip.start().await?;
        }
        
        self.sync.start().await?;
        
        for bootstrap_peer in &self.config.bootstrap_peers {
            self.connect_peer(*bootstrap_peer).await?;
        }
        
        Ok(())
    }
    
    /// Stop the network manager
    pub async fn stop(&self) -> Result<()> {
        self.sync.stop().await?;
        self.gossip.stop().await?;
        self.discovery.stop().await?;
        self.transport.stop().await?;
        
        let mut peers = self.peers.write().await;
        for peer in peers.iter_mut() {
            peer.disconnect().await?;
        }
        peers.clear();
        
        Ok(())
    }
    
    /// Connect to a peer
    pub async fn connect_peer(&self, addr: SocketAddr) -> Result<PeerId> {
        let peer_count = self.peers.read().await.len();
        if peer_count >= self.config.max_peers {
            return Err(VotingError::NetworkError(
                "Maximum peer count reached".to_string(),
            ));
        }
        
        let peer = self.transport.connect(addr).await?;
        let peer_id = peer.id();
        
        self.peers.write().await.push(peer);
        
        Ok(peer_id)
    }
    
    /// Disconnect from a peer
    pub async fn disconnect_peer(&self, peer_id: &PeerId) -> Result<()> {
        let mut peers = self.peers.write().await;
        
        if let Some(pos) = peers.iter().position(|p| p.id() == *peer_id) {
            let mut peer = peers.remove(pos);
            peer.disconnect().await?;
        }
        
        Ok(())
    }
    
    /// Get connected peers
    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        self.peers
            .read()
            .await
            .iter()
            .map(|p| p.info())
            .collect()
    }
    
    /// Get peer count
    pub async fn peer_count(&self) -> usize {
        self.peers.read().await.len()
    }
    
    /// Broadcast a message to all peers
    pub async fn broadcast(&self, message: Message) -> Result<usize> {
        let peers = self.peers.read().await;
        let mut sent_count = 0;
        
        for peer in peers.iter() {
            if peer.is_connected() {
                if peer.send(message.clone()).await.is_ok() {
                    sent_count += 1;
                }
            }
        }
        
        Ok(sent_count)
    }
    
    /// Send a message to a specific peer
    pub async fn send_to_peer(&self, peer_id: &PeerId, message: Message) -> Result<()> {
        let peers = self.peers.read().await;
        
        let peer = peers
            .iter()
            .find(|p| p.id() == *peer_id)
            .ok_or_else(|| VotingError::NetworkError("Peer not found".to_string()))?;
        
        peer.send(message).await
    }
    
    /// Gossip a message to a subset of peers
    pub async fn gossip(&self, message: GossipMessage) -> Result<usize> {
        self.gossip.gossip(message, &self.peers).await
    }
    
    /// Request block sync from peers
    pub async fn sync_blocks(&self, start_height: BlockHeight, end_height: BlockHeight) -> Result<()> {
        self.sync.request_blocks(start_height, end_height, &self.peers).await
    }
    
    /// Get sync status
    pub async fn sync_status(&self) -> SyncStatus {
        self.sync.status().await
    }
    
    /// Handle incoming message
    pub async fn handle_message(&self, peer_id: PeerId, message: Message) -> Result<()> {
        self.protocol.handle(peer_id, message).await
    }
    
    /// Get network statistics
    pub fn get_stats(&self) -> NetworkStats {
        NetworkStats {
            peer_count: 0,
            inbound_count: 0,
            outbound_count: 0,
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
        }
    }
}

/// Network statistics
#[derive(Debug, Clone, Default)]
pub struct NetworkStats {
    pub peer_count: usize,
    pub inbound_count: usize,
    pub outbound_count: usize,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_config_default() {
        let config = NetworkConfig::default();
        assert_eq!(config.max_peers, 50);
        assert_eq!(config.protocol_version, 1);
        assert!(config.enable_discovery);
        assert!(config.enable_gossip);
    }

    #[test]
    fn test_network_config_custom() {
        let config = NetworkConfig {
            max_peers: 100,
            protocol_version: 2,
            enable_discovery: false,
            ..Default::default()
        };
        
        assert_eq!(config.max_peers, 100);
        assert_eq!(config.protocol_version, 2);
        assert!(!config.enable_discovery);
    }

    #[tokio::test]
    async fn test_network_manager_creation() {
        let config = NetworkConfig::default();
        let manager = NetworkManager::new(config);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_network_stats_default() {
        let stats = NetworkStats::default();
        assert_eq!(stats.peer_count, 0);
        assert_eq!(stats.messages_sent, 0);
        assert_eq!(stats.bytes_received, 0);
    }
}

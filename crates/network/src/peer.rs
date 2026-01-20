use crate::protocol::Message;
use common::{Result, Timestamp, VotingError};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Unique peer identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId(pub [u8; 32]);

impl PeerId {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    
    pub fn random() -> Self {
        let mut bytes = [0u8; 32];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut bytes);
        Self(bytes)
    }
    
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
    
    pub fn from_hex(s: &str) -> Result<Self> {
        let bytes = hex::decode(s)
            .map_err(|e| VotingError::NetworkError(format!("Invalid hex: {}", e)))?;
        
        if bytes.len() != 32 {
            return Err(VotingError::NetworkError("Invalid peer ID length".to_string()));
        }
        
        let mut id = [0u8; 32];
        id.copy_from_slice(&bytes);
        Ok(Self(id))
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.to_hex()[..8])
    }
}

/// Peer connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerStatus {
    Connecting,
    Connected,
    Disconnected,
    Banned,
}

/// Direction of peer connection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionDirection {
    Inbound,
    Outbound,
}

/// Peer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: PeerId,
    pub addr: SocketAddr,
    pub status: PeerStatus,
    pub direction: ConnectionDirection,
    pub connected_at: Timestamp,
    pub last_seen: Timestamp,
    pub protocol_version: u32,
    pub network_id: String,
    pub user_agent: String,
    pub best_height: u64,
}

impl PeerInfo {
    pub fn uptime(&self) -> u64 {
        let now = common::utils::current_timestamp();
        now.saturating_sub(self.connected_at)
    }
    
    pub fn is_stale(&self, stale_threshold: u64) -> bool {
        let now = common::utils::current_timestamp();
        now.saturating_sub(self.last_seen) > stale_threshold
    }
}

/// Peer statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PeerStats {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub errors: u64,
    pub last_error: Option<String>,
}

/// Peer connection
pub struct Peer {
    info: Arc<RwLock<PeerInfo>>,
    stats: Arc<RwLock<PeerStats>>,
    message_tx: mpsc::UnboundedSender<Message>,
    message_rx: Arc<RwLock<mpsc::UnboundedReceiver<Message>>>,
    disconnect_tx: mpsc::Sender<()>,
}

impl Peer {
    /// Create a new peer connection
    pub fn new(
        id: PeerId,
        addr: SocketAddr,
        direction: ConnectionDirection,
        protocol_version: u32,
        network_id: String,
    ) -> Self {
        let now = common::utils::current_timestamp();
        
        let info = Arc::new(RwLock::new(PeerInfo {
            id,
            addr,
            status: PeerStatus::Connecting,
            direction,
            connected_at: now,
            last_seen: now,
            protocol_version,
            network_id,
            user_agent: "voting-node/0.1.0".to_string(),
            best_height: 0,
        }));
        
        let stats = Arc::new(RwLock::new(PeerStats::default()));
        
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        let message_rx = Arc::new(RwLock::new(message_rx));
        
        let (disconnect_tx, _disconnect_rx) = mpsc::channel(1);
        
        Self {
            info,
            stats,
            message_tx,
            message_rx,
            disconnect_tx,
        }
    }
    
    /// Get peer ID
    pub fn id(&self) -> PeerId {
        let info = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.info.read())
        });
        info.id
    }
    
    /// Get peer address
    pub fn addr(&self) -> SocketAddr {
        let info = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.info.read())
        });
        info.addr
    }
    
    /// Get peer info
    pub fn info(&self) -> PeerInfo {
        let info = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.info.read())
        });
        info.clone()
    }
    
    /// Get peer stats
    pub async fn stats(&self) -> PeerStats {
        self.stats.read().await.clone()
    }
    
    /// Check if peer is connected
    pub fn is_connected(&self) -> bool {
        let info = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.info.read())
        });
        info.status == PeerStatus::Connected
    }
    
    /// Mark peer as connected
    pub async fn mark_connected(&self) {
        let mut info = self.info.write().await;
        info.status = PeerStatus::Connected;
        info.last_seen = common::utils::current_timestamp();
    }
    
    /// Mark peer as disconnected
    pub async fn mark_disconnected(&self) {
        let mut info = self.info.write().await;
        info.status = PeerStatus::Disconnected;
    }
    
    /// Update last seen timestamp
    pub async fn update_last_seen(&self) {
        let mut info = self.info.write().await;
        info.last_seen = common::utils::current_timestamp();
    }
    
    /// Update best height
    pub async fn update_best_height(&self, height: u64) {
        let mut info = self.info.write().await;
        info.best_height = height;
    }
    
    /// Send a message to the peer
    pub async fn send(&self, message: Message) -> Result<()> {
        self.message_tx
            .send(message)
            .map_err(|e| VotingError::NetworkError(format!("Failed to send message: {}", e)))?;
        
        let mut stats = self.stats.write().await;
        stats.messages_sent += 1;
        
        self.update_last_seen().await;
        
        Ok(())
    }
    
    /// Receive a message from the peer
    pub async fn receive(&self) -> Result<Option<Message>> {
        let mut rx = self.message_rx.write().await;
        
        match rx.try_recv() {
            Ok(message) => {
                let mut stats = self.stats.write().await;
                stats.messages_received += 1;
                
                self.update_last_seen().await;
                
                Ok(Some(message))
            }
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                Err(VotingError::NetworkError("Peer disconnected".to_string()))
            }
        }
    }
    
    /// Record an error
    pub async fn record_error(&self, error: String) {
        let mut stats = self.stats.write().await;
        stats.errors += 1;
        stats.last_error = Some(error);
    }
    
    /// Ban the peer
    pub async fn ban(&self) {
        let mut info = self.info.write().await;
        info.status = PeerStatus::Banned;
    }
    
    /// Check if peer is banned
    pub async fn is_banned(&self) -> bool {
        let info = self.info.read().await;
        info.status == PeerStatus::Banned
    }
    
    /// Disconnect from the peer
    pub async fn disconnect(&mut self) -> Result<()> {
        self.mark_disconnected().await;
        
        let _ = self.disconnect_tx.send(()).await;
        
        Ok(())
    }
    
    /// Get connection direction
    pub async fn direction(&self) -> ConnectionDirection {
        self.info.read().await.direction
    }
    
    /// Check if peer is inbound
    pub async fn is_inbound(&self) -> bool {
        self.direction().await == ConnectionDirection::Inbound
    }
    
    /// Check if peer is outbound
    pub async fn is_outbound(&self) -> bool {
        self.direction().await == ConnectionDirection::Outbound
    }
}

/// Peer reputation score
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Reputation(i32);

impl Reputation {
    pub const MAX: i32 = 100;
    pub const MIN: i32 = -100;
    pub const INITIAL: i32 = 0;
    
    pub fn new(score: i32) -> Self {
        Self(score.clamp(Self::MIN, Self::MAX))
    }
    
    pub fn initial() -> Self {
        Self(Self::INITIAL)
    }
    
    pub fn score(&self) -> i32 {
        self.0
    }
    
    pub fn increase(&mut self, amount: i32) {
        self.0 = (self.0 + amount).clamp(Self::MIN, Self::MAX);
    }
    
    pub fn decrease(&mut self, amount: i32) {
        self.0 = (self.0 - amount).clamp(Self::MIN, Self::MAX);
    }
    
    pub fn is_good(&self) -> bool {
        self.0 > 0
    }
    
    pub fn is_bad(&self) -> bool {
        self.0 < 0
    }
    
    pub fn should_ban(&self) -> bool {
        self.0 <= Self::MIN
    }
}

impl Default for Reputation {
    fn default() -> Self {
        Self::initial()
    }
}

/// Peer manager for tracking and managing multiple peers
pub struct PeerManager {
    peers: Arc<RwLock<Vec<Peer>>>,
    reputations: Arc<RwLock<std::collections::HashMap<PeerId, Reputation>>>,
    max_peers: usize,
}

impl PeerManager {
    pub fn new(max_peers: usize) -> Self {
        Self {
            peers: Arc::new(RwLock::new(Vec::new())),
            reputations: Arc::new(RwLock::new(std::collections::HashMap::new())),
            max_peers,
        }
    }
    
    pub async fn add_peer(&self, peer: Peer) -> Result<()> {
        let mut peers = self.peers.write().await;
        
        if peers.len() >= self.max_peers {
            return Err(VotingError::NetworkError(
                "Maximum peer count reached".to_string(),
            ));
        }
        
        peers.push(peer);
        Ok(())
    }
    
    pub async fn remove_peer(&self, peer_id: &PeerId) -> Result<()> {
        let mut peers = self.peers.write().await;
        
        if let Some(pos) = peers.iter().position(|p| p.id() == *peer_id) {
            peers.remove(pos);
        }
        
        Ok(())
    }
    
    pub async fn get_peer(&self, peer_id: &PeerId) -> Option<PeerInfo> {
        let peers = self.peers.read().await;
        peers.iter().find(|p| p.id() == *peer_id).map(|p| p.info())
    }
    
    pub async fn peer_count(&self) -> usize {
        self.peers.read().await.len()
    }
    
    pub async fn get_all_peers(&self) -> Vec<PeerInfo> {
        self.peers.read().await.iter().map(|p| p.info()).collect()
    }
    
    pub async fn get_reputation(&self, peer_id: &PeerId) -> Reputation {
        let reputations = self.reputations.read().await;
        reputations.get(peer_id).copied().unwrap_or_default()
    }
    
    pub async fn update_reputation(&self, peer_id: PeerId, change: i32) {
        let mut reputations = self.reputations.write().await;
        let reputation = reputations.entry(peer_id).or_insert_with(Reputation::initial);
        
        if change > 0 {
            reputation.increase(change);
        } else {
            reputation.decrease(change.abs());
        }
    }
    
    pub async fn should_ban(&self, peer_id: &PeerId) -> bool {
        self.get_reputation(peer_id).await.should_ban()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_id_creation() {
        let id = PeerId::random();
        let hex = id.to_hex();
        assert_eq!(hex.len(), 64);
        
        let decoded = PeerId::from_hex(&hex).unwrap();
        assert_eq!(id, decoded);
    }

    #[test]
    fn test_peer_id_display() {
        let id = PeerId::new([1u8; 32]);
        let display = format!("{}", id);
        assert_eq!(display.len(), 8);
    }

    #[test]
    fn test_peer_status() {
        assert_ne!(PeerStatus::Connected, PeerStatus::Disconnected);
        assert_ne!(PeerStatus::Connecting, PeerStatus::Banned);
    }

    #[test]
    fn test_connection_direction() {
        assert_ne!(ConnectionDirection::Inbound, ConnectionDirection::Outbound);
    }

    #[tokio::test]
    async fn test_peer_creation() {
        let id = PeerId::random();
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        
        let peer = Peer::new(
            id,
            addr,
            ConnectionDirection::Outbound,
            1,
            "test-network".to_string(),
        );
        
        assert_eq!(peer.id(), id);
        assert_eq!(peer.addr(), addr);
        assert!(!peer.is_connected());
    }

    #[tokio::test]
    async fn test_peer_connect_disconnect() {
        let id = PeerId::random();
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        
        let mut peer = Peer::new(
            id,
            addr,
            ConnectionDirection::Outbound,
            1,
            "test-network".to_string(),
        );
        
        peer.mark_connected().await;
        assert!(peer.is_connected());
        
        peer.disconnect().await.unwrap();
        assert!(!peer.is_connected());
    }

    #[tokio::test]
    async fn test_peer_stats() {
        let id = PeerId::random();
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        
        let peer = Peer::new(
            id,
            addr,
            ConnectionDirection::Outbound,
            1,
            "test-network".to_string(),
        );
        
        let stats = peer.stats().await;
        assert_eq!(stats.messages_sent, 0);
        assert_eq!(stats.messages_received, 0);
    }

    #[tokio::test]
    async fn test_peer_ban() {
        let id = PeerId::random();
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        
        let peer = Peer::new(
            id,
            addr,
            ConnectionDirection::Outbound,
            1,
            "test-network".to_string(),
        );
        
        assert!(!peer.is_banned().await);
        
        peer.ban().await;
        assert!(peer.is_banned().await);
    }

    #[test]
    fn test_reputation() {
        let mut rep = Reputation::initial();
        assert_eq!(rep.score(), 0);
        
        rep.increase(10);
        assert_eq!(rep.score(), 10);
        assert!(rep.is_good());
        
        rep.decrease(20);
        assert_eq!(rep.score(), -10);
        assert!(rep.is_bad());
    }

    #[test]
    fn test_reputation_bounds() {
        let mut rep = Reputation::initial();
        
        rep.increase(200);
        assert_eq!(rep.score(), Reputation::MAX);
        
        rep.decrease(300);
        assert_eq!(rep.score(), Reputation::MIN);
        assert!(rep.should_ban());
    }

    #[tokio::test]
    async fn test_peer_manager() {
        let manager = PeerManager::new(10);
        
        assert_eq!(manager.peer_count().await, 0);
        
        let id = PeerId::random();
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        let peer = Peer::new(id, addr, ConnectionDirection::Outbound, 1, "test".to_string());
        
        manager.add_peer(peer).await.unwrap();
        assert_eq!(manager.peer_count().await, 1);
        
        manager.remove_peer(&id).await.unwrap();
        assert_eq!(manager.peer_count().await, 0);
    }

    #[tokio::test]
    async fn test_peer_manager_reputation() {
        let manager = PeerManager::new(10);
        let id = PeerId::random();
        
        let rep = manager.get_reputation(&id).await;
        assert_eq!(rep.score(), 0);
        
        manager.update_reputation(id, 10).await;
        let rep = manager.get_reputation(&id).await;
        assert_eq!(rep.score(), 10);
        
        manager.update_reputation(id, -20).await;
        let rep = manager.get_reputation(&id).await;
        assert_eq!(rep.score(), -10);
    }

    #[test]
    fn test_peer_info_uptime() {
        let now = common::utils::current_timestamp();
        
        let info = PeerInfo {
            id: PeerId::random(),
            addr: "127.0.0.1:9000".parse().unwrap(),
            status: PeerStatus::Connected,
            direction: ConnectionDirection::Outbound,
            connected_at: now - 100,
            last_seen: now,
            protocol_version: 1,
            network_id: "test".to_string(),
            user_agent: "test".to_string(),
            best_height: 0,
        };
        
        assert!(info.uptime() >= 100);
    }

    #[test]
    fn test_peer_info_stale() {
        let now = common::utils::current_timestamp();
        
        let info = PeerInfo {
            id: PeerId::random(),
            addr: "127.0.0.1:9000".parse().unwrap(),
            status: PeerStatus::Connected,
            direction: ConnectionDirection::Outbound,
            connected_at: now,
            last_seen: now - 200,
            protocol_version: 1,
            network_id: "test".to_string(),
            user_agent: "test".to_string(),
            best_height: 0,
        };
        
        assert!(info.is_stale(100));
        assert!(!info.is_stale(300));
    }
}

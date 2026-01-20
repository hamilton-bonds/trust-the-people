use crate::peer::{Peer, PeerId, PeerInfo};
use common::{Result, Timestamp, VotingError};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// Peer discovery configuration
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// Bootstrap peers to connect to initially
    pub bootstrap_peers: Vec<SocketAddr>,
    
    /// Discovery interval in seconds
    pub discovery_interval: u64,
    
    /// Maximum number of peers to maintain
    pub max_peers: usize,
    
    /// Maximum number of peers to request per query
    pub max_peers_per_query: usize,
    
    /// Peer advertisement interval in seconds
    pub advertisement_interval: u64,
    
    /// Peer staleness threshold in seconds
    pub stale_threshold: u64,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            bootstrap_peers: vec![],
            discovery_interval: 60,
            max_peers: 50,
            max_peers_per_query: 10,
            advertisement_interval: 300,
            stale_threshold: 600,
        }
    }
}

/// Discovered peer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredPeer {
    pub addr: SocketAddr,
    pub discovered_at: Timestamp,
    pub last_seen: Timestamp,
    pub source: PeerSource,
}

/// Source of peer discovery
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerSource {
    Bootstrap,
    Peer,
    Manual,
}

impl DiscoveredPeer {
    pub fn new(addr: SocketAddr, source: PeerSource) -> Self {
        let now = common::utils::current_timestamp();
        Self {
            addr,
            discovered_at: now,
            last_seen: now,
            source,
        }
    }
    
    pub fn update_last_seen(&mut self) {
        self.last_seen = common::utils::current_timestamp();
    }
    
    pub fn is_stale(&self, threshold: u64) -> bool {
        let now = common::utils::current_timestamp();
        now.saturating_sub(self.last_seen) > threshold
    }
}

/// Peer discovery protocol
pub struct PeerDiscovery {
    config: DiscoveryConfig,
    discovered_peers: Arc<RwLock<HashMap<SocketAddr, DiscoveredPeer>>>,
    connected_peers: Arc<RwLock<HashSet<SocketAddr>>>,
    running: Arc<RwLock<bool>>,
}

impl PeerDiscovery {
    pub fn new(config: DiscoveryConfig) -> Self {
        let mut discovered_peers = HashMap::new();
        
        for addr in &config.bootstrap_peers {
            discovered_peers.insert(
                *addr,
                DiscoveredPeer::new(*addr, PeerSource::Bootstrap),
            );
        }
        
        Self {
            config,
            discovered_peers: Arc::new(RwLock::new(discovered_peers)),
            connected_peers: Arc::new(RwLock::new(HashSet::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Start peer discovery
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Err(VotingError::NetworkError(
                "Discovery already running".to_string(),
            ));
        }
        *running = true;
        
        let discovery = self.clone();
        tokio::spawn(async move {
            discovery.discovery_loop().await;
        });
        
        Ok(())
    }
    
    /// Stop peer discovery
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;
        Ok(())
    }
    
    /// Main discovery loop
    async fn discovery_loop(&self) {
        let mut ticker = interval(Duration::from_secs(self.config.discovery_interval));
        
        loop {
            ticker.tick().await;
            
            let running = *self.running.read().await;
            if !running {
                break;
            }
            
            if let Err(e) = self.discover_peers().await {
                tracing::warn!("Peer discovery error: {}", e);
            }
            
            if let Err(e) = self.clean_stale_peers().await {
                tracing::warn!("Failed to clean stale peers: {}", e);
            }
        }
    }
    
    /// Discover new peers
    async fn discover_peers(&self) -> Result<()> {
        let connected_count = self.connected_peers.read().await.len();
        
        if connected_count >= self.config.max_peers {
            return Ok(());
        }
        
        let needed = self.config.max_peers - connected_count;
        let candidates = self.get_discovery_candidates(needed).await;
        
        tracing::info!(
            "Discovered {} peer candidates, need {} more peers",
            candidates.len(),
            needed
        );
        
        Ok(())
    }
    
    /// Get candidates for connection
    async fn get_discovery_candidates(&self, count: usize) -> Vec<SocketAddr> {
        let discovered = self.discovered_peers.read().await;
        let connected = self.connected_peers.read().await;
        
        discovered
            .iter()
            .filter(|(addr, peer)| {
                !connected.contains(addr) && !peer.is_stale(self.config.stale_threshold)
            })
            .take(count)
            .map(|(addr, _)| *addr)
            .collect()
    }
    
    /// Add a discovered peer
    pub async fn add_peer(&self, addr: SocketAddr, source: PeerSource) -> Result<()> {
        let mut discovered = self.discovered_peers.write().await;
        
        if discovered.len() >= self.config.max_peers * 2 {
            return Err(VotingError::NetworkError(
                "Discovered peers limit reached".to_string(),
            ));
        }
        
        discovered
            .entry(addr)
            .and_modify(|peer| peer.update_last_seen())
            .or_insert_with(|| DiscoveredPeer::new(addr, source));
        
        Ok(())
    }
    
    /// Add multiple discovered peers
    pub async fn add_peers(&self, peers: Vec<SocketAddr>, source: PeerSource) -> Result<usize> {
        let mut count = 0;
        
        for addr in peers {
            if self.add_peer(addr, source).await.is_ok() {
                count += 1;
            }
        }
        
        Ok(count)
    }
    
    /// Mark peer as connected
    pub async fn mark_connected(&self, addr: SocketAddr) -> Result<()> {
        self.connected_peers.write().await.insert(addr);
        
        let mut discovered = self.discovered_peers.write().await;
        if let Some(peer) = discovered.get_mut(&addr) {
            peer.update_last_seen();
        }
        
        Ok(())
    }
    
    /// Mark peer as disconnected
    pub async fn mark_disconnected(&self, addr: SocketAddr) -> Result<()> {
        self.connected_peers.write().await.remove(&addr);
        Ok(())
    }
    
    /// Remove a peer from discovery
    pub async fn remove_peer(&self, addr: &SocketAddr) -> Result<()> {
        self.discovered_peers.write().await.remove(addr);
        self.connected_peers.write().await.remove(addr);
        Ok(())
    }
    
    /// Clean stale peers
    async fn clean_stale_peers(&self) -> Result<usize> {
        let threshold = self.config.stale_threshold;
        let mut discovered = self.discovered_peers.write().await;
        
        let stale_peers: Vec<SocketAddr> = discovered
            .iter()
            .filter(|(_, peer)| peer.is_stale(threshold))
            .map(|(addr, _)| *addr)
            .collect();
        
        let count = stale_peers.len();
        
        for addr in stale_peers {
            discovered.remove(&addr);
        }
        
        if count > 0 {
            tracing::debug!("Removed {} stale peers", count);
        }
        
        Ok(count)
    }
    
    /// Get all discovered peers
    pub async fn get_discovered_peers(&self) -> Vec<DiscoveredPeer> {
        self.discovered_peers
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }
    
    /// Get connected peer count
    pub async fn connected_count(&self) -> usize {
        self.connected_peers.read().await.len()
    }
    
    /// Get discovered peer count
    pub async fn discovered_count(&self) -> usize {
        self.discovered_peers.read().await.len()
    }
    
    /// Request peers from a connected peer
    pub async fn request_peers(&self, _peer_id: PeerId) -> Result<Vec<SocketAddr>> {
        Ok(vec![])
    }
    
    /// Handle peer list response
    pub async fn handle_peer_list(&self, peers: Vec<SocketAddr>) -> Result<()> {
        self.add_peers(peers, PeerSource::Peer).await?;
        Ok(())
    }
    
    /// Get discovery statistics
    pub async fn get_stats(&self) -> DiscoveryStats {
        let discovered = self.discovered_peers.read().await;
        let connected = self.connected_peers.read().await;
        
        let bootstrap_count = discovered
            .values()
            .filter(|p| p.source == PeerSource::Bootstrap)
            .count();
        
        let peer_count = discovered
            .values()
            .filter(|p| p.source == PeerSource::Peer)
            .count();
        
        let manual_count = discovered
            .values()
            .filter(|p| p.source == PeerSource::Manual)
            .count();
        
        DiscoveryStats {
            discovered_peers: discovered.len(),
            connected_peers: connected.len(),
            bootstrap_peers: bootstrap_count,
            peer_discovered: peer_count,
            manual_peers: manual_count,
        }
    }
    
    /// Check if discovery is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

impl Clone for PeerDiscovery {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            discovered_peers: Arc::clone(&self.discovered_peers),
            connected_peers: Arc::clone(&self.connected_peers),
            running: Arc::clone(&self.running),
        }
    }
}

/// Discovery statistics
#[derive(Debug, Clone, Default)]
pub struct DiscoveryStats {
    pub discovered_peers: usize,
    pub connected_peers: usize,
    pub bootstrap_peers: usize,
    pub peer_discovered: usize,
    pub manual_peers: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_config_default() {
        let config = DiscoveryConfig::default();
        assert_eq!(config.discovery_interval, 60);
        assert_eq!(config.max_peers, 50);
        assert_eq!(config.stale_threshold, 600);
    }

    #[test]
    fn test_discovered_peer() {
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        let mut peer = DiscoveredPeer::new(addr, PeerSource::Bootstrap);
        
        assert_eq!(peer.addr, addr);
        assert_eq!(peer.source, PeerSource::Bootstrap);
        assert!(!peer.is_stale(1000));
        
        peer.update_last_seen();
        assert!(!peer.is_stale(1000));
    }

    #[test]
    fn test_peer_source() {
        assert_ne!(PeerSource::Bootstrap, PeerSource::Peer);
        assert_ne!(PeerSource::Peer, PeerSource::Manual);
    }

    #[tokio::test]
    async fn test_peer_discovery_creation() {
        let config = DiscoveryConfig::default();
        let discovery = PeerDiscovery::new(config);
        
        assert_eq!(discovery.discovered_count().await, 0);
        assert_eq!(discovery.connected_count().await, 0);
    }

    #[tokio::test]
    async fn test_add_peer() {
        let config = DiscoveryConfig::default();
        let discovery = PeerDiscovery::new(config);
        
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        discovery.add_peer(addr, PeerSource::Manual).await.unwrap();
        
        assert_eq!(discovery.discovered_count().await, 1);
    }

    #[tokio::test]
    async fn test_add_multiple_peers() {
        let config = DiscoveryConfig::default();
        let discovery = PeerDiscovery::new(config);
        
        let peers = vec![
            "127.0.0.1:9000".parse().unwrap(),
            "127.0.0.1:9001".parse().unwrap(),
            "127.0.0.1:9002".parse().unwrap(),
        ];
        
        let count = discovery.add_peers(peers, PeerSource::Peer).await.unwrap();
        assert_eq!(count, 3);
        assert_eq!(discovery.discovered_count().await, 3);
    }

    #[tokio::test]
    async fn test_mark_connected() {
        let config = DiscoveryConfig::default();
        let discovery = PeerDiscovery::new(config);
        
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        discovery.add_peer(addr, PeerSource::Manual).await.unwrap();
        discovery.mark_connected(addr).await.unwrap();
        
        assert_eq!(discovery.connected_count().await, 1);
    }

    #[tokio::test]
    async fn test_mark_disconnected() {
        let config = DiscoveryConfig::default();
        let discovery = PeerDiscovery::new(config);
        
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        discovery.add_peer(addr, PeerSource::Manual).await.unwrap();
        discovery.mark_connected(addr).await.unwrap();
        
        assert_eq!(discovery.connected_count().await, 1);
        
        discovery.mark_disconnected(addr).await.unwrap();
        assert_eq!(discovery.connected_count().await, 0);
    }

    #[tokio::test]
    async fn test_remove_peer() {
        let config = DiscoveryConfig::default();
        let discovery = PeerDiscovery::new(config);
        
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        discovery.add_peer(addr, PeerSource::Manual).await.unwrap();
        
        assert_eq!(discovery.discovered_count().await, 1);
        
        discovery.remove_peer(&addr).await.unwrap();
        assert_eq!(discovery.discovered_count().await, 0);
    }

    #[tokio::test]
    async fn test_get_discovered_peers() {
        let config = DiscoveryConfig::default();
        let discovery = PeerDiscovery::new(config);
        
        let addr1: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        let addr2: SocketAddr = "127.0.0.1:9001".parse().unwrap();
        
        discovery.add_peer(addr1, PeerSource::Bootstrap).await.unwrap();
        discovery.add_peer(addr2, PeerSource::Peer).await.unwrap();
        
        let peers = discovery.get_discovered_peers().await;
        assert_eq!(peers.len(), 2);
    }

    #[tokio::test]
    async fn test_discovery_stats() {
        let config = DiscoveryConfig::default();
        let discovery = PeerDiscovery::new(config);
        
        let addr1: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        let addr2: SocketAddr = "127.0.0.1:9001".parse().unwrap();
        let addr3: SocketAddr = "127.0.0.1:9002".parse().unwrap();
        
        discovery.add_peer(addr1, PeerSource::Bootstrap).await.unwrap();
        discovery.add_peer(addr2, PeerSource::Peer).await.unwrap();
        discovery.add_peer(addr3, PeerSource::Manual).await.unwrap();
        
        discovery.mark_connected(addr1).await.unwrap();
        
        let stats = discovery.get_stats().await;
        assert_eq!(stats.discovered_peers, 3);
        assert_eq!(stats.connected_peers, 1);
        assert_eq!(stats.bootstrap_peers, 1);
        assert_eq!(stats.peer_discovered, 1);
        assert_eq!(stats.manual_peers, 1);
    }

    #[tokio::test]
    async fn test_start_stop() {
        let config = DiscoveryConfig::default();
        let discovery = PeerDiscovery::new(config);
        
        assert!(!discovery.is_running().await);
        
        discovery.start().await.unwrap();
        assert!(discovery.is_running().await);
        
        discovery.stop().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!discovery.is_running().await);
    }

    #[tokio::test]
    async fn test_bootstrap_peers() {
        let bootstrap = vec![
            "127.0.0.1:9000".parse().unwrap(),
            "127.0.0.1:9001".parse().unwrap(),
        ];
        
        let config = DiscoveryConfig {
            bootstrap_peers: bootstrap,
            ..Default::default()
        };
        
        let discovery = PeerDiscovery::new(config);
        assert_eq!(discovery.discovered_count().await, 2);
        
        let peers = discovery.get_discovered_peers().await;
        assert!(peers.iter().all(|p| p.source == PeerSource::Bootstrap));
    }

    #[tokio::test]
    async fn test_discovery_candidates() {
        let config = DiscoveryConfig::default();
        let discovery = PeerDiscovery::new(config);
        
        let peers = vec![
            "127.0.0.1:9000".parse().unwrap(),
            "127.0.0.1:9001".parse().unwrap(),
            "127.0.0.1:9002".parse().unwrap(),
        ];
        
        discovery.add_peers(peers, PeerSource::Peer).await.unwrap();
        
        let candidates = discovery.get_discovery_candidates(2).await;
        assert_eq!(candidates.len(), 2);
    }
}

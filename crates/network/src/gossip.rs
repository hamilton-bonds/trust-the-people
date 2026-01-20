use crate::peer::{Peer, PeerId};
use blockchain_core::{Block, Transaction};
use common::{BlockHash, Result, Timestamp, TxId, VotingError};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// Gossip message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GossipMessage {
    NewBlock(Block),
    NewTransaction(Transaction),
    BlockAnnouncement(BlockAnnouncement),
    TransactionAnnouncement(TransactionAnnouncement),
}

/// Block announcement (lightweight version)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockAnnouncement {
    pub height: u64,
    pub hash: BlockHash,
    pub timestamp: Timestamp,
    pub transaction_count: u32,
}

/// Transaction announcement (lightweight version)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionAnnouncement {
    pub tx_id: TxId,
    pub timestamp: Timestamp,
    pub size: usize,
}

/// Gossip message identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GossipId([u8; 32]);

impl GossipId {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    
    pub fn from_block_hash(hash: &BlockHash) -> Self {
        Self(hash.0)
    }
    
    pub fn from_tx_id(tx_id: &TxId) -> Self {
        Self(tx_id.0)
    }
    
    pub fn random() -> Self {
        Self(common::utils::random_hash())
    }
}

/// Gossip tracking entry
struct GossipEntry {
    id: GossipId,
    first_seen: Timestamp,
    propagated_to: HashSet<PeerId>,
}

impl GossipEntry {
    fn new(id: GossipId) -> Self {
        Self {
            id,
            first_seen: common::utils::current_timestamp(),
            propagated_to: HashSet::new(),
        }
    }
    
    fn add_peer(&mut self, peer_id: PeerId) {
        self.propagated_to.insert(peer_id);
    }
    
    fn has_peer(&self, peer_id: &PeerId) -> bool {
        self.propagated_to.contains(peer_id)
    }
    
    fn is_stale(&self, ttl: u64) -> bool {
        let now = common::utils::current_timestamp();
        now.saturating_sub(self.first_seen) > ttl
    }
}

/// Gossip protocol implementation
pub struct GossipProtocol {
    fanout: usize,
    seen_messages: Arc<RwLock<HashMap<GossipId, GossipEntry>>>,
    message_ttl: u64,
    running: Arc<RwLock<bool>>,
}

impl GossipProtocol {
    pub fn new(fanout: usize) -> Self {
        Self {
            fanout,
            seen_messages: Arc::new(RwLock::new(HashMap::new())),
            message_ttl: 300,
            running: Arc::new(RwLock::new(false)),
        }
    }
    
    pub fn with_ttl(mut self, ttl: u64) -> Self {
        self.message_ttl = ttl;
        self
    }
    
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Err(VotingError::NetworkError(
                "Gossip already running".to_string(),
            ));
        }
        *running = true;
        
        let protocol = self.clone();
        tokio::spawn(async move {
            protocol.cleanup_loop().await;
        });
        
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;
        Ok(())
    }
    
    async fn cleanup_loop(&self) {
        let mut ticker = interval(Duration::from_secs(60));
        
        loop {
            ticker.tick().await;
            
            let running = *self.running.read().await;
            if !running {
                break;
            }
            
            if let Err(e) = self.cleanup_stale_entries().await {
                tracing::warn!("Failed to clean gossip entries: {}", e);
            }
        }
    }
    
    async fn cleanup_stale_entries(&self) -> Result<usize> {
        let mut seen = self.seen_messages.write().await;
        
        let stale_ids: Vec<GossipId> = seen
            .iter()
            .filter(|(_, entry)| entry.is_stale(self.message_ttl))
            .map(|(id, _)| *id)
            .collect();
        
        let count = stale_ids.len();
        
        for id in stale_ids {
            seen.remove(&id);
        }
        
        if count > 0 {
            tracing::debug!("Cleaned {} stale gossip entries", count);
        }
        
        Ok(count)
    }
    
    pub async fn gossip(
        &self,
        message: GossipMessage,
        peers: &Arc<RwLock<Vec<Peer>>>,
    ) -> Result<usize> {
        let gossip_id = self.get_message_id(&message);
        
        if self.has_seen(&gossip_id).await {
            return Ok(0);
        }
        
        self.mark_seen(gossip_id).await;
        
        let target_peers = self.select_gossip_targets(peers, &gossip_id).await;
        
        let mut sent_count = 0;
        for peer in target_peers {
            if self.send_to_peer(&peer, message.clone()).await.is_ok() {
                self.mark_propagated_to(gossip_id, peer.id()).await;
                sent_count += 1;
            }
        }
        
        Ok(sent_count)
    }
    
    async fn select_gossip_targets(
        &self,
        peers: &Arc<RwLock<Vec<Peer>>>,
        gossip_id: &GossipId,
    ) -> Vec<Peer> {
        let peers_guard = peers.read().await;
        let seen = self.seen_messages.read().await;
        
        let mut candidates: Vec<Peer> = peers_guard
            .iter()
            .filter(|peer| {
                peer.is_connected()
                    && !seen
                        .get(gossip_id)
                        .map(|entry| entry.has_peer(&peer.id()))
                        .unwrap_or(false)
            })
            .cloned()
            .collect();
        
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        candidates.shuffle(&mut rng);
        
        candidates.into_iter().take(self.fanout).collect()
    }
    
    async fn send_to_peer(&self, peer: &Peer, message: GossipMessage) -> Result<()> {
        let protocol_message = self.encode_gossip_message(message)?;
        peer.send(protocol_message).await
    }
    
    fn encode_gossip_message(&self, message: GossipMessage) -> Result<crate::protocol::Message> {
        use crate::protocol::{Message, MessageType};
        
        match message {
            GossipMessage::NewBlock(block) => Message::new_block(block),
            GossipMessage::NewTransaction(tx) => Message::new_transaction(tx),
            GossipMessage::BlockAnnouncement(ann) => {
                let payload = common::utils::serialize(&ann)?;
                Ok(Message::new(MessageType::NewBlock, payload))
            }
            GossipMessage::TransactionAnnouncement(ann) => {
                let payload = common::utils::serialize(&ann)?;
                Ok(Message::new(MessageType::NewTransaction, payload))
            }
        }
    }
    
    fn get_message_id(&self, message: &GossipMessage) -> GossipId {
        match message {
            GossipMessage::NewBlock(block) => GossipId::from_block_hash(&block.hash()),
            GossipMessage::NewTransaction(tx) => GossipId::from_tx_id(&tx.id),
            GossipMessage::BlockAnnouncement(ann) => GossipId::from_block_hash(&ann.hash),
            GossipMessage::TransactionAnnouncement(ann) => GossipId::from_tx_id(&ann.tx_id),
        }
    }
    
    async fn has_seen(&self, id: &GossipId) -> bool {
        self.seen_messages.read().await.contains_key(id)
    }
    
    async fn mark_seen(&self, id: GossipId) {
        let mut seen = self.seen_messages.write().await;
        seen.entry(id).or_insert_with(|| GossipEntry::new(id));
    }
    
    async fn mark_propagated_to(&self, id: GossipId, peer_id: PeerId) {
        let mut seen = self.seen_messages.write().await;
        if let Some(entry) = seen.get_mut(&id) {
            entry.add_peer(peer_id);
        }
    }
    
    pub async fn handle_received_message(
        &self,
        message: GossipMessage,
        from_peer: PeerId,
        peers: &Arc<RwLock<Vec<Peer>>>,
    ) -> Result<()> {
        let gossip_id = self.get_message_id(&message);
        
        if !self.has_seen(&gossip_id).await {
            self.mark_seen(gossip_id).await;
            
            self.gossip(message, peers).await?;
        }
        
        self.mark_propagated_to(gossip_id, from_peer).await;
        
        Ok(())
    }
    
    pub async fn get_stats(&self) -> GossipStats {
        let seen = self.seen_messages.read().await;
        
        let total_propagations: usize = seen
            .values()
            .map(|entry| entry.propagated_to.len())
            .sum();
        
        GossipStats {
            seen_messages: seen.len(),
            total_propagations,
            average_fanout: if !seen.is_empty() {
                total_propagations as f64 / seen.len() as f64
            } else {
                0.0
            },
        }
    }
    
    pub async fn clear(&self) {
        self.seen_messages.write().await.clear();
    }
}

impl Clone for GossipProtocol {
    fn clone(&self) -> Self {
        Self {
            fanout: self.fanout,
            seen_messages: Arc::clone(&self.seen_messages),
            message_ttl: self.message_ttl,
            running: Arc::clone(&self.running),
        }
    }
}

/// Gossip statistics
#[derive(Debug, Clone, Default)]
pub struct GossipStats {
    pub seen_messages: usize,
    pub total_propagations: usize,
    pub average_fanout: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{PublicKey, Signature};

    #[test]
    fn test_gossip_id() {
        let id1 = GossipId::random();
        let id2 = GossipId::random();
        assert_ne!(id1, id2);
        
        let hash = BlockHash::new([1u8; 32]);
        let id3 = GossipId::from_block_hash(&hash);
        let id4 = GossipId::from_block_hash(&hash);
        assert_eq!(id3, id4);
    }

    #[test]
    fn test_gossip_entry() {
        let id = GossipId::random();
        let mut entry = GossipEntry::new(id);
        
        let peer_id = PeerId::random();
        assert!(!entry.has_peer(&peer_id));
        
        entry.add_peer(peer_id);
        assert!(entry.has_peer(&peer_id));
    }

    #[tokio::test]
    async fn test_gossip_protocol_creation() {
        let gossip = GossipProtocol::new(4);
        assert_eq!(gossip.fanout, 4);
    }

    #[tokio::test]
    async fn test_mark_seen() {
        let gossip = GossipProtocol::new(4);
        let id = GossipId::random();
        
        assert!(!gossip.has_seen(&id).await);
        
        gossip.mark_seen(id).await;
        assert!(gossip.has_seen(&id).await);
    }

    #[tokio::test]
    async fn test_mark_propagated() {
        let gossip = GossipProtocol::new(4);
        let id = GossipId::random();
        let peer_id = PeerId::random();
        
        gossip.mark_seen(id).await;
        gossip.mark_propagated_to(id, peer_id).await;
        
        let seen = gossip.seen_messages.read().await;
        let entry = seen.get(&id).unwrap();
        assert!(entry.has_peer(&peer_id));
    }

    #[tokio::test]
    async fn test_get_stats() {
        let gossip = GossipProtocol::new(4);
        
        let id1 = GossipId::random();
        let id2 = GossipId::random();
        
        gossip.mark_seen(id1).await;
        gossip.mark_seen(id2).await;
        
        gossip.mark_propagated_to(id1, PeerId::random()).await;
        gossip.mark_propagated_to(id1, PeerId::random()).await;
        gossip.mark_propagated_to(id2, PeerId::random()).await;
        
        let stats = gossip.get_stats().await;
        assert_eq!(stats.seen_messages, 2);
        assert_eq!(stats.total_propagations, 3);
    }

    #[tokio::test]
    async fn test_clear() {
        let gossip = GossipProtocol::new(4);
        
        gossip.mark_seen(GossipId::random()).await;
        gossip.mark_seen(GossipId::random()).await;
        
        let stats = gossip.get_stats().await;
        assert_eq!(stats.seen_messages, 2);
        
        gossip.clear().await;
        
        let stats = gossip.get_stats().await;
        assert_eq!(stats.seen_messages, 0);
    }

    #[test]
    fn test_block_announcement() {
        let ann = BlockAnnouncement {
            height: 100,
            hash: BlockHash::zero(),
            timestamp: common::utils::current_timestamp(),
            transaction_count: 5,
        };
        
        assert_eq!(ann.height, 100);
        assert_eq!(ann.transaction_count, 5);
    }

    #[test]
    fn test_transaction_announcement() {
        let ann = TransactionAnnouncement {
            tx_id: TxId::new([1u8; 32]),
            timestamp: common::utils::current_timestamp(),
            size: 1024,
        };
        
        assert_eq!(ann.size, 1024);
    }

    #[tokio::test]
    async fn test_start_stop() {
        let gossip = GossipProtocol::new(4);
        
        gossip.start().await.unwrap();
        
        let running = *gossip.running.read().await;
        assert!(running);
        
        gossip.stop().await.unwrap();
        
        let running = *gossip.running.read().await;
        assert!(!running);
    }

    #[tokio::test]
    async fn test_gossip_with_ttl() {
        let gossip = GossipProtocol::new(4).with_ttl(600);
        assert_eq!(gossip.message_ttl, 600);
    }

    #[test]
    fn test_gossip_entry_stale() {
        let id = GossipId::random();
        let entry = GossipEntry::new(id);
        
        assert!(!entry.is_stale(1000));
    }

    #[tokio::test]
    async fn test_duplicate_gossip() {
        let gossip = GossipProtocol::new(4);
        let peers = Arc::new(RwLock::new(Vec::new()));
        
        let block = Block::new(
            1,
            BlockHash::zero(),
            vec![],
            PublicKey::new([1u8; 32]),
        );
        
        let message = GossipMessage::NewBlock(block.clone());
        
        let count1 = gossip.gossip(message.clone(), &peers).await.unwrap();
        assert_eq!(count1, 0);
        
        let count2 = gossip.gossip(message, &peers).await.unwrap();
        assert_eq!(count2, 0);
    }

    #[test]
    fn test_gossip_stats_default() {
        let stats = GossipStats::default();
        assert_eq!(stats.seen_messages, 0);
        assert_eq!(stats.total_propagations, 0);
        assert_eq!(stats.average_fanout, 0.0);
    }
}

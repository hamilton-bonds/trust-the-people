use crate::peer::{Peer, PeerId};
use crate::protocol::{BlockRequest, BlockResponse, Message};
use blockchain_core::Block;
use common::{BlockHash, BlockHeight, Result, Timestamp, VotingError};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// Blockchain synchronization state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncState {
    Idle,
    Syncing,
    Catching,
    Synced,
    Failed,
}

/// Sync status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub state: SyncState,
    pub current_height: BlockHeight,
    pub target_height: BlockHeight,
    pub start_height: BlockHeight,
    pub synced_blocks: u64,
    pub pending_requests: usize,
    pub active_peers: usize,
    pub sync_speed: f64,
    pub started_at: Option<Timestamp>,
    pub estimated_completion: Option<Timestamp>,
}

impl SyncStatus {
    pub fn progress_percent(&self) -> f64 {
        if self.target_height <= self.start_height {
            return 100.0;
        }
        
        let total = self.target_height - self.start_height;
        let done = self.current_height.saturating_sub(self.start_height);
        
        (done as f64 / total as f64) * 100.0
    }
    
    pub fn remaining_blocks(&self) -> u64 {
        self.target_height.saturating_sub(self.current_height)
    }
    
    pub fn is_syncing(&self) -> bool {
        matches!(self.state, SyncState::Syncing | SyncState::Catching)
    }
}

/// Block sync request tracking
struct SyncRequest {
    request_id: u64,
    peer_id: PeerId,
    start_height: BlockHeight,
    end_height: BlockHeight,
    requested_at: Timestamp,
    timeout: u64,
}

impl SyncRequest {
    fn new(
        request_id: u64,
        peer_id: PeerId,
        start_height: BlockHeight,
        end_height: BlockHeight,
        timeout: u64,
    ) -> Self {
        Self {
            request_id,
            peer_id,
            start_height,
            end_height,
            requested_at: common::utils::current_timestamp(),
            timeout,
        }
    }
    
    fn is_timed_out(&self) -> bool {
        let now = common::utils::current_timestamp();
        now.saturating_sub(self.requested_at) > self.timeout
    }
}

/// Block synchronization protocol
pub struct BlockSync {
    batch_size: usize,
    state: Arc<RwLock<SyncState>>,
    current_height: Arc<RwLock<BlockHeight>>,
    target_height: Arc<RwLock<BlockHeight>>,
    start_height: Arc<RwLock<BlockHeight>>,
    pending_requests: Arc<RwLock<HashMap<u64, SyncRequest>>>,
    received_blocks: Arc<RwLock<VecDeque<Block>>>,
    next_request_id: Arc<RwLock<u64>>,
    request_timeout: u64,
    running: Arc<RwLock<bool>>,
    sync_start_time: Arc<RwLock<Option<Timestamp>>>,
    synced_blocks: Arc<RwLock<u64>>,
}

impl BlockSync {
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch_size,
            state: Arc::new(RwLock::new(SyncState::Idle)),
            current_height: Arc::new(RwLock::new(0)),
            target_height: Arc::new(RwLock::new(0)),
            start_height: Arc::new(RwLock::new(0)),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            received_blocks: Arc::new(RwLock::new(VecDeque::new())),
            next_request_id: Arc::new(RwLock::new(0)),
            request_timeout: 30,
            running: Arc::new(RwLock::new(false)),
            sync_start_time: Arc::new(RwLock::new(None)),
            synced_blocks: Arc::new(RwLock::new(0)),
        }
    }
    
    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.request_timeout = timeout;
        self
    }
    
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Err(VotingError::NetworkError(
                "Sync already running".to_string(),
            ));
        }
        *running = true;
        
        let sync = self.clone();
        tokio::spawn(async move {
            sync.sync_loop().await;
        });
        
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;
        Ok(())
    }
    
    async fn sync_loop(&self) {
        let mut ticker = interval(Duration::from_secs(1));
        
        loop {
            ticker.tick().await;
            
            let running = *self.running.read().await;
            if !running {
                break;
            }
            
            if let Err(e) = self.process_sync().await {
                tracing::warn!("Sync processing error: {}", e);
            }
            
            if let Err(e) = self.check_timeouts().await {
                tracing::warn!("Timeout check error: {}", e);
            }
        }
    }
    
    async fn process_sync(&self) -> Result<()> {
        let state = *self.state.read().await;
        
        match state {
            SyncState::Syncing => {
                self.process_syncing().await?;
            }
            SyncState::Catching => {
                self.process_catching().await?;
            }
            _ => {}
        }
        
        Ok(())
    }
    
    async fn process_syncing(&self) -> Result<()> {
        let current = *self.current_height.read().await;
        let target = *self.target_height.read().await;
        
        if current >= target {
            self.complete_sync().await?;
            return Ok(());
        }
        
        Ok(())
    }
    
    async fn process_catching(&self) -> Result<()> {
        Ok(())
    }
    
    async fn complete_sync(&self) -> Result<()> {
        let mut state = self.state.write().await;
        *state = SyncState::Synced;
        
        tracing::info!("Blockchain sync completed");
        
        Ok(())
    }
    
    pub async fn request_blocks(
        &self,
        start_height: BlockHeight,
        end_height: BlockHeight,
        peers: &Arc<RwLock<Vec<Peer>>>,
    ) -> Result<()> {
        if start_height >= end_height {
            return Err(VotingError::NetworkError(
                "Invalid block range".to_string(),
            ));
        }
        
        let mut state = self.state.write().await;
        *state = SyncState::Syncing;
        
        let mut current_height = self.current_height.write().await;
        *current_height = start_height;
        
        let mut target_height = self.target_height.write().await;
        *target_height = end_height;
        
        let mut start_h = self.start_height.write().await;
        *start_h = start_height;
        
        let mut sync_start = self.sync_start_time.write().await;
        *sync_start = Some(common::utils::current_timestamp());
        
        drop(state);
        drop(current_height);
        drop(target_height);
        drop(start_h);
        drop(sync_start);
        
        let mut current = start_height;
        while current < end_height {
            let batch_end = (current + self.batch_size as u64).min(end_height);
            
            if let Err(e) = self.request_block_batch(current, batch_end, peers).await {
                tracing::warn!("Failed to request blocks {}-{}: {}", current, batch_end, e);
            }
            
            current = batch_end;
        }
        
        Ok(())
    }
    
    async fn request_block_batch(
        &self,
        start: BlockHeight,
        end: BlockHeight,
        peers: &Arc<RwLock<Vec<Peer>>>,
    ) -> Result<()> {
        let peer = self.select_sync_peer(peers).await?;
        let request_id = self.next_request_id().await;
        
        let request = BlockRequest {
            start_height: start,
            end_height: end,
            max_blocks: self.batch_size,
        };
        
        let message = Message::get_blocks(request)?;
        peer.send(message).await?;
        
        let sync_request = SyncRequest::new(
            request_id,
            peer.id(),
            start,
            end,
            self.request_timeout,
        );
        
        self.pending_requests
            .write()
            .await
            .insert(request_id, sync_request);
        
        Ok(())
    }
    
    async fn select_sync_peer(&self, peers: &Arc<RwLock<Vec<Peer>>>) -> Result<Peer> {
        let peers_guard = peers.read().await;
        
        let connected_peers: Vec<&Peer> = peers_guard
            .iter()
            .filter(|p| p.is_connected())
            .collect();
        
        if connected_peers.is_empty() {
            return Err(VotingError::NetworkError("No peers available".to_string()));
        }
        
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        let peer = connected_peers
            .choose(&mut rng)
            .ok_or_else(|| VotingError::NetworkError("Failed to select peer".to_string()))?;
        
        Ok((*peer).clone())
    }
    
    async fn next_request_id(&self) -> u64 {
        let mut id = self.next_request_id.write().await;
        let current = *id;
        *id += 1;
        current
    }
    
    pub async fn handle_block_response(
        &self,
        response: BlockResponse,
        _from_peer: PeerId,
    ) -> Result<()> {
        let mut blocks = self.received_blocks.write().await;
        
        for block in response.blocks {
            blocks.push_back(block);
        }
        
        let mut synced = self.synced_blocks.write().await;
        *synced += blocks.len() as u64;
        
        let mut current = self.current_height.write().await;
        if let Some(last_block) = blocks.back() {
            *current = last_block.height();
        }
        
        Ok(())
    }
    
    pub async fn get_next_block(&self) -> Option<Block> {
        self.received_blocks.write().await.pop_front()
    }
    
    pub async fn pending_block_count(&self) -> usize {
        self.received_blocks.read().await.len()
    }
    
    async fn check_timeouts(&self) -> Result<()> {
        let mut pending = self.pending_requests.write().await;
        
        let timed_out: Vec<u64> = pending
            .iter()
            .filter(|(_, req)| req.is_timed_out())
            .map(|(id, _)| *id)
            .collect();
        
        for id in timed_out {
            if let Some(req) = pending.remove(&id) {
                tracing::warn!(
                    "Sync request {} timed out: {}-{} from peer {}",
                    id,
                    req.start_height,
                    req.end_height,
                    req.peer_id
                );
            }
        }
        
        Ok(())
    }
    
    pub async fn status(&self) -> SyncStatus {
        let state = *self.state.read().await;
        let current = *self.current_height.read().await;
        let target = *self.target_height.read().await;
        let start = *self.start_height.read().await;
        let pending = self.pending_requests.read().await.len();
        let synced = *self.synced_blocks.read().await;
        let start_time = *self.sync_start_time.read().await;
        
        let sync_speed = if let Some(started) = start_time {
            let elapsed = common::utils::current_timestamp().saturating_sub(started);
            if elapsed > 0 {
                synced as f64 / elapsed as f64
            } else {
                0.0
            }
        } else {
            0.0
        };
        
        let estimated_completion = if sync_speed > 0.0 {
            let remaining = target.saturating_sub(current);
            let time_remaining = (remaining as f64 / sync_speed) as u64;
            Some(common::utils::current_timestamp() + time_remaining)
        } else {
            None
        };
        
        SyncStatus {
            state,
            current_height: current,
            target_height: target,
            start_height: start,
            synced_blocks: synced,
            pending_requests: pending,
            active_peers: 0,
            sync_speed,
            started_at: start_time,
            estimated_completion,
        }
    }
    
    pub async fn set_state(&self, state: SyncState) {
        *self.state.write().await = state;
    }
    
    pub async fn current_height(&self) -> BlockHeight {
        *self.current_height.read().await
    }
    
    pub async fn target_height(&self) -> BlockHeight {
        *self.target_height.read().await
    }
    
    pub async fn is_syncing(&self) -> bool {
        self.status().await.is_syncing()
    }
}

impl Clone for BlockSync {
    fn clone(&self) -> Self {
        Self {
            batch_size: self.batch_size,
            state: Arc::clone(&self.state),
            current_height: Arc::clone(&self.current_height),
            target_height: Arc::clone(&self.target_height),
            start_height: Arc::clone(&self.start_height),
            pending_requests: Arc::clone(&self.pending_requests),
            received_blocks: Arc::clone(&self.received_blocks),
            next_request_id: Arc::clone(&self.next_request_id),
            request_timeout: self.request_timeout,
            running: Arc::clone(&self.running),
            sync_start_time: Arc::clone(&self.sync_start_time),
            synced_blocks: Arc::clone(&self.synced_blocks),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::PublicKey;

    #[test]
    fn test_sync_state() {
        assert_ne!(SyncState::Idle, SyncState::Syncing);
        assert_ne!(SyncState::Synced, SyncState::Failed);
    }

    #[test]
    fn test_sync_status_progress() {
        let status = SyncStatus {
            state: SyncState::Syncing,
            current_height: 50,
            target_height: 100,
            start_height: 0,
            synced_blocks: 50,
            pending_requests: 2,
            active_peers: 3,
            sync_speed: 5.0,
            started_at: None,
            estimated_completion: None,
        };
        
        assert_eq!(status.progress_percent(), 50.0);
        assert_eq!(status.remaining_blocks(), 50);
        assert!(status.is_syncing());
    }

    #[test]
    fn test_sync_status_complete() {
        let status = SyncStatus {
            state: SyncState::Synced,
            current_height: 100,
            target_height: 100,
            start_height: 0,
            synced_blocks: 100,
            pending_requests: 0,
            active_peers: 3,
            sync_speed: 10.0,
            started_at: None,
            estimated_completion: None,
        };
        
        assert_eq!(status.progress_percent(), 100.0);
        assert_eq!(status.remaining_blocks(), 0);
        assert!(!status.is_syncing());
    }

    #[tokio::test]
    async fn test_block_sync_creation() {
        let sync = BlockSync::new(100);
        assert_eq!(sync.batch_size, 100);
        
        let status = sync.status().await;
        assert_eq!(status.state, SyncState::Idle);
    }

    #[tokio::test]
    async fn test_block_sync_start_stop() {
        let sync = BlockSync::new(100);
        
        sync.start().await.unwrap();
        let running = *sync.running.read().await;
        assert!(running);
        
        sync.stop().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let running = *sync.running.read().await;
        assert!(!running);
    }

    #[tokio::test]
    async fn test_next_request_id() {
        let sync = BlockSync::new(100);
        
        let id1 = sync.next_request_id().await;
        let id2 = sync.next_request_id().await;
        let id3 = sync.next_request_id().await;
        
        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(id3, 2);
    }

    #[tokio::test]
    async fn test_handle_block_response() {
        let sync = BlockSync::new(100);
        
        let blocks = vec![
            Block::new(1, BlockHash::zero(), vec![], PublicKey::new([1u8; 32])),
            Block::new(2, BlockHash::zero(), vec![], PublicKey::new([1u8; 32])),
        ];
        
        let response = BlockResponse {
            blocks,
            start_height: 1,
            end_height: 2,
        };
        
        sync.handle_block_response(response, PeerId::random())
            .await
            .unwrap();
        
        assert_eq!(sync.pending_block_count().await, 2);
        assert_eq!(sync.current_height().await, 2);
    }

    #[tokio::test]
    async fn test_get_next_block() {
        let sync = BlockSync::new(100);
        
        let block = Block::new(1, BlockHash::zero(), vec![], PublicKey::new([1u8; 32]));
        
        let response = BlockResponse {
            blocks: vec![block.clone()],
            start_height: 1,
            end_height: 1,
        };
        
        sync.handle_block_response(response, PeerId::random())
            .await
            .unwrap();
        
        let retrieved = sync.get_next_block().await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().height(), 1);
        
        assert_eq!(sync.pending_block_count().await, 0);
    }

    #[tokio::test]
    async fn test_set_state() {
        let sync = BlockSync::new(100);
        
        sync.set_state(SyncState::Syncing).await;
        let status = sync.status().await;
        assert_eq!(status.state, SyncState::Syncing);
        
        sync.set_state(SyncState::Synced).await;
        let status = sync.status().await;
        assert_eq!(status.state, SyncState::Synced);
    }

    #[test]
    fn test_sync_request_timeout() {
        let req = SyncRequest::new(1, PeerId::random(), 0, 100, 1);
        
        assert!(!req.is_timed_out());
    }

    #[tokio::test]
    async fn test_sync_with_timeout() {
        let sync = BlockSync::new(100).with_timeout(60);
        assert_eq!(sync.request_timeout, 60);
    }

    #[tokio::test]
    async fn test_is_syncing() {
        let sync = BlockSync::new(100);
        
        assert!(!sync.is_syncing().await);
        
        sync.set_state(SyncState::Syncing).await;
        assert!(sync.is_syncing().await);
        
        sync.set_state(SyncState::Synced).await;
        assert!(!sync.is_syncing().await);
    }

    #[tokio::test]
    async fn test_target_height() {
        let sync = BlockSync::new(100);
        
        let peers = Arc::new(RwLock::new(Vec::new()));
        
        let result = sync.request_blocks(0, 100, &peers).await;
        assert!(result.is_ok());
        
        assert_eq!(sync.current_height().await, 0);
        assert_eq!(sync.target_height().await, 100);
    }

    #[tokio::test]
    async fn test_invalid_block_range() {
        let sync = BlockSync::new(100);
        let peers = Arc::new(RwLock::new(Vec::new()));
        
        let result = sync.request_blocks(100, 50, &peers).await;
        assert!(result.is_err());
    }
}

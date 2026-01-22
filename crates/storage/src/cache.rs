use common::{BlockHash, BlockHeight, Result, TxId, VotingError};
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::{Arc, RwLock};
use serde::{Deserialize, Serialize};

/// Cache for blockchain and transaction data
#[derive(Clone)]
pub struct Cache {
    /// Block cache by hash
    block_cache: Arc<RwLock<LruCache<BlockHash, Vec<u8>>>>,
    
    /// Block cache by height
    height_cache: Arc<RwLock<LruCache<BlockHeight, BlockHash>>>,
    
    /// Transaction cache
    tx_cache: Arc<RwLock<LruCache<TxId, Vec<u8>>>>,
    
    /// State data cache (arbitrary key-value)
    state_cache: Arc<RwLock<LruCache<Vec<u8>, Vec<u8>>>>,
}

impl Cache {
    /// Create a new cache with specified sizes
    pub fn new(
        block_capacity: usize,
        tx_capacity: usize,
        state_capacity: usize,
    ) -> Result<Self> {
        let block_size = NonZeroUsize::new(block_capacity)
            .ok_or_else(|| VotingError::StorageError("Block cache capacity must be > 0".to_string()))?;
        let tx_size = NonZeroUsize::new(tx_capacity)
            .ok_or_else(|| VotingError::StorageError("TX cache capacity must be > 0".to_string()))?;
        let state_size = NonZeroUsize::new(state_capacity)
            .ok_or_else(|| VotingError::StorageError("State cache capacity must be > 0".to_string()))?;
        
        Ok(Self {
            block_cache: Arc::new(RwLock::new(LruCache::new(block_size))),
            height_cache: Arc::new(RwLock::new(LruCache::new(block_size))),
            tx_cache: Arc::new(RwLock::new(LruCache::new(tx_size))),
            state_cache: Arc::new(RwLock::new(LruCache::new(state_size))),
        })
    }
    
    /// Create a cache with default sizes
    pub fn with_defaults() -> Self {
        Self::new(1000, 5000, 2000).expect("Failed to create cache with defaults")
    }
    
    /// Put a block in the cache
    pub fn put_block(&self, hash: BlockHash, height: BlockHeight, data: Vec<u8>) {
        let mut block_cache = self.block_cache.write().unwrap();
        let mut height_cache = self.height_cache.write().unwrap();
        
        block_cache.put(hash, data);
        height_cache.put(height, hash);
    }
    
    /// Get a block by hash
    pub fn get_block(&self, hash: &BlockHash) -> Option<Vec<u8>> {
        self.block_cache.write().unwrap().get(hash).cloned()
    }
    
    /// Get a block hash by height
    pub fn get_block_hash_by_height(&self, height: BlockHeight) -> Option<BlockHash> {
        self.height_cache.write().unwrap().get(&height).copied()
    }
    
    /// Get a block by height (combines height and hash lookups)
    pub fn get_block_by_height(&self, height: BlockHeight) -> Option<Vec<u8>> {
        let hash = self.get_block_hash_by_height(height)?;
        self.get_block(&hash)
    }
    
    /// Put a transaction in the cache
    pub fn put_transaction(&self, tx_id: TxId, data: Vec<u8>) {
        self.tx_cache.write().unwrap().put(tx_id, data);
    }
    
    /// Get a transaction from the cache
    pub fn get_transaction(&self, tx_id: &TxId) -> Option<Vec<u8>> {
        self.tx_cache.write().unwrap().get(tx_id).cloned()
    }
    
    /// Put arbitrary state data in the cache
    pub fn put_state(&self, key: Vec<u8>, value: Vec<u8>) {
        self.state_cache.write().unwrap().put(key, value);
    }
    
    /// Get arbitrary state data from the cache
    pub fn get_state(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.state_cache.write().unwrap().get(key).cloned()
    }
    
    /// Remove a block from the cache
    pub fn remove_block(&self, hash: &BlockHash) {
        self.block_cache.write().unwrap().pop(hash);
    }
    
    /// Remove a block by height from the cache
    pub fn remove_block_by_height(&self, height: BlockHeight) {
        if let Some(hash) = self.height_cache.write().unwrap().pop(&height) {
            self.block_cache.write().unwrap().pop(&hash);
        }
    }
    
    /// Remove a transaction from the cache
    pub fn remove_transaction(&self, tx_id: &TxId) {
        self.tx_cache.write().unwrap().pop(tx_id);
    }
    
    /// Remove state data from the cache
    pub fn remove_state(&self, key: &[u8]) {
        self.state_cache.write().unwrap().pop(key);
    }
    
    /// Clear all caches
    pub fn clear_all(&self) {
        self.block_cache.write().unwrap().clear();
        self.height_cache.write().unwrap().clear();
        self.tx_cache.write().unwrap().clear();
        self.state_cache.write().unwrap().clear();
    }
    
    /// Clear block cache only
    pub fn clear_blocks(&self) {
        self.block_cache.write().unwrap().clear();
        self.height_cache.write().unwrap().clear();
    }
    
    /// Clear transaction cache only
    pub fn clear_transactions(&self) {
        self.tx_cache.write().unwrap().clear();
    }
    
    /// Clear state cache only
    pub fn clear_state(&self) {
        self.state_cache.write().unwrap().clear();
    }
    
    /// Get cache statistics
    pub fn get_stats(&self) -> CacheStats {
        let block_cache = self.block_cache.read().unwrap();
        let height_cache = self.height_cache.read().unwrap();
        let tx_cache = self.tx_cache.read().unwrap();
        let state_cache = self.state_cache.read().unwrap();
        
        CacheStats {
            block_count: block_cache.len(),
            block_capacity: block_cache.cap().get(),
            height_count: height_cache.len(),
            height_capacity: height_cache.cap().get(),
            tx_count: tx_cache.len(),
            tx_capacity: tx_cache.cap().get(),
            state_count: state_cache.len(),
            state_capacity: state_cache.cap().get(),
        }
    }
    
    /// Calculate cache hit rate (requires external tracking)
    pub fn calculate_hit_rate(&self, hits: u64, misses: u64) -> f64 {
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }
    
    /// Check if block is cached
    pub fn contains_block(&self, hash: &BlockHash) -> bool {
        self.block_cache.read().unwrap().contains(hash)
    }
    
    /// Check if transaction is cached
    pub fn contains_transaction(&self, tx_id: &TxId) -> bool {
        self.tx_cache.read().unwrap().contains(tx_id)
    }
    
    /// Check if state key is cached
    pub fn contains_state(&self, key: &[u8]) -> bool {
        self.state_cache.read().unwrap().contains(key)
    }
    
    /// Peek at block without updating LRU
    pub fn peek_block(&self, hash: &BlockHash) -> Option<Vec<u8>> {
        self.block_cache.read().unwrap().peek(hash).cloned()
    }
    
    /// Peek at transaction without updating LRU
    pub fn peek_transaction(&self, tx_id: &TxId) -> Option<Vec<u8>> {
        self.tx_cache.read().unwrap().peek(tx_id).cloned()
    }
    
    /// Peek at state without updating LRU
    pub fn peek_state(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.state_cache.read().unwrap().peek(key).cloned()
    }
    
    /// Resize block cache
    pub fn resize_block_cache(&self, new_capacity: usize) -> Result<()> {
        let new_size = NonZeroUsize::new(new_capacity)
            .ok_or_else(|| VotingError::StorageError("Capacity must be > 0".to_string()))?;
        
        self.block_cache.write().unwrap().resize(new_size);
        self.height_cache.write().unwrap().resize(new_size);
        Ok(())
    }
    
    /// Resize transaction cache
    pub fn resize_tx_cache(&self, new_capacity: usize) -> Result<()> {
        let new_size = NonZeroUsize::new(new_capacity)
            .ok_or_else(|| VotingError::StorageError("Capacity must be > 0".to_string()))?;
        
        self.tx_cache.write().unwrap().resize(new_size);
        Ok(())
    }
    
    /// Resize state cache
    pub fn resize_state_cache(&self, new_capacity: usize) -> Result<()> {
        let new_size = NonZeroUsize::new(new_capacity)
            .ok_or_else(|| VotingError::StorageError("Capacity must be > 0".to_string()))?;
        
        self.state_cache.write().unwrap().resize(new_size);
        Ok(())
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub block_count: usize,
    pub block_capacity: usize,
    pub height_count: usize,
    pub height_capacity: usize,
    pub tx_count: usize,
    pub tx_capacity: usize,
    pub state_count: usize,
    pub state_capacity: usize,
}

impl CacheStats {
    /// Calculate block cache usage percentage
    pub fn block_usage_percent(&self) -> f64 {
        if self.block_capacity == 0 {
            0.0
        } else {
            (self.block_count as f64 / self.block_capacity as f64) * 100.0
        }
    }
    
    /// Calculate transaction cache usage percentage
    pub fn tx_usage_percent(&self) -> f64 {
        if self.tx_capacity == 0 {
            0.0
        } else {
            (self.tx_count as f64 / self.tx_capacity as f64) * 100.0
        }
    }
    
    /// Calculate state cache usage percentage
    pub fn state_usage_percent(&self) -> f64 {
        if self.state_capacity == 0 {
            0.0
        } else {
            (self.state_count as f64 / self.state_capacity as f64) * 100.0
        }
    }
    
    /// Get total cache usage across all caches
    pub fn total_items(&self) -> usize {
        self.block_count + self.tx_count + self.state_count
    }
    
    /// Get total cache capacity across all caches
    pub fn total_capacity(&self) -> usize {
        self.block_capacity + self.tx_capacity + self.state_capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_creation() {
        let cache = Cache::new(100, 200, 150).unwrap();
        let stats = cache.get_stats();
        
        assert_eq!(stats.block_capacity, 100);
        assert_eq!(stats.tx_capacity, 200);
        assert_eq!(stats.state_capacity, 150);
    }

    #[test]
    fn test_cache_defaults() {
        let cache = Cache::with_defaults();
        let stats = cache.get_stats();
        
        assert_eq!(stats.block_capacity, 1000);
        assert_eq!(stats.tx_capacity, 5000);
        assert_eq!(stats.state_capacity, 2000);
    }

    #[test]
    fn test_block_cache() {
        let cache = Cache::with_defaults();
        
        let hash = BlockHash::new([1u8; 32]);
        let height = 100;
        let data = vec![1, 2, 3, 4, 5];
        
        cache.put_block(hash, height, data.clone());
        
        assert!(cache.contains_block(&hash));
        assert_eq!(cache.get_block(&hash), Some(data.clone()));
        assert_eq!(cache.get_block_hash_by_height(height), Some(hash));
        assert_eq!(cache.get_block_by_height(height), Some(data));
    }

    #[test]
    fn test_transaction_cache() {
        let cache = Cache::with_defaults();
        
        let tx_id = TxId::new([1u8; 32]);
        let data = vec![10, 20, 30];
        
        cache.put_transaction(tx_id, data.clone());
        
        assert!(cache.contains_transaction(&tx_id));
        assert_eq!(cache.get_transaction(&tx_id), Some(data));
    }

    #[test]
    fn test_state_cache() {
        let cache = Cache::with_defaults();
        
        let key = b"test_key".to_vec();
        let value = vec![100, 200];
        
        cache.put_state(key.clone(), value.clone());
        
        assert!(cache.contains_state(&key));
        assert_eq!(cache.get_state(&key), Some(value));
    }

    #[test]
    fn test_remove_block() {
        let cache = Cache::with_defaults();
        
        let hash = BlockHash::new([1u8; 32]);
        let height = 100;
        let data = vec![1, 2, 3];
        
        cache.put_block(hash, height, data);
        assert!(cache.contains_block(&hash));
        
        cache.remove_block(&hash);
        assert!(!cache.contains_block(&hash));
    }

    #[test]
    fn test_remove_transaction() {
        let cache = Cache::with_defaults();
        
        let tx_id = TxId::new([1u8; 32]);
        let data = vec![1, 2, 3];
        
        cache.put_transaction(tx_id, data);
        assert!(cache.contains_transaction(&tx_id));
        
        cache.remove_transaction(&tx_id);
        assert!(!cache.contains_transaction(&tx_id));
    }

    #[test]
    fn test_clear_caches() {
        let cache = Cache::with_defaults();
        
        cache.put_block(BlockHash::new([1u8; 32]), 1, vec![1]);
        cache.put_transaction(TxId::new([2u8; 32]), vec![2]);
        cache.put_state(vec![3], vec![3]);
        
        let stats = cache.get_stats();
        assert_eq!(stats.block_count, 1);
        assert_eq!(stats.tx_count, 1);
        assert_eq!(stats.state_count, 1);
        
        cache.clear_all();
        
        let stats = cache.get_stats();
        assert_eq!(stats.block_count, 0);
        assert_eq!(stats.tx_count, 0);
        assert_eq!(stats.state_count, 0);
    }

    #[test]
    fn test_clear_blocks_only() {
        let cache = Cache::with_defaults();
        
        cache.put_block(BlockHash::new([1u8; 32]), 1, vec![1]);
        cache.put_transaction(TxId::new([2u8; 32]), vec![2]);
        
        cache.clear_blocks();
        
        let stats = cache.get_stats();
        assert_eq!(stats.block_count, 0);
        assert_eq!(stats.tx_count, 1);
    }

    #[test]
    fn test_lru_eviction() {
        let cache = Cache::new(2, 2, 2).unwrap();
        
        cache.put_block(BlockHash::new([1u8; 32]), 1, vec![1]);
        cache.put_block(BlockHash::new([2u8; 32]), 2, vec![2]);
        cache.put_block(BlockHash::new([3u8; 32]), 3, vec![3]);
        
        let stats = cache.get_stats();
        assert_eq!(stats.block_count, 2);
        
        assert!(!cache.contains_block(&BlockHash::new([1u8; 32])));
        assert!(cache.contains_block(&BlockHash::new([2u8; 32])));
        assert!(cache.contains_block(&BlockHash::new([3u8; 32])));
    }

    #[test]
    fn test_peek_without_updating() {
        let cache = Cache::new(2, 2, 2).unwrap();
        
        cache.put_block(BlockHash::new([1u8; 32]), 1, vec![1]);
        cache.put_block(BlockHash::new([2u8; 32]), 2, vec![2]);
        
        let _ = cache.peek_block(&BlockHash::new([1u8; 32]));
        
        cache.put_block(BlockHash::new([3u8; 32]), 3, vec![3]);
        
        assert!(!cache.contains_block(&BlockHash::new([1u8; 32])));
    }

    #[test]
    fn test_hit_rate_calculation() {
        let cache = Cache::with_defaults();
        
        assert_eq!(cache.calculate_hit_rate(75, 25), 0.75);
        assert_eq!(cache.calculate_hit_rate(100, 0), 1.0);
        assert_eq!(cache.calculate_hit_rate(0, 100), 0.0);
        assert_eq!(cache.calculate_hit_rate(0, 0), 0.0);
    }

    #[test]
    fn test_cache_stats_percentages() {
        let cache = Cache::new(100, 200, 150).unwrap();
        
        for i in 0..50 {
            cache.put_block(BlockHash::new([i; 32]), i as u64, vec![i]);
        }
        
        for i in 0..100 {
            cache.put_transaction(TxId::new([i; 32]), vec![i]);
        }
        
        let stats = cache.get_stats();
        assert_eq!(stats.block_usage_percent(), 50.0);
        assert_eq!(stats.tx_usage_percent(), 50.0);
    }

    #[test]
    fn test_resize_cache() {
        let cache = Cache::new(10, 10, 10).unwrap();
        
        for i in 0..10 {
            cache.put_block(BlockHash::new([i; 32]), i as u64, vec![i]);
        }
        
        cache.resize_block_cache(5).unwrap();
        
        let stats = cache.get_stats();
        assert_eq!(stats.block_capacity, 5);
        assert!(stats.block_count <= 5);
    }

    #[test]
    fn test_zero_capacity_error() {
        assert!(Cache::new(0, 10, 10).is_err());
        assert!(Cache::new(10, 0, 10).is_err());
        assert!(Cache::new(10, 10, 0).is_err());
    }

    #[test]
    fn test_total_stats() {
        let cache = Cache::new(100, 200, 150).unwrap();
        
        cache.put_block(BlockHash::new([1u8; 32]), 1, vec![1]);
        cache.put_transaction(TxId::new([2u8; 32]), vec![2]);
        cache.put_state(vec![3], vec![3]);
        
        let stats = cache.get_stats();
        assert_eq!(stats.total_items(), 3);
        assert_eq!(stats.total_capacity(), 450);
    }
}

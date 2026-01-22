use common::{BlockHash, BlockHeight, PublicKey, Timestamp};
use crate::block::Block;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Tracks block finality based on validator signatures
/// 
/// Finality means a block is irreversible and permanently part of the chain.
/// In our system, finality is achieved when 2/3+ validators have signed the block.
#[derive(Debug, Clone)]
pub struct FinalityTracker {
    /// Finality threshold (e.g., 0.67 for 2/3)
    threshold: f64,
    
    /// Map of block hash to finality info
    finality_map: HashMap<BlockHash, FinalityInfo>,
    
    /// Highest finalized block height
    highest_finalized_height: BlockHeight,
    
    /// Hash of highest finalized block
    highest_finalized_hash: BlockHash,
}

/// Information about a block's finality status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalityInfo {
    /// Block hash
    pub block_hash: BlockHash,
    
    /// Block height
    pub height: BlockHeight,
    
    /// Validators who have signed this block
    pub signatures: HashSet<PublicKey>,
    
    /// Total number of validators at the time
    pub total_validators: usize,
    
    /// Whether this block is finalized
    pub is_finalized: bool,
    
    /// Timestamp when finality was achieved (if finalized)
    pub finalized_at: Option<Timestamp>,
}

/// Result of checking block finality
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinalityStatus {
    /// Block is finalized (irreversible)
    Finalized,
    
    /// Block is not yet finalized
    Pending {
        /// Number of signatures received
        signatures: usize,
        
        /// Number of signatures required
        required: usize,
    },
}

impl FinalityTracker {
    /// Create a new finality tracker
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold,
            finality_map: HashMap::new(),
            highest_finalized_height: 0,
            highest_finalized_hash: BlockHash::zero(),
        }
    }
    
    /// Check and update finality for a block
    pub fn check_finality(&mut self, block: &Block, total_validators: usize) -> FinalityStatus {
        let block_hash = block.hash();
        let _signature_count = block.signature_count(); // Adding _ before variable to bypass unused warnings
        let required_signatures = self.calculate_required_signatures(total_validators);
        
        // Get or create finality info
        let info = self.finality_map.entry(block_hash).or_insert_with(|| {
            FinalityInfo {
                block_hash,
                height: block.height(),
                signatures: HashSet::new(),
                total_validators,
                is_finalized: false,
                finalized_at: None,
            }
        });
        
        // Update signatures
        for sig in &block.signatures {
            info.signatures.insert(sig.validator);
        }
        
        // Check if finality threshold is met
        if info.signatures.len() >= required_signatures && !info.is_finalized {
            info.is_finalized = true;
            info.finalized_at = Some(common::utils::current_timestamp());
            
            // Update highest finalized block if this is higher
            if block.height() > self.highest_finalized_height {
                self.highest_finalized_height = block.height();
                self.highest_finalized_hash = block_hash;
            }
            
            FinalityStatus::Finalized
        } else if info.is_finalized {
            FinalityStatus::Finalized
        } else {
            FinalityStatus::Pending {
                signatures: info.signatures.len(),
                required: required_signatures,
            }
        }
    }
    
    /// Calculate required number of signatures for finality
    fn calculate_required_signatures(&self, total_validators: usize) -> usize {
        if total_validators == 0 {
            return 0;
        }
        
        // Calculate threshold (e.g., 2/3+ for BFT)
        let required = (total_validators as f64 * self.threshold).ceil() as usize;
        required.max(1) // At least 1 signature
    }
    
    /// Check if a block is finalized
    pub fn is_finalized(&self, block_hash: &BlockHash) -> bool {
        self.finality_map
            .get(block_hash)
            .map(|info| info.is_finalized)
            .unwrap_or(false)
    }
    
    /// Get finality info for a block
    pub fn get_finality_info(&self, block_hash: &BlockHash) -> Option<&FinalityInfo> {
        self.finality_map.get(block_hash)
    }
    
    /// Get highest finalized block height
    pub fn highest_finalized_height(&self) -> BlockHeight {
        self.highest_finalized_height
    }
    
    /// Get highest finalized block hash
    pub fn highest_finalized_hash(&self) -> BlockHash {
        self.highest_finalized_hash
    }
    
    /// Get all finalized blocks
    pub fn get_finalized_blocks(&self) -> Vec<&FinalityInfo> {
        self.finality_map
            .values()
            .filter(|info| info.is_finalized)
            .collect()
    }
    
    /// Get pending (not finalized) blocks
    pub fn get_pending_blocks(&self) -> Vec<&FinalityInfo> {
        self.finality_map
            .values()
            .filter(|info| !info.is_finalized)
            .collect()
    }
    
    /// Count finalized blocks
    pub fn finalized_count(&self) -> usize {
        self.finality_map
            .values()
            .filter(|info| info.is_finalized)
            .count()
    }
    
    /// Count pending blocks
    pub fn pending_count(&self) -> usize {
        self.finality_map
            .values()
            .filter(|info| !info.is_finalized)
            .count()
    }
    
    /// Prune old finality data (remove blocks below certain height)
    pub fn prune_below_height(&mut self, min_height: BlockHeight) {
        self.finality_map.retain(|_, info| info.height >= min_height);
    }
    
    /// Clear all finality data
    pub fn clear(&mut self) {
        self.finality_map.clear();
        self.highest_finalized_height = 0;
        self.highest_finalized_hash = BlockHash::zero();
    }
    
    /// Get finality statistics
    pub fn get_stats(&self) -> FinalityStats {
        let total_blocks = self.finality_map.len();
        let finalized_blocks = self.finalized_count();
        let pending_blocks = self.pending_count();
        
        FinalityStats {
            total_blocks,
            finalized_blocks,
            pending_blocks,
            highest_finalized_height: self.highest_finalized_height,
            highest_finalized_hash: self.highest_finalized_hash,
            threshold: self.threshold,
        }
    }
}

/// Finality statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalityStats {
    pub total_blocks: usize,
    pub finalized_blocks: usize,
    pub pending_blocks: usize,
    pub highest_finalized_height: BlockHeight,
    pub highest_finalized_hash: BlockHash,
    pub threshold: f64,
}

impl FinalityStatus {
    /// Check if status is finalized
    pub fn is_finalized(&self) -> bool {
        matches!(self, FinalityStatus::Finalized)
    }
    
    /// Get signature count (if pending)
    pub fn signature_count(&self) -> Option<usize> {
        match self {
            FinalityStatus::Pending { signatures, .. } => Some(*signatures),
            FinalityStatus::Finalized => None,
        }
    }
    
    /// Get required signature count (if pending)
    pub fn required_count(&self) -> Option<usize> {
        match self {
            FinalityStatus::Pending { required, .. } => Some(*required),
            FinalityStatus::Finalized => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::Signature;
    use crate::block::Block;

    fn create_test_block(height: BlockHeight) -> Block {
        let validator = PublicKey::new([1u8; 32]);
        Block::new(height, BlockHash::zero(), vec![], validator)
    }

    fn add_signatures(block: &mut Block, count: usize) {
        for i in 0..count {
            let mut key = [0u8; 32];
            key[0] = i as u8;
            let validator = PublicKey::new(key);
            let signature = Signature::new([0u8; 64]);
            block.add_signature(validator, signature);
        }
    }

    #[test]
    fn test_finality_tracker_creation() {
        let tracker = FinalityTracker::new(0.67);
        assert_eq!(tracker.threshold, 0.67);
        assert_eq!(tracker.highest_finalized_height(), 0);
    }

    #[test]
    fn test_calculate_required_signatures() {
        let tracker = FinalityTracker::new(0.67);
        
        // 3 validators: ceil(3 * 0.67) = 2
        assert_eq!(tracker.calculate_required_signatures(3), 2);
        
        // 4 validators: ceil(4 * 0.67) = 3
        assert_eq!(tracker.calculate_required_signatures(4), 3);
        
        // 10 validators: ceil(10 * 0.67) = 7
        assert_eq!(tracker.calculate_required_signatures(10), 7);
    }

    #[test]
    fn test_finality_pending() {
        let mut tracker = FinalityTracker::new(0.67);
        let mut block = create_test_block(1);
        
        // Add 1 signature (need 2 out of 3)
        add_signatures(&mut block, 1);
        
        let status = tracker.check_finality(&block, 3);
        
        match status {
            FinalityStatus::Pending { signatures, required } => {
                assert_eq!(signatures, 1);
                assert_eq!(required, 2);
            }
            _ => panic!("Expected pending status"),
        }
    }

    #[test]
    fn test_finality_achieved() {
        let mut tracker = FinalityTracker::new(0.67);
        let mut block = create_test_block(1);
        
        // Add 2 signatures (need 2 out of 3)
        add_signatures(&mut block, 2);
        
        let status = tracker.check_finality(&block, 3);
        
        assert!(status.is_finalized());
        assert!(tracker.is_finalized(&block.hash()));
    }

    #[test]
    fn test_highest_finalized_height() {
        let mut tracker = FinalityTracker::new(0.67);
        
        let mut block1 = create_test_block(1);
        add_signatures(&mut block1, 2);
        tracker.check_finality(&block1, 3);
        
        assert_eq!(tracker.highest_finalized_height(), 1);
        
        let mut block2 = create_test_block(2);
        add_signatures(&mut block2, 2);
        tracker.check_finality(&block2, 3);
        
        assert_eq!(tracker.highest_finalized_height(), 2);
    }

    #[test]
    fn test_finality_info() {
        let mut tracker = FinalityTracker::new(0.67);
        let mut block = create_test_block(1);
        add_signatures(&mut block, 2);
        
        tracker.check_finality(&block, 3);
        
        let info = tracker.get_finality_info(&block.hash()).unwrap();
        assert_eq!(info.height, 1);
        assert_eq!(info.signatures.len(), 2);
        assert!(info.is_finalized);
        assert!(info.finalized_at.is_some());
    }

    #[test]
    fn test_finalized_count() {
        let mut tracker = FinalityTracker::new(0.67);
        
        let mut block1 = create_test_block(1);
        add_signatures(&mut block1, 2);
        tracker.check_finality(&block1, 3);
        
        let mut block2 = create_test_block(2);
        add_signatures(&mut block2, 1);
        tracker.check_finality(&block2, 3);
        
        assert_eq!(tracker.finalized_count(), 1);
        assert_eq!(tracker.pending_count(), 1);
    }

    #[test]
    fn test_prune_below_height() {
        let mut tracker = FinalityTracker::new(0.67);
        
        for i in 1..5 {
            let mut block = create_test_block(i);
            add_signatures(&mut block, 2);
            tracker.check_finality(&block, 3);
        }
        
        assert_eq!(tracker.finalized_count(), 4);
        
        tracker.prune_below_height(3);
        
        assert_eq!(tracker.finalized_count(), 2);
    }

    #[test]
    fn test_clear() {
        let mut tracker = FinalityTracker::new(0.67);
        
        let mut block = create_test_block(1);
        add_signatures(&mut block, 2);
        tracker.check_finality(&block, 3);
        
        assert_eq!(tracker.finalized_count(), 1);
        
        tracker.clear();
        
        assert_eq!(tracker.finalized_count(), 0);
        assert_eq!(tracker.highest_finalized_height(), 0);
    }

    #[test]
    fn test_finality_stats() {
        let mut tracker = FinalityTracker::new(0.67);
        
        let mut block1 = create_test_block(1);
        add_signatures(&mut block1, 2);
        tracker.check_finality(&block1, 3);
        
        let mut block2 = create_test_block(2);
        add_signatures(&mut block2, 1);
        tracker.check_finality(&block2, 3);
        
        let stats = tracker.get_stats();
        assert_eq!(stats.total_blocks, 2);
        assert_eq!(stats.finalized_blocks, 1);
        assert_eq!(stats.pending_blocks, 1);
        assert_eq!(stats.highest_finalized_height, 1);
    }

    #[test]
    fn test_finality_status_methods() {
        let finalized = FinalityStatus::Finalized;
        assert!(finalized.is_finalized());
        assert!(finalized.signature_count().is_none());
        
        let pending = FinalityStatus::Pending {
            signatures: 5,
            required: 7,
        };
        assert!(!pending.is_finalized());
        assert_eq!(pending.signature_count(), Some(5));
        assert_eq!(pending.required_count(), Some(7));
    }
}

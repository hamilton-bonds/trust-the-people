/// Storage layer for blockchain voting system with hierarchical jurisdiction support
///
/// This module provides persistent storage for:
/// - Blockchain data (blocks, transactions, chain state)
/// - Voting data (ballots, elections, results)
/// - Jurisdiction hierarchy (federal → state → county → city → precinct)
/// - Validator state and configuration
///
/// The storage system is designed to handle the complexity of the US voting system,
/// supporting queries at any level of the jurisdiction hierarchy from precinct-level
/// validation up to federal aggregate verification.

pub mod database;
pub mod blockchain_store;
pub mod state_store;
pub mod cache;
pub mod snapshot;
pub mod jurisdiction;

// Re-export commonly used types
pub use database::{Database, DatabaseConfig, DatabaseError};
pub use blockchain_store::{BlockchainStore, ChainMetadata};
pub use state_store::{StateStore, ElectionState, VoterState};
pub use cache::{Cache, CacheConfig, CacheStats};
pub use snapshot::{Snapshot, SnapshotConfig, SnapshotManager};
pub use jurisdiction::{
    Jurisdiction, JurisdictionLevel, JurisdictionTree, JurisdictionPath, 
    VotingLocation, LocationMetadata, GeoCoordinates
};

use common::{BlockHash, BlockHeight, ElectionId, Result, Timestamp, TxId, VotingError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Base data directory
    pub data_dir: PathBuf,

    /// Blockchain database path
    pub blockchain_path: PathBuf,

    /// State database path
    pub state_path: PathBuf,

    /// Cache configuration
    pub cache_config: CacheConfig,

    /// Enable compression
    pub enable_compression: bool,

    /// Enable pruning
    pub enable_pruning: bool,

    /// Pruning retention (days)
    pub pruning_retention_days: u64,

    /// Maximum database size (bytes)
    pub max_db_size: Option<usize>,

    /// Sync mode (immediate, normal, off)
    pub sync_mode: SyncMode,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
            blockchain_path: PathBuf::from("./data/blockchain"),
            state_path: PathBuf::from("./data/state"),
            cache_config: CacheConfig::default(),
            enable_compression: true,
            enable_pruning: false,
            pruning_retention_days: 365,
            max_db_size: None,
            sync_mode: SyncMode::Normal,
        }
    }
}

/// Database synchronization mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SyncMode {
    /// Sync immediately on every write (slowest, safest)
    Immediate,
    /// Sync periodically (balanced)
    Normal,
    /// No sync (fastest, data loss risk)
    Off,
}

/// Storage manager that coordinates all storage components
pub struct StorageManager {
    config: StorageConfig,
    blockchain_store: BlockchainStore,
    state_store: StateStore,
    cache: Cache,
    snapshot_manager: SnapshotManager,
    jurisdiction_tree: JurisdictionTree,
}

impl StorageManager {
    /// Create a new storage manager
    pub fn new(config: StorageConfig) -> Result<Self> {
        common::utils::ensure_dir_exists(&config.data_dir)?;
        common::utils::ensure_dir_exists(&config.blockchain_path)?;
        common::utils::ensure_dir_exists(&config.state_path)?;

        let blockchain_store = BlockchainStore::new(config.blockchain_path.clone())?;
        let state_store = StateStore::new(config.state_path.clone())?;
        let cache = Cache::new(config.cache_config.clone());
        let snapshot_manager = SnapshotManager::new(config.data_dir.join("snapshots"))?;
        let jurisdiction_tree = JurisdictionTree::new();

        Ok(Self {
            config,
            blockchain_store,
            state_store,
            cache,
            snapshot_manager,
            jurisdiction_tree,
        })
    }

    /// Get blockchain store
    pub fn blockchain(&self) -> &BlockchainStore {
        &self.blockchain_store
    }

    /// Get mutable blockchain store
    pub fn blockchain_mut(&mut self) -> &mut BlockchainStore {
        &mut self.blockchain_store
    }

    /// Get state store
    pub fn state(&self) -> &StateStore {
        &self.state_store
    }

    /// Get mutable state store
    pub fn state_mut(&mut self) -> &mut StateStore {
        &mut self.state_store
    }

    /// Get cache
    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    /// Get mutable cache
    pub fn cache_mut(&mut self) -> &mut Cache {
        &mut self.cache
    }

    /// Get jurisdiction tree
    pub fn jurisdictions(&self) -> &JurisdictionTree {
        &self.jurisdiction_tree
    }

    /// Get mutable jurisdiction tree
    pub fn jurisdictions_mut(&mut self) -> &mut JurisdictionTree {
        &mut self.jurisdiction_tree
    }

    /// Query elections by jurisdiction path
    ///
    /// Examples:
    /// - "US" - All federal elections
    /// - "US/California" - All California elections
    /// - "US/California/Los Angeles" - LA County elections
    /// - "US/California/Los Angeles/Los Angeles/001" - Specific precinct
    pub fn query_elections_by_jurisdiction(
        &self,
        jurisdiction_path: &str,
    ) -> Result<Vec<ElectionId>> {
        let path = JurisdictionPath::parse(jurisdiction_path)?;
        self.state_store.query_elections_by_path(&path)
    }

    /// Aggregate results for a jurisdiction (including all sub-jurisdictions)
    pub fn aggregate_results(
        &self,
        jurisdiction_path: &str,
        election_id: ElectionId,
    ) -> Result<AggregatedResults> {
        let path = JurisdictionPath::parse(jurisdiction_path)?;
        self.state_store.aggregate_results(&path, election_id)
    }

    /// Validate blockchain data for a specific jurisdiction
    pub fn validate_jurisdiction(
        &self,
        jurisdiction_path: &str,
    ) -> Result<JurisdictionValidation> {
        let path = JurisdictionPath::parse(jurisdiction_path)?;
        
        let elections = self.query_elections_by_jurisdiction(jurisdiction_path)?;
        let mut total_votes = 0u64;
        let mut validated_elections = 0usize;

        for election_id in &elections {
            if let Ok(state) = self.state_store.get_election_state(election_id) {
                total_votes += state.total_votes;
                validated_elections += 1;
            }
        }

        Ok(JurisdictionValidation {
            jurisdiction: path,
            elections_count: elections.len(),
            validated_elections,
            total_votes,
            validation_timestamp: common::utils::current_timestamp(),
        })
    }

    /// Create a snapshot of current state
    pub fn create_snapshot(&mut self, name: &str) -> Result<()> {
        self.snapshot_manager.create_snapshot(
            name,
            &self.blockchain_store,
            &self.state_store,
        )
    }

    /// Restore from snapshot
    pub fn restore_snapshot(&mut self, name: &str) -> Result<()> {
        self.snapshot_manager.restore_snapshot(
            name,
            &mut self.blockchain_store,
            &mut self.state_store,
        )
    }

    /// Get storage statistics
    pub fn stats(&self) -> StorageStats {
        StorageStats {
            blockchain_size: self.blockchain_store.size(),
            state_size: self.state_store.size(),
            cache_stats: self.cache.stats(),
            total_blocks: self.blockchain_store.block_count(),
            total_transactions: self.blockchain_store.transaction_count(),
            total_elections: self.state_store.election_count(),
            total_voters: self.state_store.voter_count(),
        }
    }

    /// Compact and optimize storage
    pub fn compact(&mut self) -> Result<()> {
        self.blockchain_store.compact()?;
        self.state_store.compact()?;
        Ok(())
    }

    /// Prune old data based on configuration
    pub fn prune(&mut self) -> Result<PruneStats> {
        if !self.config.enable_pruning {
            return Err(VotingError::InvalidInput(
                "Pruning is not enabled".to_string(),
            ));
        }

        let retention_seconds = self.config.pruning_retention_days * 24 * 3600;
        let cutoff = common::utils::current_timestamp() - retention_seconds;

        let pruned_blocks = self.blockchain_store.prune_before(cutoff)?;
        let pruned_state = self.state_store.prune_before(cutoff)?;

        Ok(PruneStats {
            pruned_blocks,
            pruned_state,
            cutoff_timestamp: cutoff,
        })
    }

    /// Flush all pending writes
    pub fn flush(&mut self) -> Result<()> {
        self.blockchain_store.flush()?;
        self.state_store.flush()?;
        Ok(())
    }

    /// Close storage gracefully
    pub fn close(mut self) -> Result<()> {
        self.flush()?;
        Ok(())
    }
}

/// Aggregated results for a jurisdiction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedResults {
    pub jurisdiction: JurisdictionPath,
    pub election_id: ElectionId,
    pub total_votes: u64,
    pub candidate_tallies: Vec<(String, u64)>,
    pub sub_jurisdictions: Vec<(JurisdictionPath, u64)>,
    pub aggregated_at: Timestamp,
}

/// Jurisdiction validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JurisdictionValidation {
    pub jurisdiction: JurisdictionPath,
    pub elections_count: usize,
    pub validated_elections: usize,
    pub total_votes: u64,
    pub validation_timestamp: Timestamp,
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub blockchain_size: u64,
    pub state_size: u64,
    pub cache_stats: CacheStats,
    pub total_blocks: u64,
    pub total_transactions: u64,
    pub total_elections: u64,
    pub total_voters: u64,
}

/// Pruning statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruneStats {
    pub pruned_blocks: usize,
    pub pruned_state: usize,
    pub cutoff_timestamp: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_config_default() {
        let config = StorageConfig::default();
        assert!(config.enable_compression);
        assert!(!config.enable_pruning);
        assert_eq!(config.sync_mode, SyncMode::Normal);
    }

    #[test]
    fn test_sync_mode() {
        assert_eq!(SyncMode::Normal, SyncMode::Normal);
        assert_ne!(SyncMode::Immediate, SyncMode::Off);
    }
}

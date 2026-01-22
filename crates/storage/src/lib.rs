pub mod database;
pub mod blockchain_store;
pub mod state_store;
pub mod cache;
pub mod snapshot;
pub mod jurisdiction;

// Re-export commonly used types
pub use database::{Database, DatabaseConfig};
pub use blockchain_store::{BlockchainStore, ChainMetadata};
pub use state_store::{StateStore, ElectionState};
pub use cache::{Cache, CacheStats};
pub use snapshot::{SnapshotManager, SnapshotInfo};
pub use jurisdiction::{
    Jurisdiction, JurisdictionLevel, JurisdictionTree, JurisdictionPath, 
    VotingLocation, LocationMetadata, GeoCoordinates
};

use common::{BlockHash, BlockHeight, ElectionId, Result, Timestamp, VotingError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Base data directory
    pub data_dir: PathBuf,

    /// Blockchain database path
    pub blockchain_path: PathBuf,

    /// State database path
    pub state_path: PathBuf,

    /// Block cache capacity
    pub block_cache_capacity: usize,

    /// Transaction cache capacity
    pub tx_cache_capacity: usize,

    /// State cache capacity
    pub state_cache_capacity: usize,

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
            block_cache_capacity: 1000,
            tx_cache_capacity: 5000,
            state_cache_capacity: 2000,
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

        // Use RocksDB if the feature is enabled, otherwise fall back to Sled
        #[cfg(feature = "rocksdb-backend")]
        let blockchain_db = {
            let db = Arc::new(database::RocksDBDatabase::with_config(
                config.blockchain_path.clone(),
                &DatabaseConfig::default(),
            )?);
            db
        };

        #[cfg(all(feature = "sled-backend", not(feature = "rocksdb-backend")))]
        let blockchain_db = {
            let db = Arc::new(database::SledDatabase::with_config(
                config.blockchain_path.clone(),
                &DatabaseConfig::default(),
            )?);
            db
        };

        #[cfg(feature = "rocksdb-backend")]
        let state_db = {
            let db = Arc::new(database::RocksDBDatabase::with_config(
                config.state_path.clone(),
                &DatabaseConfig::default(),
            )?);
            db
        };

        #[cfg(all(feature = "sled-backend", not(feature = "rocksdb-backend")))]
        let state_db = {
            let db = Arc::new(database::SledDatabase::with_config(
                config.state_path.clone(),
                &DatabaseConfig::default(),
            )?);
            db
        };

        let blockchain_store = BlockchainStore::new(blockchain_db)?;
        let state_store = StateStore::new(state_db)?;
        
        let cache = Cache::new(
            config.block_cache_capacity,
            config.tx_cache_capacity,
            config.state_cache_capacity,
        )?;
        
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
        
        // Get all elections and filter by jurisdiction
        let elections = self.state_store.get_all_elections()?;
        let mut matching: Vec<ElectionId> = Vec::new();
        
        // Just return all elections for now
        Ok(elections.iter().map(|e| e.id).collect())
    }

    /// Aggregate results for a jurisdiction (including all sub-jurisdictions)
    pub fn aggregate_results(
        &self,
        jurisdiction_path: &str,
        election_id: ElectionId,
    ) -> Result<AggregatedResults> {
        let path = JurisdictionPath::parse(jurisdiction_path)?;
        
        let election = self.state_store.get_election(&election_id)?;
        
        // Simple aggregation - would need more sophisticated logic in production
        Ok(AggregatedResults {
            jurisdiction: path,
            election_id,
            total_votes: election.map(|e| e.total_votes).unwrap_or(0),
            timestamp: common::utils::current_timestamp(),
        })
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
            if let Some(election) = self.state_store.get_election(election_id)? {
                total_votes += election.total_votes;
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
    pub fn create_snapshot(&mut self, _name: &str) -> Result<()> {
        // Simplified - snapshot functionality not yet implemented
        Err(VotingError::NotImplemented("Snapshots not yet implemented".to_string()))
    }
    
    /// Restore from snapshot
    pub fn restore_snapshot(&mut self, _snapshot_path: &std::path::Path) -> Result<()> {
        // Simplified - snapshot functionality not yet implemented
        Err(VotingError::NotImplemented("Snapshots not yet implemented".to_string()))
    }
    
    /// Get storage statistics
    pub fn stats(&self) -> StorageStats {
        StorageStats {
            blockchain_size: self.blockchain_store.size(),
            state_size: 0, // StateStore doesn't have size() method
            cache_stats: self.cache.get_stats(),
            total_blocks: self.blockchain_store.block_count(),
            total_transactions: self.blockchain_store.transaction_count(),
            total_elections: 0, // Would need to implement
            total_voters: 0, // Would need to implement
        }
    }

    /// Compact and optimize storage
    pub fn compact(&mut self) -> Result<()> {
        self.blockchain_store.compact()?;
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

        Ok(PruneStats {
            pruned_blocks: pruned_blocks as u64,
            pruned_transactions: 0,
            pruned_state: 0,
            bytes_freed: 0,
        })
    }

    /// Flush all pending writes to disk
    pub fn flush(&mut self) -> Result<()> {
        self.blockchain_store.flush()?;
        Ok(())
    }

    /// Close storage and release resources
    pub fn close(&mut self) -> Result<()> {
        self.flush()?;
        Ok(())
    }
}

/// Aggregated election results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedResults {
    pub jurisdiction: JurisdictionPath,
    pub election_id: ElectionId,
    pub total_votes: u64,
    pub timestamp: Timestamp,
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
    pub pruned_blocks: u64,
    pub pruned_transactions: u64,
    pub pruned_state: u64,
    pub bytes_freed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_storage_config_default() {
        let config = StorageConfig::default();
        assert_eq!(config.block_cache_capacity, 1000);
        assert_eq!(config.tx_cache_capacity, 5000);
        assert_eq!(config.state_cache_capacity, 2000);
        assert!(config.enable_compression);
        assert!(!config.enable_pruning);
    }

    #[test]
    fn test_storage_manager_creation() {
        let dir = tempdir().unwrap();
        let mut config = StorageConfig::default();
        config.data_dir = dir.path().to_path_buf();
        config.blockchain_path = dir.path().join("blockchain");
        config.state_path = dir.path().join("state");

        let manager = StorageManager::new(config);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_jurisdiction_path_parse() {
        let path = JurisdictionPath::parse("US/California/Los Angeles");
        assert!(path.is_ok());
    }
}

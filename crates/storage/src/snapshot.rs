use common::{BlockHash, BlockHeight, Result, Timestamp, VotingError};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Snapshot version
    pub version: u32,
    
    /// Block height at snapshot
    pub block_height: BlockHeight,
    
    /// Block hash at snapshot
    pub block_hash: BlockHash,
    
    /// Timestamp when snapshot was created
    pub created_at: Timestamp,
    
    /// Total size in bytes
    pub size_bytes: u64,
    
    /// Number of blocks included
    pub block_count: u64,
    
    /// Number of transactions included
    pub transaction_count: u64,
    
    /// Checksum of snapshot data
    pub checksum: [u8; 32],
    
    /// Chain ID
    pub chain_id: String,
}

/// Snapshot manager for creating and restoring blockchain snapshots
pub struct SnapshotManager {
    /// Base directory for snapshots
    snapshot_dir: PathBuf,
    
    /// Current snapshot version
    version: u32,
}

impl SnapshotManager {
    /// Current snapshot format version
    const CURRENT_VERSION: u32 = 1;
    
    /// Snapshot file extension
    const SNAPSHOT_EXTENSION: &'static str = "snapshot";
    
    /// Metadata file extension
    const METADATA_EXTENSION: &'static str = "meta";
    
    /// Create a new snapshot manager
    pub fn new(snapshot_dir: PathBuf) -> Result<Self> {
        common::utils::ensure_dir_exists(&snapshot_dir)?;
        
        Ok(Self {
            snapshot_dir,
            version: Self::CURRENT_VERSION,
        })
    }
    
    /// Create a snapshot of the current blockchain state
    pub fn create_snapshot(
        &self,
        block_height: BlockHeight,
        block_hash: BlockHash,
        chain_id: String,
        data_provider: &dyn SnapshotDataProvider,
    ) -> Result<SnapshotMetadata> {
        let timestamp = common::utils::current_timestamp();
        let snapshot_name = format!("snapshot_{}_{}", block_height, timestamp);
        
        let snapshot_path = self.snapshot_dir.join(format!("{}.{}", snapshot_name, Self::SNAPSHOT_EXTENSION));
        let metadata_path = self.snapshot_dir.join(format!("{}.{}", snapshot_name, Self::METADATA_EXTENSION));
        
        let file = File::create(&snapshot_path).map_err(|e| {
            VotingError::StorageError(format!("Failed to create snapshot file: {}", e))
        })?;
        
        let mut writer = BufWriter::new(file);
        
        let (block_count, tx_count) = self.write_snapshot_data(&mut writer, data_provider)?;
        
        writer.flush()?;
        
        let size_bytes = fs::metadata(&snapshot_path)?.len();
        
        let checksum = self.calculate_checksum(&snapshot_path)?;
        
        let metadata = SnapshotMetadata {
            version: self.version,
            block_height,
            block_hash,
            created_at: timestamp,
            size_bytes,
            block_count,
            transaction_count: tx_count,
            checksum,
            chain_id,
        };
        
        self.save_metadata(&metadata_path, &metadata)?;
        
        Ok(metadata)
    }
    
    /// Write snapshot data to file
    fn write_snapshot_data(
        &self,
        writer: &mut BufWriter<File>,
        provider: &dyn SnapshotDataProvider,
    ) -> Result<(u64, u64)> {
        writer.write_all(&self.version.to_le_bytes())?;
        
        let blocks = provider.get_blocks_for_snapshot()?;
        let block_count = blocks.len() as u64;
        
        writer.write_all(&block_count.to_le_bytes())?;
        
        let mut total_tx_count = 0u64;
        
        for block_data in blocks {
            let block_len = block_data.len() as u32;
            writer.write_all(&block_len.to_le_bytes())?;
            writer.write_all(&block_data)?;
            
            let tx_count = provider.get_transaction_count_for_block(&block_data)?;
            total_tx_count += tx_count;
        }
        
        let state_data = provider.get_state_for_snapshot()?;
        let state_len = state_data.len() as u32;
        writer.write_all(&state_len.to_le_bytes())?;
        writer.write_all(&state_data)?;
        
        Ok((block_count, total_tx_count))
    }
    
    /// Calculate checksum of snapshot file
    fn calculate_checksum(&self, path: &Path) -> Result<[u8; 32]> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        
        Ok(common::utils::hash_data(&buffer))
    }
    
    /// Save snapshot metadata to file
    fn save_metadata(&self, path: &Path, metadata: &SnapshotMetadata) -> Result<()> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, metadata)?;
        Ok(())
    }
    
    /// Load snapshot metadata from file
    pub fn load_metadata(&self, path: &Path) -> Result<SnapshotMetadata> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let metadata: SnapshotMetadata = serde_json::from_reader(reader)?;
        Ok(metadata)
    }
    
    /// Restore blockchain state from a snapshot
    pub fn restore_snapshot(
        &self,
        snapshot_path: &Path,
        data_restorer: &mut dyn SnapshotDataRestorer,
    ) -> Result<SnapshotMetadata> {
        let metadata_path = snapshot_path.with_extension(Self::METADATA_EXTENSION);
        let metadata = self.load_metadata(&metadata_path)?;
        
        let checksum = self.calculate_checksum(snapshot_path)?;
        if checksum != metadata.checksum {
            return Err(VotingError::StorageError(
                "Snapshot checksum verification failed".to_string(),
            ));
        }
        
        let file = File::open(snapshot_path)?;
        let mut reader = BufReader::new(file);
        
        self.read_snapshot_data(&mut reader, data_restorer)?;
        
        Ok(metadata)
    }
    
    /// Read snapshot data from file
    fn read_snapshot_data(
        &self,
        reader: &mut BufReader<File>,
        restorer: &mut dyn SnapshotDataRestorer,
    ) -> Result<()> {
        let mut version_bytes = [0u8; 4];
        reader.read_exact(&mut version_bytes)?;
        let version = u32::from_le_bytes(version_bytes);
        
        if version != self.version {
            return Err(VotingError::StorageError(format!(
                "Snapshot version mismatch: expected {}, got {}",
                self.version, version
            )));
        }
        
        let mut block_count_bytes = [0u8; 8];
        reader.read_exact(&mut block_count_bytes)?;
        let block_count = u64::from_le_bytes(block_count_bytes);
        
        for _ in 0..block_count {
            let mut block_len_bytes = [0u8; 4];
            reader.read_exact(&mut block_len_bytes)?;
            let block_len = u32::from_le_bytes(block_len_bytes);
            
            let mut block_data = vec![0u8; block_len as usize];
            reader.read_exact(&mut block_data)?;
            
            restorer.restore_block(block_data)?;
        }
        
        let mut state_len_bytes = [0u8; 4];
        reader.read_exact(&mut state_len_bytes)?;
        let state_len = u32::from_le_bytes(state_len_bytes);
        
        let mut state_data = vec![0u8; state_len as usize];
        reader.read_exact(&mut state_data)?;
        
        restorer.restore_state(state_data)?;
        
        Ok(())
    }
    
    /// List all available snapshots
    pub fn list_snapshots(&self) -> Result<Vec<SnapshotInfo>> {
        let mut snapshots = Vec::new();
        
        let entries = fs::read_dir(&self.snapshot_dir)?;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some(Self::METADATA_EXTENSION) {
                if let Ok(metadata) = self.load_metadata(&path) {
                    let snapshot_path = path.with_extension(Self::SNAPSHOT_EXTENSION);
                    
                    if snapshot_path.exists() {
                        snapshots.push(SnapshotInfo {
                            metadata,
                            snapshot_path,
                            metadata_path: path,
                        });
                    }
                }
            }
        }
        
        snapshots.sort_by(|a, b| b.metadata.block_height.cmp(&a.metadata.block_height));
        
        Ok(snapshots)
    }
    
    /// Get the latest snapshot
    pub fn get_latest_snapshot(&self) -> Result<Option<SnapshotInfo>> {
        let snapshots = self.list_snapshots()?;
        Ok(snapshots.into_iter().next())
    }
    
    /// Delete a snapshot
    pub fn delete_snapshot(&self, info: &SnapshotInfo) -> Result<()> {
        fs::remove_file(&info.snapshot_path)?;
        fs::remove_file(&info.metadata_path)?;
        Ok(())
    }
    
    /// Delete old snapshots, keeping only the most recent N
    pub fn prune_old_snapshots(&self, keep_count: usize) -> Result<usize> {
        let snapshots = self.list_snapshots()?;
        
        if snapshots.len() <= keep_count {
            return Ok(0);
        }
        
        let to_delete = &snapshots[keep_count..];
        let delete_count = to_delete.len();
        
        for snapshot in to_delete {
            self.delete_snapshot(snapshot)?;
        }
        
        Ok(delete_count)
    }
    
    /// Delete snapshots older than specified timestamp
    pub fn prune_snapshots_before(&self, before_timestamp: Timestamp) -> Result<usize> {
        let snapshots = self.list_snapshots()?;
        let mut deleted = 0;
        
        for snapshot in snapshots {
            if snapshot.metadata.created_at < before_timestamp {
                self.delete_snapshot(&snapshot)?;
                deleted += 1;
            }
        }
        
        Ok(deleted)
    }
    
    /// Verify snapshot integrity
    pub fn verify_snapshot(&self, info: &SnapshotInfo) -> Result<bool> {
        let checksum = self.calculate_checksum(&info.snapshot_path)?;
        Ok(checksum == info.metadata.checksum)
    }
    
    /// Get snapshot directory path
    pub fn snapshot_dir(&self) -> &Path {
        &self.snapshot_dir
    }
}

/// Information about a snapshot
#[derive(Debug, Clone)]
pub struct SnapshotInfo {
    pub metadata: SnapshotMetadata,
    pub snapshot_path: PathBuf,
    pub metadata_path: PathBuf,
}

impl SnapshotInfo {
    /// Get snapshot size in MB
    pub fn size_mb(&self) -> f64 {
        self.metadata.size_bytes as f64 / (1024.0 * 1024.0)
    }
    
    /// Get formatted creation time
    pub fn created_at_formatted(&self) -> String {
        common::utils::format_timestamp(self.metadata.created_at)
    }
}

/// Trait for providing data to be included in a snapshot
pub trait SnapshotDataProvider {
    /// Get all blocks to include in snapshot
    fn get_blocks_for_snapshot(&self) -> Result<Vec<Vec<u8>>>;
    
    /// Get transaction count for a block
    fn get_transaction_count_for_block(&self, block_data: &[u8]) -> Result<u64>;
    
    /// Get current state data
    fn get_state_for_snapshot(&self) -> Result<Vec<u8>>;
}

/// Trait for restoring data from a snapshot
pub trait SnapshotDataRestorer {
    /// Restore a block from snapshot data
    fn restore_block(&mut self, block_data: Vec<u8>) -> Result<()>;
    
    /// Restore state from snapshot data
    fn restore_state(&mut self, state_data: Vec<u8>) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    
    struct TestDataProvider {
        blocks: Vec<Vec<u8>>,
        state: Vec<u8>,
    }
    
    impl SnapshotDataProvider for TestDataProvider {
        fn get_blocks_for_snapshot(&self) -> Result<Vec<Vec<u8>>> {
            Ok(self.blocks.clone())
        }
        
        fn get_transaction_count_for_block(&self, _block_data: &[u8]) -> Result<u64> {
            Ok(5)
        }
        
        fn get_state_for_snapshot(&self) -> Result<Vec<u8>> {
            Ok(self.state.clone())
        }
    }
    
    struct TestDataRestorer {
        blocks: Vec<Vec<u8>>,
        state: Option<Vec<u8>>,
    }
    
    impl TestDataRestorer {
        fn new() -> Self {
            Self {
                blocks: Vec::new(),
                state: None,
            }
        }
    }
    
    impl SnapshotDataRestorer for TestDataRestorer {
        fn restore_block(&mut self, block_data: Vec<u8>) -> Result<()> {
            self.blocks.push(block_data);
            Ok(())
        }
        
        fn restore_state(&mut self, state_data: Vec<u8>) -> Result<()> {
            self.state = Some(state_data);
            Ok(())
        }
    }
    
    fn create_test_manager() -> SnapshotManager {
        let temp_dir = std::env::temp_dir().join(format!("snapshot_test_{}", rand::random::<u64>()));
        SnapshotManager::new(temp_dir).unwrap()
    }

    #[test]
    fn test_snapshot_manager_creation() {
        let manager = create_test_manager();
        assert!(manager.snapshot_dir().exists());
    }

    #[test]
    fn test_create_snapshot() {
        let manager = create_test_manager();
        
        let provider = TestDataProvider {
            blocks: vec![vec![1, 2, 3], vec![4, 5, 6]],
            state: vec![7, 8, 9],
        };
        
        let metadata = manager
            .create_snapshot(
                100,
                BlockHash::new([1u8; 32]),
                "test-chain".to_string(),
                &provider,
            )
            .unwrap();
        
        assert_eq!(metadata.block_height, 100);
        assert_eq!(metadata.block_count, 2);
        assert_eq!(metadata.chain_id, "test-chain");
    }

    #[test]
    fn test_restore_snapshot() {
        let manager = create_test_manager();
        
        let provider = TestDataProvider {
            blocks: vec![vec![1, 2, 3], vec![4, 5, 6]],
            state: vec![7, 8, 9],
        };
        
        let metadata = manager
            .create_snapshot(
                100,
                BlockHash::new([1u8; 32]),
                "test-chain".to_string(),
                &provider,
            )
            .unwrap();
        
        let snapshots = manager.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 1);
        
        let mut restorer = TestDataRestorer::new();
        let restored_metadata = manager
            .restore_snapshot(&snapshots[0].snapshot_path, &mut restorer)
            .unwrap();
        
        assert_eq!(restored_metadata.block_height, metadata.block_height);
        assert_eq!(restorer.blocks.len(), 2);
        assert_eq!(restorer.blocks[0], vec![1, 2, 3]);
        assert_eq!(restorer.state, Some(vec![7, 8, 9]));
    }

    #[test]
    fn test_list_snapshots() {
        let manager = create_test_manager();
        
        let provider = TestDataProvider {
            blocks: vec![vec![1, 2, 3]],
            state: vec![4, 5],
        };
        
        manager
            .create_snapshot(100, BlockHash::new([1u8; 32]), "chain1".to_string(), &provider)
            .unwrap();
        
        manager
            .create_snapshot(200, BlockHash::new([2u8; 32]), "chain1".to_string(), &provider)
            .unwrap();
        
        let snapshots = manager.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].metadata.block_height, 200);
        assert_eq!(snapshots[1].metadata.block_height, 100);
    }

    #[test]
    fn test_get_latest_snapshot() {
        let manager = create_test_manager();
        
        let provider = TestDataProvider {
            blocks: vec![vec![1]],
            state: vec![2],
        };
        
        manager
            .create_snapshot(100, BlockHash::new([1u8; 32]), "chain".to_string(), &provider)
            .unwrap();
        
        manager
            .create_snapshot(200, BlockHash::new([2u8; 32]), "chain".to_string(), &provider)
            .unwrap();
        
        let latest = manager.get_latest_snapshot().unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().metadata.block_height, 200);
    }

    #[test]
    fn test_delete_snapshot() {
        let manager = create_test_manager();
        
        let provider = TestDataProvider {
            blocks: vec![vec![1]],
            state: vec![2],
        };
        
        manager
            .create_snapshot(100, BlockHash::new([1u8; 32]), "chain".to_string(), &provider)
            .unwrap();
        
        let snapshots = manager.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 1);
        
        manager.delete_snapshot(&snapshots[0]).unwrap();
        
        let snapshots = manager.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 0);
    }

    #[test]
    fn test_prune_old_snapshots() {
        let manager = create_test_manager();
        
        let provider = TestDataProvider {
            blocks: vec![vec![1]],
            state: vec![2],
        };
        
        for i in 0..5 {
            manager
                .create_snapshot(i * 100, BlockHash::new([i as u8; 32]), "chain".to_string(), &provider)
                .unwrap();
        }
        
        let deleted = manager.prune_old_snapshots(2).unwrap();
        assert_eq!(deleted, 3);
        
        let snapshots = manager.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 2);
    }

    #[test]
    fn test_verify_snapshot() {
        let manager = create_test_manager();
        
        let provider = TestDataProvider {
            blocks: vec![vec![1, 2, 3]],
            state: vec![4, 5],
        };
        
        manager
            .create_snapshot(100, BlockHash::new([1u8; 32]), "chain".to_string(), &provider)
            .unwrap();
        
        let snapshots = manager.list_snapshots().unwrap();
        assert!(manager.verify_snapshot(&snapshots[0]).unwrap());
    }

    #[test]
    fn test_snapshot_checksum_validation() {
        let manager = create_test_manager();
        
        let provider = TestDataProvider {
            blocks: vec![vec![1, 2, 3]],
            state: vec![4, 5],
        };
        
        manager
            .create_snapshot(100, BlockHash::new([1u8; 32]), "chain".to_string(), &provider)
            .unwrap();
        
        let snapshots = manager.list_snapshots().unwrap();
        
        let mut file = File::options()
            .write(true)
            .open(&snapshots[0].snapshot_path)
            .unwrap();
        file.write_all(&[99, 99, 99]).unwrap();
        
        let mut restorer = TestDataRestorer::new();
        let result = manager.restore_snapshot(&snapshots[0].snapshot_path, &mut restorer);
        
        assert!(result.is_err());
    }

    #[test]
    fn test_snapshot_info_size_mb() {
        let manager = create_test_manager();
        
        let provider = TestDataProvider {
            blocks: vec![vec![1; 1024 * 1024]],
            state: vec![2; 1024 * 1024],
        };
        
        manager
            .create_snapshot(100, BlockHash::new([1u8; 32]), "chain".to_string(), &provider)
            .unwrap();
        
        let snapshots = manager.list_snapshots().unwrap();
        let size_mb = snapshots[0].size_mb();
        
        assert!(size_mb > 2.0);
    }
}

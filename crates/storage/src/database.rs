use common::{Result, VotingError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Database abstraction layer supporting multiple backends
///
/// This provides a common interface for key-value storage with support for:
/// - RocksDB (production, high-performance)
/// - Sled (Rust-native, embedded)
/// - In-memory (testing, development)
///
/// IMPORTANT: All methods take &self (not &mut self) to allow usage through Arc
/// Interior mutability is used where needed
pub trait Database: Send + Sync {
    /// Get a value by key
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Put a key-value pair
    fn put(&self, key: &[u8], value: &[u8]) -> Result<()>;

    /// Delete a key
    fn delete(&self, key: &[u8]) -> Result<()>;

    /// Check if key exists
    fn contains(&self, key: &[u8]) -> Result<bool>;

    /// Iterate over all keys with a prefix
    fn iter_prefix(&self, prefix: &[u8]) -> Result<Box<dyn Iterator<Item = (Vec<u8>, Vec<u8>)>>>;

    /// Get all keys with a prefix
    fn keys_with_prefix(&self, prefix: &[u8]) -> Result<Vec<Vec<u8>>>;

    /// Batch write operations
    fn write_batch(&self, operations: Vec<WriteOperation>) -> Result<()>;

    /// Flush pending writes to disk
    fn flush(&self) -> Result<()>;

    /// Compact the database
    fn compact(&self) -> Result<()>;

    /// Get database size in bytes
    fn size(&self) -> u64;

    /// Get statistics
    fn stats(&self) -> DatabaseStats;
}

/// Write operation for batch writes
#[derive(Debug, Clone)]
pub enum WriteOperation {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database path
    pub path: PathBuf,

    /// Database backend
    pub backend: DatabaseBackend,

    /// Maximum open files
    pub max_open_files: Option<usize>,

    /// Cache size in bytes
    pub cache_size: Option<usize>,

    /// Enable compression
    pub compression: bool,

    /// Write buffer size
    pub write_buffer_size: Option<usize>,

    /// Bloom filter bits per key
    pub bloom_filter_bits: Option<u32>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("./data/db"),
            #[cfg(feature = "rocksdb-backend")]
            backend: DatabaseBackend::RocksDB,
            #[cfg(all(feature = "sled-backend", not(feature = "rocksdb-backend")))]
            backend: DatabaseBackend::Sled,
            #[cfg(not(any(feature = "rocksdb-backend", feature = "sled-backend")))]
            backend: DatabaseBackend::Memory,
            max_open_files: Some(1000),
            cache_size: Some(128 * 1024 * 1024), // 128 MB
            compression: true,
            write_buffer_size: Some(64 * 1024 * 1024), // 64 MB
            bloom_filter_bits: Some(10),
        }
    }
}

/// Database backend type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DatabaseBackend {
    /// RocksDB (high-performance, production)
    RocksDB,
    /// Sled (Rust-native, embedded)
    Sled,
    /// In-memory (testing only)
    Memory,
}

/// Database statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStats {
    pub total_keys: u64,
    pub total_size: u64,
    pub read_count: u64,
    pub write_count: u64,
    pub delete_count: u64,
}

impl Default for DatabaseStats {
    fn default() -> Self {
        Self {
            total_keys: 0,
            total_size: 0,
            read_count: 0,
            write_count: 0,
            delete_count: 0,
        }
    }
}

// ============================================================================
// RocksDB Implementation
// ============================================================================

#[cfg(feature = "rocksdb-backend")]
pub struct RocksDBDatabase {
    db: rocksdb::DB,
    _path: PathBuf,
    stats: Arc<RwLock<DatabaseStats>>,
}

#[cfg(feature = "rocksdb-backend")]
impl RocksDBDatabase {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        
        let db = rocksdb::DB::open(&opts, &path)
            .map_err(|e| VotingError::DatabaseError(format!("Failed to open RocksDB: {}", e)))?;

        Ok(Self {
            db,
            _path: path,
            stats: Arc::new(RwLock::new(DatabaseStats::default())),
        })
    }

    pub fn with_config<P: AsRef<Path>>(path: P, config: &DatabaseConfig) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        
        if let Some(cache_size) = config.cache_size {
            let cache = rocksdb::Cache::new_lru_cache(cache_size);
            let mut block_opts = rocksdb::BlockBasedOptions::default();
            block_opts.set_block_cache(&cache);
            opts.set_block_based_table_factory(&block_opts);
        }

        if config.compression {
            opts.set_compression_type(rocksdb::DBCompressionType::Zstd);
        }

        if let Some(write_buffer_size) = config.write_buffer_size {
            opts.set_write_buffer_size(write_buffer_size);
        }

        if let Some(max_open_files) = config.max_open_files {
            opts.set_max_open_files(max_open_files as i32);
        }

        let db = rocksdb::DB::open(&opts, &path)
            .map_err(|e| VotingError::DatabaseError(format!("Failed to open RocksDB: {}", e)))?;

        Ok(Self {
            db,
            _path: path,
            stats: Arc::new(RwLock::new(DatabaseStats::default())),
        })
    }
}

#[cfg(feature = "rocksdb-backend")]
impl Database for RocksDBDatabase {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        if let Ok(mut stats) = self.stats.write() {
            stats.read_count += 1;
        }

        self.db
            .get(key)
            .map_err(|e| VotingError::DatabaseError(format!("Get failed: {}", e)))
    }

    fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        if let Ok(mut stats) = self.stats.write() {
            stats.write_count += 1;
        }

        self.db
            .put(key, value)
            .map_err(|e| VotingError::DatabaseError(format!("Put failed: {}", e)))
    }

    fn delete(&self, key: &[u8]) -> Result<()> {
        if let Ok(mut stats) = self.stats.write() {
            stats.delete_count += 1;
        }

        self.db
            .delete(key)
            .map_err(|e| VotingError::DatabaseError(format!("Delete failed: {}", e)))
    }

    fn contains(&self, key: &[u8]) -> Result<bool> {
        Ok(self.db.get(key)
            .map_err(|e| VotingError::DatabaseError(format!("Contains failed: {}", e)))?
            .is_some())
    }

    fn iter_prefix(&self, prefix: &[u8]) -> Result<Box<dyn Iterator<Item = (Vec<u8>, Vec<u8>)>>> {
        let mut iter = self.db.raw_iterator();
        iter.seek(prefix);
        
        let mut results = Vec::new();
        while iter.valid() {
            if let Some(key) = iter.key() {
                if !key.starts_with(prefix) {
                    break;
                }
                if let Some(value) = iter.value() {
                    results.push((key.to_vec(), value.to_vec()));
                }
            }
            iter.next();
        }
        
        Ok(Box::new(results.into_iter()))
    }

    fn keys_with_prefix(&self, prefix: &[u8]) -> Result<Vec<Vec<u8>>> {
        let mut iter = self.db.raw_iterator();
        iter.seek(prefix);
        
        let mut keys = Vec::new();
        while iter.valid() {
            if let Some(key) = iter.key() {
                if !key.starts_with(prefix) {
                    break;
                }
                keys.push(key.to_vec());
            }
            iter.next();
        }
        
        Ok(keys)
    }

    fn write_batch(&self, operations: Vec<WriteOperation>) -> Result<()> {
        let mut batch = rocksdb::WriteBatch::default();

        for op in operations {
            match op {
                WriteOperation::Put { key, value } => {
                    batch.put(key, value);
                }
                WriteOperation::Delete { key } => {
                    batch.delete(key);
                }
            }
        }

        self.db
            .write(batch)
            .map_err(|e| VotingError::DatabaseError(format!("Batch write failed: {}", e)))
    }

    fn flush(&self) -> Result<()> {
        self.db
            .flush()
            .map_err(|e| VotingError::DatabaseError(format!("Flush failed: {}", e)))
    }

    fn compact(&self) -> Result<()> {
        self.db.compact_range::<&[u8], &[u8]>(None, None);
        Ok(())
    }

    fn size(&self) -> u64 {
        // Estimate database size
        if let Ok(Some(size_str)) = self.db.property_value("rocksdb.total-sst-files-size") {
            size_str.parse().unwrap_or(0)
        } else {
            0
        }
    }

    fn stats(&self) -> DatabaseStats {
        let mut stats = self.stats.read().unwrap().clone();
        stats.total_size = self.size();
        // RocksDB doesn't provide easy key count, would need to iterate
        stats
    }
}

// ============================================================================
// Sled Implementation
// ============================================================================

#[cfg(feature = "sled-backend")]
pub struct SledDatabase {
    db: sled::Db,
    _path: PathBuf,
    stats: Arc<RwLock<DatabaseStats>>,
}

#[cfg(feature = "sled-backend")]
impl SledDatabase {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let db = sled::open(&path)
            .map_err(|e| VotingError::DatabaseError(format!("Failed to open Sled: {}", e)))?;

        Ok(Self {
            db,
            _path: path,
            stats: Arc::new(RwLock::new(DatabaseStats::default())),
        })
    }

    pub fn with_config<P: AsRef<Path>>(path: P, config: &DatabaseConfig) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        
        let mut sled_config = sled::Config::new().path(&path);

        if let Some(cache_size) = config.cache_size {
            sled_config = sled_config.cache_capacity(cache_size as u64);
        }

        // Don't enable compression to avoid the zstd conflict
        // Sled compression requires the same zstd version as rocksdb
        // which causes conflicts

        let db = sled_config
            .open()
            .map_err(|e| VotingError::DatabaseError(format!("Failed to open Sled: {}", e)))?;

        Ok(Self {
            db,
            _path: path,
            stats: Arc::new(RwLock::new(DatabaseStats::default())),
        })
    }
}

#[cfg(feature = "sled-backend")]
impl Database for SledDatabase {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        if let Ok(mut stats) = self.stats.write() {
            stats.read_count += 1;
        }

        self.db
            .get(key)
            .map_err(|e| VotingError::DatabaseError(format!("Get failed: {}", e)))
            .map(|opt| opt.map(|iv| iv.to_vec()))
    }

    fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        if let Ok(mut stats) = self.stats.write() {
            stats.write_count += 1;
        }

        self.db
            .insert(key, value)
            .map_err(|e| VotingError::DatabaseError(format!("Put failed: {}", e)))?;

        Ok(())
    }

    fn delete(&self, key: &[u8]) -> Result<()> {
        if let Ok(mut stats) = self.stats.write() {
            stats.delete_count += 1;
        }

        self.db
            .remove(key)
            .map_err(|e| VotingError::DatabaseError(format!("Delete failed: {}", e)))?;

        Ok(())
    }

    fn contains(&self, key: &[u8]) -> Result<bool> {
        self.db
            .contains_key(key)
            .map_err(|e| VotingError::DatabaseError(format!("Contains failed: {}", e)))
    }

    fn iter_prefix(&self, prefix: &[u8]) -> Result<Box<dyn Iterator<Item = (Vec<u8>, Vec<u8>)>>> {
        let iter = self.db.scan_prefix(prefix).filter_map(|result| {
            result.ok().map(|(k, v)| (k.to_vec(), v.to_vec()))
        });

        Ok(Box::new(iter.collect::<Vec<_>>().into_iter()))
    }

    fn keys_with_prefix(&self, prefix: &[u8]) -> Result<Vec<Vec<u8>>> {
        let keys: Vec<Vec<u8>> = self
            .db
            .scan_prefix(prefix)
            .filter_map(|result| result.ok().map(|(k, _)| k.to_vec()))
            .collect();

        Ok(keys)
    }

    fn write_batch(&self, operations: Vec<WriteOperation>) -> Result<()> {
        let mut batch = sled::Batch::default();

        for op in operations {
            match op {
                WriteOperation::Put { key, value } => {
                    batch.insert(key, value);
                }
                WriteOperation::Delete { key } => {
                    batch.remove(key);
                }
            }
        }

        self.db
            .apply_batch(batch)
            .map_err(|e| VotingError::DatabaseError(format!("Batch write failed: {}", e)))?;

        Ok(())
    }

    fn flush(&self) -> Result<()> {
        self.db
            .flush()
            .map_err(|e| VotingError::DatabaseError(format!("Flush failed: {}", e)))?;

        Ok(())
    }

    fn compact(&self) -> Result<()> {
        // Sled doesn't have explicit compaction
        self.flush()
    }

    fn size(&self) -> u64 {
        self.db.size_on_disk().unwrap_or(0)
    }

    fn stats(&self) -> DatabaseStats {
        let mut stats = self.stats.read().unwrap().clone();
        stats.total_keys = self.db.len() as u64;
        stats.total_size = self.size();
        stats
    }
}

// ============================================================================
// In-Memory Implementation
// ============================================================================

pub struct MemoryDatabase {
    data: Arc<RwLock<std::collections::HashMap<Vec<u8>, Vec<u8>>>>,
    stats: Arc<RwLock<DatabaseStats>>,
}

impl MemoryDatabase {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(std::collections::HashMap::new())),
            stats: Arc::new(RwLock::new(DatabaseStats::default())),
        }
    }
}

impl Default for MemoryDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl Database for MemoryDatabase {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        if let Ok(mut stats) = self.stats.write() {
            stats.read_count += 1;
        }

        let data = self.data.read().unwrap();
        Ok(data.get(key).cloned())
    }

    fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        if let Ok(mut stats) = self.stats.write() {
            stats.write_count += 1;
        }

        let mut data = self.data.write().unwrap();
        data.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn delete(&self, key: &[u8]) -> Result<()> {
        if let Ok(mut stats) = self.stats.write() {
            stats.delete_count += 1;
        }

        let mut data = self.data.write().unwrap();
        data.remove(key);
        Ok(())
    }

    fn contains(&self, key: &[u8]) -> Result<bool> {
        let data = self.data.read().unwrap();
        Ok(data.contains_key(key))
    }

    fn iter_prefix(&self, prefix: &[u8]) -> Result<Box<dyn Iterator<Item = (Vec<u8>, Vec<u8>)>>> {
        let data = self.data.read().unwrap();
        let results: Vec<(Vec<u8>, Vec<u8>)> = data
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        Ok(Box::new(results.into_iter()))
    }

    fn keys_with_prefix(&self, prefix: &[u8]) -> Result<Vec<Vec<u8>>> {
        let data = self.data.read().unwrap();
        let keys: Vec<Vec<u8>> = data
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();

        Ok(keys)
    }

    fn write_batch(&self, operations: Vec<WriteOperation>) -> Result<()> {
        let mut data = self.data.write().unwrap();

        for op in operations {
            match op {
                WriteOperation::Put { key, value } => {
                    data.insert(key, value);
                }
                WriteOperation::Delete { key } => {
                    data.remove(&key);
                }
            }
        }

        Ok(())
    }

    fn flush(&self) -> Result<()> {
        Ok(())
    }

    fn compact(&self) -> Result<()> {
        Ok(())
    }

    fn size(&self) -> u64 {
        let data = self.data.read().unwrap();
        data.iter()
            .map(|(k, v)| (k.len() + v.len()) as u64)
            .sum()
    }

    fn stats(&self) -> DatabaseStats {
        let mut stats = self.stats.read().unwrap().clone();
        let data = self.data.read().unwrap();
        stats.total_keys = data.len() as u64;
        stats.total_size = self.size();
        stats
    }
}

// ============================================================================
// Database Factory
// ============================================================================

pub struct DatabaseFactory;

impl DatabaseFactory {
    pub fn create(config: &DatabaseConfig) -> Result<Arc<dyn Database>> {
        match config.backend {
            #[cfg(feature = "rocksdb-backend")]
            DatabaseBackend::RocksDB => {
                let db = RocksDBDatabase::with_config(&config.path, config)?;
                Ok(Arc::new(db))
            }
            #[cfg(not(feature = "rocksdb-backend"))]
            DatabaseBackend::RocksDB => {
                Err(VotingError::NotImplemented(
                    "RocksDB backend not enabled. Rebuild with --features rocksdb-backend".to_string(),
                ))
            }
            
            #[cfg(feature = "sled-backend")]
            DatabaseBackend::Sled => {
                let db = SledDatabase::with_config(&config.path, config)?;
                Ok(Arc::new(db))
            }
            #[cfg(not(feature = "sled-backend"))]
            DatabaseBackend::Sled => {
                Err(VotingError::NotImplemented(
                    "Sled backend not enabled. Rebuild with --features sled-backend".to_string(),
                ))
            }
            
            DatabaseBackend::Memory => {
                let db = MemoryDatabase::new();
                Ok(Arc::new(db))
            }
        }
    }

    pub fn create_memory() -> Arc<dyn Database> {
        Arc::new(MemoryDatabase::new())
    }

    #[cfg(feature = "sled-backend")]
    pub fn create_sled<P: AsRef<Path>>(path: P) -> Result<Arc<dyn Database>> {
        let db = SledDatabase::open(path)?;
        Ok(Arc::new(db))
    }

    #[cfg(feature = "rocksdb-backend")]
    pub fn create_rocksdb<P: AsRef<Path>>(path: P) -> Result<Arc<dyn Database>> {
        let db = RocksDBDatabase::open(path)?;
        Ok(Arc::new(db))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_memory_database() {
        let db = MemoryDatabase::new();

        db.put(b"key1", b"value1").unwrap();
        assert_eq!(db.get(b"key1").unwrap(), Some(b"value1".to_vec()));

        assert!(db.contains(b"key1").unwrap());
        assert!(!db.contains(b"key2").unwrap());

        db.delete(b"key1").unwrap();
        assert_eq!(db.get(b"key1").unwrap(), None);
    }

    #[test]
    fn test_memory_database_prefix() {
        let db = MemoryDatabase::new();

        db.put(b"prefix:key1", b"value1").unwrap();
        db.put(b"prefix:key2", b"value2").unwrap();
        db.put(b"other:key3", b"value3").unwrap();

        let keys = db.keys_with_prefix(b"prefix:").unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_memory_database_batch() {
        let db = MemoryDatabase::new();

        let operations = vec![
            WriteOperation::Put {
                key: b"key1".to_vec(),
                value: b"value1".to_vec(),
            },
            WriteOperation::Put {
                key: b"key2".to_vec(),
                value: b"value2".to_vec(),
            },
            WriteOperation::Delete {
                key: b"key1".to_vec(),
            },
        ];

        db.write_batch(operations).unwrap();

        assert_eq!(db.get(b"key1").unwrap(), None);
        assert_eq!(db.get(b"key2").unwrap(), Some(b"value2".to_vec()));
    }

    #[test]
    fn test_memory_database_stats() {
        let db = MemoryDatabase::new();

        db.put(b"key1", b"value1").unwrap();
        db.get(b"key1").unwrap();
        db.delete(b"key1").unwrap();

        let stats = db.stats();
        assert_eq!(stats.write_count, 1);
        assert_eq!(stats.read_count, 1);
        assert_eq!(stats.delete_count, 1);
    }

    #[test]
    fn test_database_factory() {
        let config = DatabaseConfig {
            backend: DatabaseBackend::Memory,
            ..Default::default()
        };

        let db = DatabaseFactory::create(&config).unwrap();
        db.put(b"key", b"value").unwrap();
        assert_eq!(db.get(b"key").unwrap(), Some(b"value".to_vec()));
    }

    #[test]
    fn test_database_config_default() {
        let config = DatabaseConfig::default();
        assert!(config.compression);
    }
}

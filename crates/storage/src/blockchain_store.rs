use crate::database::{Database, DatabaseFactory, WriteOperation};
use blockchain_core::{Block, Blockchain, Transaction};
use common::{BlockHash, BlockHeight, Result, Timestamp, TxId, VotingError};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Storage for blockchain data
///
/// Stores:
/// - Blocks by height and hash
/// - Transactions by ID
/// - Chain metadata (height, genesis hash, etc.)
/// - Block indices for fast lookups
pub struct BlockchainStore {
    db: Box<dyn Database>,
    metadata: ChainMetadata,
}

impl BlockchainStore {
    /// Create a new blockchain store
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db = DatabaseFactory::create_sled(path)?;
        let metadata = Self::load_metadata(&*db)?;

        Ok(Self { db, metadata })
    }

    /// Create with in-memory database (for testing)
    pub fn new_memory() -> Self {
        let db = DatabaseFactory::create_memory();
        let metadata = ChainMetadata::default();

        Self { db, metadata }
    }

    /// Store a block
    pub fn store_block(&mut self, block: &Block) -> Result<()> {
        let height = block.height();
        let hash = block.hash();

        // Serialize block
        let block_data = bincode::serialize(block)
            .map_err(|e| VotingError::SerializationError(format!("Block serialization failed: {}", e)))?;

        // Store block by height
        let height_key = Self::height_key(height);
        self.db.put(&height_key, &block_data)?;

        // Store block by hash
        let hash_key = Self::hash_key(&hash);
        self.db.put(&hash_key, &block_data)?;

        // Index transactions
        for tx in &block.transactions {
            let tx_key = Self::tx_key(&tx.id);
            let tx_location = TxLocation {
                block_height: height,
                block_hash: hash,
            };
            let tx_location_data = bincode::serialize(&tx_location)
                .map_err(|e| VotingError::SerializationError(format!("TX location serialization failed: {}", e)))?;
            self.db.put(&tx_key, &tx_location_data)?;
        }

        // Update metadata
        if height > self.metadata.chain_height {
            self.metadata.chain_height = height;
            self.metadata.latest_block_hash = hash;
            self.metadata.latest_block_timestamp = block.timestamp();
            self.save_metadata()?;
        }

        Ok(())
    }

    /// Get block by height
    pub fn get_block_by_height(&self, height: BlockHeight) -> Result<Option<Block>> {
        let key = Self::height_key(height);
        match self.db.get(&key)? {
            Some(data) => {
                let block = bincode::deserialize(&data)
                    .map_err(|e| VotingError::DeserializationError(format!("Block deserialization failed: {}", e)))?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    /// Get block by hash
    pub fn get_block_by_hash(&self, hash: &BlockHash) -> Result<Option<Block>> {
        let key = Self::hash_key(hash);
        match self.db.get(&key)? {
            Some(data) => {
                let block = bincode::deserialize(&data)
                    .map_err(|e| VotingError::DeserializationError(format!("Block deserialization failed: {}", e)))?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    /// Get transaction by ID
    pub fn get_transaction(&self, tx_id: &TxId) -> Result<Option<Transaction>> {
        let key = Self::tx_key(tx_id);
        match self.db.get(&key)? {
            Some(data) => {
                let tx_location: TxLocation = bincode::deserialize(&data)
                    .map_err(|e| VotingError::DeserializationError(format!("TX location deserialization failed: {}", e)))?;

                // Get block containing transaction
                if let Some(block) = self.get_block_by_height(tx_location.block_height)? {
                    let tx = block.transactions.iter().find(|t| t.id == *tx_id).cloned();
                    Ok(tx)
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    /// Check if transaction exists
    pub fn contains_transaction(&self, tx_id: &TxId) -> Result<bool> {
        let key = Self::tx_key(tx_id);
        self.db.contains(&key)
    }

    /// Get latest block
    pub fn get_latest_block(&self) -> Result<Option<Block>> {
        if self.metadata.chain_height == 0 && self.metadata.latest_block_hash == BlockHash::zero() {
            return Ok(None);
        }

        self.get_block_by_height(self.metadata.chain_height)
    }

    /// Get blocks in a range
    pub fn get_blocks_range(&self, start: BlockHeight, end: BlockHeight) -> Result<Vec<Block>> {
        let mut blocks = Vec::new();

        for height in start..=end {
            if let Some(block) = self.get_block_by_height(height)? {
                blocks.push(block);
            }
        }

        Ok(blocks)
    }

    /// Get chain height
    pub fn chain_height(&self) -> BlockHeight {
        self.metadata.chain_height
    }

    /// Get genesis hash
    pub fn genesis_hash(&self) -> BlockHash {
        self.metadata.genesis_hash
    }

    /// Get metadata
    pub fn metadata(&self) -> &ChainMetadata {
        &self.metadata
    }

    /// Set genesis hash
    pub fn set_genesis_hash(&mut self, hash: BlockHash) -> Result<()> {
        self.metadata.genesis_hash = hash;
        self.save_metadata()
    }

    /// Get block count
    pub fn block_count(&self) -> u64 {
        self.metadata.chain_height + 1
    }

    /// Get transaction count
    pub fn transaction_count(&self) -> u64 {
        self.metadata.total_transactions
    }

    /// Increment transaction count
    pub fn increment_tx_count(&mut self, count: u64) -> Result<()> {
        self.metadata.total_transactions += count;
        self.save_metadata()
    }

    /// Store entire blockchain
    pub fn store_blockchain(&mut self, blockchain: &Blockchain) -> Result<()> {
        let stats = blockchain.get_stats();
        
        for height in 0..=stats.height {
            if let Some(block) = blockchain.get_block_by_height(height) {
                self.store_block(block)?;
            }
        }

        self.metadata.genesis_hash = stats.genesis_hash;
        self.metadata.total_transactions = stats.total_transactions as u64;
        self.save_metadata()?;

        Ok(())
    }

    /// Load blockchain from storage
    pub fn load_blockchain(&self) -> Result<Blockchain> {
        if self.metadata.chain_height == 0 && self.metadata.genesis_hash == BlockHash::zero() {
            return Err(VotingError::InvalidInput(
                "No blockchain data in storage".to_string(),
            ));
        }

        let genesis_block = self.get_block_by_height(0)?
            .ok_or_else(|| VotingError::BlockNotFound("Genesis block not found".to_string()))?;

        let genesis_config = blockchain_core::genesis::GenesisBlock::new(
            vec![genesis_block.header.validator],
            genesis_block.timestamp(),
        );

        let mut blockchain = Blockchain::new(genesis_config)?;

        for height in 1..=self.metadata.chain_height {
            if let Some(block) = self.get_block_by_height(height)? {
                blockchain.add_block(block)?;
            }
        }

        Ok(blockchain)
    }

    /// Prune blocks before a timestamp
    pub fn prune_before(&mut self, cutoff: Timestamp) -> Result<usize> {
        let mut pruned_count = 0;
        let mut operations = Vec::new();

        for height in 1..=self.metadata.chain_height {
            if let Some(block) = self.get_block_by_height(height)? {
                if block.timestamp() < cutoff {
                    // Don't prune genesis block
                    if height == 0 {
                        continue;
                    }

                    // Delete block by height
                    let height_key = Self::height_key(height);
                    operations.push(WriteOperation::Delete { key: height_key });

                    // Delete block by hash
                    let hash_key = Self::hash_key(&block.hash());
                    operations.push(WriteOperation::Delete { key: hash_key });

                    // Delete transaction indices
                    for tx in &block.transactions {
                        let tx_key = Self::tx_key(&tx.id);
                        operations.push(WriteOperation::Delete { key: tx_key });
                    }

                    pruned_count += 1;
                }
            }
        }

        if !operations.is_empty() {
            self.db.write_batch(operations)?;
        }

        Ok(pruned_count)
    }

    /// Compact database
    pub fn compact(&mut self) -> Result<()> {
        self.db.compact()
    }

    /// Flush pending writes
    pub fn flush(&mut self) -> Result<()> {
        self.db.flush()
    }

    /// Get database size
    pub fn size(&self) -> u64 {
        self.db.size()
    }

    // Key generation functions
    fn height_key(height: BlockHeight) -> Vec<u8> {
        let mut key = b"block:height:".to_vec();
        key.extend_from_slice(&height.to_le_bytes());
        key
    }

    fn hash_key(hash: &BlockHash) -> Vec<u8> {
        let mut key = b"block:hash:".to_vec();
        key.extend_from_slice(hash.as_bytes());
        key
    }

    fn tx_key(tx_id: &TxId) -> Vec<u8> {
        let mut key = b"tx:".to_vec();
        key.extend_from_slice(tx_id.as_bytes());
        key
    }

    fn metadata_key() -> Vec<u8> {
        b"meta:chain".to_vec()
    }

    fn load_metadata(db: &dyn Database) -> Result<ChainMetadata> {
        let key = Self::metadata_key();
        match db.get(&key)? {
            Some(data) => {
                let metadata = bincode::deserialize(&data)
                    .map_err(|e| VotingError::DeserializationError(format!("Metadata deserialization failed: {}", e)))?;
                Ok(metadata)
            }
            None => Ok(ChainMetadata::default()),
        }
    }

    fn save_metadata(&mut self) -> Result<()> {
        let key = Self::metadata_key();
        let data = bincode::serialize(&self.metadata)
            .map_err(|e| VotingError::SerializationError(format!("Metadata serialization failed: {}", e)))?;
        self.db.put(&key, &data)
    }
}

/// Chain metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainMetadata {
    /// Current chain height
    pub chain_height: BlockHeight,

    /// Genesis block hash
    pub genesis_hash: BlockHash,

    /// Latest block hash
    pub latest_block_hash: BlockHash,

    /// Latest block timestamp
    pub latest_block_timestamp: Timestamp,

    /// Total transactions
    pub total_transactions: u64,

    /// Last updated
    pub last_updated: Timestamp,
}

impl Default for ChainMetadata {
    fn default() -> Self {
        Self {
            chain_height: 0,
            genesis_hash: BlockHash::zero(),
            latest_block_hash: BlockHash::zero(),
            latest_block_timestamp: 0,
            total_transactions: 0,
            last_updated: common::utils::current_timestamp(),
        }
    }
}

/// Transaction location in blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TxLocation {
    block_height: BlockHeight,
    block_hash: BlockHash,
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockchain_core::genesis::GenesisBlock;
    use common::{ElectionId, PublicKey, Signature};

    fn create_test_genesis() -> GenesisBlock {
        GenesisBlock::new(vec![PublicKey::new([1u8; 32])], 1000000000)
    }

    fn create_test_block(height: BlockHeight, previous_hash: BlockHash) -> Block {
        let validator = PublicKey::new([1u8; 32]);
        Block::new(height, previous_hash, vec![], validator)
    }

    #[test]
    fn test_blockchain_store_creation() {
        let store = BlockchainStore::new_memory();
        assert_eq!(store.chain_height(), 0);
        assert_eq!(store.genesis_hash(), BlockHash::zero());
    }

    #[test]
    fn test_store_and_retrieve_block() {
        let mut store = BlockchainStore::new_memory();

        let block = create_test_block(1, BlockHash::zero());
        let block_hash = block.hash();

        store.store_block(&block).unwrap();

        let retrieved = store.get_block_by_height(1).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().hash(), block_hash);
    }

    #[test]
    fn test_get_block_by_hash() {
        let mut store = BlockchainStore::new_memory();

        let block = create_test_block(1, BlockHash::zero());
        let block_hash = block.hash();

        store.store_block(&block).unwrap();

        let retrieved = store.get_block_by_hash(&block_hash).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().height(), 1);
    }

    #[test]
    fn test_metadata_update() {
        let mut store = BlockchainStore::new_memory();

        let block = create_test_block(5, BlockHash::zero());
        store.store_block(&block).unwrap();

        assert_eq!(store.chain_height(), 5);
        assert_eq!(store.metadata().latest_block_hash, block.hash());
    }

    #[test]
    fn test_get_blocks_range() {
        let mut store = BlockchainStore::new_memory();

        for i in 0..5 {
            let block = create_test_block(i, BlockHash::zero());
            store.store_block(&block).unwrap();
        }

        let blocks = store.get_blocks_range(1, 3).unwrap();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].height(), 1);
        assert_eq!(blocks[2].height(), 3);
    }

    #[test]
    fn test_store_blockchain() {
        let genesis = create_test_genesis();
        let mut blockchain = Blockchain::new(genesis).unwrap();

        for i in 1..5 {
            let latest_hash = blockchain.latest_block().hash();
            let validator = PublicKey::new([1u8; 32]);
            let block = Block::new(i, latest_hash, vec![], validator);
            blockchain.add_block(block).unwrap();
        }

        let mut store = BlockchainStore::new_memory();
        store.store_blockchain(&blockchain).unwrap();

        assert_eq!(store.chain_height(), 4);
        assert_eq!(store.block_count(), 5);
    }

    #[test]
    fn test_load_blockchain() {
        let genesis = create_test_genesis();
        let mut blockchain = Blockchain::new(genesis).unwrap();

        for i in 1..3 {
            let latest_hash = blockchain.latest_block().hash();
            let validator = PublicKey::new([1u8; 32]);
            let block = Block::new(i, latest_hash, vec![], validator);
            blockchain.add_block(block).unwrap();
        }

        let mut store = BlockchainStore::new_memory();
        store.store_blockchain(&blockchain).unwrap();

        let loaded = store.load_blockchain().unwrap();
        assert_eq!(loaded.height(), blockchain.height());
    }

    #[test]
    fn test_transaction_storage() {
        let mut store = BlockchainStore::new_memory();

        let vote = blockchain_core::transaction::VoteTransaction {
            election_id: ElectionId::new([1u8; 16]),
            encrypted_vote: vec![1, 2, 3, 4],
            voter_signature: Signature::new([0u8; 64]),
        };

        let tx = blockchain_core::Transaction::new(
            blockchain_core::transaction::TransactionType::Vote(vote),
        );
        let tx_id = tx.id;

        let validator = PublicKey::new([1u8; 32]);
        let block = Block::new(1, BlockHash::zero(), vec![tx], validator);

        store.store_block(&block).unwrap();

        assert!(store.contains_transaction(&tx_id).unwrap());

        let retrieved_tx = store.get_transaction(&tx_id).unwrap();
        assert!(retrieved_tx.is_some());
        assert_eq!(retrieved_tx.unwrap().id, tx_id);
    }

    #[test]
    fn test_get_latest_block() {
        let mut store = BlockchainStore::new_memory();

        assert!(store.get_latest_block().unwrap().is_none());

        let block1 = create_test_block(1, BlockHash::zero());
        store.store_block(&block1).unwrap();

        let block2 = create_test_block(5, BlockHash::zero());
        store.store_block(&block2).unwrap();

        let latest = store.get_latest_block().unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().height(), 5);
    }

    #[test]
    fn test_set_genesis_hash() {
        let mut store = BlockchainStore::new_memory();

        let genesis_hash = BlockHash::new([1u8; 32]);
        store.set_genesis_hash(genesis_hash).unwrap();

        assert_eq!(store.genesis_hash(), genesis_hash);
    }

    #[test]
    fn test_block_count() {
        let mut store = BlockchainStore::new_memory();

        assert_eq!(store.block_count(), 1);

        let block = create_test_block(5, BlockHash::zero());
        store.store_block(&block).unwrap();

        assert_eq!(store.block_count(), 6);
    }

    #[test]
    fn test_transaction_count() {
        let mut store = BlockchainStore::new_memory();

        assert_eq!(store.transaction_count(), 0);

        store.increment_tx_count(10).unwrap();
        assert_eq!(store.transaction_count(), 10);
    }

    #[test]
    fn test_prune_blocks() {
        let mut store = BlockchainStore::new_memory();

        let block1 = Block::new(1, BlockHash::zero(), vec![], PublicKey::new([1u8; 32]));
        let mut block1_copy = block1.clone();
        block1_copy.header.timestamp = 1000;
        store.store_block(&block1_copy).unwrap();

        let block2 = Block::new(2, block1.hash(), vec![], PublicKey::new([1u8; 32]));
        let mut block2_copy = block2.clone();
        block2_copy.header.timestamp = 2000;
        store.store_block(&block2_copy).unwrap();

        let pruned = store.prune_before(1500).unwrap();
        assert_eq!(pruned, 1);

        assert!(store.get_block_by_height(1).unwrap().is_none());
        assert!(store.get_block_by_height(2).unwrap().is_some());
    }

    #[test]
    fn test_key_generation() {
        let height_key = BlockchainStore::height_key(123);
        assert!(height_key.starts_with(b"block:height:"));

        let hash = BlockHash::new([1u8; 32]);
        let hash_key = BlockchainStore::hash_key(&hash);
        assert!(hash_key.starts_with(b"block:hash:"));

        let tx_id = TxId::new([2u8; 32]);
        let tx_key = BlockchainStore::tx_key(&tx_id);
        assert!(tx_key.starts_with(b"tx:"));
    }

    #[test]
    fn test_metadata_default() {
        let metadata = ChainMetadata::default();
        assert_eq!(metadata.chain_height, 0);
        assert_eq!(metadata.genesis_hash, BlockHash::zero());
        assert_eq!(metadata.total_transactions, 0);
    }
}

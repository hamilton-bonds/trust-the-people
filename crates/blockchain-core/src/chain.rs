use common::{BlockHash, BlockHeight, PublicKey, Result, Timestamp, TxId, VotingError};
use crate::block::Block;
use crate::transaction::Transaction;
use crate::genesis::GenesisBlock;
use std::collections::HashMap;

/// The blockchain structure
#[derive(Debug, Clone)]
pub struct Blockchain {
    /// All blocks indexed by height
    blocks: Vec<Block>,
    
    /// Block hash to height mapping for quick lookup
    hash_to_height: HashMap<BlockHash, BlockHeight>,
    
    /// Transaction ID to block height mapping
    tx_to_block: HashMap<TxId, BlockHeight>,
    
    /// Current chain height
    height: BlockHeight,
    
    /// Genesis block hash
    genesis_hash: BlockHash,
}

impl Blockchain {
    /// Create a new blockchain with genesis block
    pub fn new(genesis: GenesisBlock) -> Result<Self> {
        let genesis_block = genesis.create_block()?;
        let genesis_hash = genesis_block.hash();
        let mut hash_to_height = HashMap::new();
        let mut tx_to_block = HashMap::new();
        
        hash_to_height.insert(genesis_hash, 0);
        
        // Index genesis transactions
        for tx in &genesis_block.transactions {
            tx_to_block.insert(tx.id, 0);
        }
        
        Ok(Self {
            blocks: vec![genesis_block],
            hash_to_height,
            tx_to_block,
            height: 0,
            genesis_hash,
        })
    }
    
    /// Get current blockchain height
    pub fn height(&self) -> BlockHeight {
        self.height
    }
    
    /// Get genesis block hash
    pub fn genesis_hash(&self) -> BlockHash {
        self.genesis_hash
    }
    
    /// Get the latest block
    pub fn latest_block(&self) -> &Block {
        self.blocks.last().expect("Blockchain must have at least genesis block")
    }
    
    /// Get block by height
    pub fn get_block_by_height(&self, height: BlockHeight) -> Option<&Block> {
        if height <= self.height {
            Some(&self.blocks[height as usize])
        } else {
            None
        }
    }
    
    /// Get block by hash
    pub fn get_block_by_hash(&self, hash: &BlockHash) -> Option<&Block> {
        self.hash_to_height
            .get(hash)
            .and_then(|&height| self.get_block_by_height(height))
    }
    
    /// Get transaction by ID
    pub fn get_transaction(&self, tx_id: &TxId) -> Option<&Transaction> {
        self.tx_to_block
            .get(tx_id)
            .and_then(|&height| self.get_block_by_height(height))
            .and_then(|block| block.get_transaction(tx_id))
    }
    
    /// Check if transaction exists in blockchain
    pub fn contains_transaction(&self, tx_id: &TxId) -> bool {
        self.tx_to_block.contains_key(tx_id)
    }
    
    /// Add a new block to the chain
    pub fn add_block(&mut self, block: Block) -> Result<()> {
        // Validate block structure
        block.validate()?;
        
        // Validate block links to current chain
        let latest = self.latest_block();
        block.validate_chain_link(latest)?;
        
        // Check for duplicate transactions
        for tx in &block.transactions {
            if self.contains_transaction(&tx.id) {
                return Err(VotingError::DuplicateTransaction(tx.id.to_hex()));
            }
        }
        
        // Add block to chain
        let block_height = block.height();
        let block_hash = block.hash();
        
        // Index block
        self.hash_to_height.insert(block_hash, block_height);
        
        // Index transactions
        for tx in &block.transactions {
            self.tx_to_block.insert(tx.id, block_height);
        }
        
        self.blocks.push(block);
        self.height = block_height;
        
        Ok(())
    }
    
    /// Verify the entire blockchain integrity
    pub fn verify(&self) -> Result<()> {
        if self.blocks.is_empty() {
            return Err(VotingError::InvalidBlock("Blockchain is empty".to_string()));
        }
        
        // Verify genesis block
        let genesis = &self.blocks[0];
        if genesis.height() != 0 {
            return Err(VotingError::GenesisError("Genesis height must be 0".to_string()));
        }
        if genesis.previous_hash() != BlockHash::zero() {
            return Err(VotingError::GenesisError("Genesis must have zero previous hash".to_string()));
        }
        genesis.validate()?;
        
        // Verify all subsequent blocks
        for i in 1..self.blocks.len() {
            let current = &self.blocks[i];
            let previous = &self.blocks[i - 1];
            
            current.validate()?;
            current.validate_chain_link(previous)?;
        }
        
        Ok(())
    }
    
    /// Get all blocks in range (inclusive)
    pub fn get_blocks_range(&self, start: BlockHeight, end: BlockHeight) -> Vec<&Block> {
        if start > end || end > self.height {
            return Vec::new();
        }
        
        self.blocks[start as usize..=end as usize]
            .iter()
            .collect()
    }
    
    /// Get blocks by validator
    pub fn get_blocks_by_validator(&self, validator: &PublicKey) -> Vec<&Block> {
        self.blocks
            .iter()
            .filter(|block| &block.header.validator == validator)
            .collect()
    }
    
    /// Get all transactions in blockchain
    pub fn get_all_transactions(&self) -> Vec<&Transaction> {
        self.blocks
            .iter()
            .flat_map(|block| block.transactions.iter())
            .collect()
    }
    
    /// Get transactions in a range of blocks
    pub fn get_transactions_in_range(&self, start: BlockHeight, end: BlockHeight) -> Vec<&Transaction> {
        self.get_blocks_range(start, end)
            .into_iter()
            .flat_map(|block| block.transactions.iter())
            .collect()
    }
    
    /// Count total transactions in blockchain
    pub fn transaction_count(&self) -> usize {
        self.tx_to_block.len()
    }
    
    /// Get blockchain statistics
    pub fn get_stats(&self) -> BlockchainStats {
        let total_blocks = self.blocks.len() as u64;
        let total_transactions = self.transaction_count();
        let avg_tx_per_block = if total_blocks > 0 {
            total_transactions as f64 / total_blocks as f64
        } else {
            0.0
        };
        
        BlockchainStats {
            height: self.height,
            total_blocks,
            total_transactions,
            avg_tx_per_block,
            genesis_hash: self.genesis_hash,
            latest_block_hash: self.latest_block().hash(),
            latest_block_timestamp: self.latest_block().timestamp(),
        }
    }
    
    /// Prune blocks older than specified timestamp (keeping genesis)
    pub fn prune_old_blocks(&mut self, before_timestamp: Timestamp) -> Result<usize> {
        // Never prune genesis
        let mut pruned_count = 0;
        let mut new_blocks = vec![self.blocks[0].clone()];
        
        for block in self.blocks.iter().skip(1) {
            if block.timestamp() >= before_timestamp {
                new_blocks.push(block.clone());
            } else {
                // Remove from indices
                self.hash_to_height.remove(&block.hash());
                for tx in &block.transactions {
                    self.tx_to_block.remove(&tx.id);
                }
                pruned_count += 1;
            }
        }
        
        self.blocks = new_blocks;
        
        Ok(pruned_count)
    }
}

/// Blockchain statistics
#[derive(Debug, Clone)]
pub struct BlockchainStats {
    pub height: BlockHeight,
    pub total_blocks: u64,
    pub total_transactions: usize,
    pub avg_tx_per_block: f64,
    pub genesis_hash: BlockHash,
    pub latest_block_hash: BlockHash,
    pub latest_block_timestamp: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::{TransactionType, VoteTransaction};
    use common::{ElectionId, Signature};

    fn create_test_genesis() -> GenesisBlock {
        GenesisBlock::new(
            vec![PublicKey::new([1u8; 32])],
            1000000000,
        )
    }

    fn create_test_transaction() -> Transaction {
        let vote = VoteTransaction {
            election_id: ElectionId::new([1u8; 16]),
            encrypted_vote: vec![1, 2, 3, 4],
            voter_signature: Signature::new([0u8; 64]),
        };
        Transaction::new(TransactionType::Vote(vote))
    }

    #[test]
    fn test_blockchain_creation() {
        let genesis = create_test_genesis();
        let blockchain = Blockchain::new(genesis).unwrap();
        
        assert_eq!(blockchain.height(), 0);
        assert_eq!(blockchain.blocks.len(), 1);
    }

    #[test]
    fn test_add_block() {
        let genesis = create_test_genesis();
        let mut blockchain = Blockchain::new(genesis).unwrap();
        
        let latest_hash = blockchain.latest_block().hash();
        let validator = PublicKey::new([1u8; 32]);
        let transactions = vec![create_test_transaction()];
        
        let new_block = Block::new(1, latest_hash, transactions, validator);
        
        assert!(blockchain.add_block(new_block).is_ok());
        assert_eq!(blockchain.height(), 1);
        assert_eq!(blockchain.blocks.len(), 2);
    }

    #[test]
    fn test_get_block_by_height() {
        let genesis = create_test_genesis();
        let blockchain = Blockchain::new(genesis).unwrap();
        
        let block = blockchain.get_block_by_height(0);
        assert!(block.is_some());
        assert_eq!(block.unwrap().height(), 0);
        
        let missing = blockchain.get_block_by_height(10);
        assert!(missing.is_none());
    }

    #[test]
    fn test_get_block_by_hash() {
        let genesis = create_test_genesis();
        let blockchain = Blockchain::new(genesis).unwrap();
        
        let genesis_hash = blockchain.genesis_hash();
        let block = blockchain.get_block_by_hash(&genesis_hash);
        
        assert!(block.is_some());
        assert_eq!(block.unwrap().height(), 0);
    }

    #[test]
    fn test_transaction_lookup() {
        let genesis = create_test_genesis();
        let mut blockchain = Blockchain::new(genesis).unwrap();
        
        let tx = create_test_transaction();
        let tx_id = tx.id;
        
        let latest_hash = blockchain.latest_block().hash();
        let validator = PublicKey::new([1u8; 32]);
        let new_block = Block::new(1, latest_hash, vec![tx], validator);
        
        blockchain.add_block(new_block).unwrap();
        
        assert!(blockchain.contains_transaction(&tx_id));
        assert!(blockchain.get_transaction(&tx_id).is_some());
    }

    #[test]
    fn test_duplicate_transaction_rejected() {
        let genesis = create_test_genesis();
        let mut blockchain = Blockchain::new(genesis).unwrap();
        
        let tx = create_test_transaction();
        
        let latest_hash = blockchain.latest_block().hash();
        let validator = PublicKey::new([1u8; 32]);
        let block1 = Block::new(1, latest_hash, vec![tx.clone()], validator);
        
        blockchain.add_block(block1).unwrap();
        
        let latest_hash = blockchain.latest_block().hash();
        let block2 = Block::new(2, latest_hash, vec![tx], validator);
        
        assert!(blockchain.add_block(block2).is_err());
    }

    #[test]
    fn test_verify_blockchain() {
        let genesis = create_test_genesis();
        let mut blockchain = Blockchain::new(genesis).unwrap();
        
        for i in 1..5 {
            let latest_hash = blockchain.latest_block().hash();
            let validator = PublicKey::new([1u8; 32]);
            let block = Block::new(i, latest_hash, vec![], validator);
            blockchain.add_block(block).unwrap();
        }
        
        assert!(blockchain.verify().is_ok());
    }

    #[test]
    fn test_blockchain_stats() {
        let genesis = create_test_genesis();
        let mut blockchain = Blockchain::new(genesis).unwrap();
        
        let latest_hash = blockchain.latest_block().hash();
        let validator = PublicKey::new([1u8; 32]);
        let transactions = vec![create_test_transaction(), create_test_transaction()];
        let block = Block::new(1, latest_hash, transactions, validator);
        
        blockchain.add_block(block).unwrap();
        
        let stats = blockchain.get_stats();
        assert_eq!(stats.height, 1);
        assert_eq!(stats.total_blocks, 2);
        assert_eq!(stats.total_transactions, 2);
    }

    #[test]
    fn test_get_blocks_range() {
        let genesis = create_test_genesis();
        let mut blockchain = Blockchain::new(genesis).unwrap();
        
        for i in 1..5 {
            let latest_hash = blockchain.latest_block().hash();
            let validator = PublicKey::new([1u8; 32]);
            let block = Block::new(i, latest_hash, vec![], validator);
            blockchain.add_block(block).unwrap();
        }
        
        let blocks = blockchain.get_blocks_range(1, 3);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].height(), 1);
        assert_eq!(blocks[2].height(), 3);
    }
}

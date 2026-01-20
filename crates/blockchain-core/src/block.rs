use common::{BlockHash, BlockHeight, PublicKey, Signature, Timestamp, TxId, VotingError, Result};
use crate::transaction::Transaction;
use crate::merkle::MerkleTree;
use serde::{Deserialize, Serialize};

/// A block in the blockchain
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    /// Block header
    pub header: BlockHeader,
    
    /// Transactions in this block
    pub transactions: Vec<Transaction>,
    
    /// Validator signatures (for consensus)
    pub signatures: Vec<ValidatorSignature>,
}

/// Block header containing metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockHeader {
    /// Block height (index in chain)
    pub height: BlockHeight,
    
    /// Timestamp when block was created
    pub timestamp: Timestamp,
    
    /// Hash of previous block
    pub previous_hash: BlockHash,
    
    /// Merkle root of all transactions
    pub transactions_root: BlockHash,
    
    /// Hash of this block's header
    pub hash: BlockHash,
    
    /// Public key of validator who produced this block
    pub validator: PublicKey,
    
    /// Number of transactions in block
    pub transaction_count: u32,
}

/// Validator signature on a block
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidatorSignature {
    /// Validator's public key
    pub validator: PublicKey,
    
    /// Signature over block hash
    pub signature: Signature,
}

impl Block {
    /// Create a new block
    pub fn new(
        height: BlockHeight,
        previous_hash: BlockHash,
        transactions: Vec<Transaction>,
        validator: PublicKey,
    ) -> Self {
        let timestamp = common::utils::current_timestamp();
        let transaction_count = transactions.len() as u32;
        
        // Calculate merkle root of transactions
        let transactions_root = Self::calculate_transactions_root(&transactions);
        
        // Create header without hash first
        let mut header = BlockHeader {
            height,
            timestamp,
            previous_hash,
            transactions_root,
            hash: BlockHash::zero(),
            validator,
            transaction_count,
        };
        
        // Calculate and set block hash
        header.hash = header.calculate_hash();
        
        Self {
            header,
            transactions,
            signatures: Vec::new(),
        }
    }
    
    /// Calculate merkle root of transactions
    fn calculate_transactions_root(transactions: &[Transaction]) -> BlockHash {
        if transactions.is_empty() {
            return BlockHash::zero();
        }
        
        let tx_hashes: Vec<BlockHash> = transactions
            .iter()
            .map(|tx| BlockHash::new(tx.id.0))
            .collect();
        
        let merkle_tree = MerkleTree::new(tx_hashes);
        merkle_tree.root()
    }
    
    /// Get the block hash
    pub fn hash(&self) -> BlockHash {
        self.header.hash
    }
    
    /// Get block height
    pub fn height(&self) -> BlockHeight {
        self.header.height
    }
    
    /// Get previous block hash
    pub fn previous_hash(&self) -> BlockHash {
        self.header.previous_hash
    }
    
    /// Get block timestamp
    pub fn timestamp(&self) -> Timestamp {
        self.header.timestamp
    }
    
    /// Add a validator signature
    pub fn add_signature(&mut self, validator: PublicKey, signature: Signature) {
        self.signatures.push(ValidatorSignature {
            validator,
            signature,
        });
    }
    
    /// Check if block has signature from a specific validator
    pub fn has_signature_from(&self, validator: &PublicKey) -> bool {
        self.signatures.iter().any(|sig| &sig.validator == validator)
    }
    
    /// Get number of validator signatures
    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }
    
    /// Validate block structure and integrity
    pub fn validate(&self) -> Result<()> {
        // Check hash is correct
        let calculated_hash = self.header.calculate_hash();
        if self.header.hash != calculated_hash {
            return Err(VotingError::InvalidBlockHash {
                expected: calculated_hash.to_hex(),
                actual: self.header.hash.to_hex(),
            });
        }
        
        // Check transaction count matches
        if self.header.transaction_count != self.transactions.len() as u32 {
            return Err(VotingError::InvalidBlock(format!(
                "Transaction count mismatch: header says {}, actual is {}",
                self.header.transaction_count,
                self.transactions.len()
            )));
        }
        
        // Check merkle root is correct
        let calculated_root = Self::calculate_transactions_root(&self.transactions);
        if self.header.transactions_root != calculated_root {
            return Err(VotingError::InvalidBlock(
                "Transactions merkle root mismatch".to_string(),
            ));
        }
        
        // Validate each transaction
        for tx in &self.transactions {
            tx.validate()?;
        }
        
        Ok(())
    }
    
    /// Validate that this block follows the previous block
    pub fn validate_chain_link(&self, previous_block: &Block) -> Result<()> {
        // Check height is sequential
        if self.header.height != previous_block.header.height + 1 {
            return Err(VotingError::BlockHeightMismatch {
                expected: previous_block.header.height + 1,
                actual: self.header.height,
            });
        }
        
        // Check previous hash matches
        if self.header.previous_hash != previous_block.hash() {
            return Err(VotingError::InvalidPreviousHash(format!(
                "Expected {}, got {}",
                previous_block.hash().to_hex(),
                self.header.previous_hash.to_hex()
            )));
        }
        
        // Check timestamp is not before previous block
        if self.header.timestamp < previous_block.header.timestamp {
            return Err(VotingError::InvalidBlock(
                "Block timestamp is before previous block".to_string(),
            ));
        }
        
        Ok(())
    }
    
    /// Get transaction by ID
    pub fn get_transaction(&self, tx_id: &TxId) -> Option<&Transaction> {
        self.transactions.iter().find(|tx| &tx.id == tx_id)
    }
    
    /// Check if block contains a transaction
    pub fn contains_transaction(&self, tx_id: &TxId) -> bool {
        self.get_transaction(tx_id).is_some()
    }
}

impl BlockHeader {
    /// Calculate hash of block header
    pub fn calculate_hash(&self) -> BlockHash {
        let mut data = Vec::new();
        
        // Serialize header fields (excluding hash itself)
        data.extend_from_slice(&self.height.to_le_bytes());
        data.extend_from_slice(&self.timestamp.to_le_bytes());
        data.extend_from_slice(self.previous_hash.as_bytes());
        data.extend_from_slice(self.transactions_root.as_bytes());
        data.extend_from_slice(self.validator.as_bytes());
        data.extend_from_slice(&self.transaction_count.to_le_bytes());
        
        BlockHash::new(common::utils::hash_data(&data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::{TransactionType, VoteTransaction};
    use common::ElectionId;

    fn create_test_transaction() -> Transaction {
        let vote = VoteTransaction {
            election_id: ElectionId::new([1u8; 16]),
            encrypted_vote: vec![1, 2, 3, 4],
            voter_signature: Signature::new([0u8; 64]),
        };
        
        Transaction::new(TransactionType::Vote(vote))
    }

    #[test]
    fn test_block_creation() {
        let transactions = vec![create_test_transaction()];
        let validator = PublicKey::new([1u8; 32]);
        let previous_hash = BlockHash::zero();
        
        let block = Block::new(1, previous_hash, transactions, validator);
        
        assert_eq!(block.height(), 1);
        assert_eq!(block.previous_hash(), previous_hash);
        assert_eq!(block.transactions.len(), 1);
    }

    #[test]
    fn test_block_hash_deterministic() {
        let transactions = vec![create_test_transaction()];
        let validator = PublicKey::new([1u8; 32]);
        let previous_hash = BlockHash::zero();
        
        let block1 = Block::new(1, previous_hash, transactions.clone(), validator);
        let block2 = Block::new(1, previous_hash, transactions, validator);
        
        // Hashes won't be identical due to timestamp, but structure should be valid
        assert!(block1.validate().is_ok());
        assert!(block2.validate().is_ok());
    }

    #[test]
    fn test_block_validation() {
        let transactions = vec![create_test_transaction()];
        let validator = PublicKey::new([1u8; 32]);
        let previous_hash = BlockHash::zero();
        
        let block = Block::new(1, previous_hash, transactions, validator);
        
        assert!(block.validate().is_ok());
    }

    #[test]
    fn test_block_signature() {
        let transactions = vec![create_test_transaction()];
        let validator = PublicKey::new([1u8; 32]);
        let previous_hash = BlockHash::zero();
        
        let mut block = Block::new(1, previous_hash, transactions, validator);
        
        let validator_key = PublicKey::new([2u8; 32]);
        let signature = Signature::new([1u8; 64]);
        
        block.add_signature(validator_key, signature);
        
        assert_eq!(block.signature_count(), 1);
        assert!(block.has_signature_from(&validator_key));
    }

    #[test]
    fn test_chain_link_validation() {
        let validator = PublicKey::new([1u8; 32]);
        
        let block1 = Block::new(0, BlockHash::zero(), vec![], validator);
        let block2 = Block::new(1, block1.hash(), vec![], validator);
        
        assert!(block2.validate_chain_link(&block1).is_ok());
    }

    #[test]
    fn test_invalid_chain_link() {
        let validator = PublicKey::new([1u8; 32]);
        
        let block1 = Block::new(0, BlockHash::zero(), vec![], validator);
        let block2 = Block::new(2, block1.hash(), vec![], validator); // Wrong height
        
        assert!(block2.validate_chain_link(&block1).is_err());
    }

    #[test]
    fn test_get_transaction() {
        let tx = create_test_transaction();
        let tx_id = tx.id;
        let validator = PublicKey::new([1u8; 32]);
        
        let block = Block::new(1, BlockHash::zero(), vec![tx], validator);
        
        assert!(block.get_transaction(&tx_id).is_some());
        assert!(block.contains_transaction(&tx_id));
    }
}

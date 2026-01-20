use common::{ElectionId, PublicKey, Result, Signature, Timestamp, TxId, VotingError};
use serde::{Deserialize, Serialize};

/// A transaction in the blockchain
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transaction {
    /// Unique transaction ID (hash of transaction data)
    pub id: TxId,
    
    /// Transaction type and payload
    pub tx_type: TransactionType,
    
    /// Timestamp when transaction was created
    pub timestamp: Timestamp,
    
    /// Transaction nonce (for preventing replay attacks)
    pub nonce: u64,
}

/// Different types of transactions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionType {
    /// A vote transaction
    Vote(VoteTransaction),
    
    /// Register a new validator
    ValidatorRegistration(ValidatorRegistration),
    
    /// Remove a validator
    ValidatorRemoval(ValidatorRemoval),
    
    /// Create a new election
    ElectionCreation(ElectionCreation),
    
    /// Close an election
    ElectionClosure(ElectionClosure),
}

/// Vote transaction - the core voting operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoteTransaction {
    /// ID of the election this vote is for
    pub election_id: ElectionId,
    
    /// Encrypted vote data (preserves voter privacy)
    pub encrypted_vote: Vec<u8>,
    
    /// Voter's signature (proves eligibility without revealing identity)
    pub voter_signature: Signature,
}

/// Validator registration transaction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidatorRegistration {
    /// Public key of the new validator
    pub validator_key: PublicKey,
    
    /// Metadata about the validator
    pub metadata: ValidatorMetadata,
    
    /// Signature from existing validator authorizing this
    pub authorizing_signature: Signature,
}

/// Validator removal transaction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidatorRemoval {
    /// Public key of validator to remove
    pub validator_key: PublicKey,
    
    /// Reason for removal
    pub reason: String,
    
    /// Signature from authorized entity
    pub authorizing_signature: Signature,
}

/// Election creation transaction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElectionCreation {
    /// Unique election ID
    pub election_id: ElectionId,
    
    /// Election name/title
    pub name: String,
    
    /// Election description
    pub description: String,
    
    /// List of candidates
    pub candidates: Vec<Candidate>,
    
    /// Election start timestamp
    pub start_time: Timestamp,
    
    /// Election end timestamp
    pub end_time: Timestamp,
    
    /// Signature from election authority
    pub authority_signature: Signature,
}

/// Election closure transaction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElectionClosure {
    /// ID of election to close
    pub election_id: ElectionId,
    
    /// Final vote count (encrypted or aggregated)
    pub results: Vec<u8>,
    
    /// Signature from election authority
    pub authority_signature: Signature,
}

/// Candidate information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Candidate {
    /// Candidate ID
    pub id: String,
    
    /// Candidate name
    pub name: String,
    
    /// Party affiliation (if applicable)
    pub party: Option<String>,
    
    /// Additional metadata
    pub metadata: Option<String>,
}

/// Validator metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidatorMetadata {
    /// Validator name/identifier
    pub name: String,
    
    /// Organization
    pub organization: Option<String>,
    
    /// Contact information
    pub contact: Option<String>,
}

impl Transaction {
    /// Create a new transaction
    pub fn new(tx_type: TransactionType) -> Self {
        let timestamp = common::utils::current_timestamp();
        let nonce = Self::generate_nonce();
        
        let mut tx = Self {
            id: TxId::new([0u8; 32]),
            tx_type,
            timestamp,
            nonce,
        };
        
        // Calculate and set transaction ID
        tx.id = tx.calculate_id();
        tx
    }
    
    /// Create a new transaction with specific timestamp and nonce (for testing)
    pub fn new_with_params(tx_type: TransactionType, timestamp: Timestamp, nonce: u64) -> Self {
        let mut tx = Self {
            id: TxId::new([0u8; 32]),
            tx_type,
            timestamp,
            nonce,
        };
        
        tx.id = tx.calculate_id();
        tx
    }
    
    /// Calculate transaction ID (hash of transaction data)
    fn calculate_id(&self) -> TxId {
        let data = self.serialize_for_hashing();
        TxId::new(common::utils::hash_data(&data))
    }
    
    /// Serialize transaction data for hashing (excluding ID field)
    fn serialize_for_hashing(&self) -> Vec<u8> {
        let mut data = Vec::new();
        
        // Add timestamp
        data.extend_from_slice(&self.timestamp.to_le_bytes());
        
        // Add nonce
        data.extend_from_slice(&self.nonce.to_le_bytes());
        
        // Add transaction type data
        let type_data = common::utils::serialize(&self.tx_type)
            .expect("Failed to serialize transaction type");
        data.extend_from_slice(&type_data);
        
        data
    }
    
    /// Generate a random nonce
    fn generate_nonce() -> u64 {
        use rand::Rng;
        rand::thread_rng().gen()
    }
    
    /// Validate transaction structure
    pub fn validate(&self) -> Result<()> {
        // Verify transaction ID is correct
        let calculated_id = self.calculate_id();
        if self.id != calculated_id {
            return Err(VotingError::InvalidTransaction(
                "Transaction ID mismatch".to_string(),
            ));
        }
        
        // Validate based on transaction type
        match &self.tx_type {
            TransactionType::Vote(vote) => Self::validate_vote(vote),
            TransactionType::ValidatorRegistration(reg) => Self::validate_validator_registration(reg),
            TransactionType::ValidatorRemoval(removal) => Self::validate_validator_removal(removal),
            TransactionType::ElectionCreation(creation) => Self::validate_election_creation(creation),
            TransactionType::ElectionClosure(closure) => Self::validate_election_closure(closure),
        }
    }
    
    fn validate_vote(vote: &VoteTransaction) -> Result<()> {
        if vote.encrypted_vote.is_empty() {
            return Err(VotingError::InvalidVote("Vote data is empty".to_string()));
        }
        
        // Vote data should have reasonable size limits
        if vote.encrypted_vote.len() > 1024 * 1024 {
            return Err(VotingError::InvalidVote("Vote data too large".to_string()));
        }
        
        Ok(())
    }
    
    fn validate_validator_registration(reg: &ValidatorRegistration) -> Result<()> {
        if reg.metadata.name.is_empty() {
            return Err(VotingError::InvalidTransaction(
                "Validator name cannot be empty".to_string(),
            ));
        }
        
        Ok(())
    }
    
    fn validate_validator_removal(removal: &ValidatorRemoval) -> Result<()> {
        if removal.reason.is_empty() {
            return Err(VotingError::InvalidTransaction(
                "Removal reason cannot be empty".to_string(),
            ));
        }
        
        Ok(())
    }
    
    fn validate_election_creation(creation: &ElectionCreation) -> Result<()> {
        if creation.name.is_empty() {
            return Err(VotingError::InvalidTransaction(
                "Election name cannot be empty".to_string(),
            ));
        }
        
        if creation.candidates.is_empty() {
            return Err(VotingError::InvalidTransaction(
                "Election must have at least one candidate".to_string(),
            ));
        }
        
        if creation.end_time <= creation.start_time {
            return Err(VotingError::InvalidTransaction(
                "Election end time must be after start time".to_string(),
            ));
        }
        
        Ok(())
    }
    
    fn validate_election_closure(closure: &ElectionClosure) -> Result<()> {
        if closure.results.is_empty() {
            return Err(VotingError::InvalidTransaction(
                "Election results cannot be empty".to_string(),
            ));
        }
        
        Ok(())
    }
    
    /// Get transaction size in bytes
    pub fn size(&self) -> usize {
        common::utils::serialize(self)
            .map(|data| data.len())
            .unwrap_or(0)
    }
    
    /// Check if this is a vote transaction
    pub fn is_vote(&self) -> bool {
        matches!(self.tx_type, TransactionType::Vote(_))
    }
    
    /// Get the election ID if this is a vote transaction
    pub fn get_election_id(&self) -> Option<ElectionId> {
        match &self.tx_type {
            TransactionType::Vote(vote) => Some(vote.election_id),
            TransactionType::ElectionCreation(creation) => Some(creation.election_id),
            TransactionType::ElectionClosure(closure) => Some(closure.election_id),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_vote() -> VoteTransaction {
        VoteTransaction {
            election_id: ElectionId::new([1u8; 16]),
            encrypted_vote: vec![1, 2, 3, 4],
            voter_signature: Signature::new([0u8; 64]),
        }
    }

    #[test]
    fn test_transaction_creation() {
        let vote = create_test_vote();
        let tx = Transaction::new(TransactionType::Vote(vote));
        
        assert!(tx.id.as_bytes() != &[0u8; 32]);
        assert!(tx.timestamp > 0);
        assert!(tx.nonce > 0);
    }

    #[test]
    fn test_transaction_id_deterministic() {
        let vote = create_test_vote();
        let tx1 = Transaction::new_with_params(TransactionType::Vote(vote.clone()), 1000, 42);
        let tx2 = Transaction::new_with_params(TransactionType::Vote(vote), 1000, 42);
        
        assert_eq!(tx1.id, tx2.id);
    }

    #[test]
    fn test_transaction_validation() {
        let vote = create_test_vote();
        let tx = Transaction::new(TransactionType::Vote(vote));
        
        assert!(tx.validate().is_ok());
    }

    #[test]
    fn test_invalid_vote_empty() {
        let vote = VoteTransaction {
            election_id: ElectionId::new([1u8; 16]),
            encrypted_vote: vec![],
            voter_signature: Signature::new([0u8; 64]),
        };
        let tx = Transaction::new(TransactionType::Vote(vote));
        
        assert!(tx.validate().is_err());
    }

    #[test]
    fn test_election_creation() {
        let election = ElectionCreation {
            election_id: ElectionId::new([1u8; 16]),
            name: "Presidential Election 2024".to_string(),
            description: "Federal Presidential Election".to_string(),
            candidates: vec![
                Candidate {
                    id: "1".to_string(),
                    name: "Alice".to_string(),
                    party: Some("Party A".to_string()),
                    metadata: None,
                },
                Candidate {
                    id: "2".to_string(),
                    name: "Bob".to_string(),
                    party: Some("Party B".to_string()),
                    metadata: None,
                },
            ],
            start_time: 1000,
            end_time: 2000,
            authority_signature: Signature::new([0u8; 64]),
        };
        
        let tx = Transaction::new(TransactionType::ElectionCreation(election));
        assert!(tx.validate().is_ok());
    }

    #[test]
    fn test_invalid_election_no_candidates() {
        let election = ElectionCreation {
            election_id: ElectionId::new([1u8; 16]),
            name: "Test Election".to_string(),
            description: "Invalid election".to_string(),
            candidates: vec![],
            start_time: 1000,
            end_time: 2000,
            authority_signature: Signature::new([0u8; 64]),
        };
        
        let tx = Transaction::new(TransactionType::ElectionCreation(election));
        assert!(tx.validate().is_err());
    }

    #[test]
    fn test_invalid_election_time() {
        let election = ElectionCreation {
            election_id: ElectionId::new([1u8; 16]),
            name: "Test Election".to_string(),
            description: "Invalid time".to_string(),
            candidates: vec![
                Candidate {
                    id: "1".to_string(),
                    name: "Alice".to_string(),
                    party: None,
                    metadata: None,
                },
            ],
            start_time: 2000,
            end_time: 1000,
            authority_signature: Signature::new([0u8; 64]),
        };
        
        let tx = Transaction::new(TransactionType::ElectionCreation(election));
        assert!(tx.validate().is_err());
    }

    #[test]
    fn test_is_vote() {
        let vote = create_test_vote();
        let tx = Transaction::new(TransactionType::Vote(vote));
        
        assert!(tx.is_vote());
    }

    #[test]
    fn test_get_election_id() {
        let election_id = ElectionId::new([1u8; 16]);
        let vote = VoteTransaction {
            election_id,
            encrypted_vote: vec![1, 2, 3, 4],
            voter_signature: Signature::new([0u8; 64]),
        };
        let tx = Transaction::new(TransactionType::Vote(vote));
        
        assert_eq!(tx.get_election_id(), Some(election_id));
    }

    #[test]
    fn test_validator_registration() {
        let reg = ValidatorRegistration {
            validator_key: PublicKey::new([1u8; 32]),
            metadata: ValidatorMetadata {
                name: "Test Validator".to_string(),
                organization: Some("Test Org".to_string()),
                contact: None,
            },
            authorizing_signature: Signature::new([0u8; 64]),
        };
        
        let tx = Transaction::new(TransactionType::ValidatorRegistration(reg));
        assert!(tx.validate().is_ok());
    }
}

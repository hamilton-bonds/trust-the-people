pub mod proof_of_authority;
pub mod validator_set;
pub mod finality;

pub use proof_of_authority::ProofOfAuthority;
pub use validator_set::{ValidatorSet, ValidatorInfo};
pub use finality::{FinalityTracker, FinalityStatus};

use common::{PublicKey, Result, VotingError};
use crate::block::Block;

/// Trait for consensus mechanisms
pub trait Consensus: Send + Sync {
    /// Validate a block according to consensus rules
    fn validate_block(&self, block: &Block) -> Result<()>;
    
    /// Check if a validator can produce a block at this time
    fn can_produce_block(&self, validator: &PublicKey) -> Result<bool>;
    
    /// Get the current validator responsible for block production
    fn get_current_validator(&self) -> Result<PublicKey>;
    
    /// Check if block has reached finality
    fn is_finalized(&self, block: &Block) -> bool;
    
    /// Get minimum number of signatures required for finality
    fn required_signatures(&self) -> usize;
}

/// Consensus engine that coordinates validators and finality
pub struct ConsensusEngine {
    /// Consensus mechanism (PoA, PoS, etc.)
    mechanism: Box<dyn Consensus>,
    
    /// Validator set
    validator_set: ValidatorSet,
    
    /// Finality tracker
    finality_tracker: FinalityTracker,
}

impl ConsensusEngine {
    /// Create a new consensus engine with Proof of Authority
    pub fn new_poa(
        validators: Vec<PublicKey>,
        finality_threshold: f64,
    ) -> Result<Self> {
        if validators.is_empty() {
            return Err(VotingError::ConsensusError(
                "Validator set cannot be empty".to_string(),
            ));
        }
        
        let validator_set = ValidatorSet::new(validators);
        let finality_tracker = FinalityTracker::new(finality_threshold);
        let mechanism = Box::new(ProofOfAuthority::new(validator_set.clone()));
        
        Ok(Self {
            mechanism,
            validator_set,
            finality_tracker,
        })
    }
    
    /// Validate a block according to consensus rules
    pub fn validate_block(&self, block: &Block) -> Result<()> {
        // Basic consensus validation
        self.mechanism.validate_block(block)?;
        
        // Check validator is authorized
        if !self.validator_set.is_validator(&block.header.validator) {
            return Err(VotingError::InvalidValidator(
                format!("Unknown validator: {}", block.header.validator),
            ));
        }
        
        // Validate all signatures are from known validators
        for sig in &block.signatures {
            if !self.validator_set.is_validator(&sig.validator) {
                return Err(VotingError::InvalidValidator(
                    format!("Unknown validator signature: {}", sig.validator),
                ));
            }
        }
        
        Ok(())
    }
    
    /// Check if block has reached finality
    pub fn check_finality(&mut self, block: &Block) -> Result<FinalityStatus> {
        let status = self.finality_tracker.check_finality(
            block,
            self.validator_set.validator_count(),
        );
        
        Ok(status)
    }
    
    /// Get the validator set
    pub fn validator_set(&self) -> &ValidatorSet {
        &self.validator_set
    }
    
    /// Get mutable validator set
    pub fn validator_set_mut(&mut self) -> &mut ValidatorSet {
        &mut self.validator_set
    }
    
    /// Add a new validator
    pub fn add_validator(&mut self, validator: PublicKey) -> Result<()> {
        self.validator_set.add_validator(validator)
    }
    
    /// Remove a validator
    pub fn remove_validator(&mut self, validator: &PublicKey) -> Result<()> {
        self.validator_set.remove_validator(validator)
    }
    
    /// Get current validator for block production
    pub fn get_current_validator(&self) -> Result<PublicKey> {
        self.mechanism.get_current_validator()
    }
    
    /// Check if validator can produce block
    pub fn can_produce_block(&self, validator: &PublicKey) -> Result<bool> {
        self.mechanism.can_produce_block(validator)
    }
    
    /// Get required number of signatures for finality
    pub fn required_signatures(&self) -> usize {
        self.mechanism.required_signatures()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genesis::GenesisBlock;

    fn create_test_validators(count: usize) -> Vec<PublicKey> {
        (0..count)
            .map(|i| {
                let mut key = [0u8; 32];
                key[0] = i as u8;
                PublicKey::new(key)
            })
            .collect()
    }

    #[test]
    fn test_consensus_engine_creation() {
        let validators = create_test_validators(3);
        let engine = ConsensusEngine::new_poa(validators, 0.67);
        
        assert!(engine.is_ok());
        let engine = engine.unwrap();
        assert_eq!(engine.validator_set().validator_count(), 3);
    }

    #[test]
    fn test_empty_validator_set() {
        let result = ConsensusEngine::new_poa(vec![], 0.67);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_block() {
        let validators = create_test_validators(3);
        let engine = ConsensusEngine::new_poa(validators.clone(), 0.67).unwrap();
        
        let genesis = GenesisBlock::new(validators, 1000000000);
        let block = genesis.create_block().unwrap();
        
        assert!(engine.validate_block(&block).is_ok());
    }

    #[test]
    fn test_add_remove_validator() {
        let validators = create_test_validators(2);
        let mut engine = ConsensusEngine::new_poa(validators, 0.67).unwrap();
        
        let new_validator = PublicKey::new([99u8; 32]);
        assert!(engine.add_validator(new_validator).is_ok());
        assert_eq!(engine.validator_set().validator_count(), 3);
        
        assert!(engine.remove_validator(&new_validator).is_ok());
        assert_eq!(engine.validator_set().validator_count(), 2);
    }
}

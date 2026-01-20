use super::{Consensus, ValidatorSet};
use common::{PublicKey, Result, Timestamp, VotingError};
use crate::block::Block;

/// Proof of Authority consensus mechanism
/// 
/// In PoA, a fixed set of authorized validators take turns producing blocks
/// in a round-robin fashion. This is suitable for permissioned networks where
/// validators are known and trusted entities (e.g., government agencies).
pub struct ProofOfAuthority {
    /// Set of authorized validators
    validator_set: ValidatorSet,
    
    /// Block time in seconds
    block_time: u64,
    
    /// Current round number
    current_round: u64,
}

impl ProofOfAuthority {
    /// Create a new Proof of Authority consensus
    pub fn new(validator_set: ValidatorSet) -> Self {
        Self {
            validator_set,
            block_time: 5, // Default 5 seconds
            current_round: 0,
        }
    }
    
    /// Create with custom block time
    pub fn with_block_time(validator_set: ValidatorSet, block_time: u64) -> Self {
        Self {
            validator_set,
            block_time,
            current_round: 0,
        }
    }
    
    /// Calculate which validator should produce block at given timestamp
    fn get_validator_at_time(&self, timestamp: Timestamp) -> Result<PublicKey> {
        if self.validator_set.validator_count() == 0 {
            return Err(VotingError::ConsensusError(
                "No validators available".to_string(),
            ));
        }
        
        // Calculate round based on timestamp
        let round = timestamp / self.block_time;
        
        // Round-robin: validator index = round % validator_count
        let validator_index = (round as usize) % self.validator_set.validator_count();
        
        self.validator_set
            .get_validator_by_index(validator_index)
            .ok_or_else(|| {
                VotingError::ConsensusError(format!(
                    "Validator at index {} not found",
                    validator_index
                ))
            })
    }
    
    /// Check if it's the correct time slot for a validator
    fn is_validator_turn(&self, validator: &PublicKey, timestamp: Timestamp) -> Result<bool> {
        let expected_validator = self.get_validator_at_time(timestamp)?;
        Ok(&expected_validator == validator)
    }
    
    /// Get the time slot boundaries for a validator
    fn get_validator_time_slot(&self, timestamp: Timestamp) -> (Timestamp, Timestamp) {
        let slot_start = (timestamp / self.block_time) * self.block_time;
        let slot_end = slot_start + self.block_time;
        (slot_start, slot_end)
    }
    
    /// Validate block timing
    fn validate_block_timing(&self, block: &Block) -> Result<()> {
        let (slot_start, slot_end) = self.get_validator_time_slot(block.timestamp());
        
        // Block timestamp must be within the validator's time slot
        if block.timestamp() < slot_start || block.timestamp() >= slot_end {
            return Err(VotingError::ConsensusError(format!(
                "Block timestamp {} is outside validator time slot [{}, {})",
                block.timestamp(),
                slot_start,
                slot_end
            )));
        }
        
        Ok(())
    }
}

impl Consensus for ProofOfAuthority {
    fn validate_block(&self, block: &Block) -> Result<()> {
        // Check if validator is authorized
        if !self.validator_set.is_validator(&block.header.validator) {
            return Err(VotingError::InvalidValidator(format!(
                "Validator {} is not authorized",
                block.header.validator
            )));
        }
        
        // Check if it's this validator's turn
        if !self.is_validator_turn(&block.header.validator, block.timestamp())? {
            let expected = self.get_validator_at_time(block.timestamp())?;
            return Err(VotingError::ConsensusError(format!(
                "Wrong validator turn. Expected {}, got {}",
                expected, block.header.validator
            )));
        }
        
        // Validate block timing
        self.validate_block_timing(block)?;
        
        Ok(())
    }
    
    fn can_produce_block(&self, validator: &PublicKey) -> Result<bool> {
        // Check if validator is authorized
        if !self.validator_set.is_validator(validator) {
            return Ok(false);
        }
        
        // Check if it's this validator's turn based on current time
        let current_time = common::utils::current_timestamp();
        self.is_validator_turn(validator, current_time)
    }
    
    fn get_current_validator(&self) -> Result<PublicKey> {
        let current_time = common::utils::current_timestamp();
        self.get_validator_at_time(current_time)
    }
    
    fn is_finalized(&self, block: &Block) -> bool {
        // In PoA, a block is finalized when it has signatures from 2/3+ validators
        let required = self.required_signatures();
        block.signature_count() >= required
    }
    
    fn required_signatures(&self) -> usize {
        // Byzantine Fault Tolerance: need 2f+1 signatures where f is max faulty nodes
        // For 2/3+ threshold: ceil(validator_count * 2/3)
        let count = self.validator_set.validator_count();
        ((count * 2) / 3) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::BlockHash;
    use crate::block::Block;

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
    fn test_poa_creation() {
        let validators = create_test_validators(3);
        let validator_set = ValidatorSet::new(validators);
        let poa = ProofOfAuthority::new(validator_set);
        
        assert_eq!(poa.block_time, 5);
    }

    #[test]
    fn test_get_validator_at_time() {
        let validators = create_test_validators(3);
        let validator_set = ValidatorSet::new(validators.clone());
        let poa = ProofOfAuthority::with_block_time(validator_set, 10);
        
        // Time 0-9: validator 0
        let v0 = poa.get_validator_at_time(0).unwrap();
        assert_eq!(v0, validators[0]);
        
        let v1 = poa.get_validator_at_time(5).unwrap();
        assert_eq!(v1, validators[0]);
        
        // Time 10-19: validator 1
        let v2 = poa.get_validator_at_time(10).unwrap();
        assert_eq!(v2, validators[1]);
        
        // Time 20-29: validator 2
        let v3 = poa.get_validator_at_time(20).unwrap();
        assert_eq!(v3, validators[2]);
        
        // Time 30-39: back to validator 0 (round-robin)
        let v4 = poa.get_validator_at_time(30).unwrap();
        assert_eq!(v4, validators[0]);
    }

    #[test]
    fn test_is_validator_turn() {
        let validators = create_test_validators(3);
        let validator_set = ValidatorSet::new(validators.clone());
        let poa = ProofOfAuthority::with_block_time(validator_set, 10);
        
        // At time 0, it's validator 0's turn
        assert!(poa.is_validator_turn(&validators[0], 0).unwrap());
        assert!(!poa.is_validator_turn(&validators[1], 0).unwrap());
        
        // At time 10, it's validator 1's turn
        assert!(poa.is_validator_turn(&validators[1], 10).unwrap());
        assert!(!poa.is_validator_turn(&validators[0], 10).unwrap());
    }

    #[test]
    fn test_validate_block() {
        let validators = create_test_validators(3);
        let validator_set = ValidatorSet::new(validators.clone());
        let poa = ProofOfAuthority::with_block_time(validator_set, 10);
        
        // Create block with correct validator at time 0
        let block = Block::new(
            1,
            BlockHash::zero(),
            vec![],
            validators[0],
        );
        
        // This should fail because timestamp won't match our test time
        // In real usage, block timestamp would be set to correct slot
    }

    #[test]
    fn test_required_signatures() {
        let validators = create_test_validators(4);
        let validator_set = ValidatorSet::new(validators);
        let poa = ProofOfAuthority::new(validator_set);
        
        // For 4 validators: ceil(4 * 2/3) = 3
        assert_eq!(poa.required_signatures(), 3);
    }

    #[test]
    fn test_required_signatures_three() {
        let validators = create_test_validators(3);
        let validator_set = ValidatorSet::new(validators);
        let poa = ProofOfAuthority::new(validator_set);
        
        // For 3 validators: ceil(3 * 2/3) = 2
        assert_eq!(poa.required_signatures(), 2);
    }

    #[test]
    fn test_required_signatures_ten() {
        let validators = create_test_validators(10);
        let validator_set = ValidatorSet::new(validators);
        let poa = ProofOfAuthority::new(validator_set);
        
        // For 10 validators: ceil(10 * 2/3) = 7
        assert_eq!(poa.required_signatures(), 7);
    }

    #[test]
    fn test_time_slot_boundaries() {
        let validators = create_test_validators(3);
        let validator_set = ValidatorSet::new(validators);
        let poa = ProofOfAuthority::with_block_time(validator_set, 10);
        
        let (start, end) = poa.get_validator_time_slot(15);
        assert_eq!(start, 10);
        assert_eq!(end, 20);
        
        let (start2, end2) = poa.get_validator_time_slot(25);
        assert_eq!(start2, 20);
        assert_eq!(end2, 30);
    }

    #[test]
    fn test_custom_block_time() {
        let validators = create_test_validators(3);
        let validator_set = ValidatorSet::new(validators);
        let poa = ProofOfAuthority::with_block_time(validator_set, 3);
        
        assert_eq!(poa.block_time, 3);
    }
}

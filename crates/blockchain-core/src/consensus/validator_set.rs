use common::{PublicKey, Result, Timestamp, VotingError};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Set of validators authorized to participate in consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorSet {
    /// List of validator public keys (ordered)
    validators: Vec<PublicKey>,
    
    /// Map for quick validator lookup
    validator_map: HashMap<PublicKey, ValidatorInfo>,
    
    /// Set for O(1) membership check
    validator_set: HashSet<PublicKey>,
}

/// Information about a validator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorInfo {
    /// Validator's public key
    pub public_key: PublicKey,
    
    /// When this validator was added
    pub added_at: Timestamp,
    
    /// Number of blocks produced by this validator
    pub blocks_produced: u64,
    
    /// Number of blocks validated (signed) by this validator
    pub blocks_validated: u64,
    
    /// Whether validator is currently active
    pub is_active: bool,
}

impl ValidatorSet {
    /// Create a new validator set
    pub fn new(validators: Vec<PublicKey>) -> Self {
        let current_time = common::utils::current_timestamp();
        let mut validator_map = HashMap::new();
        let mut validator_set = HashSet::new();
        
        for validator in &validators {
            let info = ValidatorInfo {
                public_key: *validator,
                added_at: current_time,
                blocks_produced: 0,
                blocks_validated: 0,
                is_active: true,
            };
            validator_map.insert(*validator, info);
            validator_set.insert(*validator);
        }
        
        Self {
            validators,
            validator_map,
            validator_set,
        }
    }
    
    /// Create an empty validator set
    pub fn empty() -> Self {
        Self {
            validators: Vec::new(),
            validator_map: HashMap::new(),
            validator_set: HashSet::new(),
        }
    }
    
    /// Add a new validator
    pub fn add_validator(&mut self, validator: PublicKey) -> Result<()> {
        if self.is_validator(&validator) {
            return Err(VotingError::ConsensusError(format!(
                "Validator {} already exists",
                validator
            )));
        }
        
        let info = ValidatorInfo {
            public_key: validator,
            added_at: common::utils::current_timestamp(),
            blocks_produced: 0,
            blocks_validated: 0,
            is_active: true,
        };
        
        self.validators.push(validator);
        self.validator_map.insert(validator, info);
        self.validator_set.insert(validator);
        
        Ok(())
    }
    
    /// Remove a validator
    pub fn remove_validator(&mut self, validator: &PublicKey) -> Result<()> {
        if !self.is_validator(validator) {
            return Err(VotingError::InvalidValidator(format!(
                "Validator {} not found",
                validator
            )));
        }
        
        self.validators.retain(|v| v != validator);
        self.validator_map.remove(validator);
        self.validator_set.remove(validator);
        
        Ok(())
    }
    
    /// Check if a public key is a validator
    pub fn is_validator(&self, validator: &PublicKey) -> bool {
        self.validator_set.contains(validator)
    }
    
    /// Get validator count
    pub fn validator_count(&self) -> usize {
        self.validators.len()
    }
    
    /// Get all validators
    pub fn validators(&self) -> &[PublicKey] {
        &self.validators
    }
    
    /// Get validator by index (for round-robin)
    pub fn get_validator_by_index(&self, index: usize) -> Option<PublicKey> {
        self.validators.get(index).copied()
    }
    
    /// Get validator info
    pub fn get_validator_info(&self, validator: &PublicKey) -> Option<&ValidatorInfo> {
        self.validator_map.get(validator)
    }
    
    /// Get mutable validator info
    pub fn get_validator_info_mut(&mut self, validator: &PublicKey) -> Option<&mut ValidatorInfo> {
        self.validator_map.get_mut(validator)
    }
    
    /// Increment blocks produced for a validator
    pub fn record_block_produced(&mut self, validator: &PublicKey) -> Result<()> {
        let info = self.get_validator_info_mut(validator)
            .ok_or_else(|| VotingError::InvalidValidator(format!(
                "Validator {} not found",
                validator
            )))?;
        
        info.blocks_produced += 1;
        Ok(())
    }
    
    /// Increment blocks validated for a validator
    pub fn record_block_validated(&mut self, validator: &PublicKey) -> Result<()> {
        let info = self.get_validator_info_mut(validator)
            .ok_or_else(|| VotingError::InvalidValidator(format!(
                "Validator {} not found",
                validator
            )))?;
        
        info.blocks_validated += 1;
        Ok(())
    }
    
    /// Set validator active status
    pub fn set_validator_active(&mut self, validator: &PublicKey, active: bool) -> Result<()> {
        let info = self.get_validator_info_mut(validator)
            .ok_or_else(|| VotingError::InvalidValidator(format!(
                "Validator {} not found",
                validator
            )))?;
        
        info.is_active = active;
        Ok(())
    }
    
    /// Get all active validators
    pub fn active_validators(&self) -> Vec<PublicKey> {
        self.validators
            .iter()
            .filter(|v| {
                self.validator_map
                    .get(v)
                    .map(|info| info.is_active)
                    .unwrap_or(false)
            })
            .copied()
            .collect()
    }
    
    /// Get active validator count
    pub fn active_validator_count(&self) -> usize {
        self.active_validators().len()
    }
    
    /// Calculate total blocks produced by all validators
    pub fn total_blocks_produced(&self) -> u64 {
        self.validator_map
            .values()
            .map(|info| info.blocks_produced)
            .sum()
    }
    
    /// Calculate total blocks validated by all validators
    pub fn total_blocks_validated(&self) -> u64 {
        self.validator_map
            .values()
            .map(|info| info.blocks_validated)
            .sum()
    }
    
    /// Get validator statistics
    pub fn get_stats(&self) -> ValidatorSetStats {
        let total_validators = self.validator_count();
        let active_validators = self.active_validator_count();
        let total_blocks_produced = self.total_blocks_produced();
        let total_blocks_validated = self.total_blocks_validated();
        
        ValidatorSetStats {
            total_validators,
            active_validators,
            inactive_validators: total_validators - active_validators,
            total_blocks_produced,
            total_blocks_validated,
        }
    }
}

/// Statistics about the validator set
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorSetStats {
    pub total_validators: usize,
    pub active_validators: usize,
    pub inactive_validators: usize,
    pub total_blocks_produced: u64,
    pub total_blocks_validated: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_validator_set_creation() {
        let validators = create_test_validators(3);
        let set = ValidatorSet::new(validators.clone());
        
        assert_eq!(set.validator_count(), 3);
        assert!(set.is_validator(&validators[0]));
        assert!(set.is_validator(&validators[1]));
        assert!(set.is_validator(&validators[2]));
    }

    #[test]
    fn test_empty_validator_set() {
        let set = ValidatorSet::empty();
        assert_eq!(set.validator_count(), 0);
    }

    #[test]
    fn test_add_validator() {
        let validators = create_test_validators(2);
        let mut set = ValidatorSet::new(validators);
        
        let new_validator = PublicKey::new([99u8; 32]);
        assert!(set.add_validator(new_validator).is_ok());
        
        assert_eq!(set.validator_count(), 3);
        assert!(set.is_validator(&new_validator));
    }

    #[test]
    fn test_add_duplicate_validator() {
        let validators = create_test_validators(2);
        let mut set = ValidatorSet::new(validators.clone());
        
        let result = set.add_validator(validators[0]);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_validator() {
        let validators = create_test_validators(3);
        let mut set = ValidatorSet::new(validators.clone());
        
        assert!(set.remove_validator(&validators[1]).is_ok());
        
        assert_eq!(set.validator_count(), 2);
        assert!(!set.is_validator(&validators[1]));
        assert!(set.is_validator(&validators[0]));
        assert!(set.is_validator(&validators[2]));
    }

    #[test]
    fn test_remove_nonexistent_validator() {
        let validators = create_test_validators(2);
        let mut set = ValidatorSet::new(validators);
        
        let nonexistent = PublicKey::new([99u8; 32]);
        let result = set.remove_validator(&nonexistent);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_validator_by_index() {
        let validators = create_test_validators(3);
        let set = ValidatorSet::new(validators.clone());
        
        assert_eq!(set.get_validator_by_index(0), Some(validators[0]));
        assert_eq!(set.get_validator_by_index(1), Some(validators[1]));
        assert_eq!(set.get_validator_by_index(2), Some(validators[2]));
        assert_eq!(set.get_validator_by_index(3), None);
    }

    #[test]
    fn test_validator_info() {
        let validators = create_test_validators(2);
        let set = ValidatorSet::new(validators.clone());
        
        let info = set.get_validator_info(&validators[0]);
        assert!(info.is_some());
        
        let info = info.unwrap();
        assert_eq!(info.public_key, validators[0]);
        assert_eq!(info.blocks_produced, 0);
        assert_eq!(info.blocks_validated, 0);
        assert!(info.is_active);
    }

    #[test]
    fn test_record_block_produced() {
        let validators = create_test_validators(2);
        let mut set = ValidatorSet::new(validators.clone());
        
        assert!(set.record_block_produced(&validators[0]).is_ok());
        assert!(set.record_block_produced(&validators[0]).is_ok());
        
        let info = set.get_validator_info(&validators[0]).unwrap();
        assert_eq!(info.blocks_produced, 2);
    }

    #[test]
    fn test_record_block_validated() {
        let validators = create_test_validators(2);
        let mut set = ValidatorSet::new(validators.clone());
        
        assert!(set.record_block_validated(&validators[1]).is_ok());
        assert!(set.record_block_validated(&validators[1]).is_ok());
        assert!(set.record_block_validated(&validators[1]).is_ok());
        
        let info = set.get_validator_info(&validators[1]).unwrap();
        assert_eq!(info.blocks_validated, 3);
    }

    #[test]
    fn test_set_validator_active() {
        let validators = create_test_validators(2);
        let mut set = ValidatorSet::new(validators.clone());
        
        assert!(set.set_validator_active(&validators[0], false).is_ok());
        
        let info = set.get_validator_info(&validators[0]).unwrap();
        assert!(!info.is_active);
    }

    #[test]
    fn test_active_validators() {
        let validators = create_test_validators(3);
        let mut set = ValidatorSet::new(validators.clone());
        
        set.set_validator_active(&validators[1], false).unwrap();
        
        let active = set.active_validators();
        assert_eq!(active.len(), 2);
        assert!(active.contains(&validators[0]));
        assert!(!active.contains(&validators[1]));
        assert!(active.contains(&validators[2]));
    }

    #[test]
    fn test_active_validator_count() {
        let validators = create_test_validators(4);
        let mut set = ValidatorSet::new(validators.clone());
        
        assert_eq!(set.active_validator_count(), 4);
        
        set.set_validator_active(&validators[0], false).unwrap();
        set.set_validator_active(&validators[2], false).unwrap();
        
        assert_eq!(set.active_validator_count(), 2);
    }

    #[test]
    fn test_validator_stats() {
        let validators = create_test_validators(3);
        let mut set = ValidatorSet::new(validators.clone());
        
        set.record_block_produced(&validators[0]).unwrap();
        set.record_block_produced(&validators[0]).unwrap();
        set.record_block_validated(&validators[1]).unwrap();
        set.set_validator_active(&validators[2], false).unwrap();
        
        let stats = set.get_stats();
        assert_eq!(stats.total_validators, 3);
        assert_eq!(stats.active_validators, 2);
        assert_eq!(stats.inactive_validators, 1);
        assert_eq!(stats.total_blocks_produced, 2);
        assert_eq!(stats.total_blocks_validated, 1);
    }
}

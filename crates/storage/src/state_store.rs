use crate::database::Database;
use crate::jurisdiction::JurisdictionPath;
use common::{
    BlockHash, BlockHeight, ElectionId, PublicKey, Result, Timestamp, TxId, VotingError,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

/// State store for tracking current blockchain state
/// Separate from blockchain store for fast access to current state
#[derive(Clone)]
pub struct StateStore {
    /// Underlying database
    db: Arc<dyn Database>,
    
    /// In-memory cache for fast access
    cache: Arc<RwLock<StateCache>>,
}

/// In-memory cache of current state
struct StateCache {
    /// Current blockchain height
    current_height: BlockHeight,
    
    /// Latest block hash
    latest_block_hash: BlockHash,
    
    /// Active validators
    active_validators: HashSet<PublicKey>,
    
    /// Active elections
    active_elections: HashMap<ElectionId, ElectionState>,
    
    /// Transaction pool (pending transactions)
    tx_pool: HashMap<TxId, PendingTransaction>,
    
    /// Voter participation tracking
    voter_participation: HashMap<ElectionId, HashSet<VoterIdentifier>>,
}

/// State of an election
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectionState {
    /// Election ID
    pub id: ElectionId,
    
    /// Election name
    pub name: String,
    
    /// Election description
    pub description: String,
    
    /// Jurisdiction path
    pub jurisdiction_path: JurisdictionPath,
    
    /// List of candidates
    pub candidates: Vec<CandidateInfo>,
    
    /// Start timestamp
    pub start_time: Timestamp,
    
    /// End timestamp
    pub end_time: Timestamp,
    
    /// Current vote count
    pub total_votes: u64,
    
    /// Is election active
    pub is_active: bool,
    
    /// Is election finalized
    pub is_finalized: bool,
    
    /// Block height when created
    pub created_at_height: BlockHeight,
    
    /// Block height when closed (if closed)
    pub closed_at_height: Option<BlockHeight>,
}

/// Candidate information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateInfo {
    pub id: String,
    pub name: String,
    pub party: Option<String>,
    pub metadata: Option<String>,
}

/// Pending transaction in the pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTransaction {
    /// Transaction ID
    pub tx_id: TxId,
    
    /// Transaction data (serialized)
    pub data: Vec<u8>,
    
    /// When transaction was added to pool
    pub added_at: Timestamp,
    
    /// Priority (higher = more important)
    pub priority: u32,
}

/// Voter identifier (anonymized)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VoterIdentifier(pub [u8; 32]);

impl VoterIdentifier {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    
    pub fn from_signature(signature: &common::Signature) -> Self {
        use blake2::{Blake2b, Digest};
        use blake2::digest::consts::U32;
        
        let mut hasher = Blake2b::<U32>::new();
        hasher.update(signature.as_bytes());
        let result = hasher.finalize();
        
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&result);
        Self(bytes)
    }
}

impl StateStore {
    /// Create a new state store
    pub fn new(db: Arc<dyn Database>) -> Result<Self> {
        let cache = Arc::new(RwLock::new(StateCache {
            current_height: 0,
            latest_block_hash: BlockHash::zero(),
            active_validators: HashSet::new(),
            active_elections: HashMap::new(),
            tx_pool: HashMap::new(),
            voter_participation: HashMap::new(),
        }));
        
        let store = Self { db, cache };
        store.load_state_from_db()?;
        Ok(store)
    }
    
    /// Load state from database into cache
    fn load_state_from_db(&self) -> Result<()> {
        // Load current height
        let height_key = b"state:current_height";
        if let Some(height_bytes) = self.db.get(height_key)? {
            let height = u64::from_le_bytes(
                height_bytes
                    .try_into()
                    .map_err(|_| VotingError::StorageError("Invalid height bytes".to_string()))?,
            );
            
            let mut cache = self.cache.write().unwrap();
            cache.current_height = height;
        }
        
        // Load latest block hash
        let hash_key = b"state:latest_block_hash";
        if let Some(hash_bytes) = self.db.get(hash_key)? {
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&hash_bytes);
            
            let mut cache = self.cache.write().unwrap();
            cache.latest_block_hash = BlockHash::new(hash);
        }
        
        // Load active validators
        self.load_validators()?;
        
        // Load active elections
        self.load_active_elections()?;
        
        Ok(())
    }
    
    /// Load validators from database
    fn load_validators(&self) -> Result<()> {
        let prefix = b"state:validator:";
        let keys = self.db.keys_with_prefix(prefix)?;
        
        let mut cache = self.cache.write().unwrap();
        cache.active_validators.clear();
        
        for key in keys {
            // Extract public key from key
            if key.len() == prefix.len() + 32 {
                let mut pk_bytes = [0u8; 32];
                pk_bytes.copy_from_slice(&key[prefix.len()..]);
                cache.active_validators.insert(PublicKey::new(pk_bytes));
            }
        }
        
        Ok(())
    }
    
    /// Load active elections from database
    fn load_active_elections(&self) -> Result<()> {
        let prefix = b"state:election:";
        let keys = self.db.keys_with_prefix(prefix)?;
        
        let mut cache = self.cache.write().unwrap();
        cache.active_elections.clear();
        
        for key in keys {
            if let Some(value) = self.db.get(&key)? {
                let election: ElectionState = common::utils::deserialize(&value)?;
                if election.is_active {
                    cache.active_elections.insert(election.id, election);
                }
            }
        }
        
        Ok(())
    }
    
    /// Get current blockchain height
    pub fn get_current_height(&self) -> BlockHeight {
        self.cache.read().unwrap().current_height
    }
    
    /// Get latest block hash
    pub fn get_latest_block_hash(&self) -> BlockHash {
        self.cache.read().unwrap().latest_block_hash
    }
    
    /// Update current height and block hash
    pub fn update_chain_tip(&self, height: BlockHeight, block_hash: BlockHash) -> Result<()> {
        // Update cache
        {
            let mut cache = self.cache.write().unwrap();
            cache.current_height = height;
            cache.latest_block_hash = block_hash;
        }
        
        // Persist to database
        let height_key = b"state:current_height";
        self.db.put(height_key, &height.to_le_bytes())?;
        
        let hash_key = b"state:latest_block_hash";
        self.db.put(hash_key, block_hash.as_bytes())?;
        
        Ok(())
    }
    
    /// Add a validator to the active set
    pub fn add_validator(&self, validator: PublicKey) -> Result<()> {
        // Update cache
        {
            let mut cache = self.cache.write().unwrap();
            cache.active_validators.insert(validator);
        }
        
        // Persist to database
        let key = format!("state:validator:{}", validator.to_hex());
        self.db.put(key.as_bytes(), &[1])?;
        
        Ok(())
    }
    
    /// Remove a validator from the active set
    pub fn remove_validator(&self, validator: &PublicKey) -> Result<()> {
        // Update cache
        {
            let mut cache = self.cache.write().unwrap();
            cache.active_validators.remove(validator);
        }
        
        // Remove from database
        let key = format!("state:validator:{}", validator.to_hex());
        self.db.delete(key.as_bytes())?;
        
        Ok(())
    }
    
    /// Get all active validators
    pub fn get_active_validators(&self) -> Vec<PublicKey> {
        self.cache
            .read()
            .unwrap()
            .active_validators
            .iter()
            .copied()
            .collect()
    }
    
    /// Check if a validator is active
    pub fn is_validator_active(&self, validator: &PublicKey) -> bool {
        self.cache
            .read()
            .unwrap()
            .active_validators
            .contains(validator)
    }
    
    /// Get number of active validators
    pub fn validator_count(&self) -> usize {
        self.cache.read().unwrap().active_validators.len()
    }
    
    /// Create a new election
    pub fn create_election(&self, election: ElectionState) -> Result<()> {
        // Update cache
        {
            let mut cache = self.cache.write().unwrap();
            cache.active_elections.insert(election.id, election.clone());
            cache.voter_participation.insert(election.id, HashSet::new());
        }
        
        // Persist to database
        let key = format!("state:election:{}", election.id.to_hex());
        let data = common::utils::serialize(&election)?;
        self.db.put(key.as_bytes(), &data)?;
        
        Ok(())
    }
    
    /// Close an election
    pub fn close_election(
        &self,
        election_id: &ElectionId,
        closed_at_height: BlockHeight,
    ) -> Result<()> {
        // Update cache
        {
            let mut cache = self.cache.write().unwrap();
            if let Some(election) = cache.active_elections.get_mut(election_id) {
                election.is_active = false;
                election.is_finalized = true;
                election.closed_at_height = Some(closed_at_height);
            }
        }
        
        // Update database
        if let Some(mut election) = self.get_election(election_id)? {
            election.is_active = false;
            election.is_finalized = true;
            election.closed_at_height = Some(closed_at_height);
            
            let key = format!("state:election:{}", election_id.to_hex());
            let data = common::utils::serialize(&election)?;
            self.db.put(key.as_bytes(), &data)?;
        }
        
        Ok(())
    }
    
    /// Get election state
    pub fn get_election(&self, election_id: &ElectionId) -> Result<Option<ElectionState>> {
        // Try cache first
        {
            let cache = self.cache.read().unwrap();
            if let Some(election) = cache.active_elections.get(election_id) {
                return Ok(Some(election.clone()));
            }
        }
        
        // Fall back to database
        let key = format!("state:election:{}", election_id.to_hex());
        if let Some(data) = self.db.get(key.as_bytes())? {
            let election: ElectionState = common::utils::deserialize(&data)?;
            Ok(Some(election))
        } else {
            Ok(None)
        }
    }
    
    /// Get all elections (for jurisdiction queries)
    pub fn get_all_elections(&self) -> Result<Vec<ElectionState>> {
        let prefix = b"state:election:";
        let keys = self.db.keys_with_prefix(prefix)?;
        
        let mut elections = Vec::new();
        for key in keys {
            if let Some(data) = self.db.get(&key)? {
                let election: ElectionState = common::utils::deserialize(&data)?;
                elections.push(election);
            }
        }
        
        Ok(elections)
    }
    
    /// Get all active elections
    pub fn get_active_elections(&self) -> Vec<ElectionState> {
        self.cache
            .read()
            .unwrap()
            .active_elections
            .values()
            .cloned()
            .collect()
    }
    
    /// Check if election is active
    pub fn is_election_active(&self, election_id: &ElectionId) -> bool {
        self.cache
            .read()
            .unwrap()
            .active_elections
            .get(election_id)
            .map(|e| e.is_active)
            .unwrap_or(false)
    }
    
    /// Increment vote count for an election
    pub fn increment_vote_count(&self, election_id: &ElectionId) -> Result<()> {
        // Update cache
        {
            let mut cache = self.cache.write().unwrap();
            if let Some(election) = cache.active_elections.get_mut(election_id) {
                election.total_votes += 1;
            }
        }
        
        // Update database
        if let Some(mut election) = self.get_election(election_id)? {
            election.total_votes += 1;
            
            let key = format!("state:election:{}", election_id.to_hex());
            let data = common::utils::serialize(&election)?;
            self.db.put(key.as_bytes(), &data)?;
        }
        
        Ok(())
    }
    
    /// Record voter participation
    pub fn record_voter_participation(
        &self,
        election_id: &ElectionId,
        voter_id: VoterIdentifier,
    ) -> Result<bool> {
        let mut cache = self.cache.write().unwrap();
        
        if let Some(voters) = cache.voter_participation.get_mut(election_id) {
            let is_new = voters.insert(voter_id);
            Ok(is_new)
        } else {
            Ok(false)
        }
    }
    
    /// Check if voter has already voted
    pub fn has_voter_participated(
        &self,
        election_id: &ElectionId,
        voter_id: &VoterIdentifier,
    ) -> bool {
        self.cache
            .read()
            .unwrap()
            .voter_participation
            .get(election_id)
            .map(|voters| voters.contains(voter_id))
            .unwrap_or(false)
    }
    
    /// Add transaction to mempool
    pub fn add_pending_transaction(&self, tx: PendingTransaction) -> Result<()> {
        let mut cache = self.cache.write().unwrap();
        cache.tx_pool.insert(tx.tx_id, tx);
        Ok(())
    }
    
    /// Remove transaction from mempool
    pub fn remove_pending_transaction(&self, tx_id: &TxId) -> Result<()> {
        let mut cache = self.cache.write().unwrap();
        cache.tx_pool.remove(tx_id);
        Ok(())
    }
    
    /// Get pending transaction
    pub fn get_pending_transaction(&self, tx_id: &TxId) -> Option<PendingTransaction> {
        self.cache
            .read()
            .unwrap()
            .tx_pool
            .get(tx_id)
            .cloned()
    }
    
    /// Get all pending transactions
    pub fn get_pending_transactions(&self) -> Vec<PendingTransaction> {
        self.cache
            .read()
            .unwrap()
            .tx_pool
            .values()
            .cloned()
            .collect()
    }
    
    /// Clear old pending transactions
    pub fn prune_pending_transactions(&self, cutoff_time: Timestamp) -> Result<usize> {
        let mut cache = self.cache.write().unwrap();
        let mut removed_count = 0;
        
        cache.tx_pool.retain(|_, tx| {
            let keep = tx.added_at >= cutoff_time;
            if !keep {
                removed_count += 1;
            }
            keep
        });
        
        Ok(removed_count)
    }
    
    /// Get state statistics
    pub fn get_stats(&self) -> StateStats {
        let cache = self.cache.read().unwrap();
        
        StateStats {
            current_height: cache.current_height,
            latest_block_hash: cache.latest_block_hash,
            active_validators: cache.active_validators.len(),
            active_elections: cache.active_elections.len(),
            pending_transactions: cache.tx_pool.len(),
            total_votes_tracked: cache
                .voter_participation
                .values()
                .map(|v| v.len())
                .sum(),
        }
    }
    
    /// Clear all state (for testing)
    #[cfg(test)]
    pub fn clear(&self) -> Result<()> {
        let mut cache = self.cache.write().unwrap();
        cache.current_height = 0;
        cache.latest_block_hash = BlockHash::zero();
        cache.active_validators.clear();
        cache.active_elections.clear();
        cache.tx_pool.clear();
        cache.voter_participation.clear();
        Ok(())
    }
}

/// State statistics
#[derive(Debug, Clone)]
pub struct StateStats {
    pub current_height: BlockHeight,
    pub latest_block_hash: BlockHash,
    pub active_validators: usize,
    pub active_elections: usize,
    pub pending_transactions: usize,
    pub total_votes_tracked: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_state_store() -> StateStore {
        let db: Arc<dyn Database> = Arc::new(crate::database::MemoryDatabase::new());
        StateStore::new(db).unwrap()
    }

    #[test]
    fn test_state_store_creation() {
        let store = create_test_state_store();
        assert_eq!(store.get_current_height(), 0);
        assert_eq!(store.get_latest_block_hash(), BlockHash::zero());
    }

    #[test]
    fn test_update_chain_tip() {
        let store = create_test_state_store();
        
        let hash = BlockHash::new([1u8; 32]);
        store.update_chain_tip(100, hash).unwrap();
        
        assert_eq!(store.get_current_height(), 100);
        assert_eq!(store.get_latest_block_hash(), hash);
    }

    #[test]
    fn test_validator_management() {
        let store = create_test_state_store();
        
        let validator1 = PublicKey::new([1u8; 32]);
        let validator2 = PublicKey::new([2u8; 32]);
        
        store.add_validator(validator1).unwrap();
        store.add_validator(validator2).unwrap();
        
        assert_eq!(store.validator_count(), 2);
        assert!(store.is_validator_active(&validator1));
        assert!(store.is_validator_active(&validator2));
        
        store.remove_validator(&validator1).unwrap();
        assert_eq!(store.validator_count(), 1);
        assert!(!store.is_validator_active(&validator1));
    }

    #[test]
    fn test_election_management() {
        let store = create_test_state_store();
        
        let election_id = ElectionId::new([1u8; 16]);
        let election = ElectionState {
            id: election_id,
            name: "Test Election".to_string(),
            description: "Test".to_string(),
            jurisdiction_path: JurisdictionPath::parse("US/Test").unwrap(),
            candidates: vec![],
            start_time: 1000,
            end_time: 2000,
            total_votes: 0,
            is_active: true,
            is_finalized: false,
            created_at_height: 100,
            closed_at_height: None,
        };
        
        store.create_election(election.clone()).unwrap();
        
        assert!(store.is_election_active(&election_id));
        assert_eq!(store.get_active_elections().len(), 1);
        
        let retrieved = store.get_election(&election_id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test Election");
    }

    #[test]
    fn test_close_election() {
        let store = create_test_state_store();
        
        let election_id = ElectionId::new([1u8; 16]);
        let election = ElectionState {
            id: election_id,
            name: "Test Election".to_string(),
            description: "Test".to_string(),
            jurisdiction_path: JurisdictionPath::parse("US/Test").unwrap(),
            candidates: vec![],
            start_time: 1000,
            end_time: 2000,
            total_votes: 0,
            is_active: true,
            is_finalized: false,
            created_at_height: 100,
            closed_at_height: None,
        };
        
        store.create_election(election).unwrap();
        store.close_election(&election_id, 200).unwrap();
        
        assert!(!store.is_election_active(&election_id));
        
        let closed = store.get_election(&election_id).unwrap().unwrap();
        assert!(!closed.is_active);
        assert!(closed.is_finalized);
        assert_eq!(closed.closed_at_height, Some(200));
    }

    #[test]
    fn test_voter_participation() {
        let store = create_test_state_store();
        
        let election_id = ElectionId::new([1u8; 16]);
        let election = ElectionState {
            id: election_id,
            name: "Test".to_string(),
            description: "Test".to_string(),
            jurisdiction_path: JurisdictionPath::parse("US/Test").unwrap(),
            candidates: vec![],
            start_time: 1000,
            end_time: 2000,
            total_votes: 0,
            is_active: true,
            is_finalized: false,
            created_at_height: 100,
            closed_at_height: None,
        };
        
        store.create_election(election).unwrap();
        
        let voter1 = VoterIdentifier::new([1u8; 32]);
        let voter2 = VoterIdentifier::new([2u8; 32]);
        
        // First vote should succeed
        assert!(store.record_voter_participation(&election_id, voter1).unwrap());
        assert!(store.has_voter_participated(&election_id, &voter1));
        
        // Duplicate vote should return false
        assert!(!store.record_voter_participation(&election_id, voter1).unwrap());
        
        // Different voter should succeed
        assert!(store.record_voter_participation(&election_id, voter2).unwrap());
    }
}

use common::{Address, ElectionId, PublicKey, Result, Signature, Timestamp, VotingError};
use crypto::keys::KeyPair;
use crypto::signatures::{sign, verify};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voter {
    pub id: VoterId,
    pub public_key: PublicKey,
    pub address: Address,
    pub status: VoterStatus,
    pub credentials: Option<VoterCredentials>,
    pub registered_at: Timestamp,
    pub elections_voted: HashSet<ElectionId>,
}

impl Voter {
    pub fn new(keypair: &KeyPair) -> Self {
        let public_key = keypair.public_key_copy();
        let address = public_key.to_address();
        let id = VoterId::from_address(&address);

        Self {
            id,
            public_key: public_key.to_common(),
            address,
            status: VoterStatus::Unregistered,
            credentials: None,
            registered_at: common::utils::current_timestamp(),
            elections_voted: HashSet::new(),
        }
    }

    pub fn with_credentials(keypair: &KeyPair, credentials: VoterCredentials) -> Self {
        let mut voter = Self::new(keypair);
        voter.credentials = Some(credentials);
        voter.status = VoterStatus::Registered;
        voter
    }

    pub fn register(&mut self, credentials: VoterCredentials) -> Result<()> {
        if self.status == VoterStatus::Registered {
            return Err(VotingError::InvalidInput(
                "Voter already registered".to_string(),
            ));
        }

        self.credentials = Some(credentials);
        self.status = VoterStatus::Registered;
        Ok(())
    }

    pub fn is_registered(&self) -> bool {
        matches!(self.status, VoterStatus::Registered | VoterStatus::Verified)
    }

    pub fn is_eligible(&self, election_id: ElectionId) -> bool {
        self.is_registered() && !self.has_voted(election_id)
    }

    pub fn has_voted(&self, election_id: ElectionId) -> bool {
        self.elections_voted.contains(&election_id)
    }

    pub fn mark_voted(&mut self, election_id: ElectionId) -> Result<()> {
        if self.has_voted(election_id) {
            return Err(VotingError::AlreadyVoted);
        }

        self.elections_voted.insert(election_id);
        Ok(())
    }

    pub fn revoke(&mut self) {
        self.status = VoterStatus::Revoked;
    }

    pub fn verify(&mut self) {
        if self.status == VoterStatus::Registered {
            self.status = VoterStatus::Verified;
        }
    }

    pub fn suspend(&mut self, reason: String) {
        self.status = VoterStatus::Suspended { reason };
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VoterId(pub [u8; 32]);

impl VoterId {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn from_address(address: &Address) -> Self {
        Self(*address.as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| VotingError::CryptoError(format!("Invalid hex: {}", e)))?;

        if bytes.len() != 32 {
            return Err(VotingError::CryptoError(
                "Invalid voter ID length".to_string(),
            ));
        }

        let mut id = [0u8; 32];
        id.copy_from_slice(&bytes);
        Ok(Self(id))
    }
}

impl std::fmt::Display for VoterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoterStatus {
    Unregistered,
    Registered,
    Verified,
    Suspended { reason: String },
    Revoked,
}

impl std::fmt::Display for VoterStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VoterStatus::Unregistered => write!(f, "unregistered"),
            VoterStatus::Registered => write!(f, "registered"),
            VoterStatus::Verified => write!(f, "verified"),
            VoterStatus::Suspended { reason } => write!(f, "suspended: {}", reason),
            VoterStatus::Revoked => write!(f, "revoked"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoterCredentials {
    pub voter_id: VoterId,
    pub jurisdiction: String,
    pub issued_at: Timestamp,
    pub expires_at: Option<Timestamp>,
    pub credential_hash: Vec<u8>,
}

impl VoterCredentials {
    pub fn new(voter_id: VoterId, jurisdiction: String) -> Self {
        let issued_at = common::utils::current_timestamp();
        let credential_hash = Self::compute_hash(&voter_id, &jurisdiction, issued_at);

        Self {
            voter_id,
            jurisdiction,
            issued_at,
            expires_at: None,
            credential_hash,
        }
    }

    pub fn with_expiration(
        voter_id: VoterId,
        jurisdiction: String,
        expires_at: Timestamp,
    ) -> Self {
        let mut credentials = Self::new(voter_id, jurisdiction);
        credentials.expires_at = Some(expires_at);
        credentials
    }

    pub fn is_valid(&self, current_time: Timestamp) -> bool {
        if let Some(expires_at) = self.expires_at {
            current_time < expires_at
        } else {
            true
        }
    }

    pub fn is_expired(&self, current_time: Timestamp) -> bool {
        !self.is_valid(current_time)
    }

    fn compute_hash(voter_id: &VoterId, jurisdiction: &str, issued_at: Timestamp) -> Vec<u8> {
        crypto::hash::hash_blake2b_multiple(&[
            voter_id.as_bytes(),
            jurisdiction.as_bytes(),
            &issued_at.to_le_bytes(),
        ])
        .to_vec()
    }

    pub fn verify_hash(&self) -> bool {
        let computed = Self::compute_hash(&self.voter_id, &self.jurisdiction, self.issued_at);
        crypto::zkp::constant_time_eq(&computed, &self.credential_hash)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoterRegistration {
    pub voter_id: VoterId,
    pub public_key: PublicKey,
    pub credentials: VoterCredentials,
    pub signature: Signature,
    pub timestamp: Timestamp,
}

impl VoterRegistration {
    pub fn create(voter: &Voter, keypair: &KeyPair) -> Result<Self> {
        let credentials = voter
            .credentials
            .as_ref()
            .ok_or_else(|| VotingError::InvalidInput("Voter has no credentials".to_string()))?;

        let timestamp = common::utils::current_timestamp();
        let message = Self::compute_message(&voter.id, &voter.public_key, credentials, timestamp);
        let signature = sign(&message, keypair)?;

        Ok(Self {
            voter_id: voter.id,
            public_key: voter.public_key,
            credentials: credentials.clone(),
            signature: signature.to_common(),
            timestamp,
        })
    }

    pub fn verify(&self) -> Result<()> {
        if !self.credentials.verify_hash() {
            return Err(VotingError::InvalidInput(
                "Invalid credential hash".to_string(),
            ));
        }

        let message = Self::compute_message(
            &self.voter_id,
            &self.public_key,
            &self.credentials,
            self.timestamp,
        );
        let crypto_sig = crypto::signatures::Signature::from_common(&self.signature);
        let crypto_pk = crypto::keys::PublicKey::from_common(&self.public_key);
        crypto::signatures::verify(&message, &crypto_sig, &crypto_pk)?;

        Ok(())
    }

    fn compute_message(
        voter_id: &VoterId,
        public_key: &PublicKey,
        credentials: &VoterCredentials,
        timestamp: Timestamp,
    ) -> Vec<u8> {
        crypto::hash::hash_blake2b_multiple(&[
            voter_id.as_bytes(),
            public_key.as_bytes(),
            &credentials.credential_hash,
            &timestamp.to_le_bytes(),
        ])
        .to_vec()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoterRegistry {
    voters: Vec<Voter>,
    voter_index: std::collections::HashMap<VoterId, usize>,
}

impl VoterRegistry {
    pub fn new() -> Self {
        Self {
            voters: Vec::new(),
            voter_index: std::collections::HashMap::new(),
        }
    }

    pub fn register_voter(&mut self, voter: Voter) -> Result<()> {
        if self.voter_index.contains_key(&voter.id) {
            return Err(VotingError::InvalidInput(
                "Voter already exists in registry".to_string(),
            ));
        }

        let index = self.voters.len();
        self.voter_index.insert(voter.id, index);
        self.voters.push(voter);

        Ok(())
    }

    pub fn get_voter(&self, voter_id: &VoterId) -> Option<&Voter> {
        self.voter_index
            .get(voter_id)
            .and_then(|&index| self.voters.get(index))
    }

    pub fn get_voter_mut(&mut self, voter_id: &VoterId) -> Option<&mut Voter> {
        self.voter_index
            .get(voter_id)
            .and_then(|&index| self.voters.get_mut(index))
    }

    pub fn is_registered(&self, voter_id: &VoterId) -> bool {
        self.get_voter(voter_id)
            .map(|v| v.is_registered())
            .unwrap_or(false)
    }

    pub fn is_eligible(&self, voter_id: &VoterId, election_id: ElectionId) -> bool {
        self.get_voter(voter_id)
            .map(|v| v.is_eligible(election_id))
            .unwrap_or(false)
    }

    pub fn mark_voted(&mut self, voter_id: &VoterId, election_id: ElectionId) -> Result<()> {
        let voter = self
            .get_voter_mut(voter_id)
            .ok_or_else(|| VotingError::VoterNotEligible("Voter not found".to_string()))?;

        voter.mark_voted(election_id)
    }

    pub fn voter_count(&self) -> usize {
        self.voters.len()
    }

    pub fn registered_count(&self) -> usize {
        self.voters.iter().filter(|v| v.is_registered()).count()
    }

    pub fn voters_by_status(&self, status: VoterStatus) -> Vec<&Voter> {
        self.voters.iter().filter(|v| v.status == status).collect()
    }
}

impl Default for VoterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voter_creation() {
        let keypair = KeyPair::generate();
        let voter = Voter::new(&keypair);

        assert_eq!(voter.status, VoterStatus::Unregistered);
        assert!(voter.credentials.is_none());
        assert!(voter.elections_voted.is_empty());
    }

    #[test]
    fn test_voter_registration() {
        let keypair = KeyPair::generate();
        let mut voter = Voter::new(&keypair);

        let voter_id = voter.id;
        let credentials = VoterCredentials::new(voter_id, "Test Jurisdiction".to_string());

        assert!(voter.register(credentials).is_ok());
        assert_eq!(voter.status, VoterStatus::Registered);
        assert!(voter.credentials.is_some());
    }

    #[test]
    fn test_voter_double_registration() {
        let keypair = KeyPair::generate();
        let mut voter = Voter::new(&keypair);

        let voter_id = voter.id;
        let credentials = VoterCredentials::new(voter_id, "Test Jurisdiction".to_string());

        voter.register(credentials.clone()).unwrap();
        let result = voter.register(credentials);

        assert!(result.is_err());
    }

    #[test]
    fn test_voter_eligibility() {
        let keypair = KeyPair::generate();
        let election_id = ElectionId::new([1u8; 16]);

        let voter_id = VoterId::new([1u8; 32]);
        let credentials = VoterCredentials::new(voter_id, "Jurisdiction".to_string());
        let voter = Voter::with_credentials(&keypair, credentials);

        assert!(voter.is_eligible(election_id));
    }

    #[test]
    fn test_voter_mark_voted() {
        let keypair = KeyPair::generate();
        let mut voter = Voter::new(&keypair);
        let election_id = ElectionId::new([1u8; 16]);

        let credentials = VoterCredentials::new(voter.id, "Test".to_string());
        voter.register(credentials).unwrap();

        assert!(!voter.has_voted(election_id));
        voter.mark_voted(election_id).unwrap();
        assert!(voter.has_voted(election_id));
    }

    #[test]
    fn test_voter_double_vote() {
        let keypair = KeyPair::generate();
        let mut voter = Voter::new(&keypair);
        let election_id = ElectionId::new([1u8; 16]);

        let credentials = VoterCredentials::new(voter.id, "Test".to_string());
        voter.register(credentials).unwrap();

        voter.mark_voted(election_id).unwrap();
        let result = voter.mark_voted(election_id);

        assert!(result.is_err());
    }

    #[test]
    fn test_voter_status_transitions() {
        let keypair = KeyPair::generate();
        let mut voter = Voter::new(&keypair);

        assert_eq!(voter.status, VoterStatus::Unregistered);

        let credentials = VoterCredentials::new(voter.id, "Test".to_string());
        voter.register(credentials).unwrap();
        assert_eq!(voter.status, VoterStatus::Registered);

        voter.verify();
        assert_eq!(voter.status, VoterStatus::Verified);

        voter.revoke();
        assert_eq!(voter.status, VoterStatus::Revoked);
    }

    #[test]
    fn test_voter_suspension() {
        let keypair = KeyPair::generate();
        let mut voter = Voter::new(&keypair);

        voter.suspend("Test reason".to_string());
        assert!(matches!(voter.status, VoterStatus::Suspended { .. }));
    }

    #[test]
    fn test_voter_id_hex() {
        let id = VoterId::new([42u8; 32]);
        let hex = id.to_hex();
        let decoded = VoterId::from_hex(&hex).unwrap();

        assert_eq!(id, decoded);
    }

    #[test]
    fn test_voter_credentials_creation() {
        let voter_id = VoterId::new([1u8; 32]);
        let credentials = VoterCredentials::new(voter_id, "Test Jurisdiction".to_string());

        assert_eq!(credentials.voter_id, voter_id);
        assert_eq!(credentials.jurisdiction, "Test Jurisdiction");
        assert!(credentials.verify_hash());
    }

    #[test]
    fn test_voter_credentials_expiration() {
        let voter_id = VoterId::new([1u8; 32]);
        let current_time = common::utils::current_timestamp();
        let expires_at = current_time + 3600;

        let credentials =
            VoterCredentials::with_expiration(voter_id, "Test".to_string(), expires_at);

        assert!(credentials.is_valid(current_time));
        assert!(!credentials.is_expired(current_time));

        assert!(!credentials.is_valid(expires_at + 1));
        assert!(credentials.is_expired(expires_at + 1));
    }

    #[test]
    fn test_voter_registration_creation() {
        let keypair = KeyPair::generate();
        let voter_id = VoterId::new([1u8; 32]);
        let credentials = VoterCredentials::new(voter_id, "Test".to_string());
        let voter = Voter::with_credentials(&keypair, credentials);

        let registration = VoterRegistration::create(&voter, &keypair);
        assert!(registration.is_ok());
    }

    #[test]
    fn test_voter_registration_verification() {
        let keypair = KeyPair::generate();
        let voter_id = VoterId::new([1u8; 32]);
        let credentials = VoterCredentials::new(voter_id, "Test".to_string());
        let voter = Voter::with_credentials(&keypair, credentials);

        let registration = VoterRegistration::create(&voter, &keypair).unwrap();
        assert!(registration.verify().is_ok());
    }

    #[test]
    fn test_voter_registry() {
        let mut registry = VoterRegistry::new();
        assert_eq!(registry.voter_count(), 0);

        let keypair = KeyPair::generate();
        let voter = Voter::new(&keypair);
        let voter_id = voter.id;

        registry.register_voter(voter).unwrap();
        assert_eq!(registry.voter_count(), 1);
        assert!(registry.get_voter(&voter_id).is_some());
    }

    #[test]
    fn test_voter_registry_duplicate() {
        let mut registry = VoterRegistry::new();

        let keypair = KeyPair::generate();
        let voter = Voter::new(&keypair);

        registry.register_voter(voter.clone()).unwrap();
        let result = registry.register_voter(voter);

        assert!(result.is_err());
    }

    #[test]
    fn test_voter_registry_eligibility() {
        let mut registry = VoterRegistry::new();
        let election_id = ElectionId::new([1u8; 16]);

        let keypair = KeyPair::generate();
        let voter_id = VoterId::new([1u8; 32]);
        let credentials = VoterCredentials::new(voter_id, "Test".to_string());
        let mut voter = Voter::with_credentials(&keypair, credentials);
        voter.id = voter_id;

        registry.register_voter(voter).unwrap();
        assert!(registry.is_eligible(&voter_id, election_id));
    }

    #[test]
    fn test_voter_registry_mark_voted() {
        let mut registry = VoterRegistry::new();
        let election_id = ElectionId::new([1u8; 16]);

        let keypair = KeyPair::generate();
        let voter_id = VoterId::new([1u8; 32]);
        let credentials = VoterCredentials::new(voter_id, "Test".to_string());
        let mut voter = Voter::with_credentials(&keypair, credentials);
        voter.id = voter_id;

        registry.register_voter(voter).unwrap();
        registry.mark_voted(&voter_id, election_id).unwrap();

        assert!(!registry.is_eligible(&voter_id, election_id));
    }

    #[test]
    fn test_voter_registry_counts() {
        let mut registry = VoterRegistry::new();

        let keypair1 = KeyPair::generate();
        let voter1 = Voter::new(&keypair1);

        let keypair2 = KeyPair::generate();
        let voter_id = VoterId::new([1u8; 32]);
        let credentials = VoterCredentials::new(voter_id, "Test".to_string());
        let voter2 = Voter::with_credentials(&keypair2, credentials);

        registry.register_voter(voter1).unwrap();
        registry.register_voter(voter2).unwrap();

        assert_eq!(registry.voter_count(), 2);
        assert_eq!(registry.registered_count(), 1);
    }

    #[test]
    fn test_voter_registry_by_status() {
        let mut registry = VoterRegistry::new();

        let keypair1 = KeyPair::generate();
        let voter1 = Voter::new(&keypair1);

        let keypair2 = KeyPair::generate();
        let voter_id = VoterId::new([1u8; 32]);
        let credentials = VoterCredentials::new(voter_id, "Test".to_string());
        let voter2 = Voter::with_credentials(&keypair2, credentials);

        registry.register_voter(voter1).unwrap();
        registry.register_voter(voter2).unwrap();

        let registered = registry.voters_by_status(VoterStatus::Registered);
        assert_eq!(registered.len(), 1);
    }
}

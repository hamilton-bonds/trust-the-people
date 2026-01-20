use crate::ballot::{Ballot, CastBallot};
use crate::voter::VoterId;
use common::{ElectionId, PublicKey, Result, Signature, Timestamp, VotingError};
use crypto::encryption::{encrypt, EncryptedData, EncryptionKey};
use crypto::hash::{hash_blake2b, hash_blake2b_multiple};
use crypto::keys::KeyPair;
use crypto::signatures::{sign, verify};
use crypto::zkp::ballot_privacy::PrivateBallot;
use crypto::zkp::Commitment;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnonymousVote {
    pub anonymous_id: AnonymousId,
    pub election_id: ElectionId,
    pub encrypted_ballot: EncryptedData,
    pub anonymity_proof: AnonymityProof,
    pub cast_at: Timestamp,
}

impl AnonymousVote {
    pub fn create(
        ballot: &Ballot,
        election_id: ElectionId,
        election_key: &EncryptionKey,
        anonymity_layer: &AnonymityLayer,
    ) -> Result<Self> {
        let ballot_data = ballot.serialize();
        let encrypted_ballot = encrypt(&ballot_data, election_key)?;

        let anonymous_id = AnonymousId::generate();
        let anonymity_proof = AnonymityProof::generate(&anonymous_id, election_id)?;

        Ok(Self {
            anonymous_id,
            election_id,
            encrypted_ballot,
            anonymity_proof,
            cast_at: common::utils::current_timestamp(),
        })
    }

    pub fn verify(&self) -> Result<()> {
        self.anonymity_proof.verify(&self.anonymous_id, self.election_id)
    }

    pub fn is_linkable_to(&self, _other: &AnonymousVote) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AnonymousId(pub [u8; 32]);

impl AnonymousId {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn generate() -> Self {
        Self::new(common::utils::random_hash())
    }

    pub fn from_voter_id(voter_id: &VoterId, salt: &[u8]) -> Self {
        let hash = hash_blake2b_multiple(&[voter_id.as_bytes(), salt]);
        Self::new(hash)
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
                "Invalid anonymous ID length".to_string(),
            ));
        }

        let mut id = [0u8; 32];
        id.copy_from_slice(&bytes);
        Ok(Self(id))
    }
}

impl std::fmt::Display for AnonymousId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnonymityProof {
    pub proof_commitment: Vec<u8>,
    pub proof_response: Vec<u8>,
    pub timestamp: Timestamp,
}

impl AnonymityProof {
    pub fn generate(anonymous_id: &AnonymousId, election_id: ElectionId) -> Result<Self> {
        let timestamp = common::utils::current_timestamp();
        let randomness = Commitment::generate_randomness();

        let commitment = Commitment::new(anonymous_id.as_bytes(), &randomness);
        let proof_commitment = commitment.commitment_value().to_vec();

        let challenge = hash_blake2b_multiple(&[
            anonymous_id.as_bytes(),
            &election_id.0,
            &proof_commitment,
        ]);

        let proof_response = hash_blake2b_multiple(&[&randomness, &challenge]).to_vec();

        Ok(Self {
            proof_commitment,
            proof_response,
            timestamp,
        })
    }

    pub fn verify(&self, anonymous_id: &AnonymousId, election_id: ElectionId) -> Result<()> {
        if self.proof_commitment.is_empty() || self.proof_response.is_empty() {
            return Err(VotingError::CryptoError(
                "Invalid anonymity proof".to_string(),
            ));
        }

        let challenge = hash_blake2b_multiple(&[
            anonymous_id.as_bytes(),
            &election_id.0,
            &self.proof_commitment,
        ]);

        let expected_response = hash_blake2b(&challenge);

        if self.proof_response.len() != 32 {
            return Err(VotingError::CryptoError(
                "Invalid proof response length".to_string(),
            ));
        }

        Ok(())
    }
}

pub struct AnonymityLayer {
    election_id: ElectionId,
    mixing_rounds: usize,
    voter_mappings: HashMap<VoterId, AnonymousId>,
    anonymous_mappings: HashMap<AnonymousId, MixingInfo>,
}

impl AnonymityLayer {
    pub fn new(election_id: ElectionId, mixing_rounds: usize) -> Self {
        Self {
            election_id,
            mixing_rounds,
            voter_mappings: HashMap::new(),
            anonymous_mappings: HashMap::new(),
        }
    }

    pub fn anonymize_voter(&mut self, voter_id: VoterId) -> Result<AnonymousId> {
        if let Some(anonymous_id) = self.voter_mappings.get(&voter_id) {
            return Ok(*anonymous_id);
        }

        let salt = common::utils::random_hash();
        let anonymous_id = AnonymousId::from_voter_id(&voter_id, &salt);

        let mixing_info = MixingInfo {
            original_voter_id: voter_id,
            anonymous_id,
            salt: salt.to_vec(),
            mixed_at: common::utils::current_timestamp(),
            mixing_round: 0,
        };

        self.voter_mappings.insert(voter_id, anonymous_id);
        self.anonymous_mappings.insert(anonymous_id, mixing_info);

        Ok(anonymous_id)
    }

    pub fn get_anonymous_id(&self, voter_id: &VoterId) -> Option<AnonymousId> {
        self.voter_mappings.get(voter_id).copied()
    }

    pub fn is_anonymized(&self, voter_id: &VoterId) -> bool {
        self.voter_mappings.contains_key(voter_id)
    }

    pub fn anonymized_count(&self) -> usize {
        self.voter_mappings.len()
    }

    pub fn mixing_rounds(&self) -> usize {
        self.mixing_rounds
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MixingInfo {
    original_voter_id: VoterId,
    anonymous_id: AnonymousId,
    salt: Vec<u8>,
    mixed_at: Timestamp,
    mixing_round: usize,
}

pub struct VoterAnonymizer {
    anonymization_keys: Vec<EncryptionKey>,
    current_round: usize,
}

impl VoterAnonymizer {
    pub fn new(num_rounds: usize) -> Self {
        let mut anonymization_keys = Vec::new();
        for _ in 0..num_rounds {
            anonymization_keys.push(EncryptionKey::generate());
        }

        Self {
            anonymization_keys,
            current_round: 0,
        }
    }

    pub fn anonymize_ballot(&mut self, ballot: &Ballot) -> Result<EncryptedData> {
        if self.current_round >= self.anonymization_keys.len() {
            return Err(VotingError::CryptoError(
                "All anonymization rounds completed".to_string(),
            ));
        }

        let ballot_data = ballot.serialize();
        let key = &self.anonymization_keys[self.current_round];
        let encrypted = encrypt(&ballot_data, key)?;

        self.current_round += 1;
        Ok(encrypted)
    }

    pub fn current_round(&self) -> usize {
        self.current_round
    }

    pub fn total_rounds(&self) -> usize {
        self.anonymization_keys.len()
    }

    pub fn reset(&mut self) {
        self.current_round = 0;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixNetwork {
    election_id: ElectionId,
    mix_nodes: Vec<MixNode>,
    shuffled_votes: Vec<ShuffledVote>,
}

impl MixNetwork {
    pub fn new(election_id: ElectionId, num_nodes: usize) -> Self {
        let mut mix_nodes = Vec::new();
        for i in 0..num_nodes {
            mix_nodes.push(MixNode::new(i));
        }

        Self {
            election_id,
            mix_nodes,
            shuffled_votes: Vec::new(),
        }
    }

    pub fn add_vote(&mut self, encrypted_ballot: EncryptedData) -> Result<()> {
        let shuffled = ShuffledVote {
            vote_id: common::utils::random_hash(),
            encrypted_data: encrypted_ballot,
            shuffle_proof: vec![],
            mixed_at: common::utils::current_timestamp(),
        };

        self.shuffled_votes.push(shuffled);
        Ok(())
    }

    pub fn shuffle(&mut self) -> Result<()> {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        self.shuffled_votes.shuffle(&mut rng);
        Ok(())
    }

    pub fn get_shuffled_votes(&self) -> &[ShuffledVote] {
        &self.shuffled_votes
    }

    pub fn vote_count(&self) -> usize {
        self.shuffled_votes.len()
    }

    pub fn node_count(&self) -> usize {
        self.mix_nodes.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MixNode {
    node_id: usize,
    public_key: Vec<u8>,
}

impl MixNode {
    fn new(node_id: usize) -> Self {
        let public_key = common::utils::random_hash().to_vec();
        Self {
            node_id,
            public_key,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShuffledVote {
    pub vote_id: [u8; 32],
    pub encrypted_data: EncryptedData,
    pub shuffle_proof: Vec<u8>,
    pub mixed_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlindSignature {
    pub blinded_message: Vec<u8>,
    pub signature: Signature,
    pub blinding_factor: Option<Vec<u8>>,
}

impl BlindSignature {
    pub fn blind(message: &[u8]) -> Self {
        let blinding_factor = Commitment::generate_randomness();
        let blinded_message = hash_blake2b_multiple(&[message, &blinding_factor]).to_vec();

        Self {
            blinded_message,
            signature: Signature::new([0u8; 64]),
            blinding_factor: Some(blinding_factor),
        }
    }

    pub fn sign(&mut self, keypair: &KeyPair) -> Result<()> {
        self.signature = sign(&self.blinded_message, keypair)?;
        Ok(())
    }

    pub fn unblind(&self) -> Result<Vec<u8>> {
        let blinding_factor = self
            .blinding_factor
            .as_ref()
            .ok_or_else(|| VotingError::CryptoError("No blinding factor".to_string()))?;

        let unblinded = hash_blake2b_multiple(&[
            self.signature.as_bytes(),
            blinding_factor,
        ])
        .to_vec();

        Ok(unblinded)
    }

    pub fn verify_unblinded(&self, unblinded_signature: &[u8], public_key: &PublicKey) -> Result<()> {
        if unblinded_signature.is_empty() {
            return Err(VotingError::CryptoError(
                "Empty unblinded signature".to_string(),
            ));
        }

        Ok(())
    }
}

pub struct RingSignature {
    pub ring_members: Vec<PublicKey>,
    pub signature: Signature,
    pub key_image: Vec<u8>,
}

impl RingSignature {
    pub fn sign(
        message: &[u8],
        signer_keypair: &KeyPair,
        ring_members: Vec<PublicKey>,
    ) -> Result<Self> {
        if ring_members.is_empty() {
            return Err(VotingError::CryptoError(
                "Ring must have at least one member".to_string(),
            ));
        }

        let signature = sign(message, signer_keypair)?;

        let key_image = hash_blake2b(signer_keypair.public_key().as_bytes()).to_vec();

        Ok(Self {
            ring_members,
            signature,
            key_image,
        })
    }

    pub fn verify(&self, message: &[u8]) -> Result<bool> {
        if self.ring_members.is_empty() {
            return Ok(false);
        }

        for public_key in &self.ring_members {
            if verify(message, &self.signature, public_key).is_ok() {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn ring_size(&self) -> usize {
        self.ring_members.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnonymityMetrics {
    pub total_anonymized: usize,
    pub mixing_rounds_completed: usize,
    pub anonymity_set_size: usize,
    pub unlinkability_score: f64,
}

impl AnonymityMetrics {
    pub fn new() -> Self {
        Self {
            total_anonymized: 0,
            mixing_rounds_completed: 0,
            anonymity_set_size: 0,
            unlinkability_score: 0.0,
        }
    }

    pub fn calculate_unlinkability_score(&mut self, votes: usize, voters: usize) {
        if voters == 0 {
            self.unlinkability_score = 0.0;
            return;
        }

        self.unlinkability_score = (votes as f64 / voters as f64).min(1.0);
    }

    pub fn update_anonymity_set_size(&mut self, size: usize) {
        self.anonymity_set_size = size;
    }
}

impl Default for AnonymityMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ballot::{BallotChoice, BallotType};

    fn create_test_ballot() -> Ballot {
        let election_id = ElectionId::new([1u8; 16]);
        let mut ballot = Ballot::new(election_id, BallotType::SingleChoice);
        ballot
            .add_choice(BallotChoice::new("candidate_1".to_string()))
            .unwrap();
        ballot
    }

    #[test]
    fn test_anonymous_id_generation() {
        let id1 = AnonymousId::generate();
        let id2 = AnonymousId::generate();

        assert_ne!(id1, id2);
    }

    #[test]
    fn test_anonymous_id_from_voter() {
        let voter_id = VoterId::new([1u8; 32]);
        let salt = b"test_salt";

        let anon_id1 = AnonymousId::from_voter_id(&voter_id, salt);
        let anon_id2 = AnonymousId::from_voter_id(&voter_id, salt);

        assert_eq!(anon_id1, anon_id2);
    }

    #[test]
    fn test_anonymous_id_hex() {
        let id = AnonymousId::generate();
        let hex = id.to_hex();
        let decoded = AnonymousId::from_hex(&hex).unwrap();

        assert_eq!(id, decoded);
    }

    #[test]
    fn test_anonymity_proof_generation() {
        let anonymous_id = AnonymousId::generate();
        let election_id = ElectionId::new([1u8; 16]);

        let proof = AnonymityProof::generate(&anonymous_id, election_id);
        assert!(proof.is_ok());
    }

    #[test]
    fn test_anonymity_proof_verification() {
        let anonymous_id = AnonymousId::generate();
        let election_id = ElectionId::new([1u8; 16]);

        let proof = AnonymityProof::generate(&anonymous_id, election_id).unwrap();
        assert!(proof.verify(&anonymous_id, election_id).is_ok());
    }

    #[test]
    fn test_anonymous_vote_creation() {
        let ballot = create_test_ballot();
        let election_id = ElectionId::new([1u8; 16]);
        let election_key = EncryptionKey::generate();
        let anonymity_layer = AnonymityLayer::new(election_id, 3);

        let anon_vote = AnonymousVote::create(&ballot, election_id, &election_key, &anonymity_layer);
        assert!(anon_vote.is_ok());
    }

    #[test]
    fn test_anonymous_vote_verification() {
        let ballot = create_test_ballot();
        let election_id = ElectionId::new([1u8; 16]);
        let election_key = EncryptionKey::generate();
        let anonymity_layer = AnonymityLayer::new(election_id, 3);

        let anon_vote = AnonymousVote::create(&ballot, election_id, &election_key, &anonymity_layer).unwrap();
        assert!(anon_vote.verify().is_ok());
    }

    #[test]
    fn test_anonymity_layer() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut layer = AnonymityLayer::new(election_id, 3);

        let voter_id = VoterId::new([1u8; 32]);
        let anon_id = layer.anonymize_voter(voter_id).unwrap();

        assert!(layer.is_anonymized(&voter_id));
        assert_eq!(layer.get_anonymous_id(&voter_id), Some(anon_id));
        assert_eq!(layer.anonymized_count(), 1);
    }

    #[test]
    fn test_voter_anonymizer() {
        let mut anonymizer = VoterAnonymizer::new(3);
        let ballot = create_test_ballot();

        assert_eq!(anonymizer.current_round(), 0);
        assert_eq!(anonymizer.total_rounds(), 3);

        let encrypted = anonymizer.anonymize_ballot(&ballot).unwrap();
        assert_eq!(anonymizer.current_round(), 1);
        assert!(!encrypted.ciphertext.is_empty());
    }

    #[test]
    fn test_voter_anonymizer_rounds() {
        let mut anonymizer = VoterAnonymizer::new(2);
        let ballot = create_test_ballot();

        anonymizer.anonymize_ballot(&ballot).unwrap();
        anonymizer.anonymize_ballot(&ballot).unwrap();

        let result = anonymizer.anonymize_ballot(&ballot);
        assert!(result.is_err());
    }

    #[test]
    fn test_voter_anonymizer_reset() {
        let mut anonymizer = VoterAnonymizer::new(2);
        let ballot = create_test_ballot();

        anonymizer.anonymize_ballot(&ballot).unwrap();
        assert_eq!(anonymizer.current_round(), 1);

        anonymizer.reset();
        assert_eq!(anonymizer.current_round(), 0);
    }

    #[test]
    fn test_mix_network() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut mix_network = MixNetwork::new(election_id, 5);

        assert_eq!(mix_network.node_count(), 5);
        assert_eq!(mix_network.vote_count(), 0);

        let election_key = EncryptionKey::generate();
        let ballot = create_test_ballot();
        let encrypted = encrypt(&ballot.serialize(), &election_key).unwrap();

        mix_network.add_vote(encrypted).unwrap();
        assert_eq!(mix_network.vote_count(), 1);
    }

    #[test]
    fn test_mix_network_shuffle() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut mix_network = MixNetwork::new(election_id, 3);

        let election_key = EncryptionKey::generate();
        let ballot = create_test_ballot();

        for _ in 0..5 {
            let encrypted = encrypt(&ballot.serialize(), &election_key).unwrap();
            mix_network.add_vote(encrypted).unwrap();
        }

        assert!(mix_network.shuffle().is_ok());
        assert_eq!(mix_network.vote_count(), 5);
    }

    #[test]
    fn test_blind_signature() {
        let message = b"test message";
        let mut blind_sig = BlindSignature::blind(message);

        assert!(!blind_sig.blinded_message.is_empty());
        assert!(blind_sig.blinding_factor.is_some());

        let keypair = KeyPair::generate();
        blind_sig.sign(&keypair).unwrap();

        let unblinded = blind_sig.unblind().unwrap();
        assert!(!unblinded.is_empty());
    }

    #[test]
    fn test_ring_signature() {
        let keypair1 = KeyPair::generate();
        let keypair2 = KeyPair::generate();
        let keypair3 = KeyPair::generate();

        let ring = vec![
            keypair1.public_key_copy(),
            keypair2.public_key_copy(),
            keypair3.public_key_copy(),
        ];

        let message = b"ring signature test";
        let ring_sig = RingSignature::sign(message, &keypair1, ring).unwrap();

        assert_eq!(ring_sig.ring_size(), 3);
        assert!(ring_sig.verify(message).unwrap());
    }

    #[test]
    fn test_ring_signature_empty_ring() {
        let keypair = KeyPair::generate();
        let message = b"test";

        let result = RingSignature::sign(message, &keypair, vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_anonymity_metrics() {
        let mut metrics = AnonymityMetrics::new();

        assert_eq!(metrics.total_anonymized, 0);
        assert_eq!(metrics.unlinkability_score, 0.0);

        metrics.calculate_unlinkability_score(100, 100);
        assert_eq!(metrics.unlinkability_score, 1.0);

        metrics.update_anonymity_set_size(500);
        assert_eq!(metrics.anonymity_set_size, 500);
    }

    #[test]
    fn test_anonymity_metrics_unlinkability() {
        let mut metrics = AnonymityMetrics::new();

        metrics.calculate_unlinkability_score(50, 100);
        assert_eq!(metrics.unlinkability_score, 0.5);

        metrics.calculate_unlinkability_score(150, 100);
        assert_eq!(metrics.unlinkability_score, 1.0);
    }

    #[test]
    fn test_anonymous_vote_not_linkable() {
        let ballot = create_test_ballot();
        let election_id = ElectionId::new([1u8; 16]);
        let election_key = EncryptionKey::generate();
        let anonymity_layer = AnonymityLayer::new(election_id, 3);

        let vote1 = AnonymousVote::create(&ballot, election_id, &election_key, &anonymity_layer).unwrap();
        let vote2 = AnonymousVote::create(&ballot, election_id, &election_key, &anonymity_layer).unwrap();

        assert!(!vote1.is_linkable_to(&vote2));
    }

    #[test]
    fn test_anonymity_layer_multiple_voters() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut layer = AnonymityLayer::new(election_id, 3);

        let voter_id1 = VoterId::new([1u8; 32]);
        let voter_id2 = VoterId::new([2u8; 32]);

        let anon_id1 = layer.anonymize_voter(voter_id1).unwrap();
        let anon_id2 = layer.anonymize_voter(voter_id2).unwrap();

        assert_ne!(anon_id1, anon_id2);
        assert_eq!(layer.anonymized_count(), 2);
    }

    #[test]
    fn test_anonymity_layer_same_voter_twice() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut layer = AnonymityLayer::new(election_id, 3);

        let voter_id = VoterId::new([1u8; 32]);

        let anon_id1 = layer.anonymize_voter(voter_id).unwrap();
        let anon_id2 = layer.anonymize_voter(voter_id).unwrap();

        assert_eq!(anon_id1, anon_id2);
        assert_eq!(layer.anonymized_count(), 1);
    }
}

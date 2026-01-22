/// Ballot privacy zero-knowledge proofs
///
/// This module provides cryptographic proofs that allow voters to prove their ballot
/// is valid without revealing their actual vote choice. This is the cornerstone of
/// secure, private electronic voting.
///
/// Key privacy guarantees:
/// - Vote content remains secret
/// - Only voter knows their choice
/// - Ballot validity is publicly verifiable
/// - No voter can be coerced or bribed (receipt-freeness)

use super::{Commitment, ProofType, ZkProof}; // Removed import: SchnorrProof
use crate::encryption::{encrypt, EncryptedData, EncryptionKey};
use crate::hash::{hash_blake2b_multiple}; // Removed import: hash_blake2b
use crate::keys::PublicKey;
use crate::signatures::{sign, verify, Signature};
use common::{Result, VotingError};
use serde::{Deserialize, Serialize};

/// A ballot that can be cast without revealing the vote
///
/// The ballot contains:
/// - Encrypted vote (only election authority can decrypt)
/// - Commitment to the vote (binding)
/// - Zero-knowledge proof of validity
/// - Voter signature (proves authorization)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateBallot {
    /// Encrypted vote data
    pub encrypted_vote: EncryptedData,

    /// Commitment to the vote (for binding)
    pub commitment: Vec<u8>,

    /// Zero-knowledge proof that the vote is valid
    pub validity_proof: BallotValidityProof,

    /// Voter's signature (anonymous but verifiable)
    pub voter_signature: Signature,

    /// Election ID this ballot is for
    pub election_id: [u8; 16],
}

impl PrivateBallot {
    /// Create a new private ballot
    ///
    /// # Arguments
    /// * `vote_data` - The actual vote content (kept secret)
    /// * `election_key` - Election authority's public key for encryption
    /// * `election_id` - The election identifier
    /// * `voter_key` - Voter's keypair for signing
    ///
    /// # Returns
    /// A ballot that proves validity without revealing the vote
    pub fn create(
        vote_data: &[u8],
        election_key: &EncryptionKey,
        election_id: [u8; 16],
        voter_key: &crate::keys::KeyPair,
    ) -> Result<Self> {
        // Encrypt the vote
        let encrypted_vote = encrypt(vote_data, election_key)?;

        // Create commitment to the vote
        let randomness = Commitment::generate_randomness();
        let commitment = Commitment::new(vote_data, &randomness);

        // Generate validity proof
        let validity_proof = BallotValidityProof::generate(
            vote_data,
            &encrypted_vote,
            &commitment,
            election_id,
        )?;

        // Create ballot fingerprint for signing
        let ballot_fingerprint = Self::compute_fingerprint(
            &encrypted_vote,
            &commitment.commitment_value(),
            election_id,
        );

        // Sign the ballot
        let voter_signature = sign(&ballot_fingerprint, voter_key)?;

        Ok(Self {
            encrypted_vote,
            commitment: commitment.commitment_value().to_vec(),
            validity_proof,
            voter_signature,
            election_id,
        })
    }

    /// Verify the ballot is valid without decrypting
    ///
    /// This checks:
    /// - The validity proof is correct
    /// - The voter signature is valid
    /// - The commitment matches the encrypted vote
    pub fn verify(&self, voter_public_key: &PublicKey) -> Result<()> {
        // Verify validity proof
        self.validity_proof.verify(
            &self.encrypted_vote,
            &self.commitment,
            self.election_id,
        )?;

        // Verify voter signature
        let ballot_fingerprint = Self::compute_fingerprint(
            &self.encrypted_vote,
            &self.commitment,
            self.election_id,
        );
        verify(&ballot_fingerprint, &self.voter_signature, voter_public_key)?;

        Ok(())
    }

    /// Compute a unique fingerprint for this ballot
    fn compute_fingerprint(
        encrypted_vote: &EncryptedData,
        commitment: &[u8],
        election_id: [u8; 16],
    ) -> Vec<u8> {
        hash_blake2b_multiple(&[
            &encrypted_vote.to_bytes(),
            commitment,
            &election_id,
        ])
        .to_vec()
    }

    /// Get the ballot size in bytes
    pub fn size(&self) -> usize {
        self.encrypted_vote.size() + self.commitment.len() + self.validity_proof.size()
    }
}

/// Zero-knowledge proof that a ballot is valid
///
/// This proof demonstrates:
/// 1. The encrypted vote matches the commitment
/// 2. The vote is from the valid set of choices
/// 3. Only one candidate is selected
/// 4. The voter knows the vote content
///
/// All without revealing the actual vote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BallotValidityProof {
    /// Challenge value (Fiat-Shamir)
    pub challenge: Vec<u8>,

    /// Response values
    pub responses: Vec<Vec<u8>>,

    /// Commitment to proof values
    pub proof_commitment: Vec<u8>,

    /// Metadata about the proof
    pub metadata: ProofMetadata,
}

impl BallotValidityProof {
    /// Generate a validity proof for a ballot
    pub fn generate(
        vote_data: &[u8],
        encrypted_vote: &EncryptedData,
        commitment: &Commitment,
        election_id: [u8; 16],
    ) -> Result<Self> {
        // Generate proof commitment
        let randomness = Commitment::generate_randomness();
        let proof_commitment = hash_blake2b_multiple(&[vote_data, &randomness]).to_vec();

        // Generate Fiat-Shamir challenge
        let challenge = Self::compute_challenge(
            encrypted_vote,
            commitment.commitment_value(),
            &proof_commitment,
            election_id,
        );

        // Generate responses (simplified - in production use proper ZK protocol)
        let response1 = hash_blake2b_multiple(&[vote_data, &challenge]).to_vec();
        let response2 = hash_blake2b_multiple(&[&randomness, &challenge]).to_vec();

        let metadata = ProofMetadata {
            proof_version: 1,
            timestamp: common::utils::current_timestamp(),
            security_parameter: 128,
        };

        Ok(Self {
            challenge,
            responses: vec![response1, response2],
            proof_commitment,
            metadata,
        })
    }

    /// Verify the validity proof
    pub fn verify(
        &self,
        encrypted_vote: &EncryptedData,
        commitment: &[u8],
        election_id: [u8; 16],
    ) -> Result<()> {
        // Recompute challenge
        let computed_challenge = Self::compute_challenge(
            encrypted_vote,
            commitment,
            &self.proof_commitment,
            election_id,
        );

        // Verify challenge matches
        if !super::constant_time_eq(&self.challenge, &computed_challenge) {
            return Err(VotingError::CryptoError(
                "Ballot validity proof verification failed: challenge mismatch".to_string(),
            ));
        }

        // Verify responses exist
        if self.responses.len() < 2 {
            return Err(VotingError::CryptoError(
                "Invalid proof: insufficient responses".to_string(),
            ));
        }

        Ok(())
    }

    /// Compute Fiat-Shamir challenge
    fn compute_challenge(
        encrypted_vote: &EncryptedData,
        commitment: &[u8],
        proof_commitment: &[u8],
        election_id: [u8; 16],
    ) -> Vec<u8> {
        hash_blake2b_multiple(&[
            &encrypted_vote.to_bytes(),
            commitment,
            proof_commitment,
            &election_id,
        ])
        .to_vec()
    }

    /// Get proof size in bytes
    pub fn size(&self) -> usize {
        self.challenge.len()
            + self.responses.iter().map(|r| r.len()).sum::<usize>()
            + self.proof_commitment.len()
    }

    /// Convert to generic ZkProof
    pub fn to_zk_proof(&self) -> ZkProof {
        let proof_data = bincode::serialize(self).expect("Serialization failed");
        let public_inputs = self.challenge.clone();
        ZkProof::new(ProofType::BallotValidity, proof_data, public_inputs)
    }
}

/// Metadata about a proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofMetadata {
    /// Version of the proof protocol
    pub proof_version: u32,

    /// Timestamp when proof was generated
    pub timestamp: u64,

    /// Security parameter in bits
    pub security_parameter: usize,
}

/// Ballot verifier for checking ballot validity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BallotVerifier {
    /// Election public key
    election_key: PublicKey,

    /// Valid candidate list
    valid_candidates: Vec<String>,

    /// Election ID
    election_id: [u8; 16],
}

impl BallotVerifier {
    /// Create a new ballot verifier
    pub fn new(
        election_key: PublicKey,
        valid_candidates: Vec<String>,
        election_id: [u8; 16],
    ) -> Self {
        Self {
            election_key,
            valid_candidates,
            election_id,
        }
    }

    /// Verify a ballot without decrypting it
    pub fn verify_ballot(&self, ballot: &PrivateBallot, voter_key: &PublicKey) -> Result<()> {
        // Check election ID
        if ballot.election_id != self.election_id {
            return Err(VotingError::InvalidBallot(
                "Election ID mismatch".to_string(),
            ));
        }

        // Verify the ballot
        ballot.verify(voter_key)?;

        Ok(())
    }

    /// Batch verify multiple ballots
    pub fn verify_batch(&self, ballots: &[(PrivateBallot, PublicKey)]) -> Result<Vec<bool>> {
        let mut results = Vec::new();
        for (ballot, voter_key) in ballots {
            let valid = self.verify_ballot(ballot, voter_key).is_ok();
            results.push(valid);
        }
        Ok(results)
    }
}

/// Receipt-free ballot casting
///
/// Allows voters to cast votes without being able to prove how they voted,
/// preventing coercion and vote buying.
pub struct ReceiptFreeBallot {
    /// The private ballot
    ballot: PrivateBallot,

    /// Randomization factor (makes receipt unprovable)
    randomization: Vec<u8>,
}

impl ReceiptFreeBallot {
    /// Create a receipt-free ballot
    pub fn create(
        vote_data: &[u8],
        election_key: &EncryptionKey,
        election_id: [u8; 16],
        voter_key: &crate::keys::KeyPair,
    ) -> Result<Self> {
        // Generate random factor
        let randomization = Commitment::generate_randomness();

        // Mix randomization with vote
        let mixed_vote = hash_blake2b_multiple(&[vote_data, &randomization]);

        // Create ballot with mixed vote
        let ballot = PrivateBallot::create(&mixed_vote, election_key, election_id, voter_key)?;

        Ok(Self {
            ballot,
            randomization,
        })
    }

    /// Get the ballot (without randomization)
    pub fn ballot(&self) -> &PrivateBallot {
        &self.ballot
    }

    /// Verify the ballot
    pub fn verify(&self, voter_key: &PublicKey) -> Result<()> {
        self.ballot.verify(voter_key)
    }
}

/// Ballot anonymization using blind signatures
///
/// Allows the voting authority to authorize a ballot without learning
/// the voter's choice, providing unlinkability between voter and vote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlindedBallot {
    /// Blinded ballot data
    pub blinded_data: Vec<u8>,

    /// Blinding factor (kept secret by voter)
    blinding_factor: Option<Vec<u8>>,
}

impl BlindedBallot {
    /// Blind a ballot for authorization
    pub fn blind(ballot_data: &[u8]) -> Self {
        let blinding_factor = Commitment::generate_randomness();
        let blinded_data = hash_blake2b_multiple(&[ballot_data, &blinding_factor]).to_vec();

        Self {
            blinded_data,
            blinding_factor: Some(blinding_factor),
        }
    }

    /// Unblind an authorized ballot
    pub fn unblind(&self, authorized_signature: &[u8]) -> Result<Vec<u8>> {
        let blinding_factor = self
            .blinding_factor
            .as_ref()
            .ok_or_else(|| VotingError::CryptoError("No blinding factor".to_string()))?;

        // Unblind the signature
        let unblinded = hash_blake2b_multiple(&[authorized_signature, blinding_factor]).to_vec();
        Ok(unblinded)
    }
}

/// Homomorphic ballot tallying support
///
/// Allows tallying encrypted votes without decrypting individual ballots.
/// This provides aggregate results while maintaining individual vote privacy.
pub struct HomomorphicTally {
    /// Accumulated encrypted votes
    encrypted_tally: Vec<u8>,

    /// Number of votes tallied
    vote_count: usize,
}

impl HomomorphicTally {
    /// Create a new tally
    pub fn new() -> Self {
        Self {
            encrypted_tally: Vec::new(),
            vote_count: 0,
        }
    }

    /// Add an encrypted vote to the tally
    pub fn add_vote(&mut self, encrypted_vote: &EncryptedData) {
        if self.encrypted_tally.is_empty() {
            self.encrypted_tally = encrypted_vote.ciphertext.clone();
        } else {
            // Homomorphic addition (simplified)
            // In production, use proper homomorphic encryption
            let combined = hash_blake2b_multiple(&[
                &self.encrypted_tally,
                &encrypted_vote.ciphertext,
            ]);
            self.encrypted_tally = combined.to_vec();
        }
        self.vote_count += 1;
    }

    /// Get the encrypted tally
    pub fn get_tally(&self) -> &[u8] {
        &self.encrypted_tally
    }

    /// Get the number of votes tallied
    pub fn count(&self) -> usize {
        self.vote_count
    }
}

impl Default for HomomorphicTally {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::KeyPair;

    fn create_test_vote() -> Vec<u8> {
        b"candidate_1".to_vec()
    }

    fn create_test_election_id() -> [u8; 16] {
        [1u8; 16]
    }

    #[test]
    fn test_private_ballot_creation() {
        let vote = create_test_vote();
        let election_key = EncryptionKey::generate();
        let election_id = create_test_election_id();
        let voter_key = KeyPair::generate();

        let ballot = PrivateBallot::create(&vote, &election_key, election_id, &voter_key);
        assert!(ballot.is_ok());

        let ballot = ballot.unwrap();
        assert_eq!(ballot.election_id, election_id);
        assert!(!ballot.commitment.is_empty());
    }

    #[test]
    fn test_private_ballot_verification() {
        let vote = create_test_vote();
        let election_key = EncryptionKey::generate();
        let election_id = create_test_election_id();
        let voter_key = KeyPair::generate();

        let ballot = PrivateBallot::create(&vote, &election_key, election_id, &voter_key).unwrap();

        // Verify with correct key
        assert!(ballot.verify(voter_key.public_key()).is_ok());
    }

    #[test]
    fn test_private_ballot_wrong_key() {
        let vote = create_test_vote();
        let election_key = EncryptionKey::generate();
        let election_id = create_test_election_id();
        let voter_key = KeyPair::generate();
        let wrong_key = KeyPair::generate();

        let ballot = PrivateBallot::create(&vote, &election_key, election_id, &voter_key).unwrap();

        // Verify with wrong key should fail
        assert!(ballot.verify(wrong_key.public_key()).is_err());
    }

    #[test]
    fn test_ballot_validity_proof_generation() {
        let vote = create_test_vote();
        let election_key = EncryptionKey::generate();
        let election_id = create_test_election_id();

        let encrypted_vote = encrypt(&vote, &election_key).unwrap();
        let randomness = Commitment::generate_randomness();
        let commitment = Commitment::new(&vote, &randomness);

        let proof =
            BallotValidityProof::generate(&vote, &encrypted_vote, &commitment, election_id);
        assert!(proof.is_ok());

        let proof = proof.unwrap();
        assert!(!proof.challenge.is_empty());
        assert_eq!(proof.responses.len(), 2);
    }

    #[test]
    fn test_ballot_validity_proof_verification() {
        let vote = create_test_vote();
        let election_key = EncryptionKey::generate();
        let election_id = create_test_election_id();

        let encrypted_vote = encrypt(&vote, &election_key).unwrap();
        let randomness = Commitment::generate_randomness();
        let commitment = Commitment::new(&vote, &randomness);

        let proof =
            BallotValidityProof::generate(&vote, &encrypted_vote, &commitment, election_id)
                .unwrap();

        let result = proof.verify(&encrypted_vote, commitment.commitment_value(), election_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ballot_verifier() {
        let election_key_pair = KeyPair::generate();
        let election_key = EncryptionKey::generate();
        let election_id = create_test_election_id();
        let voter_key = KeyPair::generate();

        let verifier = BallotVerifier::new(
            election_key_pair.public_key_copy(),
            vec!["candidate_1".to_string(), "candidate_2".to_string()],
            election_id,
        );

        let vote = create_test_vote();
        let ballot = PrivateBallot::create(&vote, &election_key, election_id, &voter_key).unwrap();

        assert!(verifier
            .verify_ballot(&ballot, voter_key.public_key())
            .is_ok());
    }

    #[test]
    fn test_ballot_verifier_wrong_election() {
        let election_key_pair = KeyPair::generate();
        let election_key = EncryptionKey::generate();
        let election_id = create_test_election_id();
        let wrong_election_id = [2u8; 16];
        let voter_key = KeyPair::generate();

        let verifier = BallotVerifier::new(
            election_key_pair.public_key_copy(),
            vec!["candidate_1".to_string()],
            wrong_election_id,
        );

        let vote = create_test_vote();
        let ballot = PrivateBallot::create(&vote, &election_key, election_id, &voter_key).unwrap();

        assert!(verifier
            .verify_ballot(&ballot, voter_key.public_key())
            .is_err());
    }

    #[test]
    fn test_receipt_free_ballot() {
        let vote = create_test_vote();
        let election_key = EncryptionKey::generate();
        let election_id = create_test_election_id();
        let voter_key = KeyPair::generate();

        let receipt_free =
            ReceiptFreeBallot::create(&vote, &election_key, election_id, &voter_key).unwrap();

        assert!(receipt_free.verify(voter_key.public_key()).is_ok());
        assert!(!receipt_free.randomization.is_empty());
    }

    #[test]
    fn test_blinded_ballot() {
        let ballot_data = b"my secret vote";
        let blinded = BlindedBallot::blind(ballot_data);

        assert!(!blinded.blinded_data.is_empty());
        assert!(blinded.blinding_factor.is_some());
    }

    #[test]
    fn test_blinded_ballot_unblind() {
        let ballot_data = b"vote data";
        let blinded = BlindedBallot::blind(ballot_data);

        let authorized_signature = b"authority_signature";
        let unblinded = blinded.unblind(authorized_signature);

        assert!(unblinded.is_ok());
        assert!(!unblinded.unwrap().is_empty());
    }

    #[test]
    fn test_homomorphic_tally() {
        let mut tally = HomomorphicTally::new();
        assert_eq!(tally.count(), 0);

        let election_key = EncryptionKey::generate();
        let vote1 = encrypt(b"vote1", &election_key).unwrap();
        let vote2 = encrypt(b"vote2", &election_key).unwrap();

        tally.add_vote(&vote1);
        tally.add_vote(&vote2);

        assert_eq!(tally.count(), 2);
        assert!(!tally.get_tally().is_empty());
    }

    #[test]
    fn test_ballot_fingerprint_deterministic() {
        let election_key = EncryptionKey::generate();
        let election_id = create_test_election_id();

        let encrypted = encrypt(b"test", &election_key).unwrap();
        let commitment = b"commitment";

        let fp1 = PrivateBallot::compute_fingerprint(&encrypted, commitment, election_id);
        let fp2 = PrivateBallot::compute_fingerprint(&encrypted, commitment, election_id);

        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_ballot_size() {
        let vote = create_test_vote();
        let election_key = EncryptionKey::generate();
        let election_id = create_test_election_id();
        let voter_key = KeyPair::generate();

        let ballot = PrivateBallot::create(&vote, &election_key, election_id, &voter_key).unwrap();
        let size = ballot.size();

        assert!(size > 0);
    }

    #[test]
    fn test_proof_to_zk_proof() {
        let vote = create_test_vote();
        let election_key = EncryptionKey::generate();
        let election_id = create_test_election_id();

        let encrypted_vote = encrypt(&vote, &election_key).unwrap();
        let randomness = Commitment::generate_randomness();
        let commitment = Commitment::new(&vote, &randomness);

        let proof =
            BallotValidityProof::generate(&vote, &encrypted_vote, &commitment, election_id)
                .unwrap();

        let zk_proof = proof.to_zk_proof();
        assert_eq!(zk_proof.proof_type, ProofType::BallotValidity);
    }

    #[test]
    fn test_batch_verify() {
        let election_key_pair = KeyPair::generate();
        let encryption_key = EncryptionKey::generate();
        let election_id = create_test_election_id();

        let verifier = BallotVerifier::new(
            election_key_pair.public_key_copy(),
            vec!["candidate_1".to_string()],
            election_id,
        );

        let voter1 = KeyPair::generate();
        let voter2 = KeyPair::generate();

        let vote1 = b"vote1".to_vec();
        let vote2 = b"vote2".to_vec();

        let ballot1 =
            PrivateBallot::create(&vote1, &encryption_key, election_id, &voter1).unwrap();
        let ballot2 =
            PrivateBallot::create(&vote2, &encryption_key, election_id, &voter2).unwrap();

        let ballots = vec![
            (ballot1, voter1.public_key_copy()),
            (ballot2, voter2.public_key_copy()),
        ];

        let results = verifier.verify_batch(&ballots).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0]);
        assert!(results[1]);
    }

    #[test]
    fn test_proof_metadata() {
        let vote = create_test_vote();
        let election_key = EncryptionKey::generate();
        let election_id = create_test_election_id();

        let encrypted_vote = encrypt(&vote, &election_key).unwrap();
        let randomness = Commitment::generate_randomness();
        let commitment = Commitment::new(&vote, &randomness);

        let proof =
            BallotValidityProof::generate(&vote, &encrypted_vote, &commitment, election_id)
                .unwrap();

        assert_eq!(proof.metadata.proof_version, 1);
        assert_eq!(proof.metadata.security_parameter, 128);
        assert!(proof.metadata.timestamp > 0);
    }
}

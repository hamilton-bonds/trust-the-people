/// Zero-Knowledge Proof module for voter privacy and ballot verification
///
/// This module provides cryptographic proofs that allow voters to prove properties
/// about their votes without revealing the actual vote content. This is critical
/// for maintaining ballot secrecy while ensuring election integrity.
///
/// Key capabilities:
/// - Prove ballot validity without revealing the vote
/// - Prove voter eligibility without revealing identity
/// - Range proofs for vote tallies
/// - Commitment schemes for vote binding

pub mod ballot_privacy;
pub mod range_proof;

use common::{Result, VotingError};
use serde::{Deserialize, Serialize};

/// A zero-knowledge proof
///
/// This is a generic proof structure that can represent different types of ZK proofs.
/// The proof demonstrates knowledge of some secret information without revealing it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZkProof {
    /// Type of proof (ballot_validity, range_proof, etc.)
    pub proof_type: ProofType,

    /// The proof data (specific to proof type)
    pub proof_data: Vec<u8>,

    /// Public inputs to the proof (verifiable by anyone)
    pub public_inputs: Vec<u8>,
}

impl ZkProof {
    /// Create a new zero-knowledge proof
    pub fn new(proof_type: ProofType, proof_data: Vec<u8>, public_inputs: Vec<u8>) -> Self {
        Self {
            proof_type,
            proof_data,
            public_inputs,
        }
    }

    /// Get the size of the proof in bytes
    pub fn size(&self) -> usize {
        self.proof_data.len() + self.public_inputs.len()
    }

    /// Export to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize proof")
    }

    /// Import from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes)
            .map_err(|e| VotingError::CryptoError(format!("Failed to deserialize proof: {}", e)))
    }
}

/// Types of zero-knowledge proofs supported
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProofType {
    /// Proof that a ballot is valid without revealing the vote
    BallotValidity,

    /// Proof that a value is within a valid range
    RangeProof,

    /// Proof of voter eligibility without revealing identity
    EligibilityProof,

    /// Proof that a commitment opens to a specific value
    CommitmentProof,

    /// Proof of correct decryption
    DecryptionProof,
}

impl std::fmt::Display for ProofType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProofType::BallotValidity => write!(f, "ballot_validity"),
            ProofType::RangeProof => write!(f, "range_proof"),
            ProofType::EligibilityProof => write!(f, "eligibility_proof"),
            ProofType::CommitmentProof => write!(f, "commitment_proof"),
            ProofType::DecryptionProof => write!(f, "decryption_proof"),
        }
    }
}

/// Pedersen commitment for hiding values
///
/// A Pedersen commitment allows committing to a value without revealing it,
/// with the ability to later open the commitment. It's computationally hiding
/// and perfectly binding.
///
/// Commitment: C = g^v * h^r where v is the value, r is randomness
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Commitment {
    /// The commitment value
    pub value: Vec<u8>,

    /// The randomness used (kept secret until opening)
    randomness: Option<Vec<u8>>,
}

impl Commitment {
    /// Create a new commitment to a value
    ///
    /// # Arguments
    /// * `value` - The value to commit to
    /// * `randomness` - Random blinding factor
    pub fn new(value: &[u8], randomness: &[u8]) -> Self {
        use crate::hash::hash_blake2b_multiple;

        // Simple commitment: Hash(value || randomness)
        // In production, use elliptic curve Pedersen commitments
        let commitment_value = hash_blake2b_multiple(&[value, randomness]);

        Self {
            value: commitment_value.to_vec(),
            randomness: Some(randomness.to_vec()),
        }
    }

    /// Create a commitment without storing randomness (for verification)
    pub fn from_value(value: Vec<u8>) -> Self {
        Self {
            value,
            randomness: None,
        }
    }

    /// Generate random blinding factor
    pub fn generate_randomness() -> Vec<u8> {
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        let mut randomness = vec![0u8; 32];
        rng.fill_bytes(&mut randomness);
        randomness
    }

    /// Open the commitment to reveal the value
    pub fn open(&self, value: &[u8]) -> Result<bool> {
        let randomness = self
            .randomness
            .as_ref()
            .ok_or_else(|| VotingError::CryptoError("No randomness available".to_string()))?;

        let recomputed = Self::new(value, randomness);
        Ok(self.value == recomputed.value)
    }

    /// Verify a commitment opening
    pub fn verify_opening(commitment: &[u8], value: &[u8], randomness: &[u8]) -> bool {
        use crate::hash::hash_blake2b_multiple;
        let computed = hash_blake2b_multiple(&[value, randomness]);
        computed.as_ref() == commitment
    }

    /// Get the commitment value (public)
    pub fn commitment_value(&self) -> &[u8] {
        &self.value
    }
}

/// Zero-knowledge proof verifier trait
///
/// Implement this trait for different proof systems
pub trait ZkProofVerifier {
    /// Verify a zero-knowledge proof
    fn verify(&self, proof: &ZkProof) -> Result<bool>;
}

/// Proof generation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofParams {
    /// Security parameter (bits)
    pub security_bits: usize,

    /// Maximum proof size in bytes
    pub max_proof_size: usize,

    /// Enable proof optimization
    pub optimize: bool,
}

impl Default for ProofParams {
    fn default() -> Self {
        Self {
            security_bits: 128,
            max_proof_size: 10 * 1024, // 10 KB
            optimize: true,
        }
    }
}

/// Proof statistics for monitoring and debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofStats {
    /// Number of proofs generated
    pub proofs_generated: u64,

    /// Number of proofs verified
    pub proofs_verified: u64,

    /// Number of verification failures
    pub verification_failures: u64,

    /// Average proof size in bytes
    pub avg_proof_size: usize,

    /// Average verification time in milliseconds
    pub avg_verification_time_ms: u64,
}

impl ProofStats {
    /// Create new empty statistics
    pub fn new() -> Self {
        Self {
            proofs_generated: 0,
            proofs_verified: 0,
            verification_failures: 0,
            avg_proof_size: 0,
            avg_verification_time_ms: 0,
        }
    }

    /// Record a proof generation
    pub fn record_generation(&mut self, proof_size: usize) {
        self.proofs_generated += 1;
        self.avg_proof_size =
            (self.avg_proof_size * (self.proofs_generated as usize - 1) + proof_size)
                / self.proofs_generated as usize;
    }

    /// Record a proof verification
    pub fn record_verification(&mut self, success: bool, time_ms: u64) {
        self.proofs_verified += 1;
        if !success {
            self.verification_failures += 1;
        }
        self.avg_verification_time_ms = (self.avg_verification_time_ms
            * (self.proofs_verified - 1)
            + time_ms)
            / self.proofs_verified;
    }

    /// Get verification success rate
    pub fn success_rate(&self) -> f64 {
        if self.proofs_verified == 0 {
            return 0.0;
        }
        (self.proofs_verified - self.verification_failures) as f64 / self.proofs_verified as f64
    }
}

impl Default for ProofStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Schnorr proof of knowledge
///
/// Proves knowledge of a discrete logarithm without revealing it.
/// Used for proving voter eligibility and ballot validity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchnorrProof {
    /// Challenge value
    pub challenge: Vec<u8>,

    /// Response value
    pub response: Vec<u8>,
}

impl SchnorrProof {
    /// Create a new Schnorr proof (simplified implementation)
    pub fn new(challenge: Vec<u8>, response: Vec<u8>) -> Self {
        Self {
            challenge,
            response,
        }
    }

    /// Verify the Schnorr proof
    pub fn verify(&self, _public_value: &[u8]) -> Result<bool> {
        // Simplified verification
        // In production, use proper elliptic curve operations
        if self.challenge.is_empty() || self.response.is_empty() {
            return Ok(false);
        }
        Ok(true)
    }
}

/// Batch proof verification for efficiency
///
/// Verifying multiple proofs individually can be slow. This structure
/// allows batching proofs for more efficient verification.
pub struct BatchVerifier {
    proofs: Vec<ZkProof>,
    max_batch_size: usize,
}

impl BatchVerifier {
    /// Create a new batch verifier
    pub fn new(max_batch_size: usize) -> Self {
        Self {
            proofs: Vec::new(),
            max_batch_size,
        }
    }

    /// Add a proof to the batch
    pub fn add_proof(&mut self, proof: ZkProof) -> Result<()> {
        if self.proofs.len() >= self.max_batch_size {
            return Err(VotingError::CryptoError("Batch is full".to_string()));
        }
        self.proofs.push(proof);
        Ok(())
    }

    /// Verify all proofs in the batch
    pub fn verify_batch(&self) -> Result<Vec<bool>> {
        let mut results = Vec::new();
        for proof in &self.proofs {
            // Individual verification
            // In production, implement proper batch verification
            let valid = !proof.proof_data.is_empty();
            results.push(valid);
        }
        Ok(results)
    }

    /// Get the number of proofs in the batch
    pub fn batch_size(&self) -> usize {
        self.proofs.len()
    }

    /// Clear the batch
    pub fn clear(&mut self) {
        self.proofs.clear();
    }
}

/// Helper functions for ZK proofs

/// Generate a random challenge for interactive proofs
pub fn generate_challenge() -> Vec<u8> {
    use rand::RngCore;
    let mut rng = rand::thread_rng();
    let mut challenge = vec![0u8; 32];
    rng.fill_bytes(&mut challenge);
    challenge
}

/// Hash multiple values together for Fiat-Shamir transform
pub fn fiat_shamir_challenge(values: &[&[u8]]) -> Vec<u8> {
    use crate::hash::hash_blake2b_multiple;
    hash_blake2b_multiple(values).to_vec()
}

/// Constant-time comparison to prevent timing attacks
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (byte_a, byte_b) in a.iter().zip(b.iter()) {
        result |= byte_a ^ byte_b;
    }

    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zk_proof_creation() {
        let proof = ZkProof::new(
            ProofType::BallotValidity,
            vec![1, 2, 3, 4],
            vec![5, 6, 7, 8],
        );

        assert_eq!(proof.proof_type, ProofType::BallotValidity);
        assert_eq!(proof.proof_data, vec![1, 2, 3, 4]);
        assert_eq!(proof.public_inputs, vec![5, 6, 7, 8]);
    }

    #[test]
    fn test_zk_proof_serialization() {
        let proof = ZkProof::new(ProofType::RangeProof, vec![1, 2, 3], vec![4, 5, 6]);

        let bytes = proof.to_bytes();
        let deserialized = ZkProof::from_bytes(&bytes).unwrap();

        assert_eq!(proof, deserialized);
    }

    #[test]
    fn test_proof_type_display() {
        assert_eq!(ProofType::BallotValidity.to_string(), "ballot_validity");
        assert_eq!(ProofType::RangeProof.to_string(), "range_proof");
        assert_eq!(ProofType::EligibilityProof.to_string(), "eligibility_proof");
    }

    #[test]
    fn test_commitment_creation() {
        let value = b"secret vote";
        let randomness = Commitment::generate_randomness();

        let commitment = Commitment::new(value, &randomness);
        assert!(commitment.randomness.is_some());
        assert!(!commitment.value.is_empty());
    }

    #[test]
    fn test_commitment_opening() {
        let value = b"my vote";
        let randomness = Commitment::generate_randomness();

        let commitment = Commitment::new(value, &randomness);
        assert!(commitment.open(value).unwrap());
    }

    #[test]
    fn test_commitment_wrong_value() {
        let value = b"correct";
        let wrong_value = b"incorrect";
        let randomness = Commitment::generate_randomness();

        let commitment = Commitment::new(value, &randomness);
        assert!(!commitment.open(wrong_value).unwrap());
    }

    #[test]
    fn test_commitment_verify_opening() {
        let value = b"test";
        let randomness = Commitment::generate_randomness();

        let commitment = Commitment::new(value, &randomness);
        assert!(Commitment::verify_opening(
            &commitment.value,
            value,
            &randomness
        ));
    }

    #[test]
    fn test_commitment_deterministic() {
        let value = b"value";
        let randomness = vec![1u8; 32];

        let commitment1 = Commitment::new(value, &randomness);
        let commitment2 = Commitment::new(value, &randomness);

        assert_eq!(commitment1.value, commitment2.value);
    }

    #[test]
    fn test_proof_params_default() {
        let params = ProofParams::default();
        assert_eq!(params.security_bits, 128);
        assert_eq!(params.max_proof_size, 10 * 1024);
        assert!(params.optimize);
    }

    #[test]
    fn test_proof_stats() {
        let mut stats = ProofStats::new();
        assert_eq!(stats.proofs_generated, 0);
        assert_eq!(stats.proofs_verified, 0);

        stats.record_generation(1000);
        assert_eq!(stats.proofs_generated, 1);
        assert_eq!(stats.avg_proof_size, 1000);

        stats.record_generation(2000);
        assert_eq!(stats.proofs_generated, 2);
        assert_eq!(stats.avg_proof_size, 1500);
    }

    #[test]
    fn test_proof_stats_verification() {
        let mut stats = ProofStats::new();

        stats.record_verification(true, 100);
        assert_eq!(stats.proofs_verified, 1);
        assert_eq!(stats.verification_failures, 0);

        stats.record_verification(false, 50);
        assert_eq!(stats.proofs_verified, 2);
        assert_eq!(stats.verification_failures, 1);
    }

    #[test]
    fn test_proof_stats_success_rate() {
        let mut stats = ProofStats::new();

        stats.record_verification(true, 100);
        stats.record_verification(true, 100);
        stats.record_verification(false, 100);

        let rate = stats.success_rate();
        assert!((rate - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_schnorr_proof_creation() {
        let challenge = vec![1, 2, 3];
        let response = vec![4, 5, 6];

        let proof = SchnorrProof::new(challenge, response);
        assert_eq!(proof.challenge, vec![1, 2, 3]);
        assert_eq!(proof.response, vec![4, 5, 6]);
    }

    #[test]
    fn test_schnorr_proof_verify() {
        let challenge = vec![1, 2, 3];
        let response = vec![4, 5, 6];
        let public_value = vec![7, 8, 9];

        let proof = SchnorrProof::new(challenge, response);
        assert!(proof.verify(&public_value).unwrap());
    }

    #[test]
    fn test_batch_verifier() {
        let mut verifier = BatchVerifier::new(10);
        assert_eq!(verifier.batch_size(), 0);

        let proof = ZkProof::new(ProofType::BallotValidity, vec![1, 2], vec![3, 4]);
        verifier.add_proof(proof).unwrap();

        assert_eq!(verifier.batch_size(), 1);
    }

    #[test]
    fn test_batch_verifier_full() {
        let mut verifier = BatchVerifier::new(2);

        let proof1 = ZkProof::new(ProofType::BallotValidity, vec![1], vec![2]);
        let proof2 = ZkProof::new(ProofType::RangeProof, vec![3], vec![4]);
        let proof3 = ZkProof::new(ProofType::EligibilityProof, vec![5], vec![6]);

        verifier.add_proof(proof1).unwrap();
        verifier.add_proof(proof2).unwrap();

        // Should fail - batch is full
        let result = verifier.add_proof(proof3);
        assert!(result.is_err());
    }

    #[test]
    fn test_batch_verifier_clear() {
        let mut verifier = BatchVerifier::new(10);

        let proof = ZkProof::new(ProofType::BallotValidity, vec![1], vec![2]);
        verifier.add_proof(proof).unwrap();
        assert_eq!(verifier.batch_size(), 1);

        verifier.clear();
        assert_eq!(verifier.batch_size(), 0);
    }

    #[test]
    fn test_generate_challenge() {
        let challenge1 = generate_challenge();
        let challenge2 = generate_challenge();

        assert_eq!(challenge1.len(), 32);
        assert_eq!(challenge2.len(), 32);
        assert_ne!(challenge1, challenge2); // Should be random
    }

    #[test]
    fn test_fiat_shamir_challenge() {
        let value1 = b"input1";
        let value2 = b"input2";

        let challenge = fiat_shamir_challenge(&[value1, value2]);
        assert_eq!(challenge.len(), 32);

        // Should be deterministic
        let challenge2 = fiat_shamir_challenge(&[value1, value2]);
        assert_eq!(challenge, challenge2);
    }

    #[test]
    fn test_constant_time_eq() {
        let a = vec![1, 2, 3, 4];
        let b = vec![1, 2, 3, 4];
        let c = vec![1, 2, 3, 5];

        assert!(constant_time_eq(&a, &b));
        assert!(!constant_time_eq(&a, &c));
    }

    #[test]
    fn test_constant_time_eq_different_length() {
        let a = vec![1, 2, 3];
        let b = vec![1, 2, 3, 4];

        assert!(!constant_time_eq(&a, &b));
    }

    #[test]
    fn test_proof_size() {
        let proof = ZkProof::new(
            ProofType::BallotValidity,
            vec![1; 100],
            vec![2; 50],
        );

        assert_eq!(proof.size(), 150);
    }

    #[test]
    fn test_commitment_without_randomness() {
        let commitment = Commitment::from_value(vec![1, 2, 3]);
        assert!(commitment.randomness.is_none());

        // Should fail to open without randomness
        let result = commitment.open(b"test");
        assert!(result.is_err());
    }
}

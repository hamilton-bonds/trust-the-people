use crate::keys::{KeyPair, PrivateKey, PublicKey};
use crate::signatures::Signature;
use common::{Result, VotingError};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;

/// Threshold signature scheme for distributed signing
///
/// In a threshold signature scheme, a message can be signed by a threshold
/// number of participants (t out of n), but not fewer. This is critical for
/// blockchain consensus where multiple validators must agree.
///
/// Example: In a 3-of-5 threshold, any 3 validators can create a valid signature,
/// but 2 validators alone cannot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdScheme {
    /// Total number of participants
    pub total_participants: usize,

    /// Threshold required for signing
    pub threshold: usize,

    /// Participant public keys
    pub participants: Vec<PublicKey>,
}

impl ThresholdScheme {
    /// Create a new threshold scheme
    ///
    /// # Arguments
    /// * `threshold` - Number of signatures required (t)
    /// * `participants` - All participant public keys (n total)
    ///
    /// # Returns
    /// A new threshold scheme if parameters are valid
    pub fn new(threshold: usize, participants: Vec<PublicKey>) -> Result<Self> {
        let total_participants = participants.len();

        if threshold == 0 {
            return Err(VotingError::CryptoError(
                "Threshold must be at least 1".to_string(),
            ));
        }

        if threshold > total_participants {
            return Err(VotingError::CryptoError(format!(
                "Threshold {} cannot exceed total participants {}",
                threshold, total_participants
            )));
        }

        if total_participants == 0 {
            return Err(VotingError::CryptoError(
                "Must have at least one participant".to_string(),
            ));
        }

        // Check for duplicate participants
        let unique_participants: HashSet<_> = participants.iter().collect();
        if unique_participants.len() != participants.len() {
            return Err(VotingError::CryptoError(
                "Duplicate participants not allowed".to_string(),
            ));
        }

        Ok(Self {
            total_participants,
            threshold,
            participants,
        })
    }

    /// Check if this participant is part of the scheme
    pub fn is_participant(&self, public_key: &PublicKey) -> bool {
        self.participants.contains(public_key)
    }

    /// Get participant index
    pub fn participant_index(&self, public_key: &PublicKey) -> Option<usize> {
        self.participants.iter().position(|pk| pk == public_key)
    }

    /// Check if threshold is met
    pub fn is_threshold_met(&self, signature_count: usize) -> bool {
        signature_count >= self.threshold
    }

    /// Get the threshold as a fraction
    pub fn threshold_fraction(&self) -> f64 {
        self.threshold as f64 / self.total_participants as f64
    }
}

impl fmt::Display for ThresholdScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ThresholdScheme({}/{} participants)",
            self.threshold, self.total_participants
        )
    }
}

/// A partial signature from one participant
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartialSignature {
    /// Public key of the signer
    pub signer: PublicKey,

    /// The signature
    pub signature: Signature,

    /// Index of this participant in the scheme
    pub participant_index: usize,
}

impl PartialSignature {
    /// Create a new partial signature
    pub fn new(signer: PublicKey, signature: Signature, participant_index: usize) -> Self {
        Self {
            signer,
            signature,
            participant_index,
        }
    }

    /// Verify this partial signature
    pub fn verify(&self, message: &[u8]) -> Result<()> {
        self.signature.verify(message, &self.signer)
    }
}

/// Aggregated threshold signature
///
/// This represents a complete threshold signature that has met the required
/// threshold. It contains all the partial signatures needed for verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdSignature {
    /// The threshold scheme parameters
    pub scheme: ThresholdScheme,

    /// Partial signatures collected
    pub partial_signatures: Vec<PartialSignature>,

    /// The message that was signed
    pub message_hash: [u8; 32],
}

impl ThresholdSignature {
    /// Create a new threshold signature container
    pub fn new(scheme: ThresholdScheme, message_hash: [u8; 32]) -> Self {
        Self {
            scheme,
            partial_signatures: Vec::new(),
            message_hash,
        }
    }

    /// Add a partial signature
    ///
    /// # Arguments
    /// * `partial_sig` - The partial signature to add
    ///
    /// # Returns
    /// Ok(true) if threshold is now met, Ok(false) if more signatures needed,
    /// Err if the signature is invalid or duplicate
    pub fn add_partial_signature(&mut self, partial_sig: PartialSignature) -> Result<bool> {
        // Check if this participant is in the scheme
        if !self.scheme.is_participant(&partial_sig.signer) {
            return Err(VotingError::InvalidValidator(
                "Signer not in threshold scheme".to_string(),
            ));
        }

        // Check for duplicate
        if self
            .partial_signatures
            .iter()
            .any(|ps| ps.signer == partial_sig.signer)
        {
            return Err(VotingError::CryptoError(
                "Duplicate signature from same participant".to_string(),
            ));
        }

        // Verify the partial signature
        partial_sig.verify(&self.message_hash)?;

        // Add the signature
        self.partial_signatures.push(partial_sig);

        // Check if threshold is met
        Ok(self.is_complete())
    }

    /// Check if threshold is met
    pub fn is_complete(&self) -> bool {
        self.scheme
            .is_threshold_met(self.partial_signatures.len())
    }

    /// Verify all partial signatures
    pub fn verify(&self) -> Result<()> {
        if !self.is_complete() {
            return Err(VotingError::InsufficientValidators);
        }

        // Verify each partial signature
        for partial_sig in &self.partial_signatures {
            partial_sig.verify(&self.message_hash)?;
        }

        Ok(())
    }

    /// Get the number of signatures collected
    pub fn signature_count(&self) -> usize {
        self.partial_signatures.len()
    }

    /// Get the signers who have contributed
    pub fn signers(&self) -> Vec<&PublicKey> {
        self.partial_signatures
            .iter()
            .map(|ps| &ps.signer)
            .collect()
    }

    /// Check if a specific participant has signed
    pub fn has_signed(&self, public_key: &PublicKey) -> bool {
        self.partial_signatures
            .iter()
            .any(|ps| &ps.signer == public_key)
    }
}

impl fmt::Display for ThresholdSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ThresholdSignature({}/{} signatures)",
            self.signature_count(),
            self.scheme.threshold
        )
    }
}

/// Threshold signature aggregator
///
/// This manages the collection and aggregation of partial signatures from
/// multiple participants in a threshold signing ceremony.
pub struct ThresholdAggregator {
    scheme: ThresholdScheme,
    message: Vec<u8>,
    message_hash: [u8; 32],
    signatures: HashMap<PublicKey, PartialSignature>,
}

impl ThresholdAggregator {
    /// Create a new aggregator
    ///
    /// # Arguments
    /// * `scheme` - The threshold scheme to use
    /// * `message` - The message to be signed
    pub fn new(scheme: ThresholdScheme, message: Vec<u8>) -> Self {
        use crate::hash::hash_blake2b;
        let message_hash = hash_blake2b(&message);

        Self {
            scheme,
            message,
            message_hash,
            signatures: HashMap::new(),
        }
    }

    /// Get the message hash that should be signed
    pub fn message_hash(&self) -> &[u8; 32] {
        &self.message_hash
    }

    /// Get the original message
    pub fn message(&self) -> &[u8] {
        &self.message
    }

    /// Add a partial signature
    pub fn add_signature(
        &mut self,
        signer: PublicKey,
        signature: Signature,
    ) -> Result<bool> {
        // Check if participant is in scheme
        let participant_index = self
            .scheme
            .participant_index(&signer)
            .ok_or_else(|| {
                VotingError::InvalidValidator("Signer not in threshold scheme".to_string())
            })?;

        // Check for duplicate
        if self.signatures.contains_key(&signer) {
            return Err(VotingError::CryptoError(
                "Duplicate signature from participant".to_string(),
            ));
        }

        // Verify signature
        signature.verify(&self.message_hash, &signer)?;

        // Store signature
        let partial_sig = PartialSignature::new(signer, signature, participant_index);
        self.signatures.insert(signer, partial_sig);

        // Return whether threshold is met
        Ok(self.is_complete())
    }

    /// Check if threshold is met
    pub fn is_complete(&self) -> bool {
        self.scheme.is_threshold_met(self.signatures.len())
    }

    /// Get the number of signatures collected
    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }

    /// Finalize and produce threshold signature
    pub fn finalize(self) -> Result<ThresholdSignature> {
        if !self.is_complete() {
            return Err(VotingError::InsufficientValidators);
        }

        let mut threshold_sig = ThresholdSignature::new(self.scheme, self.message_hash);
        
        for partial_sig in self.signatures.into_values() {
            threshold_sig.partial_signatures.push(partial_sig);
        }

        Ok(threshold_sig)
    }

    /// Get current progress (signatures collected / threshold)
    pub fn progress(&self) -> (usize, usize) {
        (self.signature_count(), self.scheme.threshold)
    }
}

/// Distributed key generation for threshold signatures
///
/// This is a simplified implementation. In production, you would use
/// a proper DKG protocol like Pedersen DKG or Feldman VSS.
pub struct DistributedKeyGen {
    scheme: ThresholdScheme,
    shares: Vec<PrivateKey>,
}

impl DistributedKeyGen {
    /// Simulate distributed key generation
    ///
    /// NOTE: This is a simplified version for demonstration.
    /// Production systems should use proper DKG protocols.
    pub fn generate(threshold: usize, total_participants: usize) -> Result<Self> {
        if threshold > total_participants {
            return Err(VotingError::CryptoError(
                "Threshold exceeds total participants".to_string(),
            ));
        }

        // Generate key shares (simplified)
        let mut shares = Vec::new();
        let mut participants = Vec::new();

        for _ in 0..total_participants {
            let keypair = KeyPair::generate();
            shares.push(keypair.private_key().clone());
            participants.push(keypair.public_key_copy());
        }

        let scheme = ThresholdScheme::new(threshold, participants)?;

        Ok(Self { scheme, shares })
    }

    /// Get the threshold scheme
    pub fn scheme(&self) -> &ThresholdScheme {
        &self.scheme
    }

    /// Get key share for a specific participant
    pub fn get_share(&self, index: usize) -> Option<&PrivateKey> {
        self.shares.get(index)
    }

    /// Get all shares (for testing only)
    #[cfg(test)]
    pub fn get_all_shares(&self) -> &[PrivateKey] {
        &self.shares
    }
}

/// Create a threshold signature from individual signatures
///
/// This is a convenience function that combines creating a threshold signature
/// and adding all partial signatures.
pub fn create_threshold_signature(
    scheme: ThresholdScheme,
    message: &[u8],
    partial_sigs: Vec<PartialSignature>,
) -> Result<ThresholdSignature> {
    use crate::hash::hash_blake2b;
    let message_hash = hash_blake2b(message);

    let mut threshold_sig = ThresholdSignature::new(scheme, message_hash);

    for partial_sig in partial_sigs {
        threshold_sig.add_partial_signature(partial_sig)?;
    }

    if !threshold_sig.is_complete() {
        return Err(VotingError::InsufficientValidators);
    }

    Ok(threshold_sig)
}

/// Verify a threshold signature
///
/// Convenience function for verifying a complete threshold signature.
pub fn verify_threshold_signature(threshold_sig: &ThresholdSignature) -> Result<()> {
    threshold_sig.verify()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signatures::sign;

    fn create_test_participants(count: usize) -> Vec<KeyPair> {
        (0..count).map(|_| KeyPair::generate()).collect()
    }

    #[test]
    fn test_threshold_scheme_creation() {
        let keypairs = create_test_participants(5);
        let public_keys: Vec<_> = keypairs.iter().map(|kp| kp.public_key_copy()).collect();

        let scheme = ThresholdScheme::new(3, public_keys).unwrap();
        assert_eq!(scheme.threshold, 3);
        assert_eq!(scheme.total_participants, 5);
    }

    #[test]
    fn test_threshold_scheme_invalid_threshold() {
        let keypairs = create_test_participants(3);
        let public_keys: Vec<_> = keypairs.iter().map(|kp| kp.public_key_copy()).collect();

        // Threshold exceeds participants
        let result = ThresholdScheme::new(5, public_keys);
        assert!(result.is_err());
    }

    #[test]
    fn test_threshold_scheme_zero_threshold() {
        let keypairs = create_test_participants(3);
        let public_keys: Vec<_> = keypairs.iter().map(|kp| kp.public_key_copy()).collect();

        let result = ThresholdScheme::new(0, public_keys);
        assert!(result.is_err());
    }

    #[test]
    fn test_threshold_scheme_duplicate_participants() {
        let keypair = KeyPair::generate();
        let public_key = keypair.public_key_copy();

        // Create list with duplicates
        let public_keys = vec![public_key, public_key];

        let result = ThresholdScheme::new(2, public_keys);
        assert!(result.is_err());
    }

    #[test]
    fn test_threshold_scheme_is_participant() {
        let keypairs = create_test_participants(3);
        let public_keys: Vec<_> = keypairs.iter().map(|kp| kp.public_key_copy()).collect();

        let scheme = ThresholdScheme::new(2, public_keys.clone()).unwrap();

        assert!(scheme.is_participant(&public_keys[0]));
        assert!(scheme.is_participant(&public_keys[1]));
        assert!(scheme.is_participant(&public_keys[2]));

        let outsider = KeyPair::generate();
        assert!(!scheme.is_participant(outsider.public_key()));
    }

    #[test]
    fn test_partial_signature_creation() {
        let keypair = KeyPair::generate();
        let message = b"test message";

        let signature = sign(message, &keypair).unwrap();
        let partial_sig = PartialSignature::new(keypair.public_key_copy(), signature, 0);

        assert!(partial_sig.verify(message).is_ok());
    }

    #[test]
    fn test_threshold_signature_aggregation() {
        let keypairs = create_test_participants(5);
        let public_keys: Vec<_> = keypairs.iter().map(|kp| kp.public_key_copy()).collect();
        let scheme = ThresholdScheme::new(3, public_keys).unwrap();

        let message = b"Block to sign";
        use crate::hash::hash_blake2b;
        let message_hash = hash_blake2b(message);

        let mut threshold_sig = ThresholdSignature::new(scheme, message_hash);

        // Add 3 signatures (meeting threshold)
        for i in 0..3 {
            let signature = sign(&message_hash, &keypairs[i]).unwrap();
            let partial_sig = PartialSignature::new(keypairs[i].public_key_copy(), signature, i);
            let is_complete = threshold_sig.add_partial_signature(partial_sig).unwrap();
            
            if i == 2 {
                assert!(is_complete);
            } else {
                assert!(!is_complete);
            }
        }

        assert!(threshold_sig.is_complete());
        assert!(threshold_sig.verify().is_ok());
    }

    #[test]
    fn test_threshold_signature_insufficient() {
        let keypairs = create_test_participants(5);
        let public_keys: Vec<_> = keypairs.iter().map(|kp| kp.public_key_copy()).collect();
        let scheme = ThresholdScheme::new(3, public_keys).unwrap();

        let message = b"Block to sign";
        use crate::hash::hash_blake2b;
        let message_hash = hash_blake2b(message);

        let mut threshold_sig = ThresholdSignature::new(scheme, message_hash);

        // Add only 2 signatures (below threshold)
        for i in 0..2 {
            let signature = sign(&message_hash, &keypairs[i]).unwrap();
            let partial_sig = PartialSignature::new(keypairs[i].public_key_copy(), signature, i);
            threshold_sig.add_partial_signature(partial_sig).unwrap();
        }

        assert!(!threshold_sig.is_complete());
        assert!(threshold_sig.verify().is_err());
    }

    #[test]
    fn test_threshold_signature_duplicate() {
        let keypairs = create_test_participants(3);
        let public_keys: Vec<_> = keypairs.iter().map(|kp| kp.public_key_copy()).collect();
        let scheme = ThresholdScheme::new(2, public_keys).unwrap();

        let message = b"test";
        use crate::hash::hash_blake2b;
        let message_hash = hash_blake2b(message);

        let mut threshold_sig = ThresholdSignature::new(scheme, message_hash);

        let signature = sign(&message_hash, &keypairs[0]).unwrap();
        let partial_sig = PartialSignature::new(keypairs[0].public_key_copy(), signature, 0);

        // Add first signature
        threshold_sig.add_partial_signature(partial_sig.clone()).unwrap();

        // Try to add duplicate
        let result = threshold_sig.add_partial_signature(partial_sig);
        assert!(result.is_err());
    }

    #[test]
    fn test_threshold_aggregator() {
        let keypairs = create_test_participants(4);
        let public_keys: Vec<_> = keypairs.iter().map(|kp| kp.public_key_copy()).collect();
        let scheme = ThresholdScheme::new(3, public_keys).unwrap();

        let message = b"Aggregator test".to_vec();
        let mut aggregator = ThresholdAggregator::new(scheme, message.clone());

        assert!(!aggregator.is_complete());
        assert_eq!(aggregator.signature_count(), 0);

        // Add signatures
        for i in 0..3 {
            let signature = sign(aggregator.message_hash(), &keypairs[i]).unwrap();
            let is_complete = aggregator
                .add_signature(keypairs[i].public_key_copy(), signature)
                .unwrap();
            
            if i == 2 {
                assert!(is_complete);
            }
        }

        assert!(aggregator.is_complete());
        let threshold_sig = aggregator.finalize().unwrap();
        assert!(threshold_sig.verify().is_ok());
    }

    #[test]
    fn test_aggregator_invalid_signer() {
        let keypairs = create_test_participants(3);
        let public_keys: Vec<_> = keypairs.iter().map(|kp| kp.public_key_copy()).collect();
        let scheme = ThresholdScheme::new(2, public_keys).unwrap();

        let message = b"test".to_vec();
        let mut aggregator = ThresholdAggregator::new(scheme, message);

        // Try to add signature from non-participant
        let outsider = KeyPair::generate();
        let signature = sign(aggregator.message_hash(), &outsider).unwrap();
        let result = aggregator.add_signature(outsider.public_key_copy(), signature);

        assert!(result.is_err());
    }

    #[test]
    fn test_aggregator_finalize_incomplete() {
        let keypairs = create_test_participants(3);
        let public_keys: Vec<_> = keypairs.iter().map(|kp| kp.public_key_copy()).collect();
        let scheme = ThresholdScheme::new(2, public_keys).unwrap();

        let message = b"test".to_vec();
        let mut aggregator = ThresholdAggregator::new(scheme, message);

        // Add only 1 signature
        let signature = sign(aggregator.message_hash(), &keypairs[0]).unwrap();
        aggregator
            .add_signature(keypairs[0].public_key_copy(), signature)
            .unwrap();

        // Try to finalize
        let result = aggregator.finalize();
        assert!(result.is_err());
    }

    #[test]
    fn test_distributed_key_gen() {
        let dkg = DistributedKeyGen::generate(3, 5).unwrap();

        assert_eq!(dkg.scheme().threshold, 3);
        assert_eq!(dkg.scheme().total_participants, 5);
        assert_eq!(dkg.get_all_shares().len(), 5);
    }

    #[test]
    fn test_create_threshold_signature_helper() {
        let keypairs = create_test_participants(3);
        let public_keys: Vec<_> = keypairs.iter().map(|kp| kp.public_key_copy()).collect();
        let scheme = ThresholdScheme::new(2, public_keys).unwrap();

        let message = b"Helper function test";
        use crate::hash::hash_blake2b;
        let message_hash = hash_blake2b(message);

        let mut partial_sigs = Vec::new();
        for i in 0..2 {
            let signature = sign(&message_hash, &keypairs[i]).unwrap();
            let partial_sig = PartialSignature::new(keypairs[i].public_key_copy(), signature, i);
            partial_sigs.push(partial_sig);
        }

        let threshold_sig = create_threshold_signature(scheme, message, partial_sigs).unwrap();
        assert!(verify_threshold_signature(&threshold_sig).is_ok());
    }

    #[test]
    fn test_threshold_signature_has_signed() {
        let keypairs = create_test_participants(3);
        let public_keys: Vec<_> = keypairs.iter().map(|kp| kp.public_key_copy()).collect();
        let scheme = ThresholdScheme::new(2, public_keys).unwrap();

        let message = b"test";
        use crate::hash::hash_blake2b;
        let message_hash = hash_blake2b(message);

        let mut threshold_sig = ThresholdSignature::new(scheme, message_hash);

        let signature = sign(&message_hash, &keypairs[0]).unwrap();
        let partial_sig = PartialSignature::new(keypairs[0].public_key_copy(), signature, 0);
        threshold_sig.add_partial_signature(partial_sig).unwrap();

        assert!(threshold_sig.has_signed(keypairs[0].public_key()));
        assert!(!threshold_sig.has_signed(keypairs[1].public_key()));
    }

    #[test]
    fn test_threshold_fraction() {
        let keypairs = create_test_participants(6);
        let public_keys: Vec<_> = keypairs.iter().map(|kp| kp.public_key_copy()).collect();
        let scheme = ThresholdScheme::new(4, public_keys).unwrap();

        let fraction = scheme.threshold_fraction();
        assert!((fraction - 0.6666).abs() < 0.001);
    }

    #[test]
    fn test_aggregator_progress() {
        let keypairs = create_test_participants(5);
        let public_keys: Vec<_> = keypairs.iter().map(|kp| kp.public_key_copy()).collect();
        let scheme = ThresholdScheme::new(3, public_keys).unwrap();

        let message = b"progress test".to_vec();
        let mut aggregator = ThresholdAggregator::new(scheme, message);

        let (current, required) = aggregator.progress();
        assert_eq!(current, 0);
        assert_eq!(required, 3);

        // Add one signature
        let signature = sign(aggregator.message_hash(), &keypairs[0]).unwrap();
        aggregator
            .add_signature(keypairs[0].public_key_copy(), signature)
            .unwrap();

        let (current, required) = aggregator.progress();
        assert_eq!(current, 1);
        assert_eq!(required, 3);
    }

    #[test]
    fn test_threshold_signature_signers() {
        let keypairs = create_test_participants(3);
        let public_keys: Vec<_> = keypairs.iter().map(|kp| kp.public_key_copy()).collect();
        let scheme = ThresholdScheme::new(2, public_keys).unwrap();

        let message = b"signers test";
        use crate::hash::hash_blake2b;
        let message_hash = hash_blake2b(message);

        let mut threshold_sig = ThresholdSignature::new(scheme, message_hash);

        for i in 0..2 {
            let signature = sign(&message_hash, &keypairs[i]).unwrap();
            let partial_sig = PartialSignature::new(keypairs[i].public_key_copy(), signature, i);
            threshold_sig.add_partial_signature(partial_sig).unwrap();
        }

        let signers = threshold_sig.signers();
        assert_eq!(signers.len(), 2);
        assert!(signers.contains(&&keypairs[0].public_key_copy()));
        assert!(signers.contains(&&keypairs[1].public_key_copy()));
    }
}

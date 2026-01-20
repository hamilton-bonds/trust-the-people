use crate::keys::{KeyPair, PrivateKey, PublicKey};
use common::{Result, Signature as CommonSignature, VotingError};
use ed25519_dalek::{Signature as Ed25519Signature, Signer, Verifier, SIGNATURE_LENGTH};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A cryptographic signature
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    bytes: [u8; SIGNATURE_LENGTH],
}

impl Signature {
    /// Create signature from bytes
    pub fn from_bytes(bytes: [u8; SIGNATURE_LENGTH]) -> Self {
        Self { bytes }
    }

    /// Get reference to signature bytes
    pub fn as_bytes(&self) -> &[u8; SIGNATURE_LENGTH] {
        &self.bytes
    }

    /// Convert to common Signature type
    pub fn to_common(&self) -> CommonSignature {
        CommonSignature::new(self.bytes)
    }

    /// Create from common Signature type
    pub fn from_common(sig: &CommonSignature) -> Self {
        Self::from_bytes(*sig.as_bytes())
    }

    /// Convert to Ed25519 signature
    pub fn to_ed25519_signature(&self) -> Result<Ed25519Signature> {
        Ed25519Signature::from_bytes(&self.bytes)
            .map_err(|e| VotingError::CryptoError(format!("Invalid signature: {}", e)))
    }

    /// Export to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(&self.bytes)
    }

    /// Import from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| VotingError::CryptoError(format!("Invalid hex: {}", e)))?;

        if bytes.len() != SIGNATURE_LENGTH {
            return Err(VotingError::CryptoError(format!(
                "Invalid signature length: expected {}, got {}",
                SIGNATURE_LENGTH,
                bytes.len()
            )));
        }

        let mut sig_bytes = [0u8; SIGNATURE_LENGTH];
        sig_bytes.copy_from_slice(&bytes);
        Ok(Self::from_bytes(sig_bytes))
    }

    /// Verify this signature is valid for given message and public key
    pub fn verify(&self, message: &[u8], public_key: &PublicKey) -> Result<()> {
        verify(message, self, public_key)
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Sign a message with a keypair
///
/// This function signs arbitrary bytes using Ed25519.
/// The signature is deterministic and can be verified by anyone with the public key.
///
/// # Arguments
/// * `message` - The message bytes to sign
/// * `keypair` - The keypair to sign with
///
/// # Returns
/// A signature that can be verified with the keypair's public key
pub fn sign(message: &[u8], keypair: &KeyPair) -> Result<Signature> {
    let ed25519_keypair = keypair.to_ed25519_keypair()?;
    let signature = ed25519_keypair.sign(message);
    Ok(Signature::from_bytes(signature.to_bytes()))
}

/// Sign a message with just a private key
///
/// This is a convenience function that derives the public key from the private key
/// and signs the message.
///
/// # Arguments
/// * `message` - The message bytes to sign
/// * `private_key` - The private key to sign with
///
/// # Returns
/// A signature that can be verified with the corresponding public key
pub fn sign_with_private(message: &[u8], private_key: &PrivateKey) -> Result<Signature> {
    let keypair = KeyPair::from_private_key(private_key.clone())?;
    sign(message, &keypair)
}

/// Verify a signature
///
/// This function verifies that a signature was created by the holder of the private key
/// corresponding to the given public key.
///
/// # Arguments
/// * `message` - The original message that was signed
/// * `signature` - The signature to verify
/// * `public_key` - The public key to verify against
///
/// # Returns
/// Ok(()) if the signature is valid, Err otherwise
pub fn verify(message: &[u8], signature: &Signature, public_key: &PublicKey) -> Result<()> {
    let ed25519_signature = signature.to_ed25519_signature()?;
    let ed25519_public = public_key.to_ed25519_public()?;

    ed25519_public
        .verify(message, &ed25519_signature)
        .map_err(|_| VotingError::SignatureVerificationFailed)?;

    Ok(())
}

/// Sign serializable data (convenience function)
///
/// This function serializes the data using bincode and then signs it.
/// Useful for signing structured data like transactions or blocks.
///
/// # Arguments
/// * `data` - Any serializable data structure
/// * `keypair` - The keypair to sign with
///
/// # Returns
/// A signature over the serialized data
pub fn sign_data<T: serde::Serialize>(data: &T, keypair: &KeyPair) -> Result<Signature> {
    let serialized = bincode::serialize(data)
        .map_err(|e| VotingError::SerializationError(format!("Serialization failed: {}", e)))?;
    sign(&serialized, keypair)
}

/// Verify a signature over serializable data
///
/// This function serializes the data and verifies the signature.
///
/// # Arguments
/// * `data` - The original data that was signed
/// * `signature` - The signature to verify
/// * `public_key` - The public key to verify against
///
/// # Returns
/// Ok(()) if the signature is valid, Err otherwise
pub fn verify_data<T: serde::Serialize>(
    data: &T,
    signature: &Signature,
    public_key: &PublicKey,
) -> Result<()> {
    let serialized = bincode::serialize(data)
        .map_err(|e| VotingError::SerializationError(format!("Serialization failed: {}", e)))?;
    verify(&serialized, signature, public_key)
}

/// A signed message containing both data and signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedMessage<T> {
    /// The message payload
    pub payload: T,

    /// The signature over the payload
    pub signature: Signature,

    /// The public key of the signer
    pub signer: PublicKey,
}

impl<T: serde::Serialize> SignedMessage<T> {
    /// Create a new signed message
    pub fn new(payload: T, keypair: &KeyPair) -> Result<Self> {
        let signature = sign_data(&payload, keypair)?;
        Ok(Self {
            payload,
            signature,
            signer: keypair.public_key_copy(),
        })
    }

    /// Verify the signature on this message
    pub fn verify(&self) -> Result<()> {
        verify_data(&self.payload, &self.signature, &self.signer)
    }

    /// Get reference to payload
    pub fn payload(&self) -> &T {
        &self.payload
    }

    /// Get signature
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    /// Get signer public key
    pub fn signer(&self) -> &PublicKey {
        &self.signer
    }

    /// Consume the message and return the payload if signature is valid
    pub fn into_payload(self) -> Result<T> {
        self.verify()?;
        Ok(self.payload)
    }
}

/// Batch signature verification for performance
///
/// Verifying multiple signatures individually can be slow. This function
/// verifies multiple signatures more efficiently.
///
/// # Arguments
/// * `messages` - Slice of (message, signature, public_key) tuples
///
/// # Returns
/// Ok(()) if all signatures are valid, Err if any signature is invalid
pub fn verify_batch(messages: &[(&[u8], &Signature, &PublicKey)]) -> Result<()> {
    // Note: Ed25519 batch verification could be optimized further with specialized libraries
    // For now, we verify each signature individually
    for (message, signature, public_key) in messages {
        verify(message, signature, public_key)?;
    }
    Ok(())
}

/// Multi-signature support for threshold signing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiSignature {
    /// Individual signatures
    pub signatures: Vec<(PublicKey, Signature)>,

    /// Threshold required (e.g., 2 out of 3)
    pub threshold: usize,
}

impl MultiSignature {
    /// Create a new multi-signature container
    pub fn new(threshold: usize) -> Self {
        Self {
            signatures: Vec::new(),
            threshold,
        }
    }

    /// Add a signature to the multi-signature
    pub fn add_signature(&mut self, public_key: PublicKey, signature: Signature) {
        self.signatures.push((public_key, signature));
    }

    /// Verify that enough valid signatures are present
    pub fn verify(&self, message: &[u8]) -> Result<()> {
        if self.signatures.len() < self.threshold {
            return Err(VotingError::InsufficientValidators);
        }

        let mut valid_count = 0;
        for (public_key, signature) in &self.signatures {
            if verify(message, signature, public_key).is_ok() {
                valid_count += 1;
            }
        }

        if valid_count >= self.threshold {
            Ok(())
        } else {
            Err(VotingError::InsufficientValidators)
        }
    }

    /// Get number of signatures
    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }

    /// Check if threshold is met
    pub fn is_threshold_met(&self, message: &[u8]) -> bool {
        self.verify(message).is_ok()
    }
}

/// Sign a hash directly (for advanced use cases)
///
/// Most users should use `sign()` instead. This is for cases where
/// you've already computed a hash and want to sign it directly.
///
/// # Arguments
/// * `hash` - A 32-byte hash to sign
/// * `keypair` - The keypair to sign with
///
/// # Returns
/// A signature over the hash
pub fn sign_hash(hash: &[u8; 32], keypair: &KeyPair) -> Result<Signature> {
    sign(hash, keypair)
}

/// Verify a signature over a hash
///
/// # Arguments
/// * `hash` - The 32-byte hash that was signed
/// * `signature` - The signature to verify
/// * `public_key` - The public key to verify against
///
/// # Returns
/// Ok(()) if the signature is valid, Err otherwise
pub fn verify_hash(hash: &[u8; 32], signature: &Signature, public_key: &PublicKey) -> Result<()> {
    verify(hash, signature, public_key)
}

/// Signature verification result with detailed information
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationResult {
    /// Signature is valid
    Valid,
    /// Signature is invalid
    Invalid,
    /// Public key is malformed
    InvalidPublicKey,
    /// Signature is malformed
    InvalidSignature,
}

impl VerificationResult {
    pub fn is_valid(&self) -> bool {
        matches!(self, VerificationResult::Valid)
    }
}

/// Verify a signature and return detailed result
///
/// This is useful when you want to distinguish between different failure modes.
///
/// # Arguments
/// * `message` - The message to verify
/// * `signature` - The signature to verify
/// * `public_key` - The public key to verify against
///
/// # Returns
/// A VerificationResult with detailed information
pub fn verify_detailed(
    message: &[u8],
    signature: &Signature,
    public_key: &PublicKey,
) -> VerificationResult {
    let ed25519_signature = match signature.to_ed25519_signature() {
        Ok(sig) => sig,
        Err(_) => return VerificationResult::InvalidSignature,
    };

    let ed25519_public = match public_key.to_ed25519_public() {
        Ok(pk) => pk,
        Err(_) => return VerificationResult::InvalidPublicKey,
    };

    match ed25519_public.verify(message, &ed25519_signature) {
        Ok(_) => VerificationResult::Valid,
        Err(_) => VerificationResult::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_and_verify() {
        let keypair = KeyPair::generate();
        let message = b"Hello, blockchain!";

        let signature = sign(message, &keypair).unwrap();
        assert!(verify(message, &signature, keypair.public_key()).is_ok());
    }

    #[test]
    fn test_sign_with_private() {
        let keypair = KeyPair::generate();
        let message = b"Test message";

        let signature = sign_with_private(message, keypair.private_key()).unwrap();
        assert!(verify(message, &signature, keypair.public_key()).is_ok());
    }

    #[test]
    fn test_verify_wrong_message() {
        let keypair = KeyPair::generate();
        let message = b"Original message";
        let wrong_message = b"Different message";

        let signature = sign(message, &keypair).unwrap();
        assert!(verify(wrong_message, &signature, keypair.public_key()).is_err());
    }

    #[test]
    fn test_verify_wrong_key() {
        let keypair1 = KeyPair::generate();
        let keypair2 = KeyPair::generate();
        let message = b"Test message";

        let signature = sign(message, &keypair1).unwrap();
        assert!(verify(message, &signature, keypair2.public_key()).is_err());
    }

    #[test]
    fn test_signature_hex() {
        let keypair = KeyPair::generate();
        let message = b"Test";

        let signature = sign(message, &keypair).unwrap();
        let hex = signature.to_hex();
        let decoded = Signature::from_hex(&hex).unwrap();

        assert_eq!(signature, decoded);
        assert!(verify(message, &decoded, keypair.public_key()).is_ok());
    }

    #[test]
    fn test_sign_data() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct TestData {
            value: u64,
            text: String,
        }

        let keypair = KeyPair::generate();
        let data = TestData {
            value: 42,
            text: "test".to_string(),
        };

        let signature = sign_data(&data, &keypair).unwrap();
        assert!(verify_data(&data, &signature, keypair.public_key()).is_ok());
    }

    #[test]
    fn test_verify_data_wrong_data() {
        #[derive(serde::Serialize)]
        struct TestData {
            value: u64,
        }

        let keypair = KeyPair::generate();
        let data1 = TestData { value: 42 };
        let data2 = TestData { value: 99 };

        let signature = sign_data(&data1, &keypair).unwrap();
        assert!(verify_data(&data2, &signature, keypair.public_key()).is_err());
    }

    #[test]
    fn test_signed_message() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct Message {
            text: String,
        }

        let keypair = KeyPair::generate();
        let msg = Message {
            text: "Hello".to_string(),
        };

        let signed = SignedMessage::new(msg, &keypair).unwrap();
        assert!(signed.verify().is_ok());
        assert_eq!(signed.signer(), keypair.public_key());
    }

    #[test]
    fn test_signed_message_into_payload() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct Message {
            value: u64,
        }

        let keypair = KeyPair::generate();
        let msg = Message { value: 123 };

        let signed = SignedMessage::new(msg.clone(), &keypair).unwrap();
        let payload = signed.into_payload().unwrap();
        assert_eq!(payload, msg);
    }

    #[test]
    fn test_verify_batch() {
        let keypair1 = KeyPair::generate();
        let keypair2 = KeyPair::generate();

        let msg1 = b"Message 1";
        let msg2 = b"Message 2";

        let sig1 = sign(msg1, &keypair1).unwrap();
        let sig2 = sign(msg2, &keypair2).unwrap();

        let batch = vec![
            (msg1.as_ref(), &sig1, keypair1.public_key()),
            (msg2.as_ref(), &sig2, keypair2.public_key()),
        ];

        assert!(verify_batch(&batch).is_ok());
    }

    #[test]
    fn test_verify_batch_with_invalid() {
        let keypair1 = KeyPair::generate();
        let keypair2 = KeyPair::generate();

        let msg1 = b"Message 1";
        let msg2 = b"Message 2";

        let sig1 = sign(msg1, &keypair1).unwrap();
        let sig2 = sign(msg2, &keypair2).unwrap();

        // Use wrong key for second signature
        let batch = vec![
            (msg1.as_ref(), &sig1, keypair1.public_key()),
            (msg2.as_ref(), &sig2, keypair1.public_key()), // Wrong!
        ];

        assert!(verify_batch(&batch).is_err());
    }

    #[test]
    fn test_multi_signature() {
        let keypair1 = KeyPair::generate();
        let keypair2 = KeyPair::generate();
        let keypair3 = KeyPair::generate();

        let message = b"Multi-sig message";

        let sig1 = sign(message, &keypair1).unwrap();
        let sig2 = sign(message, &keypair2).unwrap();
        let sig3 = sign(message, &keypair3).unwrap();

        // 2 of 3 threshold
        let mut multi_sig = MultiSignature::new(2);
        multi_sig.add_signature(keypair1.public_key_copy(), sig1);
        multi_sig.add_signature(keypair2.public_key_copy(), sig2);

        assert!(multi_sig.verify(message).is_ok());
        assert_eq!(multi_sig.signature_count(), 2);
        assert!(multi_sig.is_threshold_met(message));

        // Add third signature
        multi_sig.add_signature(keypair3.public_key_copy(), sig3);
        assert_eq!(multi_sig.signature_count(), 3);
        assert!(multi_sig.verify(message).is_ok());
    }

    #[test]
    fn test_multi_signature_insufficient() {
        let keypair1 = KeyPair::generate();
        let message = b"Test";

        let sig1 = sign(message, &keypair1).unwrap();

        // 2 of 3 threshold but only 1 signature
        let mut multi_sig = MultiSignature::new(2);
        multi_sig.add_signature(keypair1.public_key_copy(), sig1);

        assert!(multi_sig.verify(message).is_err());
        assert!(!multi_sig.is_threshold_met(message));
    }

    #[test]
    fn test_sign_hash() {
        let keypair = KeyPair::generate();
        let hash = [42u8; 32];

        let signature = sign_hash(&hash, &keypair).unwrap();
        assert!(verify_hash(&hash, &signature, keypair.public_key()).is_ok());
    }

    #[test]
    fn test_verify_detailed() {
        let keypair = KeyPair::generate();
        let message = b"Test";

        let signature = sign(message, &keypair).unwrap();

        let result = verify_detailed(message, &signature, keypair.public_key());
        assert_eq!(result, VerificationResult::Valid);
        assert!(result.is_valid());
    }

    #[test]
    fn test_verify_detailed_invalid() {
        let keypair1 = KeyPair::generate();
        let keypair2 = KeyPair::generate();
        let message = b"Test";

        let signature = sign(message, &keypair1).unwrap();

        let result = verify_detailed(message, &signature, keypair2.public_key());
        assert_eq!(result, VerificationResult::Invalid);
        assert!(!result.is_valid());
    }

    #[test]
    fn test_signature_deterministic() {
        let keypair = KeyPair::generate();
        let message = b"Deterministic test";

        let sig1 = sign(message, &keypair).unwrap();
        let sig2 = sign(message, &keypair).unwrap();

        // Ed25519 signatures are deterministic
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_signature_conversion() {
        let keypair = KeyPair::generate();
        let message = b"Test";

        let signature = sign(message, &keypair).unwrap();
        let common_sig = signature.to_common();
        let converted_back = Signature::from_common(&common_sig);

        assert_eq!(signature, converted_back);
        assert!(verify(message, &converted_back, keypair.public_key()).is_ok());
    }

    #[test]
    fn test_invalid_signature_bytes() {
        let invalid_hex = "invalid_hex_string";
        let result = Signature::from_hex(invalid_hex);
        assert!(result.is_err());
    }

    #[test]
    fn test_signature_display() {
        let keypair = KeyPair::generate();
        let message = b"Test";

        let signature = sign(message, &keypair).unwrap();
        let display_str = format!("{}", signature);

        assert_eq!(display_str, signature.to_hex());
        assert_eq!(display_str.len(), SIGNATURE_LENGTH * 2); // Hex is 2 chars per byte
    }
}

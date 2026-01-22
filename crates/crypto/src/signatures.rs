//! Digital signature operations using Ed25519
//!
//! This module provides signing and verification functionality using the
//! Ed25519 signature scheme, which is fast and secure.

use crate::KeyPair;
use crate::keys::PublicKey;
use common::{Result, VotingError};
use ed25519_dalek::{Signature as Ed25519Signature, Signer, Verifier, VerifyingKey};
use std::fmt;

/// Signature length in bytes (Ed25519 signatures are 64 bytes)
pub const SIGNATURE_LENGTH: usize = 64;

/// A cryptographic signature
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature {
    bytes: [u8; SIGNATURE_LENGTH],
}

impl Signature {
    /// Create a new signature from bytes
    pub fn new(bytes: [u8; SIGNATURE_LENGTH]) -> Self {
        Self { bytes }
    }

    /// Get signature as bytes
    pub fn as_bytes(&self) -> &[u8; SIGNATURE_LENGTH] {
        &self.bytes
    }

    /// Convert to Ed25519 signature
    pub fn to_ed25519(&self) -> Result<Ed25519Signature> {
        Ok(Ed25519Signature::from_bytes(&self.bytes))
    }

    /// Create from Ed25519 signature
    pub fn from_ed25519(sig: &Ed25519Signature) -> Self {
        Self {
            bytes: sig.to_bytes(),
        }
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.bytes)
    }

    /// Create from hex string
    pub fn from_hex(s: &str) -> Result<Self> {
        let bytes = hex::decode(s)
            .map_err(|e| VotingError::CryptoError(format!("Invalid hex: {}", e)))?;
        if bytes.len() != SIGNATURE_LENGTH {
            return Err(VotingError::CryptoError(format!(
                "Invalid signature length: expected {}, got {}",
                SIGNATURE_LENGTH,
                bytes.len()
            )));
        }
        let mut arr = [0u8; SIGNATURE_LENGTH];
        arr.copy_from_slice(&bytes);
        Ok(Self { bytes: arr })
    }

    /// Convert to common::Signature
    pub fn to_common(&self) -> common::Signature {
        common::Signature::new(self.bytes)
    }

    /// Create from common::Signature
    pub fn from_common(sig: &common::Signature) -> Self {
        Self::new(*sig.as_bytes())
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

// Custom Serialize implementation for 64-byte array
impl serde::Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.bytes)
    }
}

// Custom Deserialize implementation for 64-byte array
impl<'de> serde::Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SignatureVisitor;

        impl<'de> serde::de::Visitor<'de> for SignatureVisitor {
            type Value = Signature;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a 64-byte signature")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v.len() != SIGNATURE_LENGTH {
                    return Err(E::custom(format!(
                        "expected {} bytes, got {}",
                        SIGNATURE_LENGTH,
                        v.len()
                    )));
                }
                let mut arr = [0u8; SIGNATURE_LENGTH];
                arr.copy_from_slice(v);
                Ok(Signature { bytes: arr })
            }

            fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut arr = [0u8; SIGNATURE_LENGTH];
                for i in 0..SIGNATURE_LENGTH {
                    arr[i] = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
                }
                Ok(Signature { bytes: arr })
            }
        }

        deserializer.deserialize_bytes(SignatureVisitor)
    }
}

/// Sign a message with a keypair
///
/// # Arguments
/// * `message` - The message to sign
/// * `keypair` - The keypair to sign with
///
/// # Returns
/// A signature over the message
pub fn sign(message: &[u8], keypair: &KeyPair) -> Result<Signature> {
    let signing_key = keypair.to_signing_key()?;
    let signature = signing_key.sign(message);
    Ok(Signature::from_ed25519(&signature))
}

/// Verify a signature
///
/// # Arguments
/// * `message` - The original message
/// * `signature` - The signature to verify
/// * `public_key` - The public key to verify against
///
/// # Returns
/// Ok(()) if the signature is valid, Err otherwise
pub fn verify(message: &[u8], signature: &Signature, public_key: &PublicKey) -> Result<()> {
    let verifying_key = VerifyingKey::from_bytes(public_key.as_bytes())
        .map_err(|e| VotingError::CryptoError(format!("Invalid public key: {}", e)))?;

    let ed25519_sig = signature.to_ed25519()?;

    verifying_key
        .verify(message, &ed25519_sig)
        .map_err(|_| VotingError::SignatureVerificationFailed)
}

/// Sign a message with just a private key
pub fn sign_with_private(message: &[u8], private_key: &[u8; 32]) -> Result<Signature> {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(private_key);
    let signature = signing_key.sign(message);
    Ok(Signature::from_ed25519(&signature))
}

/// Sign serializable data
pub fn sign_data<T: serde::Serialize>(data: &T, keypair: &KeyPair) -> Result<Signature> {
    let serialized = bincode::serialize(data)
        .map_err(|e| VotingError::SerializationError(format!("Serialization failed: {}", e)))?;
    sign(&serialized, keypair)
}

/// Verify a signature over serializable data
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
#[derive(Debug, Clone)]
pub struct SignedMessage<T> {
    pub payload: T,
    pub signature: Signature,
    pub signer: PublicKey,
}

impl<T: serde::Serialize> SignedMessage<T> {
    pub fn new(payload: T, keypair: &KeyPair) -> Result<Self> {
        let signature = sign_data(&payload, keypair)?;
        Ok(Self {
            payload,
            signature,
            signer: *keypair.public_key(),
        })
    }

    pub fn verify(&self) -> Result<()> {
        verify_data(&self.payload, &self.signature, &self.signer)
    }

    pub fn payload(&self) -> &T {
        &self.payload
    }

    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    pub fn signer(&self) -> &PublicKey {
        &self.signer
    }

    pub fn into_payload(self) -> Result<T> {
        self.verify()?;
        Ok(self.payload)
    }
}

/// Batch signature verification
pub fn verify_batch(items: &[(&[u8], &Signature, &PublicKey)]) -> Result<()> {
    for (message, signature, public_key) in items {
        verify(message, signature, public_key)?;
    }
    Ok(())
}

/// Multi-signature support
pub struct MultiSignature {
    threshold: usize,
    signatures: Vec<(PublicKey, Signature)>,
}

impl MultiSignature {
    pub fn new(threshold: usize) -> Self {
        Self {
            threshold,
            signatures: Vec::new(),
        }
    }

    pub fn add_signature(&mut self, public_key: PublicKey, signature: Signature) {
        self.signatures.push((public_key, signature));
    }

    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }

    pub fn is_threshold_met(&self, message: &[u8]) -> bool {
        if self.signatures.len() < self.threshold {
            return false;
        }
        self.verify(message).is_ok()
    }

    pub fn verify(&self, message: &[u8]) -> Result<()> {
        if self.signatures.len() < self.threshold {
            return Err(VotingError::CryptoError(format!(
                "Need {} signatures, got {}",
                self.threshold,
                self.signatures.len()
            )));
        }

        for (public_key, signature) in &self.signatures {
            verify(message, signature, public_key)?;
        }

        Ok(())
    }

    pub fn signatures(&self) -> &[(PublicKey, Signature)] {
        &self.signatures
    }

    pub fn threshold(&self) -> usize {
        self.threshold
    }
}

/// Sign a hash directly
pub fn sign_hash(hash: &[u8; 32], keypair: &KeyPair) -> Result<Signature> {
    sign(hash, keypair)
}

/// Verify a signature over a hash
pub fn verify_hash(hash: &[u8; 32], signature: &Signature, public_key: &PublicKey) -> Result<()> {
    verify(hash, signature, public_key)
}

/// Verification result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationResult {
    Valid,
    Invalid,
}

impl VerificationResult {
    pub fn is_valid(&self) -> bool {
        matches!(self, VerificationResult::Valid)
    }
}

/// Verify with detailed result
pub fn verify_detailed(
    message: &[u8],
    signature: &Signature,
    public_key: &PublicKey,
) -> VerificationResult {
    match verify(message, signature, public_key) {
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

        let signed = SignedMessage::new(msg, &keypair).unwrap();
        let payload = signed.into_payload().unwrap();
        assert_eq!(payload.value, 123);
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

        let batch = vec![
            (msg1.as_ref(), &sig1, keypair1.public_key()),
            (msg2.as_ref(), &sig2, keypair1.public_key()),
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

        let mut multi_sig = MultiSignature::new(2);
        multi_sig.add_signature(keypair1.public_key_copy(), sig1);
        multi_sig.add_signature(keypair2.public_key_copy(), sig2);

        assert!(multi_sig.verify(message).is_ok());
        assert_eq!(multi_sig.signature_count(), 2);
        assert!(multi_sig.is_threshold_met(message));

        multi_sig.add_signature(keypair3.public_key_copy(), sig3);
        assert_eq!(multi_sig.signature_count(), 3);
        assert!(multi_sig.verify(message).is_ok());
    }

    #[test]
    fn test_multi_signature_insufficient() {
        let keypair1 = KeyPair::generate();
        let message = b"Test";

        let sig1 = sign(message, &keypair1).unwrap();

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
        assert_eq!(display_str.len(), SIGNATURE_LENGTH * 2);
    }
}

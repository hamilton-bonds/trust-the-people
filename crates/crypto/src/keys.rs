use common::{PublicKey as CommonPublicKey, Result, VotingError};
use ed25519_dalek::{
    SigningKey, VerifyingKey,
    PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::Zeroize;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// A private key wrapper that provides memory safety and zeroization
#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct PrivateKey {
    /// The raw secret key bytes (32 bytes for Ed25519)
    bytes: [u8; SECRET_KEY_LENGTH],
}

impl PrivateKey {
    /// Create a new private key from bytes
    pub fn from_bytes(bytes: [u8; SECRET_KEY_LENGTH]) -> Self {
        Self { bytes }
    }

    /// Get reference to key bytes (use carefully)
    pub fn as_bytes(&self) -> &[u8; SECRET_KEY_LENGTH] {
        &self.bytes
    }

    /// Convert to Ed25519 signing key
    pub fn to_signing_key(&self) -> Result<SigningKey> {
        SigningKey::from_bytes(&self.bytes);
        Ok(SigningKey::from_bytes(&self.bytes))
    }

    /// Export to hex string (use with caution)
    pub fn to_hex(&self) -> String {
        hex::encode(&self.bytes)
    }

    /// Import from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| VotingError::CryptoError(format!("Invalid hex: {}", e)))?;

        if bytes.len() != SECRET_KEY_LENGTH {
            return Err(VotingError::CryptoError(format!(
                "Invalid key length: expected {}, got {}",
                SECRET_KEY_LENGTH,
                bytes.len()
            )));
        }

        let mut key_bytes = [0u8; SECRET_KEY_LENGTH];
        key_bytes.copy_from_slice(&bytes);
        Ok(Self::from_bytes(key_bytes))
    }

    /// Export to PEM format
    pub fn to_pem(&self) -> String {
        let b64 = BASE64.encode(&self.bytes);
        format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----",
            b64
        )
    }

    /// Import from PEM format
    pub fn from_pem(pem: &str) -> Result<Self> {
        let trimmed = pem
            .lines()
            .filter(|line| !line.starts_with("-----"))
            .collect::<String>();

        let bytes = BASE64.decode(&trimmed)
            .map_err(|e| VotingError::CryptoError(format!("Invalid PEM: {}", e)))?;

        if bytes.len() != SECRET_KEY_LENGTH {
            return Err(VotingError::CryptoError(format!(
                "Invalid PEM key length: expected {}, got {}",
                SECRET_KEY_LENGTH,
                bytes.len()
            )));
        }

        let mut key_bytes = [0u8; SECRET_KEY_LENGTH];
        key_bytes.copy_from_slice(&bytes);
        Ok(Self::from_bytes(key_bytes))
    }

    /// Save to encrypted file
    pub fn save_to_file(&self, path: &std::path::Path, password: &str) -> Result<()> {
        let encrypted = self.encrypt_with_password(password)?;
        std::fs::write(path, encrypted)
            .map_err(|e| VotingError::IoError(e))?;
        Ok(())
    }

    /// Load from encrypted file
    pub fn load_from_file(path: &std::path::Path, password: &str) -> Result<Self> {
        let encrypted = std::fs::read(path)
            .map_err(|e| VotingError::IoError(e))?;
        Self::decrypt_with_password(&encrypted, password)
    }

    /// Encrypt private key with password
    pub fn encrypt_with_password(&self, password: &str) -> Result<Vec<u8>> {
        use chacha20poly1305::{
            aead::{Aead, KeyInit},
            ChaCha20Poly1305, Nonce,
        };
        use argon2::{Argon2, PasswordHasher};
        use argon2::password_hash::SaltString;

        // Generate random salt
        let salt = SaltString::generate(&mut OsRng);
        
        // Derive encryption key from password using Argon2
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| VotingError::CryptoError(format!("Password hashing failed: {}", e)))?;

        let hash = password_hash.hash.ok_or_else(|| {
            VotingError::CryptoError("Failed to derive key".to_string())
        })?;
        let key_bytes = hash.as_bytes();

        if key_bytes.len() < 32 {
            return Err(VotingError::CryptoError("Derived key too short".to_string()));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes[..32]);

        // Encrypt the private key
        let cipher = ChaCha20Poly1305::new(&key.into());
        let nonce = Nonce::from_slice(b"unique nonce"); // In production, use random nonce
        
        let ciphertext = cipher
            .encrypt(nonce, self.bytes.as_ref())
            .map_err(|e| VotingError::CryptoError(format!("Encryption failed: {}", e)))?;

        // Combine salt and ciphertext
        let mut result = Vec::new();
        result.extend_from_slice(salt.as_str().as_bytes());
        result.push(0); // Separator
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt private key with password
    pub fn decrypt_with_password(encrypted: &[u8], password: &str) -> Result<Self> {
        use chacha20poly1305::{
            aead::{Aead, KeyInit},
            ChaCha20Poly1305, Nonce,
        };
        use argon2::{Argon2, PasswordHasher};
        use argon2::password_hash::SaltString;

        // Split salt and ciphertext
        let separator_pos = encrypted
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| VotingError::CryptoError("Invalid encrypted data".to_string()))?;

        let salt_bytes = &encrypted[..separator_pos];
        let ciphertext = &encrypted[separator_pos + 1..];

        let salt_str = std::str::from_utf8(salt_bytes)
            .map_err(|_| VotingError::CryptoError("Invalid salt encoding".to_string()))?;

        let salt = SaltString::from_b64(salt_str)
            .map_err(|e| VotingError::CryptoError(format!("Invalid salt: {}", e)))?;

        // Derive decryption key
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| VotingError::CryptoError(format!("Password hashing failed: {}", e)))?;

        let hash = password_hash.hash.ok_or_else(|| {
            VotingError::CryptoError("Failed to derive key".to_string())
        })?;
        let key_bytes = hash.as_bytes();

        if key_bytes.len() < 32 {
            return Err(VotingError::CryptoError("Derived key too short".to_string()));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes[..32]);

        // Decrypt
        let cipher = ChaCha20Poly1305::new(&key.into());
        let nonce = Nonce::from_slice(b"unique nonce");

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| VotingError::CryptoError("Decryption failed (wrong password?)".to_string()))?;

        if plaintext.len() != SECRET_KEY_LENGTH {
            return Err(VotingError::CryptoError("Decrypted data has invalid length".to_string()));
        }

        let mut key_bytes = [0u8; SECRET_KEY_LENGTH];
        key_bytes.copy_from_slice(&plaintext);
        Ok(Self::from_bytes(key_bytes))
    }
}

impl fmt::Display for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED PRIVATE KEY]")
    }
}

impl fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PrivateKey([REDACTED])")
    }
}

/// A public key wrapper
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PublicKey {
    bytes: [u8; PUBLIC_KEY_LENGTH],
}

impl PublicKey {
    /// Create from bytes
    pub fn from_bytes(bytes: [u8; PUBLIC_KEY_LENGTH]) -> Self {
        Self { bytes }
    }

    /// Get reference to bytes
    pub fn as_bytes(&self) -> &[u8; PUBLIC_KEY_LENGTH] {
        &self.bytes
    }

    /// Convert to common::PublicKey
    pub fn to_common(&self) -> common::PublicKey {
        common::PublicKey::new(self.bytes)
    }
    
    /// Convert from common::PublicKey
    pub fn from_common(pk: &common::PublicKey) -> Self {
        Self::from_bytes(*pk.as_bytes())
    }

    /// Convert to Ed25519 verifying key
    pub fn to_verifying_key(&self) -> Result<VerifyingKey> {
        VerifyingKey::from_bytes(&self.bytes)
            .map_err(|e| VotingError::CryptoError(format!("Invalid public key: {}", e)))
    }

    /// Export to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(&self.bytes)
    }

    /// Import from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| VotingError::CryptoError(format!("Invalid hex: {}", e)))?;

        if bytes.len() != PUBLIC_KEY_LENGTH {
            return Err(VotingError::CryptoError(format!(
                "Invalid key length: expected {}, got {}",
                PUBLIC_KEY_LENGTH,
                bytes.len()
            )));
        }

        let mut key_bytes = [0u8; PUBLIC_KEY_LENGTH];
        key_bytes.copy_from_slice(&bytes);
        Ok(Self::from_bytes(key_bytes))
    }

    /// Verify this key is valid
    pub fn verify(&self) -> Result<()> {
        self.to_verifying_key()?;
        Ok(())
    }
    
    /// Convert to Address
    pub fn to_address(&self) -> common::Address {
        use crate::hash::hash_blake2b;
        let hash = hash_blake2b(self.as_bytes());
        common::Address::new(hash)
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// A cryptographic keypair (public and private key)
pub struct KeyPair {
    /// The private key
    private: PrivateKey,

    /// The public key
    public: PublicKey,
}

impl KeyPair {
    /// Generate a new random keypair
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        let private = PrivateKey::from_bytes(signing_key.to_bytes());
        let public = PublicKey::from_bytes(verifying_key.to_bytes());

        Self { private, public }
    }

    /// Create from existing private key
    pub fn from_private_key(private: PrivateKey) -> Result<Self> {
        let signing_key = private.to_signing_key()?;
        let verifying_key = signing_key.verifying_key();
        let public = PublicKey::from_bytes(verifying_key.to_bytes());

        Ok(Self { private, public })
    }

    /// Create from private key bytes
    pub fn from_bytes(private_bytes: [u8; SECRET_KEY_LENGTH]) -> Result<Self> {
        let private = PrivateKey::from_bytes(private_bytes);
        Self::from_private_key(private)
    }

    /// Get reference to private key
    pub fn private_key(&self) -> &PrivateKey {
        &self.private
    }

    /// Get reference to public key
    pub fn public_key(&self) -> &PublicKey {
        &self.public
    }

    /// Get copy of public key
    pub fn public_key_copy(&self) -> PublicKey {
        self.public
    }

    /// Convert to Ed25519 signing key
    pub fn to_signing_key(&self) -> Result<SigningKey> {
        self.private.to_signing_key()
    }

    /// Save keypair to file (encrypted with password)
    pub fn save_to_file(&self, path: &std::path::Path, password: &str) -> Result<()> {
        self.private.save_to_file(path, password)
    }

    /// Load keypair from file (decrypt with password)
    pub fn load_from_file(path: &std::path::Path, password: &str) -> Result<Self> {
        let private = PrivateKey::load_from_file(path, password)?;
        Self::from_private_key(private)
    }
}

impl fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyPair")
            .field("private", &"[REDACTED]")
            .field("public", &self.public)
            .finish()
    }
}

/// Key derivation function for hierarchical keys
pub struct KeyDerivation;

impl KeyDerivation {
    /// Derive a child key from a master key and index
    pub fn derive_child(master: &PrivateKey, index: u32) -> Result<PrivateKey> {
        use blake2::{Blake2b, Digest};
        use blake2::digest::consts::U32;

        let mut hasher = Blake2b::<U32>::new();
        hasher.update(master.as_bytes());
        hasher.update(&index.to_le_bytes());
        let result = hasher.finalize();

        let mut derived_bytes = [0u8; SECRET_KEY_LENGTH];
        derived_bytes.copy_from_slice(&result);

        Ok(PrivateKey::from_bytes(derived_bytes))
    }

    /// Derive multiple child keys
    pub fn derive_children(master: &PrivateKey, count: u32) -> Result<Vec<PrivateKey>> {
        (0..count)
            .map(|i| Self::derive_child(master, i))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let keypair = KeyPair::generate();
        assert_eq!(keypair.public_key().as_bytes().len(), PUBLIC_KEY_LENGTH);
        assert_eq!(
            keypair.private_key().as_bytes().len(),
            SECRET_KEY_LENGTH
        );
    }

    #[test]
    fn test_keypair_from_private() {
        let keypair1 = KeyPair::generate();
        let private_bytes = *keypair1.private_key().as_bytes();

        let keypair2 = KeyPair::from_bytes(private_bytes).unwrap();
        assert_eq!(keypair1.public_key(), keypair2.public_key());
    }

    #[test]
    fn test_public_key_hex() {
        let keypair = KeyPair::generate();
        let public = keypair.public_key();

        let hex = public.to_hex();
        let decoded = PublicKey::from_hex(&hex).unwrap();

        assert_eq!(public, &decoded);
    }

    #[test]
    fn test_private_key_hex() {
        let keypair = KeyPair::generate();
        let private = keypair.private_key();

        let hex = private.to_hex();
        let decoded = PrivateKey::from_hex(&hex).unwrap();

        assert_eq!(private.as_bytes(), decoded.as_bytes());
    }

    #[test]
    fn test_private_key_pem() {
        let keypair = KeyPair::generate();
        let private = keypair.private_key();

        let pem = private.to_pem();
        assert!(pem.contains("-----BEGIN PRIVATE KEY-----"));
        assert!(pem.contains("-----END PRIVATE KEY-----"));

        let decoded = PrivateKey::from_pem(&pem).unwrap();
        assert_eq!(private.as_bytes(), decoded.as_bytes());
    }

    #[test]
    fn test_private_key_encryption() {
        let keypair = KeyPair::generate();
        let private = keypair.private_key();

        let password = "test_password_123";
        let encrypted = private.encrypt_with_password(password).unwrap();
        let decrypted = PrivateKey::decrypt_with_password(&encrypted, password).unwrap();

        assert_eq!(private.as_bytes(), decrypted.as_bytes());
    }

    #[test]
    fn test_private_key_wrong_password() {
        let keypair = KeyPair::generate();
        let private = keypair.private_key();

        let encrypted = private.encrypt_with_password("correct").unwrap();
        let result = PrivateKey::decrypt_with_password(&encrypted, "wrong");

        assert!(result.is_err());
    }

    #[test]
    fn test_key_derivation() {
        let master = KeyPair::generate();
        let child1 = KeyDerivation::derive_child(master.private_key(), 0).unwrap();
        let child2 = KeyDerivation::derive_child(master.private_key(), 1).unwrap();

        // Children should be different
        assert_ne!(child1.as_bytes(), child2.as_bytes());

        // Derivation should be deterministic
        let child1_again = KeyDerivation::derive_child(master.private_key(), 0).unwrap();
        assert_eq!(child1.as_bytes(), child1_again.as_bytes());
    }

    #[test]
    fn test_derive_multiple_children() {
        let master = KeyPair::generate();
        let children = KeyDerivation::derive_children(master.private_key(), 5).unwrap();

        assert_eq!(children.len(), 5);

        // All children should be unique
        for i in 0..children.len() {
            for j in i + 1..children.len() {
                assert_ne!(children[i].as_bytes(), children[j].as_bytes());
            }
        }
    }

    #[test]
    fn test_public_key_verify() {
        let keypair = KeyPair::generate();
        let public = keypair.public_key();

        assert!(public.verify().is_ok());
    }

    #[test]
    fn test_invalid_public_key() {
        let invalid_bytes = [0u8; PUBLIC_KEY_LENGTH];
        let public = PublicKey::from_bytes(invalid_bytes);

        // Invalid key should fail verification
        assert!(public.verify().is_err());
    }

    #[test]
    fn test_keypair_debug_redaction() {
        let keypair = KeyPair::generate();
        let debug_str = format!("{:?}", keypair);

        assert!(debug_str.contains("[REDACTED]"));
        assert!(!debug_str.contains(&hex::encode(keypair.private_key().as_bytes())));
    }

    #[test]
    fn test_private_key_display_redaction() {
        let keypair = KeyPair::generate();
        let display_str = format!("{}", keypair.private_key());

        assert!(display_str.contains("REDACTED"));
    }

    #[test]
    fn test_public_key_conversion() {
        let keypair = KeyPair::generate();
        let public = keypair.public_key();

        let common_pk = public.to_common();
        let converted_back = PublicKey::from_common(&common_pk);

        assert_eq!(public, &converted_back);
    }

    #[test]
    fn test_keypair_file_operations() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_key.enc");

        let keypair = KeyPair::generate();
        let password = "test_password";

        // Save to file
        keypair.save_to_file(&file_path, password).unwrap();

        // Load from file
        let loaded_keypair = KeyPair::load_from_file(&file_path, password).unwrap();

        assert_eq!(
            keypair.public_key(),
            loaded_keypair.public_key()
        );
    }
}

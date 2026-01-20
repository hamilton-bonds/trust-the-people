use common::{PublicKey as CommonPublicKey, Result, VotingError};
use ed25519_dalek::{
    Keypair as Ed25519Keypair, PublicKey as Ed25519PublicKey, SecretKey as Ed25519SecretKey,
    PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::Zeroize;

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

    /// Convert to Ed25519 secret key
    pub fn to_ed25519_secret(&self) -> Result<Ed25519SecretKey> {
        Ed25519SecretKey::from_bytes(&self.bytes)
            .map_err(|e| VotingError::CryptoError(format!("Invalid secret key: {}", e)))
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
        let b64 = base64::encode(&self.bytes);
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

        let bytes = base64::decode(&trimmed)
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
            .map_err(|e| VotingError::CryptoError(format!("Failed to save key: {}", e)))?;
        Ok(())
    }

    /// Load from encrypted file
    pub fn load_from_file(path: &std::path::Path, password: &str) -> Result<Self> {
        let encrypted = std::fs::read(path)
            .map_err(|e| VotingError::CryptoError(format!("Failed to read key: {}", e)))?;
        Self::decrypt_with_password(&encrypted, password)
    }

    /// Encrypt private key with password using Argon2 + ChaCha20-Poly1305
    fn encrypt_with_password(&self, password: &str) -> Result<Vec<u8>> {
        use argon2::{Argon2, PasswordHasher};
        use argon2::password_hash::{rand_core::OsRng, SaltString};
        use chacha20poly1305::{
            aead::{Aead, KeyInit, OsRng as ChaChaRng},
            ChaCha20Poly1305, Nonce,
        };

        // Generate salt for Argon2
        let salt = SaltString::generate(&mut OsRng);

        // Derive key from password
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| VotingError::CryptoError(format!("Password hashing failed: {}", e)))?;

        let derived_key_bytes = password_hash
            .hash
            .ok_or_else(|| VotingError::CryptoError("No hash output".to_string()))?;

        // Use first 32 bytes as encryption key
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&derived_key_bytes.as_bytes()[..32]);

        let cipher = ChaCha20Poly1305::new(&key_bytes.into());

        // Generate nonce
        let nonce = ChaCha20Poly1305::generate_nonce(&mut ChaChaRng);

        // Encrypt the private key
        let ciphertext = cipher
            .encrypt(&nonce, self.bytes.as_ref())
            .map_err(|e| VotingError::CryptoError(format!("Encryption failed: {}", e)))?;

        // Format: [salt_len(1)][salt][nonce(12)][ciphertext]
        let mut output = Vec::new();
        let salt_bytes = salt.as_str().as_bytes();
        output.push(salt_bytes.len() as u8);
        output.extend_from_slice(salt_bytes);
        output.extend_from_slice(&nonce);
        output.extend_from_slice(&ciphertext);

        Ok(output)
    }

    /// Decrypt private key with password
    fn decrypt_with_password(encrypted: &[u8], password: &str) -> Result<Self> {
        use argon2::{Argon2, PasswordHasher};
        use argon2::password_hash::SaltString;
        use chacha20poly1305::{aead::Aead, ChaCha20Poly1305, KeyInit, Nonce};

        if encrypted.is_empty() {
            return Err(VotingError::CryptoError("Empty encrypted data".to_string()));
        }

        // Parse format: [salt_len(1)][salt][nonce(12)][ciphertext]
        let salt_len = encrypted[0] as usize;
        if encrypted.len() < 1 + salt_len + 12 {
            return Err(VotingError::CryptoError(
                "Invalid encrypted data format".to_string(),
            ));
        }

        let salt_bytes = &encrypted[1..1 + salt_len];
        let salt_str = std::str::from_utf8(salt_bytes)
            .map_err(|e| VotingError::CryptoError(format!("Invalid salt: {}", e)))?;
        let salt = SaltString::new(salt_str)
            .map_err(|e| VotingError::CryptoError(format!("Invalid salt string: {}", e)))?;

        let nonce_start = 1 + salt_len;
        let nonce_bytes = &encrypted[nonce_start..nonce_start + 12];
        let nonce = Nonce::from_slice(nonce_bytes);

        let ciphertext = &encrypted[nonce_start + 12..];

        // Derive key from password
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| VotingError::CryptoError(format!("Password hashing failed: {}", e)))?;

        let derived_key_bytes = password_hash
            .hash
            .ok_or_else(|| VotingError::CryptoError("No hash output".to_string()))?;

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&derived_key_bytes.as_bytes()[..32]);

        let cipher = ChaCha20Poly1305::new(&key_bytes.into());

        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| VotingError::CryptoError(format!("Decryption failed: {}", e)))?;

        if plaintext.len() != SECRET_KEY_LENGTH {
            return Err(VotingError::CryptoError(
                "Invalid decrypted key length".to_string(),
            ));
        }

        let mut private_key_bytes = [0u8; SECRET_KEY_LENGTH];
        private_key_bytes.copy_from_slice(&plaintext);
        Ok(Self::from_bytes(private_key_bytes))
    }
}

// Prevent accidental printing of private keys
impl fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PrivateKey([REDACTED])")
    }
}

impl fmt::Display for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED PRIVATE KEY]")
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

    /// Get reference to key bytes
    pub fn as_bytes(&self) -> &[u8; PUBLIC_KEY_LENGTH] {
        &self.bytes
    }

    /// Convert to common PublicKey type
    pub fn to_common(&self) -> CommonPublicKey {
        CommonPublicKey::new(self.bytes)
    }

    /// Create from common PublicKey type
    pub fn from_common(pk: &CommonPublicKey) -> Self {
        Self::from_bytes(*pk.as_bytes())
    }

    /// Convert to Ed25519 public key
    pub fn to_ed25519_public(&self) -> Result<Ed25519PublicKey> {
        Ed25519PublicKey::from_bytes(&self.bytes)
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
        self.to_ed25519_public()?;
        Ok(())
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
        let mut csprng = OsRng;
        let keypair = Ed25519Keypair::generate(&mut csprng);

        let private = PrivateKey::from_bytes(keypair.secret.to_bytes());
        let public = PublicKey::from_bytes(keypair.public.to_bytes());

        Self { private, public }
    }

    /// Create from existing private key
    pub fn from_private_key(private: PrivateKey) -> Result<Self> {
        let secret = private.to_ed25519_secret()?;
        let ed25519_public: Ed25519PublicKey = (&secret).into();
        let public = PublicKey::from_bytes(ed25519_public.to_bytes());

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

    /// Convert to Ed25519 keypair
    pub fn to_ed25519_keypair(&self) -> Result<Ed25519Keypair> {
        let secret = self.private.to_ed25519_secret()?;
        let public = self.public.to_ed25519_public()?;
        Ok(Ed25519Keypair { secret, public })
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
        use std::path::PathBuf;
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

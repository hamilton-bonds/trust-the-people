use common::{Result, VotingError};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Key, Nonce,
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Size of encryption key in bytes (256 bits for ChaCha20-Poly1305)
pub const KEY_SIZE: usize = 32;

/// Size of nonce in bytes (96 bits for ChaCha20-Poly1305)
pub const NONCE_SIZE: usize = 12;

/// Size of authentication tag in bytes (128 bits for Poly1305)
pub const TAG_SIZE: usize = 16;

/// Encrypted data container with authentication
///
/// This structure contains ciphertext along with its nonce and authentication tag.
/// ChaCha20-Poly1305 is an AEAD (Authenticated Encryption with Associated Data) cipher
/// that provides both confidentiality and authenticity.
///
/// ChaCha20-Poly1305 is approved for use in federal systems and meets:
/// - NIST SP 800-38D requirements for AEAD
/// - RFC 8439 specification
/// - Suitable for CMMC compliance when properly implemented
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedData {
    /// The nonce used for this encryption (must be unique per key)
    pub nonce: [u8; NONCE_SIZE],

    /// The encrypted ciphertext with authentication tag appended
    pub ciphertext: Vec<u8>,

    /// Optional associated data that was authenticated but not encrypted
    pub associated_data: Option<Vec<u8>>,
}

impl EncryptedData {
    /// Create new encrypted data container
    pub fn new(nonce: [u8; NONCE_SIZE], ciphertext: Vec<u8>) -> Self {
        Self {
            nonce,
            ciphertext,
            associated_data: None,
        }
    }

    /// Create with associated data
    pub fn with_associated_data(
        nonce: [u8; NONCE_SIZE],
        ciphertext: Vec<u8>,
        associated_data: Vec<u8>,
    ) -> Self {
        Self {
            nonce,
            ciphertext,
            associated_data: Some(associated_data),
        }
    }

    /// Get size of encrypted data in bytes
    pub fn size(&self) -> usize {
        NONCE_SIZE + self.ciphertext.len() + self.associated_data.as_ref().map_or(0, |ad| ad.len())
    }

    /// Export to bytes (nonce || ciphertext)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.nonce);
        bytes.extend_from_slice(&self.ciphertext);
        bytes
    }

    /// Import from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < NONCE_SIZE + TAG_SIZE {
            return Err(VotingError::CryptoError(
                "Encrypted data too short".to_string(),
            ));
        }

        let mut nonce = [0u8; NONCE_SIZE];
        nonce.copy_from_slice(&bytes[..NONCE_SIZE]);
        let ciphertext = bytes[NONCE_SIZE..].to_vec();

        Ok(Self::new(nonce, ciphertext))
    }

    /// Export to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }

    /// Import from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| VotingError::CryptoError(format!("Invalid hex: {}", e)))?;
        Self::from_bytes(&bytes)
    }
}

impl fmt::Display for EncryptedData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EncryptedData(nonce: {}, size: {} bytes)",
            hex::encode(&self.nonce),
            self.ciphertext.len()
        )
    }
}

/// Encryption key for ChaCha20-Poly1305
#[derive(Clone)]
pub struct EncryptionKey {
    key: [u8; KEY_SIZE],
}

impl EncryptionKey {
    /// Generate a new random encryption key using cryptographically secure RNG
    pub fn generate() -> Self {
        let key = ChaCha20Poly1305::generate_key(&mut OsRng);
        Self {
            key: key.into(),
        }
    }

    /// Create from existing key bytes
    pub fn from_bytes(bytes: [u8; KEY_SIZE]) -> Self {
        Self { key: bytes }
    }

    /// Import from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| VotingError::CryptoError(format!("Invalid hex: {}", e)))?;

        if bytes.len() != KEY_SIZE {
            return Err(VotingError::CryptoError(format!(
                "Invalid key length: expected {}, got {}",
                KEY_SIZE,
                bytes.len()
            )));
        }

        let mut key = [0u8; KEY_SIZE];
        key.copy_from_slice(&bytes);
        Ok(Self::from_bytes(key))
    }

    /// Export to hex (use with caution - keys are sensitive)
    pub fn to_hex(&self) -> String {
        hex::encode(&self.key)
    }

    /// Get reference to key bytes (use carefully)
    pub fn as_bytes(&self) -> &[u8; KEY_SIZE] {
        &self.key
    }

    /// Derive key from password using Argon2id (NIST approved KDF)
    ///
    /// Argon2id is recommended for password-based key derivation and meets
    /// NIST guidelines for key derivation functions.
    pub fn derive_from_password(password: &str, salt: &[u8]) -> Result<Self> {
        use argon2::{
            password_hash::{PasswordHasher, SaltString},
            Argon2, ParamsBuilder, Version,
        };

        // Use NIST-recommended parameters for Argon2id
        let params = ParamsBuilder::new()
            .m_cost(65536) // 64 MB memory
            .t_cost(3) // 3 iterations
            .p_cost(4) // 4 parallel lanes
            .output_len(KEY_SIZE)
            .build()
            .map_err(|e| VotingError::CryptoError(format!("Argon2 params error: {}", e)))?;

        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);

        // Convert salt to SaltString (encode as B64)
        let salt_b64 = base64::encode_config(salt, base64::BCRYPT);
        let salt_string = SaltString::new(&salt_b64)
            .map_err(|e| VotingError::CryptoError(format!("Invalid salt: {}", e)))?;

        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt_string)
            .map_err(|e| VotingError::CryptoError(format!("Key derivation failed: {}", e)))?;

        let hash_output = password_hash
            .hash
            .ok_or_else(|| VotingError::CryptoError("No hash output".to_string()))?;

        let mut key = [0u8; KEY_SIZE];
        key.copy_from_slice(&hash_output.as_bytes()[..KEY_SIZE]);

        Ok(Self::from_bytes(key))
    }
}

// Prevent accidental key exposure
impl fmt::Debug for EncryptionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EncryptionKey([REDACTED])")
    }
}

impl fmt::Display for EncryptionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED ENCRYPTION KEY]")
    }
}

// Zeroize key on drop for memory security
impl Drop for EncryptionKey {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.key.zeroize();
    }
}

/// Encrypt plaintext data using ChaCha20-Poly1305
///
/// ChaCha20-Poly1305 provides:
/// - Confidentiality through ChaCha20 stream cipher
/// - Authentication through Poly1305 MAC
/// - Resistance to timing attacks
/// - Performance advantages on systems without AES hardware acceleration
///
/// This cipher is specified in RFC 8439 and is approved for government use.
///
/// # Arguments
/// * `plaintext` - The data to encrypt
/// * `key` - The encryption key
///
/// # Returns
/// Encrypted data with nonce and authentication tag
pub fn encrypt(plaintext: &[u8], key: &EncryptionKey) -> Result<EncryptedData> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key.key));
    let nonce_bytes = ChaCha20Poly1305::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce_bytes, plaintext)
        .map_err(|e| VotingError::CryptoError(format!("Encryption failed: {}", e)))?;

    let mut nonce = [0u8; NONCE_SIZE];
    nonce.copy_from_slice(&nonce_bytes);

    Ok(EncryptedData::new(nonce, ciphertext))
}

/// Decrypt ciphertext using ChaCha20-Poly1305
///
/// This function verifies the authentication tag before decrypting, ensuring
/// that the data has not been tampered with.
///
/// # Arguments
/// * `encrypted` - The encrypted data to decrypt
/// * `key` - The decryption key
///
/// # Returns
/// The original plaintext if authentication succeeds, error otherwise
pub fn decrypt(encrypted: &EncryptedData, key: &EncryptionKey) -> Result<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key.key));
    let nonce = Nonce::from_slice(&encrypted.nonce);

    let plaintext = cipher
        .decrypt(nonce, encrypted.ciphertext.as_ref())
        .map_err(|e| VotingError::CryptoError(format!("Decryption failed: {}", e)))?;

    Ok(plaintext)
}

/// Encrypt with additional authenticated data (AAD)
///
/// AAD is data that is authenticated but not encrypted. This is useful for
/// including metadata or headers that must be verified but don't need confidentiality.
///
/// # Arguments
/// * `plaintext` - The data to encrypt
/// * `associated_data` - Additional data to authenticate (not encrypted)
/// * `key` - The encryption key
///
/// # Returns
/// Encrypted data with AAD included
pub fn encrypt_with_aad(
    plaintext: &[u8],
    associated_data: &[u8],
    key: &EncryptionKey,
) -> Result<EncryptedData> {
    use chacha20poly1305::aead::Payload;

    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key.key));
    let nonce_bytes = ChaCha20Poly1305::generate_nonce(&mut OsRng);

    let payload = Payload {
        msg: plaintext,
        aad: associated_data,
    };

    let ciphertext = cipher
        .encrypt(&nonce_bytes, payload)
        .map_err(|e| VotingError::CryptoError(format!("Encryption failed: {}", e)))?;

    let mut nonce = [0u8; NONCE_SIZE];
    nonce.copy_from_slice(&nonce_bytes);

    Ok(EncryptedData::with_associated_data(
        nonce,
        ciphertext,
        associated_data.to_vec(),
    ))
}

/// Decrypt with additional authenticated data (AAD)
///
/// The AAD must match exactly what was used during encryption, or
/// decryption will fail.
///
/// # Arguments
/// * `encrypted` - The encrypted data to decrypt
/// * `key` - The decryption key
///
/// # Returns
/// The original plaintext if authentication succeeds
pub fn decrypt_with_aad(encrypted: &EncryptedData, key: &EncryptionKey) -> Result<Vec<u8>> {
    use chacha20poly1305::aead::Payload;

    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key.key));
    let nonce = Nonce::from_slice(&encrypted.nonce);

    let aad = encrypted
        .associated_data
        .as_ref()
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    let payload = Payload {
        msg: &encrypted.ciphertext,
        aad,
    };

    let plaintext = cipher
        .decrypt(nonce, payload)
        .map_err(|e| VotingError::CryptoError(format!("Decryption failed: {}", e)))?;

    Ok(plaintext)
}

/// Encrypt in-place (modifies buffer)
///
/// This is more efficient for large data as it avoids allocations.
///
/// # Arguments
/// * `buffer` - Data to encrypt (will be modified in-place)
/// * `key` - The encryption key
///
/// # Returns
/// Nonce and authentication tag
pub fn encrypt_in_place(buffer: &mut Vec<u8>, key: &EncryptionKey) -> Result<[u8; NONCE_SIZE]> {
    use chacha20poly1305::aead::AeadInPlace;

    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key.key));
    let nonce_bytes = ChaCha20Poly1305::generate_nonce(&mut OsRng);

    cipher
        .encrypt_in_place(&nonce_bytes, b"", buffer)
        .map_err(|e| VotingError::CryptoError(format!("In-place encryption failed: {}", e)))?;

    let mut nonce = [0u8; NONCE_SIZE];
    nonce.copy_from_slice(&nonce_bytes);
    Ok(nonce)
}

/// Decrypt in-place (modifies buffer)
///
/// # Arguments
/// * `buffer` - Ciphertext to decrypt (will be modified in-place)
/// * `nonce` - The nonce used during encryption
/// * `key` - The decryption key
///
/// # Returns
/// Ok(()) if successful
pub fn decrypt_in_place(
    buffer: &mut Vec<u8>,
    nonce: &[u8; NONCE_SIZE],
    key: &EncryptionKey,
) -> Result<()> {
    use chacha20poly1305::aead::AeadInPlace;

    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key.key));
    let nonce_ref = Nonce::from_slice(nonce);

    cipher
        .decrypt_in_place(nonce_ref, b"", buffer)
        .map_err(|e| VotingError::CryptoError(format!("In-place decryption failed: {}", e)))?;

    Ok(())
}

/// Stream cipher for encrypting large data
///
/// This allows encrypting data in chunks without loading everything into memory.
pub struct StreamCipher {
    cipher: ChaCha20Poly1305,
    nonce: [u8; NONCE_SIZE],
}

impl StreamCipher {
    /// Create a new stream cipher with random nonce
    pub fn new(key: &EncryptionKey) -> Self {
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key.key));
        let nonce_bytes = ChaCha20Poly1305::generate_nonce(&mut OsRng);

        let mut nonce = [0u8; NONCE_SIZE];
        nonce.copy_from_slice(&nonce_bytes);

        Self { cipher, nonce }
    }

    /// Get the nonce for this stream
    pub fn nonce(&self) -> &[u8; NONCE_SIZE] {
        &self.nonce
    }

    /// Encrypt a chunk of data
    pub fn encrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>> {
        let nonce = Nonce::from_slice(&self.nonce);
        self.cipher
            .encrypt(nonce, chunk)
            .map_err(|e| VotingError::CryptoError(format!("Stream encryption failed: {}", e)))
    }

    /// Decrypt a chunk of data
    pub fn decrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>> {
        let nonce = Nonce::from_slice(&self.nonce);
        self.cipher
            .decrypt(nonce, chunk)
            .map_err(|e| VotingError::CryptoError(format!("Stream decryption failed: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = EncryptionKey::generate();
        let plaintext = b"Secret voting data";

        let encrypted = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();

        assert_eq!(plaintext.as_ref(), decrypted.as_slice());
    }

    #[test]
    fn test_encrypt_different_each_time() {
        let key = EncryptionKey::generate();
        let plaintext = b"Same message";

        let encrypted1 = encrypt(plaintext, &key).unwrap();
        let encrypted2 = encrypt(plaintext, &key).unwrap();

        // Nonces should be different
        assert_ne!(encrypted1.nonce, encrypted2.nonce);
        // Ciphertexts should be different
        assert_ne!(encrypted1.ciphertext, encrypted2.ciphertext);
    }

    #[test]
    fn test_decrypt_wrong_key() {
        let key1 = EncryptionKey::generate();
        let key2 = EncryptionKey::generate();
        let plaintext = b"Secret";

        let encrypted = encrypt(plaintext, &key1).unwrap();
        let result = decrypt(&encrypted, &key2);

        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_tampered_ciphertext() {
        let key = EncryptionKey::generate();
        let plaintext = b"Original message";

        let mut encrypted = encrypt(plaintext, &key).unwrap();
        
        // Tamper with ciphertext
        encrypted.ciphertext[0] ^= 1;

        let result = decrypt(&encrypted, &key);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_tampered_nonce() {
        let key = EncryptionKey::generate();
        let plaintext = b"Test message";

        let mut encrypted = encrypt(plaintext, &key).unwrap();
        
        // Tamper with nonce
        encrypted.nonce[0] ^= 1;

        let result = decrypt(&encrypted, &key);
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypted_data_serialization() {
        let key = EncryptionKey::generate();
        let plaintext = b"Serialization test";

        let encrypted = encrypt(plaintext, &key).unwrap();
        let bytes = encrypted.to_bytes();
        let deserialized = EncryptedData::from_bytes(&bytes).unwrap();

        assert_eq!(encrypted.nonce, deserialized.nonce);
        assert_eq!(encrypted.ciphertext, deserialized.ciphertext);
    }

    #[test]
    fn test_encrypted_data_hex() {
        let key = EncryptionKey::generate();
        let plaintext = b"Hex test";

        let encrypted = encrypt(plaintext, &key).unwrap();
        let hex = encrypted.to_hex();
        let deserialized = EncryptedData::from_hex(&hex).unwrap();

        let decrypted = decrypt(&deserialized, &key).unwrap();
        assert_eq!(plaintext.as_ref(), decrypted.as_slice());
    }

    #[test]
    fn test_encryption_key_generation() {
        let key1 = EncryptionKey::generate();
        let key2 = EncryptionKey::generate();

        // Keys should be different
        assert_ne!(key1.key, key2.key);
        assert_eq!(key1.key.len(), KEY_SIZE);
    }

    #[test]
    fn test_encryption_key_from_hex() {
        let key = EncryptionKey::generate();
        let hex = key.to_hex();
        let key2 = EncryptionKey::from_hex(&hex).unwrap();

        assert_eq!(key.key, key2.key);
    }

    #[test]
    fn test_key_derivation_from_password() {
        let password = "strong_password_123";
        let salt = b"unique_salt_value";

        let key1 = EncryptionKey::derive_from_password(password, salt).unwrap();
        let key2 = EncryptionKey::derive_from_password(password, salt).unwrap();

        // Same password and salt should produce same key
        assert_eq!(key1.key, key2.key);
    }

    #[test]
    fn test_key_derivation_different_salt() {
        let password = "password";
        let salt1 = b"salt1";
        let salt2 = b"salt2";

        let key1 = EncryptionKey::derive_from_password(password, salt1).unwrap();
        let key2 = EncryptionKey::derive_from_password(password, salt2).unwrap();

        // Different salts should produce different keys
        assert_ne!(key1.key, key2.key);
    }

    #[test]
    fn test_encrypt_with_aad() {
        let key = EncryptionKey::generate();
        let plaintext = b"Secret data";
        let aad = b"Public metadata";

        let encrypted = encrypt_with_aad(plaintext, aad, &key).unwrap();
        assert!(encrypted.associated_data.is_some());

        let decrypted = decrypt_with_aad(&encrypted, &key).unwrap();
        assert_eq!(plaintext.as_ref(), decrypted.as_slice());
    }

    #[test]
    fn test_decrypt_with_wrong_aad() {
        let key = EncryptionKey::generate();
        let plaintext = b"Data";
        let aad = b"Correct AAD";

        let mut encrypted = encrypt_with_aad(plaintext, aad, &key).unwrap();
        
        // Change AAD
        encrypted.associated_data = Some(b"Wrong AAD".to_vec());

        let result = decrypt_with_aad(&encrypted, &key);
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypt_in_place() {
        let key = EncryptionKey::generate();
        let plaintext = b"In-place test".to_vec();
        let mut buffer = plaintext.clone();

        let nonce = encrypt_in_place(&mut buffer, &key).unwrap();

        // Buffer should now contain ciphertext
        assert_ne!(buffer, plaintext);

        // Decrypt in-place
        decrypt_in_place(&mut buffer, &nonce, &key).unwrap();

        // Buffer should now contain plaintext again
        assert_eq!(buffer, plaintext);
    }

    #[test]
    fn test_stream_cipher() {
        let key = EncryptionKey::generate();
        let stream = StreamCipher::new(&key);

        let chunk1 = b"First chunk";
        let chunk2 = b"Second chunk";

        let encrypted1 = stream.encrypt_chunk(chunk1).unwrap();
        let encrypted2 = stream.encrypt_chunk(chunk2).unwrap();

        let decrypted1 = stream.decrypt_chunk(&encrypted1).unwrap();
        let decrypted2 = stream.decrypt_chunk(&encrypted2).unwrap();

        assert_eq!(chunk1.as_ref(), decrypted1.as_slice());
        assert_eq!(chunk2.as_ref(), decrypted2.as_slice());
    }

    #[test]
    fn test_empty_plaintext() {
        let key = EncryptionKey::generate();
        let plaintext = b"";

        let encrypted = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();

        assert_eq!(plaintext.as_ref(), decrypted.as_slice());
    }

    #[test]
    fn test_large_plaintext() {
        let key = EncryptionKey::generate();
        let plaintext = vec![0xAB; 100_000]; // 100 KB

        let encrypted = encrypt(&plaintext, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_encrypted_data_size() {
        let key = EncryptionKey::generate();
        let plaintext = b"Size test data";

        let encrypted = encrypt(plaintext, &key).unwrap();
        let size = encrypted.size();

        // Size should include nonce and ciphertext (with tag)
        assert_eq!(size, NONCE_SIZE + plaintext.len() + TAG_SIZE);
    }

    #[test]
    fn test_key_debug_redaction() {
        let key = EncryptionKey::generate();
        let debug_str = format!("{:?}", key);

        assert!(debug_str.contains("[REDACTED]"));
        assert!(!debug_str.contains(&hex::encode(&key.key)));
    }

    #[test]
    fn test_key_display_redaction() {
        let key = EncryptionKey::generate();
        let display_str = format!("{}", key);

        assert!(display_str.contains("REDACTED"));
    }

    #[test]
    fn test_encrypted_data_display() {
        let key = EncryptionKey::generate();
        let plaintext = b"Display test";

        let encrypted = encrypt(plaintext, &key).unwrap();
        let display_str = format!("{}", encrypted);

        assert!(display_str.contains("EncryptedData"));
        assert!(display_str.contains("nonce"));
        assert!(display_str.contains("bytes"));
    }

    #[test]
    fn test_invalid_encrypted_data_bytes() {
        let short_data = vec![0u8; 5]; // Too short
        let result = EncryptedData::from_bytes(&short_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_key_hex() {
        let result = EncryptionKey::from_hex("invalid_hex");
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_key_length() {
        let short_hex = hex::encode(&[0u8; 16]); // Only 16 bytes
        let result = EncryptionKey::from_hex(&short_hex);
        assert!(result.is_err());
    }
}

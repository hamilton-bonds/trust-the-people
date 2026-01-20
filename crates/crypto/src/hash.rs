use common::{Hash, Result, VotingError};
use blake2::{Blake2b512, Blake2s256, Digest};
use sha3::{Sha3_256, Sha3_512};
use std::fmt;

/// Hash output size for Blake2b-256 (32 bytes)
pub const BLAKE2B_256_SIZE: usize = 32;

/// Hash output size for Blake2b-512 (64 bytes)
pub const BLAKE2B_512_SIZE: usize = 64;

/// Hash output size for SHA3-256 (32 bytes)
pub const SHA3_256_SIZE: usize = 32;

/// Hash output size for SHA3-512 (64 bytes)
pub const SHA3_512_SIZE: usize = 64;

/// Hash a single piece of data using Blake2b-256
///
/// Blake2b is faster than SHA-256 and SHA-3, making it ideal for blockchain applications.
/// This is the primary hashing function used throughout the system.
///
/// # Arguments
/// * `data` - The data to hash
///
/// # Returns
/// A 32-byte hash
pub fn hash_blake2b(data: &[u8]) -> Hash {
    use blake2::digest::consts::U32;
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(data);
    let result = hasher.finalize();

    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Hash multiple pieces of data using Blake2b-256
///
/// This is useful when you need to hash data from multiple sources without
/// concatenating them first.
///
/// # Arguments
/// * `data_pieces` - Slice of data slices to hash
///
/// # Returns
/// A 32-byte hash over all data pieces
pub fn hash_blake2b_multiple(data_pieces: &[&[u8]]) -> Hash {
    use blake2::digest::consts::U32;
    let mut hasher = Blake2b::<U32>::new();
    for data in data_pieces {
        hasher.update(data);
    }
    let result = hasher.finalize();

    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Hash data using Blake2b-512 (64-byte output)
///
/// Use this when you need a longer hash output for additional security margin.
///
/// # Arguments
/// * `data` - The data to hash
///
/// # Returns
/// A 64-byte hash
pub fn hash_blake2b_512(data: &[u8]) -> [u8; 64] {
    let mut hasher = Blake2b512::new();
    hasher.update(data);
    let result = hasher.finalize();

    let mut hash = [0u8; 64];
    hash.copy_from_slice(&result);
    hash
}

/// Hash data using SHA3-256
///
/// SHA3 is the NIST standard hash function. While slower than Blake2b,
/// it may be required for compatibility with other systems.
///
/// # Arguments
/// * `data` - The data to hash
///
/// # Returns
/// A 32-byte hash
pub fn hash_sha3_256(data: &[u8]) -> Hash {
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    let result = hasher.finalize();

    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Hash multiple pieces of data using SHA3-256
///
/// # Arguments
/// * `data_pieces` - Slice of data slices to hash
///
/// # Returns
/// A 32-byte hash over all data pieces
pub fn hash_sha3_256_multiple(data_pieces: &[&[u8]]) -> Hash {
    let mut hasher = Sha3_256::new();
    for data in data_pieces {
        hasher.update(data);
    }
    let result = hasher.finalize();

    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Hash data using SHA3-512 (64-byte output)
///
/// # Arguments
/// * `data` - The data to hash
///
/// # Returns
/// A 64-byte hash
pub fn hash_sha3_512(data: &[u8]) -> [u8; 64] {
    let mut hasher = Sha3_512::new();
    hasher.update(data);
    let result = hasher.finalize();

    let mut hash = [0u8; 64];
    hash.copy_from_slice(&result);
    hash
}

/// Hash algorithm selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Blake2b256,
    Blake2b512,
    Sha3_256,
    Sha3_512,
}

impl HashAlgorithm {
    /// Get the output size for this algorithm
    pub fn output_size(&self) -> usize {
        match self {
            HashAlgorithm::Blake2b256 => BLAKE2B_256_SIZE,
            HashAlgorithm::Blake2b512 => BLAKE2B_512_SIZE,
            HashAlgorithm::Sha3_256 => SHA3_256_SIZE,
            HashAlgorithm::Sha3_512 => SHA3_512_SIZE,
        }
    }

    /// Hash data with this algorithm
    pub fn hash(&self, data: &[u8]) -> Vec<u8> {
        match self {
            HashAlgorithm::Blake2b256 => hash_blake2b(data).to_vec(),
            HashAlgorithm::Blake2b512 => hash_blake2b_512(data).to_vec(),
            HashAlgorithm::Sha3_256 => hash_sha3_256(data).to_vec(),
            HashAlgorithm::Sha3_512 => hash_sha3_512(data).to_vec(),
        }
    }
}

impl fmt::Display for HashAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HashAlgorithm::Blake2b256 => write!(f, "blake2b-256"),
            HashAlgorithm::Blake2b512 => write!(f, "blake2b-512"),
            HashAlgorithm::Sha3_256 => write!(f, "sha3-256"),
            HashAlgorithm::Sha3_512 => write!(f, "sha3-512"),
        }
    }
}

/// Incremental hasher for streaming data
///
/// Use this when you need to hash data incrementally rather than all at once.
pub struct IncrementalHasher {
    algorithm: HashAlgorithm,
    blake2b_256: Option<Blake2s256>,
    blake2b_512: Option<Blake2b512>,
    sha3_256: Option<Sha3_256>,
    sha3_512: Option<Sha3_512>,
}

impl IncrementalHasher {
    /// Create a new incremental hasher
    pub fn new(algorithm: HashAlgorithm) -> Self {
        let (blake2b_256, blake2b_512, sha3_256, sha3_512) = match algorithm {
            HashAlgorithm::Blake2b256 => (Some(Blake2s256::new()), None, None, None),
            HashAlgorithm::Blake2b512 => (None, Some(Blake2b512::new()), None, None),
            HashAlgorithm::Sha3_256 => (None, None, Some(Sha3_256::new()), None),
            HashAlgorithm::Sha3_512 => (None, None, None, Some(Sha3_512::new())),
        };

        Self {
            algorithm,
            blake2b_256,
            blake2b_512,
            sha3_256,
            sha3_512,
        }
    }

    /// Create a Blake2b-256 hasher (most common)
    pub fn blake2b() -> Self {
        Self::new(HashAlgorithm::Blake2b256)
    }

    /// Create a SHA3-256 hasher
    pub fn sha3() -> Self {
        Self::new(HashAlgorithm::Sha3_256)
    }

    /// Update the hasher with more data
    pub fn update(&mut self, data: &[u8]) {
        match self.algorithm {
            HashAlgorithm::Blake2b256 => {
                if let Some(hasher) = &mut self.blake2b_256 {
                    hasher.update(data);
                }
            }
            HashAlgorithm::Blake2b512 => {
                if let Some(hasher) = &mut self.blake2b_512 {
                    hasher.update(data);
                }
            }
            HashAlgorithm::Sha3_256 => {
                if let Some(hasher) = &mut self.sha3_256 {
                    hasher.update(data);
                }
            }
            HashAlgorithm::Sha3_512 => {
                if let Some(hasher) = &mut self.sha3_512 {
                    hasher.update(data);
                }
            }
        }
    }

    /// Finalize the hash and return the result
    pub fn finalize(self) -> Vec<u8> {
        match self.algorithm {
            HashAlgorithm::Blake2b256 => {
                let hasher = self.blake2b_256.expect("Blake2b-256 hasher not initialized");
                hasher.finalize().to_vec()
            }
            HashAlgorithm::Blake2b512 => {
                let hasher = self.blake2b_512.expect("Blake2b-512 hasher not initialized");
                hasher.finalize().to_vec()
            }
            HashAlgorithm::Sha3_256 => {
                let hasher = self.sha3_256.expect("SHA3-256 hasher not initialized");
                hasher.finalize().to_vec()
            }
            HashAlgorithm::Sha3_512 => {
                let hasher = self.sha3_512.expect("SHA3-512 hasher not initialized");
                hasher.finalize().to_vec()
            }
        }
    }

    /// Finalize and return as 32-byte hash (panics if output size is not 32)
    pub fn finalize_fixed(self) -> Hash {
        let result = self.finalize();
        if result.len() != 32 {
            panic!("Hash output is not 32 bytes");
        }
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
}

/// Hash a file
///
/// This efficiently hashes a file without loading it entirely into memory.
///
/// # Arguments
/// * `path` - Path to the file
/// * `algorithm` - Hash algorithm to use
///
/// # Returns
/// The hash of the file contents
pub fn hash_file(path: &std::path::Path, algorithm: HashAlgorithm) -> Result<Vec<u8>> {
    use std::fs::File;
    use std::io::{BufReader, Read};

    let file = File::open(path)
        .map_err(|e| VotingError::IoError(e))?;
    let mut reader = BufReader::new(file);
    let mut hasher = IncrementalHasher::new(algorithm);

    let mut buffer = [0u8; 8192];
    loop {
        let bytes_read = reader.read(&mut buffer)
            .map_err(|e| VotingError::IoError(e))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hasher.finalize())
}

/// Compute HMAC-Blake2b for message authentication
///
/// HMAC provides both integrity and authenticity verification.
///
/// # Arguments
/// * `key` - The secret key
/// * `data` - The data to authenticate
///
/// # Returns
/// A 32-byte HMAC tag
pub fn hmac_blake2b(key: &[u8], data: &[u8]) -> Hash {
    use blake2::digest::consts::U32;
    use hmac::{Hmac, Mac};

    type HmacBlake2b = Hmac<Blake2b<U32>>;

    let mut mac = HmacBlake2b::new_from_slice(key)
        .expect("HMAC can take key of any size");
    mac.update(data);
    let result = mac.finalize();

    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result.into_bytes());
    hash
}

/// Verify HMAC-Blake2b tag
///
/// # Arguments
/// * `key` - The secret key
/// * `data` - The data to verify
/// * `tag` - The HMAC tag to check
///
/// # Returns
/// Ok(()) if the tag is valid, Err otherwise
pub fn hmac_blake2b_verify(key: &[u8], data: &[u8], tag: &Hash) -> Result<()> {
    use blake2::digest::consts::U32;
    use hmac::{Hmac, Mac};

    type HmacBlake2b = Hmac<Blake2b<U32>>;

    let mut mac = HmacBlake2b::new_from_slice(key)
        .expect("HMAC can take key of any size");
    mac.update(data);
    
    mac.verify_slice(tag)
        .map_err(|_| VotingError::CryptoError("HMAC verification failed".to_string()))?;

    Ok(())
}

/// Double hash (hash of hash) for additional security
///
/// Some protocols use double hashing to prevent length extension attacks.
///
/// # Arguments
/// * `data` - The data to hash
///
/// # Returns
/// Blake2b(Blake2b(data))
pub fn double_hash_blake2b(data: &[u8]) -> Hash {
    let first_hash = hash_blake2b(data);
    hash_blake2b(&first_hash)
}

/// Merkle-Damgård construction helper
///
/// Combine two hashes into one (used in Merkle trees).
///
/// # Arguments
/// * `left` - Left hash
/// * `right` - Right hash
///
/// # Returns
/// Hash of concatenated hashes
pub fn combine_hashes(left: &Hash, right: &Hash) -> Hash {
    hash_blake2b_multiple(&[left, right])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_blake2b() {
        let data = b"Hello, blockchain!";
        let hash = hash_blake2b(data);
        
        assert_eq!(hash.len(), 32);
        
        // Hashing should be deterministic
        let hash2 = hash_blake2b(data);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_hash_blake2b_different_data() {
        let data1 = b"data1";
        let data2 = b"data2";
        
        let hash1 = hash_blake2b(data1);
        let hash2 = hash_blake2b(data2);
        
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_blake2b_multiple() {
        let data1 = b"part1";
        let data2 = b"part2";
        let data3 = b"part3";
        
        let hash_multiple = hash_blake2b_multiple(&[data1, data2, data3]);
        
        // Should be same as hashing concatenated data
        let mut concatenated = Vec::new();
        concatenated.extend_from_slice(data1);
        concatenated.extend_from_slice(data2);
        concatenated.extend_from_slice(data3);
        let hash_concat = hash_blake2b(&concatenated);
        
        assert_eq!(hash_multiple, hash_concat);
    }

    #[test]
    fn test_hash_blake2b_512() {
        let data = b"Test data";
        let hash = hash_blake2b_512(data);
        
        assert_eq!(hash.len(), 64);
        
        // Deterministic
        let hash2 = hash_blake2b_512(data);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_hash_sha3_256() {
        let data = b"SHA3 test";
        let hash = hash_sha3_256(data);
        
        assert_eq!(hash.len(), 32);
        
        // Deterministic
        let hash2 = hash_sha3_256(data);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_hash_sha3_256_multiple() {
        let data1 = b"sha3";
        let data2 = b"test";
        
        let hash_multiple = hash_sha3_256_multiple(&[data1, data2]);
        
        let mut concatenated = Vec::new();
        concatenated.extend_from_slice(data1);
        concatenated.extend_from_slice(data2);
        let hash_concat = hash_sha3_256(&concatenated);
        
        assert_eq!(hash_multiple, hash_concat);
    }

    #[test]
    fn test_hash_sha3_512() {
        let data = b"SHA3-512 test";
        let hash = hash_sha3_512(data);
        
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_hash_algorithm_output_size() {
        assert_eq!(HashAlgorithm::Blake2b256.output_size(), 32);
        assert_eq!(HashAlgorithm::Blake2b512.output_size(), 64);
        assert_eq!(HashAlgorithm::Sha3_256.output_size(), 32);
        assert_eq!(HashAlgorithm::Sha3_512.output_size(), 64);
    }

    #[test]
    fn test_hash_algorithm_hash() {
        let data = b"test";
        
        let hash_blake2b = HashAlgorithm::Blake2b256.hash(data);
        assert_eq!(hash_blake2b.len(), 32);
        
        let hash_sha3 = HashAlgorithm::Sha3_256.hash(data);
        assert_eq!(hash_sha3.len(), 32);
    }

    #[test]
    fn test_hash_algorithm_display() {
        assert_eq!(HashAlgorithm::Blake2b256.to_string(), "blake2b-256");
        assert_eq!(HashAlgorithm::Blake2b512.to_string(), "blake2b-512");
        assert_eq!(HashAlgorithm::Sha3_256.to_string(), "sha3-256");
        assert_eq!(HashAlgorithm::Sha3_512.to_string(), "sha3-512");
    }

    #[test]
    fn test_incremental_hasher() {
        let mut hasher = IncrementalHasher::blake2b();
        hasher.update(b"part1");
        hasher.update(b"part2");
        let hash = hasher.finalize_fixed();
        
        // Should equal hashing all at once
        let hash_direct = hash_blake2b(b"part1part2");
        assert_eq!(hash, hash_direct);
    }

    #[test]
    fn test_incremental_hasher_sha3() {
        let mut hasher = IncrementalHasher::sha3();
        hasher.update(b"test");
        hasher.update(b"data");
        let hash = hasher.finalize_fixed();
        
        let hash_direct = hash_sha3_256(b"testdata");
        assert_eq!(hash, hash_direct);
    }

    #[test]
    fn test_hmac_blake2b() {
        let key = b"secret_key";
        let data = b"message to authenticate";
        
        let tag = hmac_blake2b(key, data);
        assert_eq!(tag.len(), 32);
        
        // Verification should succeed
        assert!(hmac_blake2b_verify(key, data, &tag).is_ok());
    }

    #[test]
    fn test_hmac_blake2b_wrong_key() {
        let key1 = b"key1";
        let key2 = b"key2";
        let data = b"data";
        
        let tag = hmac_blake2b(key1, data);
        
        // Wrong key should fail verification
        assert!(hmac_blake2b_verify(key2, data, &tag).is_err());
    }

    #[test]
    fn test_hmac_blake2b_wrong_data() {
        let key = b"key";
        let data1 = b"data1";
        let data2 = b"data2";
        
        let tag = hmac_blake2b(key, data1);
        
        // Wrong data should fail verification
        assert!(hmac_blake2b_verify(key, data2, &tag).is_err());
    }

    #[test]
    fn test_double_hash_blake2b() {
        let data = b"test";
        let double_hash = double_hash_blake2b(data);
        
        // Manual double hash
        let first = hash_blake2b(data);
        let second = hash_blake2b(&first);
        
        assert_eq!(double_hash, second);
    }

    #[test]
    fn test_combine_hashes() {
        let hash1 = hash_blake2b(b"left");
        let hash2 = hash_blake2b(b"right");
        
        let combined = combine_hashes(&hash1, &hash2);
        assert_eq!(combined.len(), 32);
        
        // Should be deterministic
        let combined2 = combine_hashes(&hash1, &hash2);
        assert_eq!(combined, combined2);
        
        // Order matters
        let combined_reversed = combine_hashes(&hash2, &hash1);
        assert_ne!(combined, combined_reversed);
    }

    #[test]
    fn test_hash_empty_data() {
        let empty: &[u8] = &[];
        let hash = hash_blake2b(empty);
        assert_eq!(hash.len(), 32);
        
        // Empty data should produce consistent hash
        let hash2 = hash_blake2b(empty);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_hash_large_data() {
        let large_data = vec![0xAB; 1_000_000]; // 1 MB
        let hash = hash_blake2b(&large_data);
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_incremental_hasher_many_updates() {
        let mut hasher = IncrementalHasher::blake2b();
        for i in 0..100 {
            hasher.update(&[i]);
        }
        let hash = hasher.finalize_fixed();
        
        // Build equivalent data
        let mut data = Vec::new();
        for i in 0..100 {
            data.push(i);
        }
        let hash_direct = hash_blake2b(&data);
        
        assert_eq!(hash, hash_direct);
    }

    #[test]
    fn test_blake2b_vs_sha3() {
        let data = b"comparison test";
        
        let blake2b_hash = hash_blake2b(data);
        let sha3_hash = hash_sha3_256(data);
        
        // Different algorithms should produce different hashes
        assert_ne!(blake2b_hash, sha3_hash);
        
        // But both should be 32 bytes
        assert_eq!(blake2b_hash.len(), 32);
        assert_eq!(sha3_hash.len(), 32);
    }

    #[test]
    fn test_hash_file() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create temporary file
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"file contents for hashing").unwrap();
        temp_file.flush().unwrap();

        // Hash the file
        let hash = hash_file(temp_file.path(), HashAlgorithm::Blake2b256).unwrap();
        assert_eq!(hash.len(), 32);

        // Should match direct hashing
        let direct_hash = hash_blake2b(b"file contents for hashing");
        assert_eq!(hash, direct_hash.to_vec());
    }
}

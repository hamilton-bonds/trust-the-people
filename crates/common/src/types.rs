use serde::{Deserialize, Serialize};
use std::fmt;

/// Block height in the blockchain
pub type BlockHeight = u64;

/// Unix timestamp in seconds
pub type Timestamp = u64;

/// 32-byte hash output (Blake2b or SHA3-256)
pub type Hash = [u8; 32];

/// Voter/Node address (32-byte public key hash)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Address(pub [u8; 32]);

impl Address {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Cryptographic signature (64 bytes for Ed25519)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature(pub [u8; 64]);

impl Signature {
    pub fn new(bytes: [u8; 64]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 64 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Public key (32 bytes for Ed25519)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PublicKey(pub [u8; 32]);

impl PublicKey {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    /// Derive address from public key (hash of public key)
    pub fn to_address(&self) -> Address {
        use blake2::{Blake2b, Digest};
        use blake2::digest::consts::U32;
        
        let mut hasher = Blake2b::<U32>::new();
        hasher.update(&self.0);
        let result = hasher.finalize();
        
        let mut addr = [0u8; 32];
        addr.copy_from_slice(&result);
        Address(addr)
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Transaction ID (hash of transaction)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TxId(pub Hash);

impl TxId {
    pub fn new(hash: Hash) -> Self {
        Self(hash)
    }

    pub fn as_bytes(&self) -> &Hash {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }
}

impl fmt::Display for TxId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Block hash
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockHash(pub Hash);

impl BlockHash {
    pub fn new(hash: Hash) -> Self {
        Self(hash)
    }

    pub fn as_bytes(&self) -> &Hash {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    /// Special hash representing "no previous block" (genesis)
    pub fn zero() -> Self {
        Self([0u8; 32])
    }
}

impl fmt::Display for BlockHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Election ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ElectionId(pub [u8; 16]);

impl ElectionId {
    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 16 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut arr = [0u8; 16];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Display for ElectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_hex_roundtrip() {
        let bytes = [42u8; 32];
        let addr = Address::new(bytes);
        let hex = addr.to_hex();
        let decoded = Address::from_hex(&hex).unwrap();
        assert_eq!(addr, decoded);
    }

    #[test]
    fn test_signature_hex_roundtrip() {
        let bytes = [99u8; 64];
        let sig = Signature::new(bytes);
        let hex = sig.to_hex();
        let decoded = Signature::from_hex(&hex).unwrap();
        assert_eq!(sig, decoded);
    }

    #[test]
    fn test_public_key_to_address() {
        let pk_bytes = [1u8; 32];
        let pk = PublicKey::new(pk_bytes);
        let addr = pk.to_address();
        
        // Address should be deterministic
        let addr2 = pk.to_address();
        assert_eq!(addr, addr2);
    }

    #[test]
    fn test_block_hash_zero() {
        let zero = BlockHash::zero();
        assert_eq!(zero.0, [0u8; 32]);
    }
}

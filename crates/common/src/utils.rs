use crate::types::{Hash, Timestamp};
use std::time::{SystemTime, UNIX_EPOCH};

/// Get current Unix timestamp in seconds
pub fn current_timestamp() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

/// Get current Unix timestamp in milliseconds
pub fn current_timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis()
}

/// Convert bytes to hex string
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

/// Convert hex string to bytes
pub fn hex_to_bytes(hex_str: &str) -> Result<Vec<u8>, hex::FromHexError> {
    hex::decode(hex_str)
}

/// Hash arbitrary data using Blake2b-256
pub fn hash_data(data: &[u8]) -> Hash {
    use blake2::{Blake2b, Digest};
    use blake2::digest::consts::U32;
    
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(data);
    let result = hasher.finalize();
    
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Hash multiple pieces of data
pub fn hash_multiple(data_pieces: &[&[u8]]) -> Hash {
    use blake2::{Blake2b, Digest};
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

/// Serialize data to bytes using bincode
pub fn serialize<T: serde::Serialize>(data: &T) -> Result<Vec<u8>, bincode::Error> {
    bincode::serialize(data)
}

/// Deserialize data from bytes using bincode
pub fn deserialize<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, bincode::Error> {
    bincode::deserialize(bytes)
}

/// Serialize data to JSON string
pub fn to_json<T: serde::Serialize>(data: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(data)
}

/// Deserialize data from JSON string
pub fn from_json<'a, T: serde::Deserialize<'a>>(json: &'a str) -> Result<T, serde_json::Error> {
    serde_json::from_str(json)
}

/// Generate random bytes
pub fn random_bytes(len: usize) -> Vec<u8> {
    use rand::RngCore;
    let mut rng = rand::thread_rng();
    let mut bytes = vec![0u8; len];
    rng.fill_bytes(&mut bytes);
    bytes
}

/// Generate random 32-byte hash
pub fn random_hash() -> Hash {
    use rand::RngCore;
    let mut rng = rand::thread_rng();
    let mut hash = [0u8; 32];
    rng.fill_bytes(&mut hash);
    hash
}

/// Check if a file exists
pub fn file_exists(path: &std::path::Path) -> bool {
    path.exists() && path.is_file()
}

/// Check if a directory exists
pub fn dir_exists(path: &std::path::Path) -> bool {
    path.exists() && path.is_dir()
}

/// Create directory if it doesn't exist
pub fn ensure_dir_exists(path: &std::path::Path) -> std::io::Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}

/// Format timestamp as human-readable string
pub fn format_timestamp(timestamp: Timestamp) -> String {
    use chrono::{DateTime, Utc};
    let dt = DateTime::<Utc>::from_timestamp(timestamp as i64, 0)
        .unwrap_or_else(|| Utc::now());
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

/// Calculate percentage (returns value between 0.0 and 1.0)
pub fn calculate_percentage(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64
    }
}

/// Check if percentage threshold is met
pub fn meets_threshold(part: usize, total: usize, threshold: f64) -> bool {
    calculate_percentage(part, total) >= threshold
}

/// Truncate string to max length with ellipsis
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len < 3 {
        s.chars().take(max_len).collect()
    } else {
        format!("{}...", s.chars().take(max_len - 3).collect::<String>())
    }
}

/// Format hash for display (first 8 chars)
pub fn format_hash_short(hash: &Hash) -> String {
    let hex = hex::encode(hash);
    truncate_string(&hex, 8)
}

/// Sleep for specified milliseconds
pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
}

/// Retry an async operation with exponential backoff
pub async fn retry_with_backoff<F, T, E>(
    mut operation: F,
    max_retries: usize,
    initial_delay_ms: u64,
) -> Result<T, E>
where
    F: FnMut() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, E>> + Send>>,
{
    let mut delay = initial_delay_ms;
    let mut last_error = None;

    for attempt in 0..max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error = Some(e);
                if attempt < max_retries - 1 {
                    sleep_ms(delay).await;
                    delay *= 2; // Exponential backoff
                }
            }
        }
    }

    Err(last_error.unwrap())
}

/// Validate that a value is within a range
pub fn validate_range<T: PartialOrd>(value: T, min: T, max: T) -> bool {
    value >= min && value <= max
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_timestamp() {
        let ts = current_timestamp();
        assert!(ts > 0);
        assert!(ts < u64::MAX);
    }

    #[test]
    fn test_hash_data() {
        let data = b"hello world";
        let hash1 = hash_data(data);
        let hash2 = hash_data(data);
        assert_eq!(hash1, hash2); // Deterministic
        assert_eq!(hash1.len(), 32);
    }

    #[test]
    fn test_hash_multiple() {
        let data1 = b"hello";
        let data2 = b"world";
        let hash = hash_multiple(&[data1, data2]);
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_bytes_hex_roundtrip() {
        let bytes = vec![1, 2, 3, 4, 5];
        let hex = bytes_to_hex(&bytes);
        let decoded = hex_to_bytes(&hex).unwrap();
        assert_eq!(bytes, decoded);
    }

    #[test]
    fn test_random_bytes() {
        let bytes1 = random_bytes(32);
        let bytes2 = random_bytes(32);
        assert_eq!(bytes1.len(), 32);
        assert_ne!(bytes1, bytes2); // Should be different
    }

    #[test]
    fn test_random_hash() {
        let hash1 = random_hash();
        let hash2 = random_hash();
        assert_eq!(hash1.len(), 32);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_calculate_percentage() {
        assert_eq!(calculate_percentage(50, 100), 0.5);
        assert_eq!(calculate_percentage(75, 100), 0.75);
        assert_eq!(calculate_percentage(0, 100), 0.0);
        assert_eq!(calculate_percentage(100, 100), 1.0);
        assert_eq!(calculate_percentage(10, 0), 0.0); // Edge case
    }

    #[test]
    fn test_meets_threshold() {
        assert!(meets_threshold(67, 100, 0.67));
        assert!(meets_threshold(70, 100, 0.67));
        assert!(!meets_threshold(66, 100, 0.67));
        assert!(meets_threshold(2, 3, 0.66));
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 8), "hello...");
        assert_eq!(truncate_string("hi", 5), "hi");
    }

    #[test]
    fn test_format_hash_short() {
        let hash = [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let short = format_hash_short(&hash);
        assert_eq!(short.len(), 8);
        assert_eq!(short, "12345678");
    }

    #[test]
    fn test_validate_range() {
        assert!(validate_range(5, 1, 10));
        assert!(validate_range(1, 1, 10));
        assert!(validate_range(10, 1, 10));
        assert!(!validate_range(0, 1, 10));
        assert!(!validate_range(11, 1, 10));
    }

    #[test]
    fn test_serialize_deserialize() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct TestStruct {
            value: u64,
        }

        let data = TestStruct { value: 42 };
        let bytes = serialize(&data).unwrap();
        let deserialized: TestStruct = deserialize(&bytes).unwrap();
        assert_eq!(data, deserialized);
    }
}

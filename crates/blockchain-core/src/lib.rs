pub mod block;
pub mod chain;
pub mod transaction;
pub mod merkle;
pub mod genesis;
pub mod consensus;

// Re-export commonly used types
pub use block::Block;
pub use chain::Blockchain;
pub use transaction::{Transaction, TransactionType, VoteTransaction};
pub use merkle::MerkleTree;
pub use genesis::GenesisBlock;

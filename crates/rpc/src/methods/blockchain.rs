//! Blockchain-related RPC methods
//!
//! This module implements RPC methods for querying blockchain data:
//! - Chain information
//! - Block queries
//! - Transaction queries
//! - Search functionality

use crate::{
    BlockQuery, BlockResponse, BlockSearchResponse, ChainInfo, TransactionQuery,
    TransactionResponse, TransactionSearchResponse, TransactionSummary,
};
use blockchain_core::{Block, Chain, Transaction};
use common::{BlockHash, Result, TxId, VotingError};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Blockchain query service
pub struct BlockchainMethods {
    chain: Arc<RwLock<Chain>>,
}

impl BlockchainMethods {
    /// Create new blockchain methods handler
    pub fn new(chain: Arc<RwLock<Chain>>) -> Self {
        Self { chain }
    }

    /// Get blockchain information
    pub async fn chain_info(&self) -> Result<ChainInfo> {
        let chain = self.chain.read().await;
        let latest_block = chain.latest_block();

        Ok(ChainInfo {
            height: chain.height(),
            genesis_hash: chain.genesis_hash().to_hex(),
            latest_block_hash: latest_block.hash().to_hex(),
            latest_block_time: latest_block.header.timestamp,
            total_transactions: self.count_total_transactions(&chain).await,
            total_votes: self.count_total_votes(&chain).await,
            total_validators: chain.validator_count(),
            active_validators: chain.active_validator_count(),
            active_elections: chain.active_election_count(),
        })
    }

    /// Get block by hash
    pub async fn get_block(&self, hash_str: String) -> Result<BlockResponse> {
        let hash = BlockHash::from_hex(&hash_str)
            .map_err(|_| VotingError::InvalidBlock("Invalid block hash".to_string()))?;

        let chain = self.chain.read().await;
        let block = chain
            .get_block_by_hash(&hash)
            .ok_or_else(|| VotingError::BlockNotFound(hash_str))?;

        self.block_to_response(block)
    }

    /// Get block by height
    pub async fn get_block_by_height(&self, height: u64) -> Result<BlockResponse> {
        let chain = self.chain.read().await;
        let block = chain
            .get_block_by_height(height)
            .ok_or_else(|| VotingError::BlockNotFound(format!("height {}", height)))?;

        self.block_to_response(block)
    }

    /// Get transaction by hash
    pub async fn get_transaction(&self, hash_str: String) -> Result<TransactionResponse> {
        let tx_id = TxId::from_hex(&hash_str)
            .map_err(|_| VotingError::InvalidTransaction("Invalid transaction hash".to_string()))?;

        let chain = self.chain.read().await;
        let tx = chain
            .get_transaction(&tx_id)
            .ok_or_else(|| VotingError::TransactionNotFound(hash_str.clone()))?;

        let block_height = chain
            .get_transaction_block_height(&tx_id)
            .ok_or_else(|| VotingError::TransactionNotFound(hash_str))?;

        let block = chain.get_block_by_height(block_height).unwrap();

        self.transaction_to_response(tx, block)
    }

    /// Search blocks by criteria
    pub async fn search_blocks(&self, query: BlockQuery) -> Result<BlockSearchResponse> {
        let chain = self.chain.read().await;

        let start_height = query.start_height.unwrap_or(0);
        let end_height = query.end_height.unwrap_or(chain.height());
        let limit = query.limit.unwrap_or(crate::DEFAULT_QUERY_LIMIT).min(crate::MAX_QUERY_LIMIT);
        let offset = query.offset.unwrap_or(0);

        let mut matching_blocks = Vec::new();
        let mut total_matches = 0;

        for height in start_height..=end_height {
            if let Some(block) = chain.get_block_by_height(height) {
                if self.block_matches_query(block, &query) {
                    total_matches += 1;

                    if total_matches > offset && matching_blocks.len() < limit {
                        matching_blocks.push(self.block_to_response(block)?);
                    }
                }
            }
        }

        Ok(BlockSearchResponse {
            total: total_matches,
            blocks: matching_blocks,
            has_more: total_matches > offset + limit,
        })
    }

    /// Search transactions by criteria
    pub async fn search_transactions(
        &self,
        query: TransactionQuery,
    ) -> Result<TransactionSearchResponse> {
        let chain = self.chain.read().await;

        let start_height = query.start_height.unwrap_or(0);
        let end_height = query.end_height.unwrap_or(chain.height());
        let limit = query.limit.unwrap_or(crate::DEFAULT_QUERY_LIMIT).min(crate::MAX_QUERY_LIMIT);
        let offset = query.offset.unwrap_or(0);

        let mut matching_txs = Vec::new();
        let mut total_matches = 0;

        for height in start_height..=end_height {
            if let Some(block) = chain.get_block_by_height(height) {
                for tx in &block.transactions {
                    if self.transaction_matches_query(tx, &query) {
                        total_matches += 1;

                        if total_matches > offset && matching_txs.len() < limit {
                            matching_txs.push(self.transaction_to_response(tx, block)?);
                        }
                    }
                }
            }
        }

        Ok(TransactionSearchResponse {
            total: total_matches,
            transactions: matching_txs,
            has_more: total_matches > offset + limit,
        })
    }

    // Helper methods

    fn block_to_response(&self, block: &Block) -> Result<BlockResponse> {
        let transactions = block
            .transactions
            .iter()
            .map(|tx| TransactionSummary {
                id: tx.id.to_hex(),
                tx_type: tx.transaction_type_name().to_string(),
                block_height: block.height(),
                timestamp: tx.timestamp,
            })
            .collect();

        Ok(BlockResponse {
            height: block.height(),
            hash: block.hash().to_hex(),
            previous_hash: block.previous_hash().to_hex(),
            timestamp: block.header.timestamp,
            validator: block.header.validator.to_hex(),
            transactions_root: block.header.transactions_root.to_hex(),
            transaction_count: block.header.transaction_count,
            transactions,
            signature: block.signature.to_hex(),
        })
    }

    fn transaction_to_response(
        &self,
        tx: &Transaction,
        block: &Block,
    ) -> Result<TransactionResponse> {
        let data = serde_json::to_value(tx).map_err(|e| {
            VotingError::SerializationError(format!("Failed to serialize transaction: {}", e))
        })?;

        Ok(TransactionResponse {
            id: tx.id.to_hex(),
            tx_type: tx.transaction_type_name().to_string(),
            block_height: block.height(),
            block_hash: block.hash().to_hex(),
            timestamp: tx.timestamp,
            data,
            signature: tx.signature.to_hex(),
        })
    }

    fn block_matches_query(&self, block: &Block, query: &BlockQuery) -> bool {
        // Filter by validator
        if let Some(ref validator_str) = query.validator {
            if block.header.validator.to_hex() != *validator_str {
                return false;
            }
        }

        // Filter by timestamp
        if let Some(min_ts) = query.min_timestamp {
            if block.header.timestamp < min_ts {
                return false;
            }
        }

        if let Some(max_ts) = query.max_timestamp {
            if block.header.timestamp > max_ts {
                return false;
            }
        }

        true
    }

    fn transaction_matches_query(&self, tx: &Transaction, query: &TransactionQuery) -> bool {
        // Filter by transaction type
        if let Some(ref tx_type) = query.tx_type {
            if tx.transaction_type_name() != tx_type {
                return false;
            }
        }

        // Filter by election ID
        if let Some(ref election_id) = query.election_id {
            match &tx.tx_type {
                blockchain_core::TransactionType::Vote(vote_tx) => {
                    if vote_tx.election_id.to_hex() != *election_id {
                        return false;
                    }
                }
                blockchain_core::TransactionType::CreateElection(election_tx) => {
                    if election_tx.election.id.to_hex() != *election_id {
                        return false;
                    }
                }
                _ => return false,
            }
        }

        // Filter by timestamp
        if let Some(min_ts) = query.min_timestamp {
            if tx.timestamp < min_ts {
                return false;
            }
        }

        if let Some(max_ts) = query.max_timestamp {
            if tx.timestamp > max_ts {
                return false;
            }
        }

        true
    }

    async fn count_total_transactions(&self, chain: &Chain) -> u64 {
        let mut count = 0;
        for height in 0..=chain.height() {
            if let Some(block) = chain.get_block_by_height(height) {
                count += block.transactions.len() as u64;
            }
        }
        count
    }

    async fn count_total_votes(&self, chain: &Chain) -> u64 {
        let mut count = 0;
        for height in 0..=chain.height() {
            if let Some(block) = chain.get_block_by_height(height) {
                for tx in &block.transactions {
                    if matches!(tx.tx_type, blockchain_core::TransactionType::Vote(_)) {
                        count += 1;
                    }
                }
            }
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockchain_core::GenesisBlock;

    #[tokio::test]
    async fn test_chain_info() {
        let genesis = GenesisBlock::default();
        let chain = Chain::new(genesis).unwrap();
        let methods = BlockchainMethods::new(Arc::new(RwLock::new(chain)));

        let info = methods.chain_info().await.unwrap();
        assert_eq!(info.height, 0);
        assert!(info.genesis_hash.len() > 0);
    }

    #[tokio::test]
    async fn test_get_block_by_height() {
        let genesis = GenesisBlock::default();
        let chain = Chain::new(genesis).unwrap();
        let methods = BlockchainMethods::new(Arc::new(RwLock::new(chain)));

        let block = methods.get_block_by_height(0).await.unwrap();
        assert_eq!(block.height, 0);
    }

    #[tokio::test]
    async fn test_get_block_not_found() {
        let genesis = GenesisBlock::default();
        let chain = Chain::new(genesis).unwrap();
        let methods = BlockchainMethods::new(Arc::new(RwLock::new(chain)));

        let result = methods.get_block_by_height(999).await;
        assert!(result.is_err());
    }
}

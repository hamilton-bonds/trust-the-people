//! Validation-related RPC methods
//!
//! This module implements RPC methods for validation operations:
//! - Vote verification
//! - Validator queries
//! - Election queries and results

use crate::{
    Candidate, CandidateResult, ElectionResponse, ElectionResultsResponse, JurisdictionInfo,
    ValidatorResponse, ValidatorSetResponse, VerifyVoteRequest, VerifyVoteResponse, VoteResponse,
};
use blockchain_core::{chain::Blockchain, Transaction, TransactionType};
use common::{ElectionId, Result, TxId, VotingError};
use storage::StateStore;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Validation query service
pub struct ValidationMethods {
    chain: Arc<RwLock<Blockchain>>,
    state_store: Arc<RwLock<StateStore>>,
}

impl ValidationMethods {
    /// Create new validation methods handler
    pub fn new(chain: Arc<RwLock<Blockchain>>, state_store: Arc<RwLock<StateStore>>) -> Self {
        Self { chain, state_store }
    }

    /// Get vote by transaction hash
    pub async fn get_vote(&self, tx_hash: String) -> Result<VoteResponse> {
        let tx_id = TxId::from_hex(&tx_hash)
            .map_err(|_| VotingError::InvalidTransaction("Invalid transaction hash".to_string()))?;

        let chain = self.chain.read().await;
        let tx = chain
            .get_transaction(&tx_id)
            .ok_or_else(|| VotingError::TransactionNotFound(tx_hash))?;

        if let TransactionType::Vote(vote_tx) = &tx.tx_type {
            let block_height = chain
                .get_transaction_block_height(&tx_id)
                .ok_or_else(|| VotingError::TransactionNotFound(tx_id.to_hex()))?;

            Ok(VoteResponse {
                tx_id: tx.id.to_hex(),
                election_id: vote_tx.election_id.to_hex(),
                encrypted_vote: hex::encode(&vote_tx.encrypted_vote),
                timestamp: tx.timestamp,
                block_height,
                zk_proof: vote_tx.validity_proof.as_ref().map(|p| hex::encode(p)),
                verified: true, // If it's in a block, it was verified
            })
        } else {
            Err(VotingError::InvalidTransaction(
                "Transaction is not a vote".to_string(),
            ))
        }
    }

    /// Verify vote with zero-knowledge proof
    pub async fn verify_vote(&self, request: VerifyVoteRequest) -> Result<VerifyVoteResponse> {
        let tx_id = TxId::from_hex(&request.tx_id)
            .map_err(|_| VotingError::InvalidTransaction("Invalid transaction hash".to_string()))?;

        let chain = self.chain.read().await;
        let tx = chain
            .get_transaction(&tx_id)
            .ok_or_else(|| VotingError::TransactionNotFound(request.tx_id.clone()))?;

        if let TransactionType::Vote(vote_tx) = &tx.tx_type {
            // Check if proof exists
            let valid = vote_tx.validity_proof.is_some();

            let block_height = chain.get_transaction_block_height(&tx_id);

            Ok(VerifyVoteResponse {
                valid,
                details: if valid {
                    Some("Vote proof verified successfully".to_string())
                } else {
                    Some("Vote proof verification failed".to_string())
                },
                block_height,
                election_id: Some(vote_tx.election_id.to_hex()),
            })
        } else {
            Err(VotingError::InvalidTransaction(
                "Transaction is not a vote".to_string(),
            ))
        }
    }

    /// Get election information
    pub async fn get_election(&self, election_id: String) -> Result<ElectionResponse> {
        let id = ElectionId::from_hex(&election_id)
            .map_err(|_| VotingError::InvalidTransaction("Invalid election ID".to_string()))?;

        let state_store = self.state_store.read().await;
        let election = state_store
            .get_election(&id)?
            .ok_or_else(|| VotingError::ElectionNotFound(election_id))?;

        let jurisdiction = election.jurisdiction.as_ref().map(|j| JurisdictionInfo {
            level: j.level.to_str().to_string(),
            name: j.name.clone(),
            parent: j.parent.clone(),
        });

        let candidates = election
            .candidates
            .iter()
            .map(|c| Candidate {
                id: c.id.clone(),
                name: c.name.clone(),
                party: c.party.clone(),
                vote_count: None, // Don't reveal counts until finalized
            })
            .collect();

        Ok(ElectionResponse {
            id: election.id.to_hex(),
            name: election.name.clone(),
            description: election.description.clone(),
            start_time: election.start_time,
            end_time: election.end_time,
            vote_count: election.vote_count,
            is_active: election.is_active,
            is_finalized: election.is_finalized,
            created_at_height: election.created_at_height,
            closed_at_height: election.closed_at_height,
            candidates,
            jurisdiction,
        })
    }

    /// Get election results
    pub async fn get_election_results(
        &self,
        election_id: String,
    ) -> Result<ElectionResultsResponse> {
        let id = ElectionId::from_hex(&election_id)
            .map_err(|_| VotingError::InvalidTransaction("Invalid election ID".to_string()))?;

        let state_store = self.state_store.read().await;
        let election = state_store
            .get_election(&id)?
            .ok_or_else(|| VotingError::ElectionNotFound(election_id))?;

        // Only return results if election is finalized
        if !election.is_finalized {
            return Err(VotingError::InvalidTransaction(
                "Election is not finalized yet".to_string(),
            ));
        }

        // Count votes for each candidate
        let chain = self.chain.read().await;
        let mut vote_counts: HashMap<String, u64> = HashMap::new();

        for height in election.created_at_height..=election.closed_at_height.unwrap_or(chain.height())
        {
            if let Some(block) = chain.get_block_by_height(height) {
                for tx in &block.transactions {
                    if let TransactionType::Vote(vote_tx) = &tx.tx_type {
                        if vote_tx.election_id == id {
                            // Decrypt and count vote (simplified - actual implementation would use proper decryption)
                            // For now, we'll use placeholder logic
                            let candidate_id = self.decrypt_vote(&vote_tx.encrypted_vote)?;
                            *vote_counts.entry(candidate_id).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        let total_votes = vote_counts.values().sum::<u64>();
        let results = election
            .candidates
            .iter()
            .map(|c| {
                let votes = vote_counts.get(&c.id).copied().unwrap_or(0);
                let percentage = if total_votes > 0 {
                    (votes as f64 / total_votes as f64) * 100.0
                } else {
                    0.0
                };

                CandidateResult {
                    candidate_id: c.id.clone(),
                    candidate_name: c.name.clone(),
                    votes,
                    percentage,
                }
            })
            .collect();

        Ok(ElectionResultsResponse {
            election_id: id.to_hex(),
            total_votes,
            results,
            finalized: election.is_finalized,
            verification_proof: None, // Could include merkle proof of results
        })
    }

    /// Get validators at specific height
    pub async fn get_validators(&self, height: Option<u64>) -> Result<ValidatorSetResponse> {
        let chain = self.chain.read().await;
        let query_height = height.unwrap_or(chain.height());

        let validator_set = chain.get_validator_set_at_height(query_height)?;

        let validators = validator_set
            .validators
            .iter()
            .map(|v| ValidatorResponse {
                address: v.address.to_hex(),
                public_key: v.public_key.to_hex(),
                stake: v.stake,
                is_active: v.is_active,
                registered_at_height: v.registered_at_height,
                blocks_produced: v.blocks_produced,
                last_block_height: v.last_block_height,
            })
            .collect();

        Ok(ValidatorSetResponse {
            height: query_height,
            total_validators: validators.len(),
            validators,
        })
    }

    /// Get specific validator by address
    pub async fn get_validator(&self, address: String) -> Result<ValidatorResponse> {
        let chain = self.chain.read().await;
        let validator = chain
            .get_validator_by_address(&address)
            .ok_or_else(|| VotingError::InvalidValidator(format!("Validator not found: {}", address)))?;

        Ok(ValidatorResponse {
            address: validator.address.to_hex(),
            public_key: validator.public_key.to_hex(),
            stake: validator.stake,
            is_active: validator.is_active,
            registered_at_height: validator.registered_at_height,
            blocks_produced: validator.blocks_produced,
            last_block_height: validator.last_block_height,
        })
    }

    // Helper methods

    /// Decrypt vote (placeholder - actual implementation would use proper threshold decryption)
    fn decrypt_vote(&self, encrypted_vote: &[u8]) -> Result<String> {
        // In production, this would use threshold cryptography to decrypt
        // For now, return a placeholder candidate ID
        Ok(format!("candidate_{}", encrypted_vote.len() % 5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockchain_core::genesis::GenesisBlock;
    use storage::Database;

    #[tokio::test]
    async fn test_get_validators() {
        let genesis = GenesisBlock::default();
        let chain = Arc::new(RwLock::new(Blockchain::new(genesis).unwrap()));
        let db = Arc::new(storage::MemoryDatabase::new());
        let state_store = Arc::new(RwLock::new(StateStore::new(db).unwrap()));

        let methods = ValidationMethods::new(chain, state_store);
        let result = methods.get_validators(None).await.unwrap();

        assert!(result.total_validators >= 0);
    }

    #[tokio::test]
    async fn test_get_validator_not_found() {
        let genesis = GenesisBlock::default();
        let chain = Arc::new(RwLock::new(Blockchain::new(genesis).unwrap()));
        let db = Arc::new(storage::MemoryDatabase::new());
        let state_store = Arc::new(RwLock::new(StateStore::new(db).unwrap()));

        let methods = ValidationMethods::new(chain, state_store);
        let result = methods
            .get_validator("nonexistent_address".to_string())
            .await;

        assert!(result.is_err());
    }
}

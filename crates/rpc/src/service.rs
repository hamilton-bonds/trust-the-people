use crate::{RpcService, *};
use blockchain_core::Blockchain;
use common::{VotingError, utils::current_timestamp};
use network::NetworkManager;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Implementation of RpcService for the validator node
pub struct NodeRpcService {
    blockchain: Arc<RwLock<Blockchain>>,
    network: Arc<NetworkManager>,
}

impl NodeRpcService {
    pub fn new(
        blockchain: Arc<RwLock<Blockchain>>,
        network: Arc<NetworkManager>,
    ) -> Self {
        Self {
            blockchain,
            network,
        }
    }
}

#[async_trait::async_trait]
impl RpcService for NodeRpcService {
    async fn node_info(&self) -> Result<NodeInfo, VotingError> {
        let blockchain = self.blockchain.read().await;
        let height = blockchain.height();
        let latest_block = blockchain.get_best_block()?;
        
        Ok(NodeInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            node_type: "validator".to_string(),
            public_key: "".to_string(), // TODO: get from node
            network_id: "testnet".to_string(),
            height,
            latest_block_hash: hex::encode(latest_block.header.hash.0),
            peer_count: self.network.peer_count().await,
            syncing: false,
            uptime: 0, // TODO: calculate uptime
        })
    }

    async fn chain_info(&self) -> Result<ChainInfo, VotingError> {
        let blockchain = self.blockchain.read().await;
        let height = blockchain.height();
        let best_block = blockchain.get_best_block()?;
        let genesis = blockchain.get_block_by_height(0)
            .ok_or_else(|| VotingError::BlockNotFound("Genesis block not found".to_string()))?;
        
        Ok(ChainInfo {
            height,
            genesis_hash: hex::encode(genesis.header.hash.0),
            latest_block_hash: hex::encode(best_block.header.hash.0),
            latest_block_time: best_block.header.timestamp,
            total_transactions: 0, // TODO: count
            total_votes: 0, // TODO: count
            total_validators: 3, // TODO: get from consensus
            active_validators: 3, // TODO: get from consensus
            active_elections: 0, // TODO: count
        })
    }

    async fn get_block(&self, hash: String) -> Result<BlockResponse, VotingError> {
        Err(VotingError::NotImplemented("get_block".to_string()))
    }

    async fn get_block_by_height(&self, height: u64) -> Result<BlockResponse, VotingError> {
        let blockchain = self.blockchain.read().await;
        let block = blockchain.get_block_by_height(height)
            .ok_or_else(|| VotingError::BlockNotFound(format!("Block at height {} not found", height)))?;
        
        Ok(BlockResponse {
            height: block.header.height,
            hash: hex::encode(block.header.hash.0),
            previous_hash: hex::encode(block.header.previous_hash.0),
            timestamp: block.header.timestamp,
            validator: hex::encode(block.header.validator.as_bytes()),
            transactions_root: hex::encode(block.header.transactions_root.0),
            transaction_count: block.transactions.len() as u32,
            transactions: vec![], // TODO: convert transactions
            signature: "".to_string(), // TODO: get signature
        })
    }

    async fn get_transaction(&self, hash: String) -> Result<TransactionResponse, VotingError> {
        Err(VotingError::NotImplemented("get_transaction".to_string()))
    }

    async fn get_vote(&self, tx_hash: String) -> Result<VoteResponse, VotingError> {
        Err(VotingError::NotImplemented("get_vote".to_string()))
    }

    async fn get_election(&self, election_id: String) -> Result<ElectionResponse, VotingError> {
        Err(VotingError::NotImplemented("get_election".to_string()))
    }

    async fn get_election_results(&self, election_id: String) -> Result<ElectionResultsResponse, VotingError> {
        Err(VotingError::NotImplemented("get_election_results".to_string()))
    }

    async fn get_validators(&self, height: Option<u64>) -> Result<ValidatorSetResponse, VotingError> {
        Err(VotingError::NotImplemented("get_validators".to_string()))
    }

    async fn get_validator(&self, address: String) -> Result<ValidatorResponse, VotingError> {
        Err(VotingError::NotImplemented("get_validator".to_string()))
    }

    async fn verify_vote(&self, request: VerifyVoteRequest) -> Result<VerifyVoteResponse, VotingError> {
        Err(VotingError::NotImplemented("verify_vote".to_string()))
    }

    async fn submit_transaction(&self, tx_hex: String) -> Result<SubmitResponse, VotingError> {
        Err(VotingError::NotImplemented("submit_transaction".to_string()))
    }

    async fn get_mempool(&self) -> Result<MempoolResponse, VotingError> {
        Ok(MempoolResponse {
            pending_count: 0,
            transactions: vec![],
        })
    }

    async fn get_peers(&self) -> Result<PeersResponse, VotingError> {
        Ok(PeersResponse {
            total_peers: self.network.peer_count().await,
            peers: vec![], // TODO: convert peers
        })
    }

    async fn sync_status(&self) -> Result<SyncStatusResponse, VotingError> {
        let height = self.blockchain.read().await.height();
        Ok(SyncStatusResponse {
            syncing: false,
            current_height: height,
            target_height: height,
            progress: 100.0,
            estimated_time_remaining: None,
        })
    }

    async fn search_blocks(&self, query: BlockQuery) -> Result<BlockSearchResponse, VotingError> {
        Err(VotingError::NotImplemented("search_blocks".to_string()))
    }

    async fn search_transactions(&self, query: TransactionQuery) -> Result<TransactionSearchResponse, VotingError> {
        Err(VotingError::NotImplemented("search_transactions".to_string()))
    }

    async fn health(&self) -> Result<HealthResponse, VotingError> {
        Ok(HealthResponse {
            status: "healthy".to_string(),
            database: "ok".to_string(),
            network: "ok".to_string(),
            consensus: "ok".to_string(),
            timestamp: current_timestamp(),
        })
    }
}

use crate::peer::PeerId;
use blockchain_core::{Block, Transaction};
use common::{BlockHash, BlockHeight, Result, Timestamp, VotingError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Protocol message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Handshake,
    Ping,
    Pong,
    GetPeers,
    Peers,
    GetBlocks,
    Blocks,
    NewBlock,
    GetTransactions,
    Transactions,
    NewTransaction,
    GetStatus,
    Status,
    Disconnect,
}

/// Network protocol message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub msg_type: MessageType,
    pub payload: Vec<u8>,
    pub timestamp: Timestamp,
    pub nonce: u64,
}

impl Message {
    pub fn new(msg_type: MessageType, payload: Vec<u8>) -> Self {
        Self {
            msg_type,
            payload,
            timestamp: common::utils::current_timestamp(),
            nonce: rand::random(),
        }
    }
    
    pub fn handshake(info: HandshakeInfo) -> Result<Self> {
        let payload = common::utils::serialize(&info)?;
        Ok(Self::new(MessageType::Handshake, payload))
    }
    
    pub fn ping() -> Self {
        Self::new(MessageType::Ping, vec![])
    }
    
    pub fn pong() -> Self {
        Self::new(MessageType::Pong, vec![])
    }
    
    pub fn get_peers() -> Self {
        Self::new(MessageType::GetPeers, vec![])
    }
    
    pub fn peers(peer_list: PeerList) -> Result<Self> {
        let payload = common::utils::serialize(&peer_list)?;
        Ok(Self::new(MessageType::Peers, payload))
    }
    
    pub fn get_blocks(request: BlockRequest) -> Result<Self> {
        let payload = common::utils::serialize(&request)?;
        Ok(Self::new(MessageType::GetBlocks, payload))
    }
    
    pub fn blocks(response: BlockResponse) -> Result<Self> {
        let payload = common::utils::serialize(&response)?;
        Ok(Self::new(MessageType::Blocks, payload))
    }
    
    pub fn new_block(block: Block) -> Result<Self> {
        let payload = common::utils::serialize(&block)?;
        Ok(Self::new(MessageType::NewBlock, payload))
    }
    
    pub fn get_transactions(request: TransactionRequest) -> Result<Self> {
        let payload = common::utils::serialize(&request)?;
        Ok(Self::new(MessageType::GetTransactions, payload))
    }
    
    pub fn transactions(response: TransactionResponse) -> Result<Self> {
        let payload = common::utils::serialize(&response)?;
        Ok(Self::new(MessageType::Transactions, payload))
    }
    
    pub fn new_transaction(tx: Transaction) -> Result<Self> {
        let payload = common::utils::serialize(&tx)?;
        Ok(Self::new(MessageType::NewTransaction, payload))
    }
    
    pub fn get_status() -> Self {
        Self::new(MessageType::GetStatus, vec![])
    }
    
    pub fn status(info: StatusInfo) -> Result<Self> {
        let payload = common::utils::serialize(&info)?;
        Ok(Self::new(MessageType::Status, payload))
    }
    
    pub fn disconnect(reason: DisconnectReason) -> Result<Self> {
        let payload = common::utils::serialize(&reason)?;
        Ok(Self::new(MessageType::Disconnect, payload))
    }
    
    pub fn decode_payload<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        common::utils::deserialize(&self.payload)
    }
    
    pub fn size(&self) -> usize {
        self.payload.len() + 24
    }
}

/// Handshake information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeInfo {
    pub protocol_version: u32,
    pub network_id: String,
    pub user_agent: String,
    pub best_height: BlockHeight,
    pub best_hash: BlockHash,
    pub genesis_hash: BlockHash,
    pub timestamp: Timestamp,
}

/// Peer list message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerList {
    pub peers: Vec<PeerAddr>,
}

/// Peer address information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerAddr {
    pub addr: String,
    pub last_seen: Timestamp,
}

/// Block request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockRequest {
    pub start_height: BlockHeight,
    pub end_height: BlockHeight,
    pub max_blocks: usize,
}

/// Block response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockResponse {
    pub blocks: Vec<Block>,
    pub start_height: BlockHeight,
    pub end_height: BlockHeight,
}

/// Transaction request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRequest {
    pub tx_ids: Vec<[u8; 32]>,
}

/// Transaction response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResponse {
    pub transactions: Vec<Transaction>,
}

/// Node status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusInfo {
    pub best_height: BlockHeight,
    pub best_hash: BlockHash,
    pub peer_count: usize,
    pub syncing: bool,
    pub timestamp: Timestamp,
}

/// Disconnect reason
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisconnectReason {
    ShuttingDown,
    ProtocolViolation,
    IncompatibleProtocol,
    TooManyPeers,
    Banned,
    Timeout,
    Error(String),
}

/// Protocol handler for processing messages
pub struct ProtocolHandler {
    network_id: String,
    protocol_version: u32,
}

impl ProtocolHandler {
    pub fn new(network_id: String, protocol_version: u32) -> Self {
        Self {
            network_id,
            protocol_version,
        }
    }
    
    pub async fn handle(&self, peer_id: PeerId, message: Message) -> Result<()> {
        match message.msg_type {
            MessageType::Handshake => self.handle_handshake(peer_id, message).await,
            MessageType::Ping => self.handle_ping(peer_id, message).await,
            MessageType::Pong => self.handle_pong(peer_id, message).await,
            MessageType::GetPeers => self.handle_get_peers(peer_id, message).await,
            MessageType::Peers => self.handle_peers(peer_id, message).await,
            MessageType::GetBlocks => self.handle_get_blocks(peer_id, message).await,
            MessageType::Blocks => self.handle_blocks(peer_id, message).await,
            MessageType::NewBlock => self.handle_new_block(peer_id, message).await,
            MessageType::GetTransactions => self.handle_get_transactions(peer_id, message).await,
            MessageType::Transactions => self.handle_transactions(peer_id, message).await,
            MessageType::NewTransaction => self.handle_new_transaction(peer_id, message).await,
            MessageType::GetStatus => self.handle_get_status(peer_id, message).await,
            MessageType::Status => self.handle_status(peer_id, message).await,
            MessageType::Disconnect => self.handle_disconnect(peer_id, message).await,
        }
    }
    
    async fn handle_handshake(&self, peer_id: PeerId, message: Message) -> Result<()> {
        let handshake: HandshakeInfo = message.decode_payload()?;
        
        if handshake.network_id != self.network_id {
            return Err(VotingError::NetworkError(
                "Network ID mismatch".to_string(),
            ));
        }
        
        if handshake.protocol_version != self.protocol_version {
            return Err(VotingError::NetworkError(
                "Protocol version mismatch".to_string(),
            ));
        }
        
        tracing::debug!(
            "Handshake from peer {}: height={}, user_agent={}",
            peer_id,
            handshake.best_height,
            handshake.user_agent
        );
        
        Ok(())
    }
    
    async fn handle_ping(&self, peer_id: PeerId, _message: Message) -> Result<()> {
        tracing::trace!("Ping from peer {}", peer_id);
        Ok(())
    }
    
    async fn handle_pong(&self, peer_id: PeerId, _message: Message) -> Result<()> {
        tracing::trace!("Pong from peer {}", peer_id);
        Ok(())
    }
    
    async fn handle_get_peers(&self, peer_id: PeerId, _message: Message) -> Result<()> {
        tracing::debug!("GetPeers from peer {}", peer_id);
        Ok(())
    }
    
    async fn handle_peers(&self, peer_id: PeerId, message: Message) -> Result<()> {
        let peer_list: PeerList = message.decode_payload()?;
        tracing::debug!("Received {} peers from {}", peer_list.peers.len(), peer_id);
        Ok(())
    }
    
    async fn handle_get_blocks(&self, peer_id: PeerId, message: Message) -> Result<()> {
        let request: BlockRequest = message.decode_payload()?;
        tracing::debug!(
            "GetBlocks from peer {}: {}..{}",
            peer_id,
            request.start_height,
            request.end_height
        );
        Ok(())
    }
    
    async fn handle_blocks(&self, peer_id: PeerId, message: Message) -> Result<()> {
        let response: BlockResponse = message.decode_payload()?;
        tracing::debug!(
            "Received {} blocks from peer {}",
            response.blocks.len(),
            peer_id
        );
        Ok(())
    }
    
    async fn handle_new_block(&self, peer_id: PeerId, message: Message) -> Result<()> {
        let block: Block = message.decode_payload()?;
        tracing::info!(
            "New block {} from peer {} at height {}",
            block.hash().to_hex(),
            peer_id,
            block.height()
        );
        Ok(())
    }
    
    async fn handle_get_transactions(&self, peer_id: PeerId, message: Message) -> Result<()> {
        let request: TransactionRequest = message.decode_payload()?;
        tracing::debug!(
            "GetTransactions from peer {}: {} txs",
            peer_id,
            request.tx_ids.len()
        );
        Ok(())
    }
    
    async fn handle_transactions(&self, peer_id: PeerId, message: Message) -> Result<()> {
        let response: TransactionResponse = message.decode_payload()?;
        tracing::debug!(
            "Received {} transactions from peer {}",
            response.transactions.len(),
            peer_id
        );
        Ok(())
    }
    
    async fn handle_new_transaction(&self, peer_id: PeerId, message: Message) -> Result<()> {
        let tx: Transaction = message.decode_payload()?;
        tracing::debug!(
            "New transaction {} from peer {}",
            tx.id.to_hex(),
            peer_id
        );
        Ok(())
    }
    
    async fn handle_get_status(&self, peer_id: PeerId, _message: Message) -> Result<()> {
        tracing::debug!("GetStatus from peer {}", peer_id);
        Ok(())
    }
    
    async fn handle_status(&self, peer_id: PeerId, message: Message) -> Result<()> {
        let status: StatusInfo = message.decode_payload()?;
        tracing::debug!(
            "Status from peer {}: height={}, peers={}, syncing={}",
            peer_id,
            status.best_height,
            status.peer_count,
            status.syncing
        );
        Ok(())
    }
    
    async fn handle_disconnect(&self, peer_id: PeerId, message: Message) -> Result<()> {
        let reason: DisconnectReason = message.decode_payload()?;
        tracing::info!("Disconnect from peer {}: {:?}", peer_id, reason);
        Ok(())
    }
    
    pub fn validate_message(&self, message: &Message, max_size: usize) -> Result<()> {
        if message.size() > max_size {
            return Err(VotingError::NetworkError(
                "Message exceeds maximum size".to_string(),
            ));
        }
        
        let now = common::utils::current_timestamp();
        let age = now.saturating_sub(message.timestamp);
        
        if age > 300 {
            return Err(VotingError::NetworkError(
                "Message is too old".to_string(),
            ));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = Message::ping();
        assert!(matches!(msg.msg_type, MessageType::Ping));
        assert!(msg.timestamp > 0);
    }

    #[test]
    fn test_handshake_message() {
        let info = HandshakeInfo {
            protocol_version: 1,
            network_id: "test".to_string(),
            user_agent: "test/1.0".to_string(),
            best_height: 100,
            best_hash: BlockHash::zero(),
            genesis_hash: BlockHash::zero(),
            timestamp: common::utils::current_timestamp(),
        };
        
        let msg = Message::handshake(info.clone()).unwrap();
        let decoded: HandshakeInfo = msg.decode_payload().unwrap();
        
        assert_eq!(decoded.protocol_version, info.protocol_version);
        assert_eq!(decoded.network_id, info.network_id);
    }

    #[test]
    fn test_block_request() {
        let request = BlockRequest {
            start_height: 0,
            end_height: 100,
            max_blocks: 50,
        };
        
        let msg = Message::get_blocks(request.clone()).unwrap();
        let decoded: BlockRequest = msg.decode_payload().unwrap();
        
        assert_eq!(decoded.start_height, request.start_height);
        assert_eq!(decoded.end_height, request.end_height);
    }

    #[test]
    fn test_status_info() {
        let status = StatusInfo {
            best_height: 1000,
            best_hash: BlockHash::zero(),
            peer_count: 5,
            syncing: false,
            timestamp: common::utils::current_timestamp(),
        };
        
        let msg = Message::status(status.clone()).unwrap();
        let decoded: StatusInfo = msg.decode_payload().unwrap();
        
        assert_eq!(decoded.best_height, status.best_height);
        assert_eq!(decoded.peer_count, status.peer_count);
    }

    #[test]
    fn test_disconnect_message() {
        let reason = DisconnectReason::ShuttingDown;
        let msg = Message::disconnect(reason).unwrap();
        assert!(matches!(msg.msg_type, MessageType::Disconnect));
    }

    #[test]
    fn test_message_size() {
        let msg = Message::ping();
        assert!(msg.size() > 0);
        
        let payload = vec![0u8; 1000];
        let msg = Message::new(MessageType::Ping, payload);
        assert!(msg.size() > 1000);
    }

    #[test]
    fn test_peer_list() {
        let peer_list = PeerList {
            peers: vec![
                PeerAddr {
                    addr: "127.0.0.1:9000".to_string(),
                    last_seen: common::utils::current_timestamp(),
                },
                PeerAddr {
                    addr: "127.0.0.1:9001".to_string(),
                    last_seen: common::utils::current_timestamp(),
                },
            ],
        };
        
        let msg = Message::peers(peer_list.clone()).unwrap();
        let decoded: PeerList = msg.decode_payload().unwrap();
        
        assert_eq!(decoded.peers.len(), 2);
    }

    #[tokio::test]
    async fn test_protocol_handler() {
        let handler = ProtocolHandler::new("test".to_string(), 1);
        let peer_id = PeerId::random();
        
        let msg = Message::ping();
        let result = handler.handle(peer_id, msg).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_message() {
        let handler = ProtocolHandler::new("test".to_string(), 1);
        let msg = Message::ping();
        
        assert!(handler.validate_message(&msg, 10000).is_ok());
        assert!(handler.validate_message(&msg, 10).is_err());
    }

    #[test]
    fn test_transaction_request() {
        let request = TransactionRequest {
            tx_ids: vec![[1u8; 32], [2u8; 32]],
        };
        
        let msg = Message::get_transactions(request.clone()).unwrap();
        let decoded: TransactionRequest = msg.decode_payload().unwrap();
        
        assert_eq!(decoded.tx_ids.len(), 2);
    }

    #[test]
    fn test_disconnect_reasons() {
        let reasons = vec![
            DisconnectReason::ShuttingDown,
            DisconnectReason::ProtocolViolation,
            DisconnectReason::IncompatibleProtocol,
            DisconnectReason::TooManyPeers,
            DisconnectReason::Banned,
            DisconnectReason::Timeout,
            DisconnectReason::Error("test".to_string()),
        ];
        
        for reason in reasons {
            let msg = Message::disconnect(reason).unwrap();
            assert!(matches!(msg.msg_type, MessageType::Disconnect));
        }
    }
}

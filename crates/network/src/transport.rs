use crate::peer::{ConnectionDirection, Peer, PeerId};
use crate::protocol::Message;
use common::{Result, VotingError};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{timeout, Duration};

/// Transport layer configuration
#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub listen_addr: SocketAddr,
    pub connection_timeout: u64,
    pub max_message_size: usize,
    pub rate_limit: u32,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:9000".parse().unwrap(),
            connection_timeout: 30,
            max_message_size: 10 * 1024 * 1024,
            rate_limit: 100,
        }
    }
}

/// Transport layer for peer-to-peer networking
pub struct Transport {
    config: TransportConfig,
    listener: Arc<RwLock<Option<TcpListener>>>,
    connections: Arc<RwLock<Vec<Connection>>>,
    shutdown_tx: Arc<RwLock<Option<mpsc::Sender<()>>>>,
}

impl Transport {
    pub fn new(config: TransportConfig) -> Result<Self> {
        Ok(Self {
            config,
            listener: Arc::new(RwLock::new(None)),
            connections: Arc::new(RwLock::new(Vec::new())),
            shutdown_tx: Arc::new(RwLock::new(None)),
        })
    }
    
    pub async fn start(&self) -> Result<()> {
        let listener = TcpListener::bind(self.config.listen_addr)
            .await
            .map_err(|e| VotingError::NetworkError(format!("Failed to bind listener: {}", e)))?;
        
        tracing::info!("Transport listening on {}", self.config.listen_addr);
        
        *self.listener.write().await = Some(listener);
        
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        *self.shutdown_tx.write().await = Some(shutdown_tx);
        
        let transport = self.clone();
        tokio::spawn(async move {
            transport.accept_loop(shutdown_rx).await;
        });
        
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.write().await.take() {
            let _ = tx.send(()).await;
        }
        
        *self.listener.write().await = None;
        
        let mut connections = self.connections.write().await;
        for conn in connections.drain(..) {
            let _ = conn.close().await;
        }
        
        Ok(())
    }
    
    async fn accept_loop(&self, mut shutdown_rx: mpsc::Receiver<()>) {
        loop {
            // Check if we should shutdown first
            if shutdown_rx.try_recv().is_ok() {
                break;
            }
            
            // Get a reference to the listener
            let listener_guard = self.listener.read().await;
            
            let Some(ref listener) = *listener_guard else {
                drop(listener_guard);
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                continue;
            };
            
            // Accept with timeout to periodically check shutdown
            let accept_result = tokio::time::timeout(
                tokio::time::Duration::from_millis(100),
                listener.accept()
            ).await;
            
            drop(listener_guard);
            
            match accept_result {
                Ok(Ok((stream, addr))) => {
                    if let Err(e) = self.handle_inbound_connection(stream, addr).await {
                        tracing::warn!("Failed to handle inbound connection from {}: {}", addr, e);
                    }
                }
                Ok(Err(e)) => {
                    tracing::error!("Accept error: {}", e);
                }
                Err(_) => {
                    // Timeout - continue loop to check shutdown
                    continue;
                }
            }
        }
    }
    
    async fn handle_inbound_connection(
        &self,
        stream: TcpStream,
        addr: SocketAddr,
    ) -> Result<()> {
        tracing::debug!("Accepted inbound connection from {}", addr);
        
        let connection = Connection::new(
            stream,
            addr,
            ConnectionDirection::Inbound,
            self.config.connection_timeout,
            self.config.max_message_size,
        );
        
        self.connections.write().await.push(connection);
        
        Ok(())
    }
    
    pub async fn connect(&self, addr: SocketAddr) -> Result<Peer> {
        let stream = timeout(
            Duration::from_secs(self.config.connection_timeout),
            TcpStream::connect(addr),
        )
        .await
        .map_err(|_| VotingError::NetworkError("Connection timeout".to_string()))?
        .map_err(|e| VotingError::NetworkError(format!("Failed to connect: {}", e)))?;
        
        tracing::debug!("Connected to {}", addr);
        
        let connection = Connection::new(
            stream,
            addr,
            ConnectionDirection::Outbound,
            self.config.connection_timeout,
            self.config.max_message_size,
        );
        
        self.connections.write().await.push(connection.clone());
        
        let peer_id = PeerId::random();
        let peer = Peer::new(
            peer_id,
            addr,
            ConnectionDirection::Outbound,
            1,
            "voting-network".to_string(),
        );
        
        peer.mark_connected().await;
        
        Ok(peer)
    }
    
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }
    
    pub async fn get_stats(&self) -> TransportStats {
        let connections = self.connections.read().await;
        
        let inbound = connections
            .iter()
            .filter(|c| c.direction == ConnectionDirection::Inbound)
            .count();
        
        let outbound = connections.len() - inbound;
        
        TransportStats {
            total_connections: connections.len(),
            inbound_connections: inbound,
            outbound_connections: outbound,
            bytes_sent: 0,
            bytes_received: 0,
        }
    }
}

impl Clone for Transport {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            listener: Arc::clone(&self.listener),
            connections: Arc::clone(&self.connections),
            shutdown_tx: Arc::clone(&self.shutdown_tx),
        }
    }
}

/// Individual peer connection
#[derive(Clone)]
struct Connection {
    stream: Arc<RwLock<TcpStream>>,
    addr: SocketAddr,
    direction: ConnectionDirection,
    timeout: u64,
    max_message_size: usize,
}

impl Connection {
    fn new(
        stream: TcpStream,
        addr: SocketAddr,
        direction: ConnectionDirection,
        timeout: u64,
        max_message_size: usize,
    ) -> Self {
        Self {
            stream: Arc::new(RwLock::new(stream)),
            addr,
            direction,
            timeout,
            max_message_size,
        }
    }
    
    async fn send_message(&self, message: Message) -> Result<()> {
        let data = Self::encode_message(&message)?;
        
        if data.len() > self.max_message_size {
            return Err(VotingError::NetworkError(
                "Message exceeds maximum size".to_string(),
            ));
        }
        
        let mut stream = self.stream.write().await;
        
        let len_bytes = (data.len() as u32).to_be_bytes();
        stream
            .write_all(&len_bytes)
            .await
            .map_err(|e| VotingError::NetworkError(format!("Failed to write length: {}", e)))?;
        
        stream
            .write_all(&data)
            .await
            .map_err(|e| VotingError::NetworkError(format!("Failed to write message: {}", e)))?;
        
        stream
            .flush()
            .await
            .map_err(|e| VotingError::NetworkError(format!("Failed to flush: {}", e)))?;
        
        Ok(())
    }
    
    async fn receive_message(&self) -> Result<Message> {
        let mut stream = self.stream.write().await;
        
        let mut len_bytes = [0u8; 4];
        timeout(
            Duration::from_secs(self.timeout),
            stream.read_exact(&mut len_bytes),
        )
        .await
        .map_err(|_| VotingError::NetworkError("Read timeout".to_string()))?
        .map_err(|e| VotingError::NetworkError(format!("Failed to read length: {}", e)))?;
        
        let len = u32::from_be_bytes(len_bytes) as usize;
        
        if len > self.max_message_size {
            return Err(VotingError::NetworkError(
                "Message exceeds maximum size".to_string(),
            ));
        }
        
        let mut data = vec![0u8; len];
        timeout(
            Duration::from_secs(self.timeout),
            stream.read_exact(&mut data),
        )
        .await
        .map_err(|_| VotingError::NetworkError("Read timeout".to_string()))?
        .map_err(|e| VotingError::NetworkError(format!("Failed to read message: {}", e)))?;
        
        Self::decode_message(&data)
    }
    
    fn encode_message(message: &Message) -> Result<Vec<u8>> {
        common::utils::serialize(message)
            .map_err(|e| VotingError::SerializationError(format!("{}", e)))
    }
    
    fn decode_message(data: &[u8]) -> Result<Message> {
        common::utils::deserialize(data)
            .map_err(|e| VotingError::DeserializationError(format!("{}", e)))
    }
    
    async fn close(&self) -> Result<()> {
        let mut stream = self.stream.write().await;
        stream
            .shutdown()
            .await
            .map_err(|e| VotingError::NetworkError(format!("Failed to shutdown: {}", e)))?;
        Ok(())
    }
}

/// Transport statistics
#[derive(Debug, Clone, Default)]
pub struct TransportStats {
    pub total_connections: usize,
    pub inbound_connections: usize,
    pub outbound_connections: usize,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Message framing for wire protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessageFrame {
    version: u8,
    flags: u8,
    payload_length: u32,
    payload: Vec<u8>,
}

impl MessageFrame {
    const VERSION: u8 = 1;
    
    fn new(payload: Vec<u8>) -> Self {
        Self {
            version: Self::VERSION,
            flags: 0,
            payload_length: payload.len() as u32,
            payload,
        }
    }
    
    fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(6 + self.payload.len());
        data.push(self.version);
        data.push(self.flags);
        data.extend_from_slice(&self.payload_length.to_be_bytes());
        data.extend_from_slice(&self.payload);
        data
    }
    
    fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 6 {
            return Err(VotingError::NetworkError(
                "Frame too short".to_string(),
            ));
        }
        
        let version = data[0];
        if version != Self::VERSION {
            return Err(VotingError::NetworkError(
                "Unsupported protocol version".to_string(),
            ));
        }
        
        let flags = data[1];
        let payload_length = u32::from_be_bytes([data[2], data[3], data[4], data[5]]);
        
        if data.len() < 6 + payload_length as usize {
            return Err(VotingError::NetworkError(
                "Incomplete frame".to_string(),
            ));
        }
        
        let payload = data[6..6 + payload_length as usize].to_vec();
        
        Ok(Self {
            version,
            flags,
            payload_length,
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_config_default() {
        let config = TransportConfig::default();
        assert_eq!(config.connection_timeout, 30);
        assert_eq!(config.max_message_size, 10 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_transport_creation() {
        let config = TransportConfig::default();
        let transport = Transport::new(config);
        assert!(transport.is_ok());
    }

    #[tokio::test]
    async fn test_transport_stats() {
        let config = TransportConfig::default();
        let transport = Transport::new(config).unwrap();
        
        let stats = transport.get_stats().await;
        assert_eq!(stats.total_connections, 0);
    }

    #[test]
    fn test_message_frame_encode_decode() {
        let payload = vec![1, 2, 3, 4, 5];
        let frame = MessageFrame::new(payload.clone());
        
        let encoded = frame.encode();
        let decoded = MessageFrame::decode(&encoded).unwrap();
        
        assert_eq!(decoded.version, MessageFrame::VERSION);
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn test_message_frame_version() {
        let frame = MessageFrame::new(vec![1, 2, 3]);
        assert_eq!(frame.version, 1);
    }

    #[test]
    fn test_message_frame_invalid_version() {
        let mut data = vec![99, 0, 0, 0, 0, 3, 1, 2, 3];
        let result = MessageFrame::decode(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_message_frame_too_short() {
        let data = vec![1, 2, 3];
        let result = MessageFrame::decode(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_message_frame_incomplete() {
        let data = vec![1, 0, 0, 0, 0, 10, 1, 2];
        let result = MessageFrame::decode(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_transport_stats_default() {
        let stats = TransportStats::default();
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.bytes_sent, 0);
        assert_eq!(stats.bytes_received, 0);
    }

    #[test]
    fn test_message_frame_empty_payload() {
        let frame = MessageFrame::new(vec![]);
        assert_eq!(frame.payload_length, 0);
        
        let encoded = frame.encode();
        let decoded = MessageFrame::decode(&encoded).unwrap();
        assert!(decoded.payload.is_empty());
    }

    #[test]
    fn test_message_frame_large_payload() {
        let payload = vec![42u8; 1000];
        let frame = MessageFrame::new(payload.clone());
        
        let encoded = frame.encode();
        assert_eq!(encoded.len(), 6 + 1000);
        
        let decoded = MessageFrame::decode(&encoded).unwrap();
        assert_eq!(decoded.payload, payload);
    }
}

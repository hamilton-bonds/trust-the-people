//! RPC server implementation for JSON-RPC 2.0 protocol
//!
//! This module provides the HTTP server that handles JSON-RPC requests
//! and routes them to the appropriate handlers.

use crate::{
    handlers::RpcHandler, JsonRpcError, JsonRpcRequest, JsonRpcResponse, RateLimiter, RequestId,
    RpcConfig, RpcService,
};
use common::{Result, VotingError};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// RPC server that handles JSON-RPC 2.0 requests
pub struct RpcServer {
    config: RpcConfig,
    service: Arc<dyn RpcService>,
    rate_limiter: Option<Arc<RateLimiter>>,
    shutdown: Arc<RwLock<bool>>,
}

impl RpcServer {
    /// Create a new RPC server
    pub fn new(config: RpcConfig, service: Arc<dyn RpcService>) -> Self {
        let rate_limiter = if config.enable_rate_limit {
            Some(Arc::new(RateLimiter::new(config.rate_limit_per_second)))
        } else {
            None
        };

        Self {
            config,
            service,
            rate_limiter,
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the RPC server
    pub async fn start(&self) -> Result<()> {
        let addr = format!("{}:{}", self.config.listen_addr, self.config.port);
        let socket_addr: SocketAddr = addr
            .parse()
            .map_err(|e| VotingError::NetworkError(format!("Invalid address: {}", e)))?;

        let listener = TcpListener::bind(socket_addr)
            .await
            .map_err(|e| VotingError::NetworkError(format!("Failed to bind: {}", e)))?;

        info!("RPC server listening on {}", addr);

        // Spawn rate limiter cleanup task
        if let Some(rate_limiter) = &self.rate_limiter {
            let limiter = Arc::clone(rate_limiter);
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                    limiter.cleanup().await;
                }
            });
        }

        loop {
            // Check for shutdown signal
            if *self.shutdown.read().await {
                info!("RPC server shutting down");
                break;
            }

            // Accept new connection
            let (stream, peer_addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                    continue;
                }
            };

            // Clone necessary data for the task
            let config = self.config.clone();
            let service = Arc::clone(&self.service);
            let rate_limiter = self.rate_limiter.clone();

            // Spawn task to handle the connection
            tokio::spawn(async move {
                if let Err(e) =
                    handle_connection(stream, peer_addr, config, service, rate_limiter).await
                {
                    error!("Error handling connection from {}: {}", peer_addr, e);
                }
            });
        }

        Ok(())
    }

    /// Shutdown the RPC server
    pub async fn shutdown(&self) {
        info!("Initiating RPC server shutdown");
        let mut shutdown = self.shutdown.write().await;
        *shutdown = true;
    }

    /// Get server configuration
    pub fn config(&self) -> &RpcConfig {
        &self.config
    }
}

/// Handle a single HTTP connection
async fn handle_connection(
    mut stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    config: RpcConfig,
    service: Arc<dyn RpcService>,
    rate_limiter: Option<Arc<RateLimiter>>,
) -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let peer_ip = peer_addr.ip().to_string();

    // Check rate limit
    if let Some(limiter) = &rate_limiter {
        if !limiter.is_allowed(&peer_ip).await {
            warn!("Rate limit exceeded for {}", peer_ip);
            let response = create_error_response(
                RequestId::Null,
                JsonRpcError::custom(-32000, "Rate limit exceeded"),
            );
            send_response(&mut stream, &response).await?;
            return Ok(());
        }
    }

    // Read HTTP request with timeout
    let mut buffer = vec![0u8; config.max_request_size];
    let bytes_read = tokio::time::timeout(
        tokio::time::Duration::from_secs(config.request_timeout),
        stream.read(&mut buffer),
    )
    .await
    .map_err(|_| VotingError::NetworkError("Request timeout".to_string()))?
    .map_err(|e| VotingError::NetworkError(format!("Failed to read request: {}", e)))?;

    if bytes_read == 0 {
        return Ok(());
    }

    let request_data = &buffer[..bytes_read];

    // Parse HTTP request
    let (method, body) = match parse_http_request(request_data) {
        Ok(parsed) => parsed,
        Err(e) => {
            error!("Failed to parse HTTP request: {}", e);
            let response = create_error_response(
                RequestId::Null,
                JsonRpcError::parse_error("Invalid HTTP request"),
            );
            send_response(&mut stream, &response).await?;
            return Ok(());
        }
    };

    // Only accept POST requests
    if method != "POST" {
        let response = create_error_response(
            RequestId::Null,
            JsonRpcError::invalid_request("Only POST method is supported"),
        );
        send_response(&mut stream, &response).await?;
        return Ok(());
    }

    // Check authentication if enabled
    if config.enable_auth {
        if let Some(api_key) = &config.api_key {
            // Extract Authorization header from request_data
            let auth_valid = check_authorization(request_data, api_key);
            if !auth_valid {
                let response = create_error_response(
                    RequestId::Null,
                    JsonRpcError::custom(-32001, "Unauthorized"),
                );
                send_response(&mut stream, &response).await?;
                return Ok(());
            }
        }
    }

    // Parse JSON-RPC request
    let rpc_request: JsonRpcRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            error!("Failed to parse JSON-RPC request: {}", e);
            let response = create_error_response(
                RequestId::Null,
                JsonRpcError::parse_error("Invalid JSON-RPC request"),
            );
            send_response(&mut stream, &response).await?;
            return Ok(());
        }
    };

    debug!(
        "Received RPC request: method={}, id={:?}",
        rpc_request.method, rpc_request.id
    );

    // Validate JSON-RPC version
    if rpc_request.jsonrpc != "2.0" {
        let response = create_error_response(
            rpc_request.id,
            JsonRpcError::invalid_request("JSON-RPC version must be 2.0"),
        );
        send_response(&mut stream, &response).await?;
        return Ok(());
    }

    // Handle the RPC request
    let handler = RpcHandler::new(service);
    let response = handler.handle_request(rpc_request).await;

    // Send response
    send_response(&mut stream, &response).await?;

    Ok(())
}

/// Parse HTTP request and extract method and body
fn parse_http_request(data: &[u8]) -> Result<(String, Vec<u8>)> {
    let request_str = std::str::from_utf8(data)
        .map_err(|e| VotingError::NetworkError(format!("Invalid UTF-8: {}", e)))?;

    // Find the double CRLF that separates headers from body
    let header_end = request_str
        .find("\r\n\r\n")
        .ok_or_else(|| VotingError::NetworkError("Invalid HTTP request".to_string()))?;

    let headers = &request_str[..header_end];
    let body_start = header_end + 4;

    // Extract method from first line
    let first_line = headers
        .lines()
        .next()
        .ok_or_else(|| VotingError::NetworkError("Empty HTTP request".to_string()))?;

    let method = first_line
        .split_whitespace()
        .next()
        .ok_or_else(|| VotingError::NetworkError("Invalid HTTP method".to_string()))?
        .to_string();

    let body = data[body_start..].to_vec();

    Ok((method, body))
}

/// Check if Authorization header matches API key
fn check_authorization(request_data: &[u8], api_key: &str) -> bool {
    let request_str = match std::str::from_utf8(request_data) {
        Ok(s) => s,
        Err(_) => return false,
    };

    for line in request_str.lines() {
        if line.to_lowercase().starts_with("authorization:") {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() == 2 {
                let auth_value = parts[1].trim();
                // Support both "Bearer <token>" and direct token
                if auth_value == api_key || auth_value == format!("Bearer {}", api_key) {
                    return true;
                }
            }
        }
    }

    false
}

/// Create error response
fn create_error_response(id: RequestId, error: JsonRpcError) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: None,
        error: Some(error),
        id,
    }
}

/// Send HTTP response with JSON-RPC payload
async fn send_response(
    stream: &mut tokio::net::TcpStream,
    response: &JsonRpcResponse,
) -> Result<()> {
    use tokio::io::AsyncWriteExt;

    let json = serde_json::to_vec(response)
        .map_err(|e| VotingError::SerializationError(format!("Failed to serialize: {}", e)))?;

    let http_response = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n",
        json.len()
    );

    stream
        .write_all(http_response.as_bytes())
        .await
        .map_err(|e| VotingError::NetworkError(format!("Failed to write headers: {}", e)))?;

    stream
        .write_all(&json)
        .await
        .map_err(|e| VotingError::NetworkError(format!("Failed to write body: {}", e)))?;

    stream
        .flush()
        .await
        .map_err(|e| VotingError::NetworkError(format!("Failed to flush: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_http_request() {
        let request = b"POST /rpc HTTP/1.1\r\n\
                        Host: localhost:8545\r\n\
                        Content-Type: application/json\r\n\
                        Content-Length: 13\r\n\
                        \r\n\
                        {\"test\":true}";

        let (method, body) = parse_http_request(request).unwrap();
        assert_eq!(method, "POST");
        assert_eq!(body, b"{\"test\":true}");
    }

    #[test]
    fn test_parse_http_request_invalid() {
        let request = b"invalid request";
        assert!(parse_http_request(request).is_err());
    }

    #[test]
    fn test_check_authorization_bearer() {
        let request = b"POST /rpc HTTP/1.1\r\n\
                        Authorization: Bearer secret123\r\n\
                        \r\n";

        assert!(check_authorization(request, "secret123"));
        assert!(!check_authorization(request, "wrong"));
    }

    #[test]
    fn test_check_authorization_direct() {
        let request = b"POST /rpc HTTP/1.1\r\n\
                        Authorization: secret123\r\n\
                        \r\n";

        assert!(check_authorization(request, "secret123"));
    }

    #[test]
    fn test_create_error_response() {
        let response = create_error_response(
            RequestId::Number(1),
            JsonRpcError::method_not_found("test"),
        );

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());
        assert_eq!(response.id, RequestId::Number(1));
    }
}

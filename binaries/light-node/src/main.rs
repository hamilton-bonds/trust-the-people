//! Light Node Binary
//!
//! This binary runs a light node that maintains only block headers and
//! verifies specific transactions on demand. Light nodes are ideal for
//! resource-constrained environments while still providing cryptographic
//! verification of the blockchain.

use clap::Parser;
use common::{logger, Result, VotingError};
use node::{LightNode, NodeConfig, NodeType};
use std::path::PathBuf;
use tokio::signal;
use tracing::{error, info, warn};

/// Light node for blockchain voting system
#[derive(Parser, Debug)]
#[clap(name = "light-node")]
#[clap(version, about = "Blockchain voting light node", long_about = None)]
struct Args {
    /// Path to configuration file
    #[clap(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Path to genesis block file
    #[clap(short, long, value_name = "FILE")]
    genesis: Option<PathBuf>,

    /// Data directory for block headers storage
    #[clap(short, long, value_name = "DIR", default_value = "./data/light-node")]
    data_dir: PathBuf,

    /// Network listen address
    #[clap(short = 'l', long, value_name = "ADDR", default_value = "0.0.0.0:9002")]
    listen_addr: String,

    /// Bootstrap peers (comma-separated)
    #[clap(short, long, value_name = "PEERS")]
    bootstrap_peers: Option<String>,

    /// Enable RPC server
    #[clap(long, default_value = "true")]
    enable_rpc: bool,

    /// RPC server address
    #[clap(long, value_name = "ADDR", default_value = "127.0.0.1:8547")]
    rpc_addr: String,

    /// Enable metrics server
    #[clap(long)]
    enable_metrics: bool,

    /// Metrics server address
    #[clap(long, value_name = "ADDR", default_value = "127.0.0.1:9092")]
    metrics_addr: String,

    /// Log level (trace, debug, info, warn, error)
    #[clap(long, default_value = "info")]
    log_level: String,

    /// Enable file logging
    #[clap(long)]
    log_to_file: bool,

    /// Log file path
    #[clap(long, value_name = "FILE")]
    log_file: Option<PathBuf>,

    /// Network ID (mainnet, testnet, devnet)
    #[clap(long, default_value = "mainnet")]
    network_id: String,

    /// Maximum number of peers to connect to
    #[clap(long, default_value = "20")]
    max_peers: usize,

    /// Cache size in MB
    #[clap(long, default_value = "64")]
    cache_size_mb: usize,

    /// Trusted checkpoint block height (optional)
    #[clap(long)]
    checkpoint_height: Option<u64>,

    /// Trusted checkpoint block hash (optional)
    #[clap(long)]
    checkpoint_hash: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let logging_config = common::config::LoggingConfig {
        level: parse_log_level(&args.log_level)?,
        log_to_console: true,
        log_to_file: args.log_to_file,
        log_file_path: args.log_file.clone().unwrap_or_else(|| {
            args.data_dir.join("logs").join("light-node.log")
        }),
        format: common::config::LogFormat::Pretty,
    };

    logger::init_logger(&logging_config)
        .map_err(|e| VotingError::ConfigurationError(format!("Failed to initialize logger: {}", e)))?;

    info!("Starting light node");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
    info!("Data directory: {}", args.data_dir.display());

    if args.checkpoint_height.is_some() && args.checkpoint_hash.is_some() {
        info!(
            "Using trusted checkpoint: height={}, hash={}",
            args.checkpoint_height.unwrap(),
            args.checkpoint_hash.as_ref().unwrap()
        );
    } else if args.checkpoint_height.is_some() || args.checkpoint_hash.is_some() {
        warn!("Checkpoint height and hash must both be provided. Ignoring incomplete checkpoint.");
    }

    let config = if let Some(config_path) = args.config {
        info!("Loading configuration from: {}", config_path.display());
        NodeConfig::from_file(config_path.to_str().unwrap())
            .map_err(|e| VotingError::ConfigurationError(format!("Failed to load config: {}", e)))?
    } else {
        info!("Using command-line configuration");
        build_config_from_args(&args)?
    };

    config.validate()
        .map_err(|e| VotingError::ConfigurationError(e))?;

    info!("Configuration validated successfully");
    info!("Network ID: {}", config.network.network_id);
    info!("Listen address: {}", config.network.listen_addr);
    info!("Bootstrap peers: {}", config.network.bootstrap_peers.len());
    info!("Cache size: {}MB", config.storage.cache_size_mb);

    if config.enable_rpc {
        info!("RPC server will be available at: {}", args.rpc_addr);
    }

    if config.enable_metrics {
        info!("Metrics server will be available at: {}", args.metrics_addr);
    }

    info!("Light node uses minimal storage and bandwidth");
    info!("Only block headers are stored locally");

    let light_node = LightNode::new(config)
        .map_err(|e| {
            error!("Failed to create light node: {}", e);
            e
        })?;

    info!("Light node created successfully");

    info!("Starting light node...");
    light_node.start().await.map_err(|e| {
        error!("Failed to start light node: {}", e);
        e
    })?;

    info!("Light node started successfully");
    info!("Node is now syncing block headers");
    info!("Press Ctrl+C to stop the node");

    let node_status_task = {
        let light_node = light_node.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let status = light_node.status().await;
                info!(
                    "Light node status: Height={} Peers={} Syncing={}",
                    status.current_height,
                    status.peer_count,
                    status.is_syncing
                );
            }
        })
    };

    let shutdown_signal = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        info!("Received shutdown signal");
    };

    tokio::select! {
        _ = shutdown_signal => {
            info!("Initiating graceful shutdown...");
        }
    }

    node_status_task.abort();

    info!("Stopping light node...");
    light_node.stop().await?;
    info!("Light node stopped successfully");
    info!("Goodbye!");

    Ok(())
}

fn build_config_from_args(args: &Args) -> Result<NodeConfig> {
    std::fs::create_dir_all(&args.data_dir)
        .map_err(|e| VotingError::ConfigurationError(format!("Failed to create data directory: {}", e)))?;

    if args.log_to_file {
        let log_dir = args.data_dir.join("logs");
        std::fs::create_dir_all(&log_dir)
            .map_err(|e| VotingError::ConfigurationError(format!("Failed to create log directory: {}", e)))?;
    }

    let bootstrap_peers = if let Some(peers_str) = &args.bootstrap_peers {
        let peers: Vec<_> = peers_str
            .split(',')
            .filter_map(|s| {
                let trimmed = s.trim();
                match trimmed.parse() {
                    Ok(addr) => Some(addr),
                    Err(_) => {
                        warn!("Invalid bootstrap peer address: {}", trimmed);
                        None
                    }
                }
            })
            .collect();
        
        if peers.is_empty() {
            warn!("No valid bootstrap peers provided. Light node requires peers to sync.");
            return Err(VotingError::ConfigurationError(
                "Light nodes require at least one bootstrap peer".to_string()
            ));
        } else {
            info!("Configured {} bootstrap peer(s)", peers.len());
        }
        
        peers
    } else {
        return Err(VotingError::ConfigurationError(
            "Light nodes require bootstrap peers. Use --bootstrap-peers".to_string()
        ));
    };

    let network_config = common::config::NetworkConfig {
        listen_addr: args.listen_addr.parse()
            .map_err(|e| VotingError::ConfigurationError(format!("Invalid listen address: {}", e)))?,
        bootstrap_peers,
        max_peers: args.max_peers,
        connection_timeout: 30,
        enable_discovery: true,
        network_id: args.network_id.clone(),
    };

    let storage_config = common::config::StorageConfig {
        db_path: args.data_dir.join("headers"),
        cache_size_mb: args.cache_size_mb,
        enable_pruning: true,
        pruning_retention_days: 7,
        enable_compression: true,
    };

    let consensus_config = common::config::ConsensusConfig {
        block_time_ms: 10000,
        min_validators: 3,
        max_validators: 100,
        finality_threshold: 0.67,
    };

    let rpc_config = common::config::RpcConfig {
        enabled: args.enable_rpc,
        listen_addr: args.rpc_addr.clone(),
        max_connections: 50,
        request_timeout_ms: 30000,
        enable_cors: true,
        allowed_origins: vec!["*".to_string()],
    };

    Ok(NodeConfig {
        node_type: NodeType::Light,
        network: network_config,
        storage: storage_config,
        consensus: consensus_config,
        genesis_path: args.genesis.clone(),
        validator_key_path: None,
        data_dir: args.data_dir.clone(),
        enable_metrics: args.enable_metrics,
        metrics_addr: if args.enable_metrics {
            Some(args.metrics_addr.clone())
        } else {
            None
        },
        enable_rpc: args.enable_rpc,
        rpc_addr: if args.enable_rpc {
            Some(args.rpc_addr.clone())
        } else {
            None
        },
        log_level: args.log_level.clone(),
        log_to_file: args.log_to_file,
        log_file_path: args.log_file.clone(),
    })
}

fn parse_log_level(level: &str) -> Result<common::config::LogLevel> {
    match level.to_lowercase().as_str() {
        "trace" => Ok(common::config::LogLevel::Trace),
        "debug" => Ok(common::config::LogLevel::Debug),
        "info" => Ok(common::config::LogLevel::Info),
        "warn" => Ok(common::config::LogLevel::Warn),
        "error" => Ok(common::config::LogLevel::Error),
        _ => Err(VotingError::ConfigurationError(format!(
            "Invalid log level: {}. Must be one of: trace, debug, info, warn, error",
            level
        ))),
    }
}

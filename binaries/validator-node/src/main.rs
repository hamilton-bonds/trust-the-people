//! Validator Node Binary
//!
//! This binary runs a validator node that produces and validates blocks
//! in the blockchain voting system.

use clap::Parser;
use common::{logger, Result, VotingError};
use node::{NodeConfig, NodeType, ValidatorNode};
use std::path::PathBuf;
use tokio::signal;
use tracing::{error, info};

/// Validator node for blockchain voting system
#[derive(Parser, Debug)]
#[clap(name = "validator-node")]
#[clap(version, about = "Blockchain voting validator node", long_about = None)]
struct Args {
    /// Path to configuration file
    #[clap(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Path to genesis block file
    #[clap(short, long, value_name = "FILE")]
    genesis: Option<PathBuf>,

    /// Path to validator key file
    #[clap(short = 'k', long, value_name = "FILE")]
    validator_key: PathBuf,

    /// Data directory for blockchain storage
    #[clap(short, long, value_name = "DIR", default_value = "./data/validator")]
    data_dir: PathBuf,

    /// Network listen address
    #[clap(short = 'l', long, value_name = "ADDR", default_value = "0.0.0.0:9000")]
    listen_addr: String,

    /// Bootstrap peers (comma-separated)
    #[clap(short, long, value_name = "PEERS")]
    bootstrap_peers: Option<String>,

    /// Enable RPC server
    #[clap(long)]
    enable_rpc: bool,

    /// RPC server address
    #[clap(long, value_name = "ADDR", default_value = "127.0.0.1:8545")]
    rpc_addr: String,

    /// Enable metrics server
    #[clap(long)]
    enable_metrics: bool,

    /// Metrics server address
    #[clap(long, value_name = "ADDR", default_value = "127.0.0.1:9090")]
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
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let logging_config = common::config::LoggingConfig {
        level: parse_log_level(&args.log_level)?,
        log_to_console: true,
        log_to_file: args.log_to_file,
        log_file_path: args.log_file.clone().unwrap_or_else(|| {
            args.data_dir.join("logs").join("validator.log")
        }),
        format: common::config::LogFormat::Pretty,
    };

    logger::init_logger(&logging_config)
        .map_err(|e| VotingError::ConfigurationError(format!("Failed to initialize logger: {}", e)))?;

    info!("Starting validator node");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
    info!("Data directory: {}", args.data_dir.display());
    info!("Validator key: {}", args.validator_key.display());

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

    if config.enable_rpc {
        info!("RPC server will be available at: {}", args.rpc_addr);
    }

    if config.enable_metrics {
        info!("Metrics server will be available at: {}", args.metrics_addr);
    }

    let validator_node = ValidatorNode::new(config)
        .map_err(|e| {
            error!("Failed to create validator node: {}", e);
            e
        })?;

    info!("Validator node created successfully");
    info!("Public key: {}", validator_node.validator_public_key());

    info!("Starting validator node...");
    validator_node.start().await.map_err(|e| {
        error!("Failed to start validator node: {}", e);
        e
    })?;

    info!("Validator node started successfully");
    info!("Node is now producing and validating blocks");

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

    info!("Stopping validator node...");
    validator_node.stop().await?;
    info!("Validator node stopped successfully");

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
        peers_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect()
    } else {
        vec![]
    };

    let network_config = common::config::NetworkConfig {
        listen_addr: args.listen_addr.parse()
            .map_err(|e| VotingError::ConfigurationError(format!("Invalid listen address: {}", e)))?,
        bootstrap_peers,
        max_peers: 50,
        connection_timeout: 30,
        enable_discovery: true,
        network_id: args.network_id.clone(),
    };

    let storage_config = common::config::StorageConfig {
        db_path: args.data_dir.join("blockchain"),
        cache_size_mb: 256,
        enable_pruning: false,
        pruning_retention_days: 0,
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
        max_connections: 100,
        request_timeout_ms: 30000,
        enable_cors: true,
        allowed_origins: vec!["*".to_string()],
    };

    Ok(NodeConfig {
        node_type: NodeType::Validator,
        network: network_config,
        storage: storage_config,
        consensus: consensus_config,
        genesis_path: args.genesis.clone(),
        validator_key_path: Some(args.validator_key.clone()),
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

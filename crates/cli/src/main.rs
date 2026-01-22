//! Command-line interface for the blockchain voting system

use clap::{Parser, Subcommand};
use common::Result;
use std::path::PathBuf;
use tracing::error;

mod commands;
mod ui;

use commands::{genesis, keygen, node, query, verify};

/// Blockchain Voting System CLI
#[derive(Parser)]
#[command(name = "voting-cli")]
#[command(version = "1.0.0")]
#[command(about = "Blockchain-based voting system for secure, transparent elections")]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Configuration file path
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    /// Data directory
    #[arg(short, long, global = true)]
    data_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate validator keypair
    GenerateKeys {
        /// Output file path for the keypair
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Generate genesis block
    GenerateGenesis {
        /// Comma-separated list of validator key files
        #[arg(short, long)]
        validators: String,

        /// Chain ID (e.g., "testnet-1", "mainnet")
        #[arg(short, long, default_value = "voting-chain")]
        chain_id: String,

        /// Output file path for genesis.json
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Node operations (start, stop, status)
    #[command(subcommand)]
    Node(NodeCommands),

    /// Query blockchain data
    #[command(subcommand)]
    Query(QueryCommands),

    /// Verify votes and receipts
    #[command(subcommand)]
    Verify(VerifyCommands),
}

#[derive(Subcommand)]
enum NodeCommands {
    /// Start a node
    Start {
        /// Node type (validator, full, light)
        #[arg(short, long, default_value = "full")]
        node_type: String,

        /// Path to genesis file
        #[arg(short, long)]
        genesis: Option<PathBuf>,

        /// Path to validator key (for validator nodes)
        #[arg(short = 'k', long)]
        validator_key: Option<PathBuf>,

        /// Enable RPC server
        #[arg(long, default_value = "true")]
        rpc: bool,

        /// RPC listen address
        #[arg(long, default_value = "127.0.0.1:8545")]
        rpc_addr: String,

        /// Enable metrics
        #[arg(long, default_value = "true")]
        metrics: bool,

        /// Metrics listen address
        #[arg(long, default_value = "127.0.0.1:9090")]
        metrics_addr: String,
    },

    /// Stop a running node
    Stop,

    /// Get node status
    Status {
        /// RPC endpoint
        #[arg(short, long, default_value = "http://127.0.0.1:8545")]
        rpc: String,
    },

    /// Get node information
    Info {
        /// RPC endpoint
        #[arg(short, long, default_value = "http://127.0.0.1:8545")]
        rpc: String,
    },
}

#[derive(Subcommand)]
enum QueryCommands {
    /// Query a block
    Block {
        /// Block identifier (height or hash)
        #[arg(short, long)]
        identifier: String,

        /// RPC endpoint
        #[arg(short, long, default_value = "http://127.0.0.1:8545")]
        rpc: String,
    },

    /// Query a transaction
    Transaction {
        /// Transaction hash
        #[arg(short, long)]
        hash: String,

        /// RPC endpoint
        #[arg(short, long, default_value = "http://127.0.0.1:8545")]
        rpc: String,
    },

    /// Query an election
    Election {
        /// Election ID
        #[arg(short, long)]
        id: String,

        /// RPC endpoint
        #[arg(short, long, default_value = "http://127.0.0.1:8545")]
        rpc: String,
    },

    /// Query election results
    Results {
        /// Election ID
        #[arg(short, long)]
        id: String,

        /// RPC endpoint
        #[arg(short, long, default_value = "http://127.0.0.1:8545")]
        rpc: String,
    },

    /// Query a validator
    Validator {
        /// Validator address
        #[arg(short, long)]
        address: String,

        /// RPC endpoint
        #[arg(short, long, default_value = "http://127.0.0.1:8545")]
        rpc: String,
    },

    /// Query all validators
    Validators {
        /// Block height (optional)
        #[arg(short, long)]
        height: Option<u64>,

        /// RPC endpoint
        #[arg(short, long, default_value = "http://127.0.0.1:8545")]
        rpc: String,
    },

    /// Query chain information
    Chain {
        /// RPC endpoint
        #[arg(short, long, default_value = "http://127.0.0.1:8545")]
        rpc: String,
    },
}

#[derive(Subcommand)]
enum VerifyCommands {
    /// Verify a vote receipt
    Receipt {
        /// Receipt ID
        #[arg(short, long)]
        receipt: String,

        /// RPC endpoint
        #[arg(short, long, default_value = "http://127.0.0.1:8545")]
        rpc: String,
    },

    /// Verify a vote transaction
    Vote {
        /// Transaction hash
        #[arg(short, long)]
        tx_hash: String,

        /// Zero-knowledge proof
        #[arg(short, long)]
        proof: String,

        /// RPC endpoint
        #[arg(short, long, default_value = "http://127.0.0.1:8545")]
        rpc: String,
    },

    /// Verify blockchain integrity
    Chain {
        /// Start height
        #[arg(short, long, default_value = "0")]
        start: u64,

        /// End height
        #[arg(short, long)]
        end: Option<u64>,

        /// RPC endpoint
        #[arg(short, long, default_value = "http://127.0.0.1:8545")]
        rpc: String,
    },

    /// Verify election results
    Election {
        /// Election ID
        #[arg(short, long)]
        id: String,

        /// RPC endpoint
        #[arg(short, long, default_value = "http://127.0.0.1:8545")]
        rpc: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
        )
        .init();

    // Execute command
    let result = match cli.command {
        Commands::GenerateKeys { output } => {
            keygen::generate_keys(&output)
        }

        Commands::GenerateGenesis {
            validators,
            chain_id,
            output,
        } => {
            genesis::generate_genesis(&validators, &chain_id, &output)
        }

        Commands::Node(cmd) => match cmd {
            NodeCommands::Start {
                node_type,
                genesis,
                validator_key,
                rpc,
                rpc_addr,
                metrics,
                metrics_addr,
            } => {
                node::start(
                    node_type,
                    genesis,
                    validator_key,
                    cli.data_dir,
                    rpc,
                    rpc_addr,
                    metrics,
                    metrics_addr,
                )
                .await
            }
            NodeCommands::Stop => node::stop().await,
            NodeCommands::Status { rpc } => node::status(&rpc).await,
            NodeCommands::Info { rpc } => node::info(&rpc).await,
        },

        Commands::Query(cmd) => match cmd {
            QueryCommands::Block { identifier, rpc } => query::block(&identifier, &rpc).await,
            QueryCommands::Transaction { hash, rpc } => query::transaction(&hash, &rpc).await,
            QueryCommands::Election { id, rpc } => query::election(&id, &rpc).await,
            QueryCommands::Results { id, rpc } => query::results(&id, &rpc).await,
            QueryCommands::Validator { address, rpc } => query::validator(&address, &rpc).await,
            QueryCommands::Validators { height, rpc } => query::validators(height, &rpc).await,
            QueryCommands::Chain { rpc } => query::chain(&rpc).await,
        },

        Commands::Verify(cmd) => match cmd {
            VerifyCommands::Receipt { receipt, rpc } => verify::receipt(&receipt, &rpc).await,
            VerifyCommands::Vote {
                tx_hash,
                proof,
                rpc,
            } => verify::vote(&tx_hash, &proof, &rpc).await,
            VerifyCommands::Chain { start, end, rpc } => verify::chain(start, end, &rpc).await,
            VerifyCommands::Election { id, rpc } => verify::election(&id, &rpc).await,
        },
    };

    // Handle result
    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("Command failed: {}", e);
            std::process::exit(1);
        }
    }
}

# Trust the People - A Blockchain Voting System

A decentralized, cryptographically-verified voting system built on blockchain technology. This system enables transparent, auditable, and secure elections where anyone can download and run a node to independently verify results.

## Why This Matters

- **Transparency**: Every vote is recorded on an immutable blockchain that anyone can audit
- **Decentralization**: No single authority controls the system - run your own validator node
- **Verification**: Download the blockchain and verify election results yourself
- **Privacy**: Zero-knowledge proofs protect voter anonymity while ensuring vote validity
- **Security**: Ed25519 signatures, Blake2b hashing, and Proof-of-Authority consensus

## Features

- **Multiple Node Types**: Validator, full, and light nodes for different use cases
- **Cryptographic Verification**: Ed25519 signatures with zero-knowledge proofs
- **Real-time Analytics**: Built-in anomaly detection and statistical analysis
- **JSON-RPC API**: Query blockchain data and submit transactions
- **Fraud Detection**: Automated pattern recognition and duplicate vote prevention
- **Cross-platform**: Linux, macOS, and Windows support

## Quick Start

### Prerequisites

- Rust 1.75.0 or later
- 4GB RAM minimum (8GB recommended)
- 10GB free disk space

### Installation

```bash
git clone https://github.com/hamilton-bonds/trust-the-people.git
cd blockchain-voting
cargo build --release
```

Binaries will be in `target/release/`:
- `validator-node` - Produces and validates blocks
- `full-node` - Maintains complete blockchain, validates blocks
- `light-node` - Header-only verification for resource-constrained environments

### Running a Full Node

```bash
# Generate or obtain genesis block
./scripts/generate-genesis.sh

# Start full node
./target/release/full-node \
  --genesis ./config/genesis.json \
  --data-dir ./data/full-node \
  --bootstrap-peers "127.0.0.1:9000" \
  --enable-rpc \
  --rpc-addr "127.0.0.1:8546"
```

### Running a Validator Node

```bash
# Generate validator keys
cargo run --bin cli -- generate-keys --output ./keys/validator.pem

# Start validator
./target/release/validator-node \
  --genesis ./config/genesis.json \
  --validator-key ./keys/validator.pem \
  --data-dir ./data/validator \
  --enable-rpc
```

### Using the CLI

```bash
# Check node status
cargo run --bin cli -- node status --rpc-endpoint http://127.0.0.1:8546

# Query blockchain
cargo run --bin cli -- query block --height 100

# Verify a vote
cargo run --bin cli -- verify vote --tx-id <transaction-id>

# Run analytics
cargo run --bin cli -- analytics detect-anomalies --election-id <election-id>
```

## Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Validator  │────▶│  Full Node  │────▶│ Light Node  │
│    Node     │     │             │     │             │
└─────────────┘     └─────────────┘     └─────────────┘
      │                    │                    │
      └────────────────────┴────────────────────┘
                           │
                    ┌──────┴──────┐
                    │  Blockchain │
                    │   Storage   │
                    └─────────────┘
```

## Project Structure

```
trust-the-people/
├── crates/              # Core library crates
│   ├── common/          # Shared types and utilities
│   ├── blockchain-core/ # Block, chain, and consensus
│   ├── crypto/          # Cryptographic primitives
│   ├── network/         # P2P networking
│   ├── storage/         # Database layer
│   ├── node/            # Node implementations
│   ├── rpc/             # JSON-RPC server
│   ├── cli/             # Command-line interface
│   └── analytics/       # Analytics and fraud detection
├── binaries/            # Executable binaries
│   ├── validator-node/
│   ├── full-node/
│   └── light-node/
└── config/              # Configuration files
```

## Configuration

Edit configuration files in `config/`:

- `validator.toml` - Validator node settings
- `full-node.toml` - Full node settings
- `genesis.json` - Genesis block configuration
- `network.toml` - Network parameters

Example configuration:

```toml
[network]
listen_addr = "0.0.0.0:9000"
bootstrap_peers = ["validator1.example.com:9000"]
max_peers = 50

[storage]
db_path = "./data/blockchain"
cache_size_mb = 256
enable_pruning = false
```

## API Reference

### JSON-RPC Endpoints

```bash
# Get node info
curl -X POST http://127.0.0.1:8546 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"node_info","id":1}'

# Get block by height
curl -X POST http://127.0.0.1:8546 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"get_block_by_height","params":{"height":100},"id":1}'

# Verify vote
curl -X POST http://127.0.0.1:8546 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"verify_vote","params":{"tx_id":"..."},"id":1}'
```

See [docs/rpc-api.md](docs/rpc-api.md) for complete API documentation.

## Security

- **Ed25519 signatures** for transaction authentication
- **Blake2b hashing** for block integrity
- **ChaCha20-Poly1305** for encrypted data
- **Zero-knowledge proofs** for voter privacy
- **Proof-of-Authority consensus** with BFT finality

See [docs/security.md](docs/security.md) for security considerations.

## Analytics

Built-in fraud detection and anomaly analysis:

```bash
# Detect anomalies
cargo run --bin cli -- analytics detect-anomalies --election-id <id>

# Generate audit report
cargo run --bin cli -- analytics audit --election-id <id> --output report.html

# Compare elections
cargo run --bin cli -- analytics compare --current <id1> --baseline <id2>
```

See [docs/analytics.md](docs/analytics.md) for analytics features.

## Testing

```bash
# Run all tests
cargo test --workspace

# Run specific crate tests
cargo test -p blockchain-core

# Run integration tests
cargo test --test integration

# Run benchmarks
cargo bench
```

## Deployment

### Docker

```bash
# Build validator image
docker build -f docker/Dockerfile.validator -t voting-validator .

# Run with docker-compose
docker-compose -f docker/docker-compose.yml up
```

### Production

See [docs/node-setup.md](docs/node-setup.md) for production deployment guide.

## Documentation

- [Architecture Overview](docs/architecture.md)
- [Cryptography Details](docs/cryptography.md)
- [Consensus Mechanism](docs/consensus.md)
- [Node Setup Guide](docs/node-setup.md)
- [RPC API Reference](docs/rpc-api.md)
- [Security Considerations](docs/security.md)

## License

This project is dual-licensed under MIT OR Apache-2.0.

## Links

- **Repository**: https://github.com/hamilton-bonds/trust-the-people
- **Documentation**: https://docs.working-on-getting-a-domain.org
- **Issue Tracker**: https://github.com/hamilton-bonds/trust-the-people/issues

## Disclaimer

This is a public node system for blockchain verification. It does not handle voter registration or election management, which are managed by authorized government systems in separate repositories.

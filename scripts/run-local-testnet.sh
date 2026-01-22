#!/bin/bash
# Local Testnet Script
# Starts a 5-node local testnet: 3 validators, 1 full node, 1 light node

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

echo "=========================================="
echo "  Blockchain Voting Local Testnet"
echo "=========================================="
echo ""

# Check if binaries exist
if [ ! -f "./target/release/validator-node" ]; then
    echo "Error: Binaries not found. Please run 'cargo build --release' first."
    exit 1
fi

if [ ! -f "./target/release/voting-cli" ]; then
    echo "Error: CLI binary not found. Please run 'cargo build --release' first."
    exit 1
fi

# Clean up previous testnet data
echo "Cleaning up previous testnet data..."
rm -rf ./data/testnet
mkdir -p ./data/testnet/{validator1,validator2,validator3,full1,light1}
mkdir -p ./data/testnet/keys

echo "✓ Testnet directories created"
echo ""

# Generate validator keys
echo "Generating validator keys..."
./target/release/voting-cli generate-keys --output ./data/testnet/keys/validator1.pem
./target/release/voting-cli generate-keys --output ./data/testnet/keys/validator2.pem
./target/release/voting-cli generate-keys --output ./data/testnet/keys/validator3.pem
echo "✓ Generated 3 validator keypairs"
echo ""

# Generate genesis block
echo "Generating genesis block..."
./target/release/voting-cli generate-genesis \
  --validators ./data/testnet/keys/validator1.pem,./data/testnet/keys/validator2.pem,./data/testnet/keys/validator3.pem \
  --chain-id testnet-1 \
  --output ./data/testnet/genesis.json
echo "✓ Genesis block created"
echo ""

# Create PID file directory
mkdir -p ./data/testnet/pids

echo "Starting nodes..."
echo ""

# Start Validator 1 (Bootstrap node)
echo "Starting Validator 1 (bootstrap)..."
./target/release/validator-node \
  --genesis ./data/testnet/genesis.json \
  --validator-key ./data/testnet/keys/validator1.pem \
  --data-dir ./data/testnet/validator1 \
  --listen-addr 127.0.0.1:9000 \
  --rpc-addr 127.0.0.1:8545 \
  --enable-rpc \
  --network-id testnet > ./data/testnet/validator1/node.log 2>&1 &
echo $! > ./data/testnet/pids/validator1.pid
echo "  ✓ Validator 1 started (PID: $(cat ./data/testnet/pids/validator1.pid))"
echo "    Listen: 127.0.0.1:9000"
echo "    RPC:    http://127.0.0.1:8545"

sleep 3

# Start Validator 2
echo ""
echo "Starting Validator 2..."
./target/release/validator-node \
  --genesis ./data/testnet/genesis.json \
  --validator-key ./data/testnet/keys/validator2.pem \
  --data-dir ./data/testnet/validator2 \
  --listen-addr 127.0.0.1:9001 \
  --rpc-addr 127.0.0.1:8546 \
  --bootstrap-peers 127.0.0.1:9000 \
  --enable-rpc \
  --network-id testnet > ./data/testnet/validator2/node.log 2>&1 &
echo $! > ./data/testnet/pids/validator2.pid
echo "  ✓ Validator 2 started (PID: $(cat ./data/testnet/pids/validator2.pid))"
echo "    Listen: 127.0.0.1:9001"
echo "    RPC:    http://127.0.0.1:8546"

sleep 3

# Start Validator 3
echo ""
echo "Starting Validator 3..."
./target/release/validator-node \
  --genesis ./data/testnet/genesis.json \
  --validator-key ./data/testnet/keys/validator3.pem \
  --data-dir ./data/testnet/validator3 \
  --listen-addr 127.0.0.1:9002 \
  --rpc-addr 127.0.0.1:8547 \
  --bootstrap-peers 127.0.0.1:9000,127.0.0.1:9001 \
  --enable-rpc \
  --network-id testnet > ./data/testnet/validator3/node.log 2>&1 &
echo $! > ./data/testnet/pids/validator3.pid
echo "  ✓ Validator 3 started (PID: $(cat ./data/testnet/pids/validator3.pid))"
echo "    Listen: 127.0.0.1:9002"
echo "    RPC:    http://127.0.0.1:8547"

sleep 3

# Start Full Node
echo ""
echo "Starting Full Node..."
./target/release/full-node \
  --genesis ./data/testnet/genesis.json \
  --data-dir ./data/testnet/full1 \
  --listen-addr 127.0.0.1:9003 \
  --rpc-addr 127.0.0.1:8548 \
  --bootstrap-peers 127.0.0.1:9000,127.0.0.1:9001,127.0.0.1:9002 \
  --enable-rpc \
  --network-id testnet > ./data/testnet/full1/node.log 2>&1 &
echo $! > ./data/testnet/pids/full1.pid
echo "  ✓ Full Node started (PID: $(cat ./data/testnet/pids/full1.pid))"
echo "    Listen: 127.0.0.1:9003"
echo "    RPC:    http://127.0.0.1:8548"

sleep 3

# Start Light Node
echo ""
echo "Starting Light Node..."
./target/release/light-node \
  --genesis ./data/testnet/genesis.json \
  --data-dir ./data/testnet/light1 \
  --listen-addr 127.0.0.1:9004 \
  --rpc-addr 127.0.0.1:8549 \
  --bootstrap-peers 127.0.0.1:9000,127.0.0.1:9003 \
  --enable-rpc \
  --network-id testnet > ./data/testnet/light1/node.log 2>&1 &
echo $! > ./data/testnet/pids/light1.pid
echo "  ✓ Light Node started (PID: $(cat ./data/testnet/pids/light1.pid))"
echo "    Listen: 127.0.0.1:9004"
echo "    RPC:    http://127.0.0.1:8549"

sleep 2

echo ""
echo "=========================================="
echo "  Testnet Started Successfully!"
echo "=========================================="
echo ""
echo "Node Information:"
echo "  Validator 1: http://127.0.0.1:8545"
echo "  Validator 2: http://127.0.0.1:8546"
echo "  Validator 3: http://127.0.0.1:8547"
echo "  Full Node:   http://127.0.0.1:8548"
echo "  Light Node:  http://127.0.0.1:8549"
echo ""
echo "Logs are in: ./data/testnet/*/node.log"
echo ""
echo "Test Commands:"
echo "  # Check node status"
echo "  curl -s http://127.0.0.1:8545 -d '{\"jsonrpc\":\"2.0\",\"method\":\"node_info\",\"id\":1}' | jq"
echo ""
echo "  # Check all nodes"
echo "  for port in 8545 8546 8547 8548 8549; do"
echo "    echo \"Node on port \$port:\""
echo "    curl -s http://127.0.0.1:\$port -d '{\"jsonrpc\":\"2.0\",\"method\":\"node_info\",\"id\":1}' | jq .result.node_type"
echo "  done"
echo ""
echo "  # Watch blockchain growth"
echo "  watch -n 2 'curl -s http://127.0.0.1:8545 -d \"{\\\"jsonrpc\\\":\\\"2.0\\\",\\\"method\\\":\\\"chain_info\\\",\\\"id\\\":1}\" | jq'"
echo ""
echo "To stop the testnet:"
echo "  ./scripts/stop-local-testnet.sh"
echo ""
echo "To check status:"
echo "  ./scripts/check-testnet-status.sh"
echo ""
echo "Press Ctrl+C to stop monitoring (nodes will continue running)"
echo ""

# Tail logs from validator 1 to keep script running and show activity
tail -f ./data/testnet/validator1/node.log

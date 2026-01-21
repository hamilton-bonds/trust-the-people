#!/bin/bash
# Check Testnet Status Script
# Queries all nodes and displays their current status

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

echo "=========================================="
echo "  Testnet Status Check"
echo "=========================================="
echo ""

# Check if jq is available
if ! command -v jq &> /dev/null; then
    echo "Warning: jq not found. Install jq for formatted output."
    USE_JQ=false
else
    USE_JQ=true
fi

# Function to check a node
check_node() {
    local name=$1
    local port=$2
    
    printf "%-15s " "$name:"
    
    response=$(curl -s -m 2 http://127.0.0.1:$port \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"node_info","id":1}' 2>/dev/null)
    
    if [ $? -eq 0 ] && [ -n "$response" ]; then
        if [ "$USE_JQ" = true ]; then
            node_type=$(echo "$response" | jq -r '.result.node_type // "unknown"')
            height=$(echo "$response" | jq -r '.result.height // 0')
            peers=$(echo "$response" | jq -r '.result.peer_count // 0')
            syncing=$(echo "$response" | jq -r '.result.syncing // false')
            
            if [ "$syncing" = "true" ]; then
                sync_status="SYNCING"
            else
                sync_status="SYNCED"
            fi
            
            printf "✓ RUNNING  | Type: %-10s | Height: %5s | Peers: %2s | %s\n" \
                "$node_type" "$height" "$peers" "$sync_status"
        else
            echo "✓ RUNNING  | Port: $port"
        fi
    else
        echo "✗ NOT RUNNING"
    fi
}

# Check all nodes
check_node "Validator 1" 8545
check_node "Validator 2" 8546
check_node "Validator 3" 8547
check_node "Full Node" 8548
check_node "Light Node" 8549

echo ""
echo "=========================================="
echo ""

# Check if any nodes are running
if pgrep -f "validator-node" > /dev/null || pgrep -f "full-node" > /dev/null || pgrep -f "light-node" > /dev/null; then
    echo "Active Processes:"
    echo ""
    ps aux | grep -E "(validator-node|full-node|light-node)" | grep -v grep | awk '{printf "  PID: %-6s  CPU: %-5s  MEM: %-5s  CMD: %s\n", $2, $3"%", $4"%", $11}'
    echo ""
    
    echo "Logs available at:"
    echo "  ./data/testnet/validator1/node.log"
    echo "  ./data/testnet/validator2/node.log"
    echo "  ./data/testnet/validator3/node.log"
    echo "  ./data/testnet/full1/node.log"
    echo "  ./data/testnet/light1/node.log"
else
    echo "No testnet nodes currently running."
    echo ""
    echo "To start testnet:"
    echo "  ./scripts/run-local-testnet.sh"
fi

echo ""

#!/bin/bash
# Stop Local Testnet Script
# Gracefully stops all running testnet nodes

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

echo "=========================================="
echo "  Stopping Local Testnet"
echo "=========================================="
echo ""

PID_DIR="./data/testnet/pids"

if [ ! -d "$PID_DIR" ]; then
    echo "No testnet running (PID directory not found)"
    exit 0
fi

# Function to stop a node gracefully
stop_node() {
    local name=$1
    local pid_file="$PID_DIR/$name.pid"
    
    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        if ps -p $pid > /dev/null 2>&1; then
            echo "Stopping $name (PID: $pid)..."
            kill -TERM $pid
            
            # Wait up to 10 seconds for graceful shutdown
            local count=0
            while ps -p $pid > /dev/null 2>&1 && [ $count -lt 10 ]; do
                sleep 1
                count=$((count + 1))
            done
            
            # Force kill if still running
            if ps -p $pid > /dev/null 2>&1; then
                echo "  Force stopping $name..."
                kill -9 $pid
            fi
            
            echo "  ✓ $name stopped"
        else
            echo "  $name not running (stale PID file)"
        fi
        rm -f "$pid_file"
    else
        echo "  $name PID file not found"
    fi
}

# Stop nodes in reverse order (light -> full -> validators)
stop_node "light1"
stop_node "full1"
stop_node "validator3"
stop_node "validator2"
stop_node "validator1"

# Clean up PID directory
if [ -d "$PID_DIR" ]; then
    rmdir "$PID_DIR" 2>/dev/null || true
fi

echo ""
echo "=========================================="
echo "  Testnet Stopped"
echo "=========================================="
echo ""
echo "Data preserved in: ./data/testnet/"
echo ""
echo "To clean up all testnet data:"
echo "  rm -rf ./data/testnet"
echo ""
echo "To restart testnet:"
echo "  ./scripts/run-local-testnet.sh"
echo ""

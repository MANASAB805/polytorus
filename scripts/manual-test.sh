#!/bin/bash

# Manual Testing Helper Script for PolyTorus Testnet
# This script provides convenient commands for manual testing

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }

# Helper functions
show_help() {
    echo "PolyTorus Testnet Manual Testing Helper"
    echo "======================================="
    echo ""
    echo "Usage: $0 [COMMAND]"
    echo ""
    echo "Commands:"
    echo "  start          Build and start the testnet"
    echo "  stop           Stop and cleanup the testnet"
    echo "  status         Show testnet status"
    echo "  logs [NODE]    Show logs for a specific node or all nodes"
    echo "  exec [NODE]    Execute commands in a specific node"
    echo "  test-tx        Send a test transaction"
    echo "  test-p2p       Test P2P connectivity"
    echo "  nodes          List all running nodes"
    echo "  help           Show this help message"
    echo ""
    echo "Available nodes: bootstrap, validator1, validator2, fullnode1, fullnode2"
}

build_and_start() {
    log_info "Building Docker image..."
    cd "$PROJECT_ROOT"
    docker build -f Dockerfile.testnet -t polytorus:testnet .
    
    log_info "Starting testnet..."
    sudo containerlab deploy -t testnet.yml
    
    log_success "Testnet started! Waiting for nodes to initialize..."
    sleep 30
    
    show_status
}

stop_testnet() {
    log_info "Stopping testnet..."
    cd "$PROJECT_ROOT"
    sudo containerlab destroy -t testnet.yml --cleanup
    log_success "Testnet stopped"
}

show_status() {
    log_info "Testnet Status:"
    docker ps --filter "name=clab-polytorus-testnet" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
    
    echo ""
    log_info "Node IPs:"
    nodes=("bootstrap" "validator1" "validator2" "fullnode1" "fullnode2")
    for node in "${nodes[@]}"; do
        ip=$(docker inspect clab-polytorus-testnet-$node --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' 2>/dev/null || echo "N/A")
        echo "  $node: $ip"
    done
}

show_logs() {
    local node=$1
    if [ -z "$node" ]; then
        log_info "Showing logs for all nodes:"
        nodes=("bootstrap" "validator1" "validator2" "fullnode1" "fullnode2")
        for n in "${nodes[@]}"; do
            echo "=== $n ==="
            docker logs clab-polytorus-testnet-$n --tail 20
            echo ""
        done
    else
        log_info "Showing logs for $node:"
        docker logs clab-polytorus-testnet-$node --tail 50
    fi
}

exec_node() {
    local node=$1
    if [ -z "$node" ]; then
        log_warning "Please specify a node name"
        echo "Available nodes: bootstrap, validator1, validator2, fullnode1, fullnode2"
        return 1
    fi
    
    log_info "Connecting to $node..."
    docker exec -it clab-polytorus-testnet-$node bash
}

test_transaction() {
    log_info "Sending test transaction from validator1..."
    docker exec clab-polytorus-testnet-validator1 polytorus send --from alice --to bob --amount 1000
}

test_p2p() {
    log_info "Testing P2P connectivity..."
    
    # Check if any P2P processes are running
    nodes=("bootstrap" "validator1" "validator2" "fullnode1" "fullnode2")
    for node in "${nodes[@]}"; do
        log_info "Checking P2P processes on $node..."
        docker exec clab-polytorus-testnet-$node ps aux | grep polytorus || echo "No polytorus processes found"
    done
}

list_nodes() {
    log_info "Available nodes in testnet:"
    docker ps --filter "name=clab-polytorus-testnet" --format "{{.Names}}" | sed 's/clab-polytorus-testnet-//'
}

# Main execution
case "$1" in
    start)
        build_and_start
        ;;
    stop)
        stop_testnet
        ;;
    status)
        show_status
        ;;
    logs)
        show_logs "$2"
        ;;
    exec)
        exec_node "$2"
        ;;
    test-tx)
        test_transaction
        ;;
    test-p2p)
        test_p2p
        ;;
    nodes)
        list_nodes
        ;;
    help|--help|-h)
        show_help
        ;;
    "")
        show_help
        ;;
    *)
        log_warning "Unknown command: $1"
        show_help
        exit 1
        ;;
esac
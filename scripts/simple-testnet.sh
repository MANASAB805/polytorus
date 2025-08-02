#!/bin/bash

# PolyTorus Simple Testnet Manager
# Manages simple Docker-based testnet for PolyTorus blockchain

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
PROJECT_DIR="$( cd "$SCRIPT_DIR/.." &> /dev/null && pwd )"

cd "$PROJECT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Build the simple Docker image
build_image() {
    log_info "Building simple PolyTorus Docker image..."
    cargo build --release
    docker build -f Dockerfile.simple -t polytorus:simple .
    log_success "Simple Docker image built successfully"
}

# Start the testnet
start_testnet() {
    log_info "Starting PolyTorus simple testnet..."
    docker-compose -f docker-compose.simple.yml up -d
    log_success "Testnet started successfully"
    
    log_info "Waiting for nodes to initialize..."
    sleep 10
    
    show_status
}

# Stop the testnet
stop_testnet() {
    log_info "Stopping PolyTorus testnet..."
    docker-compose -f docker-compose.simple.yml down
    log_success "Testnet stopped successfully"
}

# Show testnet status
show_status() {
    log_info "Testnet Status:"
    echo "=============="
    docker-compose -f docker-compose.simple.yml ps
    echo ""
    
    log_info "Container Health:"
    docker ps --filter "name=polytorus" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
}

# Show logs for a specific node
show_logs() {
    local node=$1
    if [ -z "$node" ]; then
        log_error "Please specify a node name (bootstrap, node1, node2, client)"
        exit 1
    fi
    
    local container_name="polytorus-$node"
    log_info "Showing logs for $container_name..."
    docker logs -f "$container_name"
}

# Execute a command in a container
exec_command() {
    local node=$1
    shift
    local cmd="$@"
    
    if [ -z "$node" ]; then
        log_error "Please specify a node name"
        exit 1
    fi
    
    local container_name="polytorus-$node"
    log_info "Executing command in $container_name: $cmd"
    docker exec -it "$container_name" $cmd
}

# Send a test transaction using client container
send_transaction() {
    local from=${1:-"alice"}
    local to=${2:-"bob"}
    local amount=${3:-"1000"}
    
    log_info "Sending transaction: $from -> $to ($amount units)"
    
    # Use the client container to send transaction
    if docker exec polytorus-client ./polytorus send --from "$from" --to "$to" --amount "$amount"; then
        log_success "Transaction sent successfully!"
    else
        log_error "Transaction failed!"
        exit 1
    fi
}

# Initialize genesis on client
init_genesis() {
    log_info "Initializing genesis on client..."
    if docker exec polytorus-client ./polytorus start; then
        log_success "Genesis initialized successfully!"
    else
        log_error "Genesis initialization failed!"
        exit 1
    fi
}

# Get blockchain status
blockchain_status() {
    log_info "Getting blockchain status..."
    if docker exec polytorus-client ./polytorus status; then
        log_success "Status retrieved successfully"
    else
        log_error "Could not retrieve blockchain status"
        exit 1
    fi
}

# Get network status from nodes
network_status() {
    log_info "Getting network status from nodes..."
    
    for node in bootstrap node1 node2; do
        echo ""
        log_info "Network stats for $node:"
        if docker exec polytorus-$node ./polytorus network-status 2>/dev/null; then
            log_success "Stats retrieved successfully"
        else
            log_warning "Could not retrieve stats from $node (expected for P2P nodes)"
        fi
        echo "---"
    done
}

# Get peer information
peer_info() {
    log_info "Getting peer information from nodes..."
    
    for node in bootstrap node1 node2; do
        echo ""
        log_info "Peer info for $node:"
        if docker exec polytorus-$node ./polytorus peers 2>/dev/null; then
            log_success "Peer info retrieved successfully"
        else
            log_warning "Could not retrieve peer info from $node (expected for P2P nodes)"
        fi
        echo "---"
    done
}

# Test full transaction flow
test_transactions() {
    log_info "Testing full transaction flow..."
    echo "================================"
    
    # Wait for network to be ready
    log_info "Waiting for network to be ready..."
    sleep 15
    
    # Initialize genesis
    init_genesis
    
    # Wait a bit more
    sleep 5
    
    # Get initial status
    log_info "Initial blockchain status:"
    blockchain_status
    
    # Send multiple transactions
    log_info "Sending test transactions..."
    send_transaction "alice" "bob" "1000"
    sleep 2
    send_transaction "bob" "charlie" "500"
    sleep 2
    send_transaction "charlie" "alice" "250"
    
    # Get final status
    log_info "Final blockchain status:"
    blockchain_status
    
    log_success "Transaction testing completed!"
}

# Main script logic
case "${1:-help}" in
    "build")
        build_image
        ;;
    "start")
        start_testnet
        ;;
    "stop")
        stop_testnet
        ;;
    "status")
        show_status
        ;;
    "logs")
        show_logs "$2"
        ;;
    "exec")
        shift
        exec_command "$@"
        ;;
    "network-status")
        network_status
        ;;
    "peers")
        peer_info
        ;;
    "send-tx")
        send_transaction "$2" "$3" "$4"
        ;;
    "init-genesis")
        init_genesis
        ;;
    "blockchain-status")
        blockchain_status
        ;;
    "test")
        test_transactions
        ;;
    "cleanup")
        log_info "Cleaning up testnet environment..."
        docker-compose -f docker-compose.simple.yml down -v
        log_success "Cleanup completed"
        ;;
    "help"|*)
        echo "PolyTorus Simple Testnet Manager"
        echo "==============================="
        echo ""
        echo "Usage: $0 <command> [options]"
        echo ""
        echo "Commands:"
        echo "  build                     Build the simple Docker image"
        echo "  start                     Start the testnet"
        echo "  stop                      Stop the testnet"
        echo "  status                    Show testnet status"
        echo "  logs <node>              Show logs for a node (bootstrap, node1, node2, client)"
        echo "  exec <node> <command>    Execute command in a container"
        echo "  network-status           Get network statistics from all nodes"
        echo "  peers                    Get peer information from all nodes"
        echo "  send-tx [from] [to] [amount]  Send a transaction (defaults: alice bob 1000)"
        echo "  init-genesis             Initialize genesis on client"
        echo "  blockchain-status        Get blockchain status"
        echo "  test                     Run full transaction test"
        echo "  cleanup                  Clean up all containers and data"
        echo "  help                     Show this help message"
        echo ""
        echo "Examples:"
        echo "  $0 build                 # Build the image"
        echo "  $0 start                 # Start testnet"
        echo "  $0 send-tx alice bob 500 # Send 500 from alice to bob"
        echo "  $0 logs bootstrap        # Show bootstrap node logs"
        echo "  $0 test                  # Run complete transaction test"
        ;;
esac
#!/bin/bash

# PolyTorus E2E Test Script for Container Lab Environment
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "🧪 PolyTorus E2E Testing Suite"
echo "================================"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed or not in PATH"
        exit 1
    fi
    
    if ! command -v containerlab &> /dev/null; then
        log_error "Container Lab is not installed or not in PATH"
        exit 1
    fi
    
    log_success "Prerequisites check passed"
}

# Build Docker image
build_image() {
    log_info "Building PolyTorus testnet Docker image..."
    
    cd "$PROJECT_ROOT"
    
    if docker build -f Dockerfile.testnet -t polytorus:testnet .; then
        log_success "Docker image built successfully"
    else
        log_error "Failed to build Docker image"
        exit 1
    fi
}

# Deploy testnet
deploy_testnet() {
    log_info "Deploying PolyTorus testnet..."
    
    cd "$PROJECT_ROOT"
    
    # Clean up any existing deployment
    sudo containerlab destroy -t testnet.yml --cleanup 2>/dev/null || true
    
    # Deploy the testnet
    if sudo containerlab deploy -t testnet.yml; then
        log_success "Testnet deployed successfully"
    else
        log_error "Failed to deploy testnet"
        exit 1
    fi
    
    # Wait for nodes to start
    log_info "Waiting for nodes to initialize..."
    sleep 30
}

# Test node connectivity
test_connectivity() {
    log_info "Testing node connectivity..."
    
    # Get container IPs
    bootstrap_ip=$(docker inspect clab-polytorus-testnet-bootstrap --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}')
    validator1_ip=$(docker inspect clab-polytorus-testnet-validator1 --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}')
    validator2_ip=$(docker inspect clab-polytorus-testnet-validator2 --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}')
    fullnode1_ip=$(docker inspect clab-polytorus-testnet-fullnode1 --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}')
    fullnode2_ip=$(docker inspect clab-polytorus-testnet-fullnode2 --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}')
    
    echo "Node IPs:"
    echo "  Bootstrap: $bootstrap_ip"
    echo "  Validator1: $validator1_ip"
    echo "  Validator2: $validator2_ip"
    echo "  FullNode1: $fullnode1_ip"
    echo "  FullNode2: $fullnode2_ip"
    
    # Test ping connectivity
    nodes=("bootstrap" "validator1" "validator2" "fullnode1" "fullnode2")
    for node in "${nodes[@]}"; do
        if docker exec clab-polytorus-testnet-$node ping -c 1 $bootstrap_ip >/dev/null 2>&1; then
            log_success "Node $node can reach bootstrap node"
        else
            log_warning "Node $node cannot reach bootstrap node"
        fi
    done
}

# Test blockchain operations
test_blockchain_operations() {
    log_info "Testing blockchain operations..."
    
    # Test 1: Initialize blockchain on bootstrap node
    log_info "Test 1: Initializing blockchain on bootstrap node"
    if docker exec clab-polytorus-testnet-bootstrap polytorus start; then
        log_success "Blockchain initialized on bootstrap node"
    else
        log_warning "Failed to initialize blockchain on bootstrap node"
    fi
    
    # Test 2: Send transaction from validator1
    log_info "Test 2: Sending transaction from validator1"
    if docker exec clab-polytorus-testnet-validator1 polytorus send --from alice --to bob --amount 1000; then
        log_success "Transaction sent successfully from validator1"
    else
        log_warning "Failed to send transaction from validator1"
    fi
    
    # Test 3: Check status on multiple nodes
    log_info "Test 3: Checking blockchain status on all nodes"
    nodes=("bootstrap" "validator1" "validator2" "fullnode1" "fullnode2")
    for node in "${nodes[@]}"; do
        log_info "Checking status on $node..."
        if docker exec clab-polytorus-testnet-$node polytorus status; then
            log_success "Status check passed on $node"
        else
            log_warning "Status check failed on $node"
        fi
    done
}

# Test P2P networking
test_p2p_networking() {
    log_info "Testing P2P networking..."
    
    # Start P2P nodes in background
    log_info "Starting P2P networking on nodes..."
    
    # Start bootstrap node with P2P
    docker exec -d clab-polytorus-testnet-bootstrap polytorus start-p2p --node-id bootstrap-node --listen-port 8080
    sleep 5
    
    # Start validator nodes with bootstrap peer
    docker exec -d clab-polytorus-testnet-validator1 polytorus start-p2p --node-id validator-1 --listen-port 8080 --bootstrap-peers bootstrap:8080
    sleep 5
    
    docker exec -d clab-polytorus-testnet-validator2 polytorus start-p2p --node-id validator-2 --listen-port 8080 --bootstrap-peers bootstrap:8080,validator1:8080
    sleep 5
    
    # Start full nodes
    docker exec -d clab-polytorus-testnet-fullnode1 polytorus start-p2p --node-id fullnode-1 --listen-port 8080 --bootstrap-peers bootstrap:8080,validator1:8080
    sleep 5
    
    docker exec -d clab-polytorus-testnet-fullnode2 polytorus start-p2p --node-id fullnode-2 --listen-port 8080 --bootstrap-peers validator1:8080,validator2:8080
    
    log_info "P2P nodes started, waiting for connections to establish..."
    sleep 30
    
    log_success "P2P networking test completed"
}

# Generate test report
generate_report() {
    log_info "Generating test report..."
    
    REPORT_FILE="$PROJECT_ROOT/e2e-test-report.txt"
    
    cat > "$REPORT_FILE" << EOF
PolyTorus E2E Test Report
========================
Generated: $(date)

Network Topology:
- Bootstrap Node (bootstrap): Entry point for new nodes
- Validator Node 1 (validator1): Primary validator
- Validator Node 2 (validator2): Secondary validator  
- Full Node 1 (fullnode1): Non-validating full node
- Full Node 2 (fullnode2): Non-validating full node

Test Results:
EOF
    
    # Check container status
    echo "" >> "$REPORT_FILE"
    echo "Container Status:" >> "$REPORT_FILE"
    docker ps --filter "name=clab-polytorus-testnet" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}" >> "$REPORT_FILE"
    
    # Check logs for each node
    echo "" >> "$REPORT_FILE"
    echo "Node Logs Summary:" >> "$REPORT_FILE"
    nodes=("bootstrap" "validator1" "validator2" "fullnode1" "fullnode2")
    for node in "${nodes[@]}"; do
        echo "--- $node ---" >> "$REPORT_FILE"
        docker logs clab-polytorus-testnet-$node --tail 10 >> "$REPORT_FILE" 2>&1
        echo "" >> "$REPORT_FILE"
    done
    
    log_success "Test report generated: $REPORT_FILE"
}

# Cleanup function
cleanup() {
    log_info "Cleaning up testnet..."
    cd "$PROJECT_ROOT"
    sudo containerlab destroy -t testnet.yml --cleanup
    log_success "Cleanup completed"
}

# Main execution
main() {
    echo "Starting E2E tests..."
    
    check_prerequisites
    build_image
    deploy_testnet
    
    # Run tests
    test_connectivity
    test_blockchain_operations
    test_p2p_networking
    
    # Generate report
    generate_report
    
    # Ask user if they want to keep the testnet running
    echo ""
    read -p "Keep testnet running for manual testing? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        cleanup
    else
        log_info "Testnet is still running. Use 'sudo containerlab destroy -t testnet.yml' to stop it."
        log_info "Access nodes:"
        log_info "  Bootstrap: docker exec -it clab-polytorus-testnet-bootstrap bash"
        log_info "  Validator1: docker exec -it clab-polytorus-testnet-validator1 bash"
        log_info "  Validator2: docker exec -it clab-polytorus-testnet-validator2 bash"
        log_info "  FullNode1: docker exec -it clab-polytorus-testnet-fullnode1 bash"
        log_info "  FullNode2: docker exec -it clab-polytorus-testnet-fullnode2 bash"
    fi
    
    log_success "E2E tests completed!"
}

# Handle script interruption
trap cleanup EXIT

# Run main function
main "$@"
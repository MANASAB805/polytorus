#!/bin/bash

# PolyTorus 100 Transaction Stress Test
# Tests blockchain progression with 100 consecutive transactions

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

# Array of users for transactions
USERS=("alice" "bob" "charlie" "david" "eve" "frank" "grace" "henry")

# Function to get random user
get_random_user() {
    echo "${USERS[$RANDOM % ${#USERS[@]}]}"
}

# Function to get random amount
get_random_amount() {
    echo $((100 + $RANDOM % 900))  # Random amount between 100-999
}

# Function to send a transaction
send_transaction() {
    local tx_num=$1
    local from=$(get_random_user)
    local to=$(get_random_user)
    local amount=$(get_random_amount)
    
    # Ensure from and to are different
    while [ "$from" = "$to" ]; do
        to=$(get_random_user)
    done
    
    log_info "Transaction #$tx_num: $from -> $to ($amount units)"
    
    if ./scripts/simple-testnet.sh send-tx "$from" "$to" "$amount" >/dev/null 2>&1; then
        log_success "Transaction #$tx_num completed"
        return 0
    else
        log_error "Transaction #$tx_num failed"
        return 1
    fi
}

# Function to check blockchain status
check_status() {
    local expected_height=$1
    log_info "Checking blockchain status (expected height: $expected_height)..."
    
    # Get status and extract height
    local status_output=$(./scripts/simple-testnet.sh blockchain-status 2>/dev/null)
    local actual_height=$(echo "$status_output" | grep "Chain Height:" | awk '{print $3}')
    
    if [ "$actual_height" = "$expected_height" ]; then
        log_success "Height matches: $actual_height"
        return 0
    else
        log_error "Height mismatch: expected $expected_height, got $actual_height"
        return 1
    fi
}

# Main stress test function
run_stress_test() {
    local num_transactions=${1:-100}
    
    log_info "Starting stress test with $num_transactions transactions"
    log_info "========================================================"
    
    # Get initial status
    log_info "Getting initial blockchain status..."
    ./scripts/simple-testnet.sh blockchain-status | head -10
    
    local start_time=$(date +%s)
    local failed_count=0
    
    # Send transactions
    for i in $(seq 1 $num_transactions); do
        if [ $((i % 10)) -eq 0 ]; then
            log_info "Progress: $i/$num_transactions transactions completed"
        fi
        
        if ! send_transaction $i; then
            ((failed_count++))
        fi
        
        # Small delay to avoid overwhelming the system
        sleep 0.1
    done
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    log_info "All transactions submitted. Waiting for final state..."
    sleep 5
    
    # Check final status
    log_info "Final blockchain status:"
    ./scripts/simple-testnet.sh blockchain-status
    
    # Calculate statistics
    local success_count=$((num_transactions - failed_count))
    local tps=$(echo "scale=2; $success_count / $duration" | bc -l)
    
    echo ""
    log_info "Stress Test Results:"
    log_info "==================="
    log_info "Total transactions: $num_transactions"
    log_success "Successful: $success_count"
    if [ $failed_count -gt 0 ]; then
        log_warning "Failed: $failed_count"
    else
        log_success "Failed: $failed_count"
    fi
    log_info "Duration: ${duration}s"
    log_info "Throughput: ${tps} TPS"
    
    # Get actual starting height from initial status
    local initial_status=$(./scripts/simple-testnet.sh blockchain-status 2>/dev/null)
    local initial_height=$(echo "$initial_status" | grep "Chain Height:" | awk '{print $3}')
    
    # Verify final height matches expected
    local expected_final_height=$((initial_height + success_count))
    if check_status $expected_final_height; then
        log_success "Blockchain height verification passed!"
        return 0
    else
        log_error "Blockchain height verification failed!"
        return 1
    fi
}

# Handle different commands
case "${1:-test}" in
    "test")
        run_stress_test 100
        ;;
    "quick")
        run_stress_test 10
        ;;
    "custom")
        if [ -z "$2" ]; then
            log_error "Please specify number of transactions: $0 custom <number>"
            exit 1
        fi
        run_stress_test "$2"
        ;;
    "help"|*)
        echo "PolyTorus Stress Test"
        echo "===================="
        echo ""
        echo "Usage: $0 <command>"
        echo ""
        echo "Commands:"
        echo "  test     Run 100 transaction stress test (default)"
        echo "  quick    Run 10 transaction quick test"
        echo "  custom   Run custom number of transactions"
        echo "  help     Show this help message"
        echo ""
        echo "Examples:"
        echo "  $0 test           # Run 100 transactions"
        echo "  $0 quick          # Run 10 transactions"
        echo "  $0 custom 50      # Run 50 transactions"
        ;;
esac
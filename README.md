# PolyTorus

A cutting-edge modular blockchain platform designed for the post-quantum era.

## Project Overview

PolyTorus features a 4-layer modular architecture with separate crates for execution, settlement, consensus, and data availability, providing unprecedented customization and optimization capabilities.

## Architecture

### 4-Layer Modular Architecture

1. **Execution Layer** (`crates/execution/`)
   - WASM-based smart contract execution with gas metering
   - Account-based state management with rollback capabilities
   - Host functions for blockchain operations (balance queries, transfers)

2. **Settlement Layer** (`crates/settlement/`)
   - Optimistic rollup processing with fraud proof verification
   - Challenge submission and processing with time-based expiration
   - Settlement history tracking and penalty system

3. **Consensus Layer** (`crates/consensus/`)
   - Proof-of-Work consensus with configurable difficulty
   - Block validation (structure, PoW, timestamps, transactions)
   - Validator management and mining capabilities

4. **Data Availability Layer** (`crates/data-availability/`)
   - Merkle tree construction and proof verification
   - Data storage with metadata and integrity checks
   - Network-aware data distribution simulation

### Additional Components

- **Traits** (`crates/traits/`) - Shared interfaces and types
- **Main Orchestrator** (`src/main.rs`) - Coordinates all layers
- **External Wallet** - Cryptographic wallet functionality ([github.com/PolyTorus/wallet](https://github.com/PolyTorus/wallet))

## Installation

### Prerequisites

- Rust 1.87 nightly or later
- OpenFHE (MachinaIO fork with `feat/improve_determinant` branch)
- System libraries: cmake, libgmp-dev, libntl-dev, libboost-all-dev

### Setup

```bash
git clone https://github.com/PolyTorus/polytorus.git
cd polytorus

# Build development version
cargo build

# Build release version
cargo build --release
```

## Usage

### Basic Commands

```bash
# Start blockchain node
cargo run start

# Show help
cargo run -- --help

# Process transaction
cargo run send --from alice --to bob --amount 100

# Check status
cargo run status
```

### Running Tests

```bash
# All tests
cargo test

# Individual layer tests
cargo test -p execution
cargo test -p settlement
cargo test -p consensus
cargo test -p data-availability

# Specific test
cargo test test_block_creation -- --nocapture
```

### Code Quality

```bash
# Check compilation
cargo check

# Run clippy
cargo clippy -- -D warnings

# Format code
cargo fmt

# Build without warnings
cargo build 2>&1 | grep -i warning && echo "Warnings found!" || echo "Clean build!"
```

## Configuration

Configuration is managed through:
- `config/modular.toml` - Layer-specific settings
- Layer configuration structs with sensible defaults
- Test configurations for development

## Current Status

- 31 total tests across all layers
- Zero compiler warnings
- Production-ready modular architecture
- Clean separation of concerns between layers

## License

See LICENSE file for details.
# PolyTorus Smart Contract Examples

This directory contains example smart contracts for the PolyTorus blockchain platform. These contracts are written in WebAssembly Text Format (WAT) and demonstrate various use cases.

## Available Examples

### 1. Token Contract (`token/simple_token.wat`)
A basic ERC20-like token implementation with the following features:
- Token initialization with total supply
- Balance tracking for addresses
- Transfer functionality between addresses
- State persistence using host functions

**Key Functions:**
- `initialize(owner_address)` - Initialize token with total supply to owner
- `get_token_balance(address)` - Get token balance for an address
- `transfer(from, to, amount)` - Transfer tokens between addresses

### 2. Voting Contract (`voting/simple_voting.wat`)
A decentralized voting system with time-based proposals:
- Create proposals with voting deadlines
- Cast votes (one vote per address)
- Track vote counts
- Prevent double voting
- Time-based voting periods

**Key Functions:**
- `create_proposal(proposal_id, duration)` - Create a new proposal
- `vote(proposal_id, voter_address)` - Cast a vote
- `get_vote_count(proposal_id)` - Get current vote count
- `has_voted(proposal_id, voter_address)` - Check if address has voted

### 3. Escrow Contract (`escrow/simple_escrow.wat`)
A trustless escrow system for secure transactions:
- Create escrows between buyer and seller
- Hold funds until conditions are met
- Time-based automatic refunds
- Multiple escrow states (pending, completed, cancelled, refunded)

**Key Functions:**
- `create_escrow(id, buyer, seller, amount, timeout)` - Create new escrow
- `complete_escrow(id)` - Release funds to seller
- `cancel_escrow(id)` - Cancel escrow (buyer only)
- `refund_escrow(id)` - Refund after timeout
- `get_escrow_state(id)` - Get current escrow state

## Compiling WAT to WASM

To use these contracts, you need to compile them from WAT (WebAssembly Text) to WASM (WebAssembly Binary):

```bash
# Install WebAssembly Binary Toolkit
cargo install wabt

# Compile a contract
wat2wasm token/simple_token.wat -o token/simple_token.wasm
wat2wasm voting/simple_voting.wat -o voting/simple_voting.wasm
wat2wasm escrow/simple_escrow.wat -o escrow/simple_escrow.wasm
```

## Contract Structure

All contracts follow the PolyTorus smart contract interface:

1. **Required Export**: `verify` function
   ```wat
   (func (export "verify") (param i32 i32 i32 i32) (result i32))
   ```
   - Parameters: witness_ptr, witness_len, params_ptr, params_len
   - Returns: 1 for success, 0 for failure

2. **Available Host Functions**:
   - `get_balance(address)` - Query blockchain balance
   - `log(message)` - Emit log messages
   - `get_state(key)` / `set_state(key, value)` - Persistent storage
   - `get_timestamp()` - Current blockchain timestamp
   - `verify_signature(msg, sig, pubkey)` - Signature verification
   - `sha256(data)` - Hash computation
   - `get_block_height()` - Current block height

## Deploying Contracts (Programmatic)

Currently, contract deployment requires using the PolyTorus API programmatically:

```rust
use execution::execution_engine::PolyTorusExecutionLayer;
use traits::ScriptType;

// Read compiled WASM
let wasm_bytes = std::fs::read("simple_token.wasm")?;

// Deploy contract
let script_hash = execution_layer.deploy_script(
    "owner_address",
    ScriptType::Wasm(wasm_bytes),
    vec![], // initialization parameters
    Some("My Token Contract")
).await?;

println!("Contract deployed at: {}", script_hash);
```

## Testing Contracts

You can test contracts using the script engine directly:

```rust
// Load and execute contract
let result = script_engine.execute_script(
    &ScriptType::Wasm(wasm_bytes),
    &witness_data,
    &params,
    &context,
    gas_limit
)?;

assert_eq!(result.success, true);
```

## Security Considerations

1. **Gas Limits**: All contracts consume gas for operations
2. **Memory Limits**: Contracts have restricted memory (256 pages max)
3. **No Threading**: Contracts run in single-threaded environment
4. **Sandboxed Execution**: No access to system resources
5. **Deterministic**: All operations must be deterministic

## Future Improvements

- CLI commands for easy deployment and interaction
- Higher-level language support (Rust, AssemblyScript)
- Contract templates and scaffolding tools
- Interactive contract testing framework
- Gas optimization tools
- Contract verification and auditing tools
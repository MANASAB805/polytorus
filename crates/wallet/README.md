# PolyTorus Wallet

[![Crates.io](https://img.shields.io/crates/v/polytorus-wallet.svg)](https://crates.io/crates/polytorus-wallet)
[![Documentation](https://docs.rs/polytorus-wallet/badge.svg)](https://docs.rs/polytorus-wallet)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/PolyTorus/polytorus-wallet)

A comprehensive cryptocurrency wallet library for Rust, providing cryptographic key management, address generation, and signature operations. Designed for blockchain applications with support for multiple signature schemes and address formats.

## Features

- 🔐 **Multi-signature support**: Ed25519 and secp256k1 cryptographic schemes
- 🏠 **Address formats**: Hex, Base58, Base58Check, Bech32, and Blake3Hex encoding
- 🌱 **HD wallets**: BIP39 mnemonic phrases and hierarchical deterministic key derivation
- 🔒 **Security**: Memory zeroization and secure key handling
- 📦 **Serialization**: Optional serde support for wallet persistence
- 🚀 **Performance**: Optimized for blockchain applications
- 🛠 **Extensible**: Modular design for easy integration

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
polytorus-wallet = "0.1"
```

### Basic Usage

```rust
use polytorus_wallet::*;

// Create a new Ed25519 wallet
let mut wallet = Wallet::new_ed25519().unwrap();

// Get the default address
let address = wallet.default_address().unwrap();
println!("Address: {}", address);

// Sign a message
let message = b"Hello, PolyTorus!";
let signature = wallet.sign(message).unwrap();

// Verify the signature
let is_valid = wallet.verify(message, &signature).unwrap();
assert!(is_valid);
```

### HD Wallet with BIP39

```rust
use polytorus_wallet::*;

// Create HD wallet with mnemonic
let hd_wallet = HdWallet::new().unwrap();
println!("Mnemonic: {}", hd_wallet.mnemonic().phrase());

// Derive a wallet at specific path
let wallet = hd_wallet.derive_wallet("m/44'/0'/0'/0/0", KeyType::Ed25519).unwrap();

// Or use BIP44 standard paths
let receiving_wallet = hd_wallet.derive_receiving_wallet(
    coin_types::POLYTORUS, // coin type
    0,                     // account
    0,                     // address index
    KeyType::Ed25519
).unwrap();
```

### Address Generation

```rust
use polytorus_wallet::*;

let mut wallet = Wallet::new_secp256k1().unwrap();

// Generate addresses in different formats
let hex_addr = wallet.get_address(AddressFormat::Hex).unwrap();
let base58_addr = wallet.get_address(AddressFormat::Base58Check).unwrap();
let bech32_addr = wallet.get_address(AddressFormat::Bech32).unwrap();

println!("Hex:     {}", hex_addr);
println!("Base58:  {}", base58_addr);
println!("Bech32:  {}", bech32_addr);
```

### Wallet Manager

```rust
use polytorus_wallet::*;

let mut manager = WalletManager::new();

// Add multiple wallets
let wallet1 = Wallet::new_ed25519().unwrap().with_label("Main Wallet");
let wallet2 = Wallet::new_secp256k1().unwrap().with_label("Trading Wallet");

manager.add_wallet(wallet1);
manager.add_wallet(wallet2);

// Find wallet by label
if let Some((index, wallet)) = manager.find_by_label("Main Wallet") {
    println!("Found wallet at index {}", index);
}

// Use active wallet
if let Some(wallet) = manager.active_wallet() {
    println!("Active wallet: {:?}", wallet.label());
}
```

## Supported Cryptographic Schemes

### Ed25519
- **Use case**: Modern, fast, and secure signature scheme
- **Applications**: Cardano, Solana, modern blockchain systems
- **Advantages**: Small keys, fast verification, side-channel resistance

### secp256k1
- **Use case**: Bitcoin and Ethereum compatible signatures
- **Applications**: Bitcoin, Ethereum, most EVM chains
- **Advantages**: Wide ecosystem support, recovery capabilities

## Address Formats

| Format | Description | Example Use Case |
|--------|-------------|------------------|
| `Hex` | Simple hexadecimal encoding | Development, testing |
| `Base58` | Bitcoin-style encoding | Bitcoin-compatible systems |
| `Base58Check` | Base58 with checksum | Bitcoin addresses |
| `Bech32` | Modern encoding with error detection | Modern Bitcoin, Cardano |
| `Blake3Hex` | Blake3 hash with hex encoding | PolyTorus native |

## Security Features

- **Memory zeroization**: Private keys are automatically cleared from memory
- **Secure random generation**: Uses OS-level cryptographically secure random number generation
- **No private key serialization**: Private keys are never included in serialized data by default
- **Constant-time operations**: Signature verification uses constant-time algorithms

## Optional Features

Enable additional functionality with cargo features:

```toml
[dependencies]
polytorus-wallet = { version = "0.1", features = ["bip39", "serde_support"] }
```

- `bip39` (default): BIP39 mnemonic phrase and HD wallet support
- `serde_support`: Serialization support for wallet data structures

## Examples

See the `examples/` directory for more comprehensive examples:

- `basic_wallet.rs`: Basic wallet operations
- `hd_wallet.rs`: HD wallet and BIP39 usage
- `multi_signature.rs`: Working with different signature schemes
- `address_formats.rs`: Address generation and validation
- `wallet_manager.rs`: Managing multiple wallets

## Integration with PolyTorus

This wallet library is designed to integrate seamlessly with the PolyTorus blockchain ecosystem:

```rust
use polytorus_wallet::*;
use polytorus_execution::*; // PolyTorus execution layer

// Create validator wallet
let validator_wallet = Wallet::new_ed25519().unwrap()
    .with_label("Validator Node");

// Get validator address for settlement layer
let validator_address = validator_wallet.default_address().unwrap();

// Sign transactions
let transaction = /* ... create transaction ... */;
let signature = validator_wallet.sign(&transaction_bytes).unwrap();
```

## Development Status

This library is part of the PolyTorus blockchain project and is actively developed. While functional, it is intended for development and testing purposes. Production use should wait for security audits and stable releases.

## Roadmap

- [ ] Hardware wallet support (Ledger, Trezor)
- [ ] Multi-signature wallet schemes
- [ ] Additional signature algorithms (BLS, post-quantum)
- [ ] Key derivation standards beyond BIP32/BIP44
- [ ] Threshold signature schemes
- [ ] Zero-knowledge proof integration

## Contributing

We welcome contributions! Please see our [Contributing Guidelines](CONTRIBUTING.md) for details.

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Security

If you discover any security vulnerabilities, please report them responsibly by emailing security@polytorus.org rather than opening public issues.

## Links

- [PolyTorus Main Repository](https://github.com/PolyTorus/polytorus)
- [Documentation](https://docs.rs/polytorus-wallet)
- [Crates.io](https://crates.io/crates/polytorus-wallet)
- [PolyTorus Website](https://polytorus.org)

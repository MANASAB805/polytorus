// Wallet modules
pub mod address;
pub mod encoding;
pub mod error;
pub mod hd_wallet;
pub mod keypair;
pub mod signature;
pub mod wallet;


// Re-export all types
pub use address::{Address, AddressFormat};
pub use encoding::{encode_hex, decode_hex, encode_base58, decode_base58};
pub use error::WalletError;
pub use hd_wallet::{HdWallet, Mnemonic};
pub use keypair::{KeyPair, KeyType};
pub use signature::Signature;
pub use wallet::{Wallet, WalletManager};

// Common coin types for HD wallets
pub mod coin_types {
    pub const BITCOIN: u32 = 0;
    pub const ETHEREUM: u32 = 60;
    pub const CARDANO: u32 = 1815;
    pub const SOLANA: u32 = 501;
    pub const POLYTORUS: u32 = 9999;
    pub const CUSTOM: u32 = 9999;
}

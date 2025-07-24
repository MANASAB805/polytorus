//! Error types for the PolyTorus Wallet library

use thiserror::Error;

/// Result type alias for wallet operations
pub type Result<T> = std::result::Result<T, WalletError>;

/// Comprehensive error types for wallet operations
#[derive(Error, Debug, Clone, PartialEq)]
pub enum WalletError {
    /// Cryptographic operation failed
    #[error("Cryptographic error: {message}")]
    CryptographicError { message: String },

    /// Invalid key format or data
    #[error("Invalid key: {message}")]
    InvalidKey { message: String },

    /// Invalid signature format or verification failed
    #[error("Invalid signature: {message}")]
    InvalidSignature { message: String },

    /// Invalid address format
    #[error("Invalid address format: {format}")]
    InvalidAddressFormat { format: String },

    /// Address derivation failed
    #[error("Address derivation failed: {message}")]
    AddressDerivationError { message: String },

    /// Encoding/decoding error
    #[error("Encoding error: {message}")]
    EncodingError { message: String },

    /// Serialization error
    #[error("Serialization error: {message}")]
    SerializationError { message: String },

    /// BIP39 mnemonic related errors
    #[cfg(feature = "bip39")]
    #[error("Mnemonic error: {message}")]
    MnemonicError { message: String },

    /// HD wallet derivation errors
    #[cfg(feature = "bip39")]
    #[error("HD wallet error: {message}")]
    HdWalletError { message: String },

    /// Invalid input parameters
    #[error("Invalid input: {message}")]
    InvalidInput { message: String },

    /// Key storage/retrieval errors
    #[error("Key storage error: {message}")]
    KeyStorageError { message: String },

    /// Generic wallet operation error
    #[error("Wallet operation failed: {message}")]
    OperationError { message: String },
}

impl WalletError {
    /// Create a new cryptographic error
    pub fn cryptographic<S: Into<String>>(message: S) -> Self {
        Self::CryptographicError { message: message.into() }
    }

    /// Create a new invalid key error
    pub fn invalid_key<S: Into<String>>(message: S) -> Self {
        Self::InvalidKey { message: message.into() }
    }

    /// Create a new invalid signature error
    pub fn invalid_signature<S: Into<String>>(message: S) -> Self {
        Self::InvalidSignature { message: message.into() }
    }

    /// Create a new address derivation error
    pub fn address_derivation<S: Into<String>>(message: S) -> Self {
        Self::AddressDerivationError { message: message.into() }
    }

    /// Create a new encoding error
    pub fn encoding<S: Into<String>>(message: S) -> Self {
        Self::EncodingError { message: message.into() }
    }

    /// Create a new operation error
    pub fn operation<S: Into<String>>(message: S) -> Self {
        Self::OperationError { message: message.into() }
    }
}

// Conversion from external error types
impl From<hex::FromHexError> for WalletError {
    fn from(err: hex::FromHexError) -> Self {
        Self::EncodingError { message: format!("Hex decoding failed: {}", err) }
    }
}

impl From<base58::FromBase58Error> for WalletError {
    fn from(err: base58::FromBase58Error) -> Self {
        Self::EncodingError { message: format!("Base58 decoding failed: {:?}", err) }
    }
}

impl From<bech32::Error> for WalletError {
    fn from(err: bech32::Error) -> Self {
        Self::EncodingError { message: format!("Bech32 encoding/decoding failed: {}", err) }
    }
}

#[cfg(feature = "serde_support")]
impl From<serde_json::Error> for WalletError {
    fn from(err: serde_json::Error) -> Self {
        Self::SerializationError { message: format!("JSON serialization failed: {}", err) }
    }
}

impl From<ed25519_dalek::SignatureError> for WalletError {
    fn from(err: ed25519_dalek::SignatureError) -> Self {
        Self::CryptographicError { message: format!("Ed25519 signature error: {}", err) }
    }
}

impl From<secp256k1::Error> for WalletError {
    fn from(err: secp256k1::Error) -> Self {
        Self::CryptographicError { message: format!("secp256k1 error: {}", err) }
    }
}

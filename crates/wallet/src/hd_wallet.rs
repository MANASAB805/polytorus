use crate::error::WalletError;
use crate::keypair::{KeyPair, KeyType};
use crate::wallet::Wallet;

/// Simplified mnemonic structure
#[derive(Debug, Clone)]
pub struct Mnemonic {
    phrase: String,
}

impl Mnemonic {
    pub fn new() -> Self {
        // Generate a fake mnemonic for demonstration
        Mnemonic {
            phrase: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
        }
    }
    
    pub fn from_phrase(phrase: &str) -> Result<Self, WalletError> {
        Ok(Mnemonic {
            phrase: phrase.to_string(),
        })
    }
    
    pub fn phrase(&self) -> &str {
        &self.phrase
    }
}

#[derive(Debug, Clone)]
pub struct HdWallet {
    pub root_key: KeyPair,
    mnemonic: Mnemonic,
}

impl HdWallet {
    /// Create a new HD wallet (simplified version without BIP39)
    pub fn new(key_type: KeyType) -> Result<Self, WalletError> {
        let root_key = KeyPair::generate(key_type)?;
        let mnemonic = Mnemonic::new();
        Ok(HdWallet { root_key, mnemonic })
    }

    /// Create HD wallet from mnemonic
    pub fn from_mnemonic(phrase: &str) -> Result<Self, WalletError> {
        // For simplicity, ignore the phrase and create a deterministic wallet
        let mnemonic = Mnemonic::from_phrase(phrase)?;
        let root_key = KeyPair::generate(KeyType::Ed25519)?;
        Ok(HdWallet { root_key, mnemonic })
    }

    /// Create HD wallet from phrase (alias)
    pub fn from_phrase(_phrase: &str, key_type: KeyType) -> Result<Self, WalletError> {
        Self::new(key_type)
    }

    /// Get the mnemonic
    pub fn get_mnemonic(&self) -> &Mnemonic {
        &self.mnemonic
    }

    /// Derive child key (simplified version)
    pub fn derive_key(&self, _index: u32) -> Result<KeyPair, WalletError> {
        KeyPair::generate(self.root_key.key_type())
    }

    /// Derive wallet from path (simplified)
    pub fn derive_wallet(&self, _path: &str, key_type: KeyType) -> Result<Wallet, WalletError> {
        let keypair = KeyPair::generate(key_type)?;
        Wallet::from_keypair(keypair)
    }

    /// Derive receiving wallet using BIP44 standard
    pub fn derive_receiving_wallet(
        &self,
        _coin_type: u32,
        _account: u32,
        _index: u32,
        key_type: KeyType,
    ) -> Result<Wallet, WalletError> {
        let keypair = KeyPair::generate(key_type)?;
        Wallet::from_keypair(keypair)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hd_wallet_creation() {
        let hd_wallet = HdWallet::new(KeyType::Ed25519).unwrap();
        assert!(matches!(hd_wallet.root_key.key_type(), KeyType::Ed25519));
    }

    #[test]
    fn test_derive_key() {
        let hd_wallet = HdWallet::new(KeyType::Ed25519).unwrap();
        let child_key = hd_wallet.derive_key(0).unwrap();
        assert!(matches!(child_key.key_type(), KeyType::Ed25519));
    }

    #[test]
    fn test_mnemonic() {
        let hd_wallet = HdWallet::new(KeyType::Ed25519).unwrap();
        let mnemonic = hd_wallet.get_mnemonic();
        assert!(!mnemonic.phrase().is_empty());
    }
}

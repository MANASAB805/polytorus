//! Wallet implementation

use crate::error::{Result, WalletError};
use crate::keypair::{KeyPair, KeyType};
use crate::address::{Address, AddressFormat};
use crate::signature::{Signature, SignatureScheme};
use std::collections::HashMap;

/// Main wallet structure
#[derive(Clone)]
pub struct Wallet {
    keypair: KeyPair,
    label: Option<String>,
    default_format: AddressFormat,
    cached_addresses: HashMap<AddressFormat, Address>,
}

impl Wallet {
    /// Create new Ed25519 wallet
    pub fn new_ed25519() -> Result<Self> {
        let keypair = KeyPair::generate_ed25519()?;
        Ok(Self {
            keypair,
            label: None,
            default_format: AddressFormat::Hex,
            cached_addresses: HashMap::new(),
        })
    }

    /// Create new secp256k1 wallet
    pub fn new_secp256k1() -> Result<Self> {
        let keypair = KeyPair::generate_secp256k1()?;
        Ok(Self {
            keypair,
            label: None,
            default_format: AddressFormat::Hex,
            cached_addresses: HashMap::new(),
        })
    }

    /// Create wallet from existing keypair
    pub fn from_keypair(keypair: KeyPair) -> Result<Self> {
        Ok(Self {
            keypair,
            label: None,
            default_format: AddressFormat::Hex,
            cached_addresses: HashMap::new(),
        })
    }

    /// Set wallet label
    pub fn with_label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    /// Set wallet label
    pub fn set_label(&mut self, label: &str) {
        self.label = Some(label.to_string());
    }

    /// Get wallet label
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Get key type
    pub fn key_type(&self) -> KeyType {
        self.keypair.key_type()
    }

    /// Get address in specified format
    pub fn get_address(&mut self, format: AddressFormat) -> Result<Address> {
        if let Some(cached) = self.cached_addresses.get(&format) {
            return Ok(cached.clone());
        }

        let public_key = self.keypair.public_key_bytes()?;
        let address = Address::from_public_key(&public_key, format)?;
        self.cached_addresses.insert(format, address.clone());
        Ok(address)
    }

    /// Get default address
    pub fn default_address(&mut self) -> Result<Address> {
        self.get_address(self.default_format)
    }

    /// Sign message
    pub fn sign(&self, message: &[u8]) -> Result<Signature> {
        let signature_bytes = self.keypair.sign(message)?;
        let scheme = match self.keypair.key_type() {
            KeyType::Ed25519 => SignatureScheme::Ed25519,
            KeyType::Secp256k1 => SignatureScheme::Secp256k1,
        };
        Ok(Signature::new(signature_bytes, scheme))
    }

    /// Verify signature
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<bool> {
        // Check if signature scheme matches key type
        let expected_scheme = match self.keypair.key_type() {
            KeyType::Ed25519 => SignatureScheme::Ed25519,
            KeyType::Secp256k1 => SignatureScheme::Secp256k1,
        };

        if signature.scheme != expected_scheme {
            return Ok(false);
        }

        self.keypair.verify(message, &signature.bytes)
    }
}

/// Wallet manager for handling multiple wallets
pub struct WalletManager {
    wallets: Vec<Wallet>,
    active_index: Option<usize>,
}

impl WalletManager {
    /// Create new wallet manager
    pub fn new() -> Self {
        Self {
            wallets: Vec::new(),
            active_index: None,
        }
    }

    /// Add wallet to manager
    pub fn add_wallet(&mut self, wallet: Wallet) {
        self.wallets.push(wallet);
        if self.active_index.is_none() {
            self.active_index = Some(0);
        }
    }

    /// Get number of wallets
    pub fn wallet_count(&self) -> usize {
        self.wallets.len()
    }

    /// Get all wallets
    pub fn wallets(&self) -> &[Wallet] {
        &self.wallets
    }

    /// Get active wallet index
    pub fn active_wallet_index(&self) -> Option<usize> {
        self.active_index
    }

    /// Set active wallet
    pub fn set_active_wallet(&mut self, index: usize) -> Result<()> {
        if index >= self.wallets.len() {
            return Err(WalletError::operation("Wallet index out of bounds"));
        }
        self.active_index = Some(index);
        Ok(())
    }

    /// Get active wallet
    pub fn active_wallet(&self) -> Option<&Wallet> {
        self.active_index.and_then(|i| self.wallets.get(i))
    }

    /// Get active wallet (mutable)
    pub fn active_wallet_mut(&mut self) -> Option<&mut Wallet> {
        if let Some(index) = self.active_index {
            self.wallets.get_mut(index)
        } else {
            None
        }
    }

    /// Find wallet by label
    pub fn find_by_label(&self, label: &str) -> Option<(usize, &Wallet)> {
        for (index, wallet) in self.wallets.iter().enumerate() {
            if wallet.label() == Some(label) {
                return Some((index, wallet));
            }
        }
        None
    }

    /// Remove wallet
    pub fn remove_wallet(&mut self, index: usize) -> Result<Wallet> {
        if index >= self.wallets.len() {
            return Err(WalletError::operation("Wallet index out of bounds"));
        }

        let wallet = self.wallets.remove(index);
        
        // Update active index if necessary
        if let Some(active) = self.active_index {
            if active == index {
                self.active_index = if self.wallets.is_empty() {
                    None
                } else {
                    Some(0)
                };
            } else if active > index {
                self.active_index = Some(active - 1);
            }
        }

        Ok(wallet)
    }
}

impl Default for WalletManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_creation() {
        let wallet = Wallet::new_ed25519().unwrap();
        assert_eq!(wallet.key_type(), KeyType::Ed25519);
    }

    #[test]
    fn test_wallet_manager() {
        let mut manager = WalletManager::new();
        
        let wallet1 = Wallet::new_ed25519().unwrap().with_label("Test Wallet");
        manager.add_wallet(wallet1);
        
        assert_eq!(manager.wallet_count(), 1);
        assert!(manager.find_by_label("Test Wallet").is_some());
    }

    #[test]
    fn test_sign_verify() {
        let wallet = Wallet::new_ed25519().unwrap();
        let message = b"test message";
        
        let signature = wallet.sign(message).unwrap();
        let is_valid = wallet.verify(message, &signature).unwrap();
        assert!(is_valid);
    }
}

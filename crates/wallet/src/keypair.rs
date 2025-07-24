//! Key pair management for different cryptographic schemes

use crate::error::{Result, WalletError};
use ed25519_dalek::{SigningKey as Ed25519SigningKey, Signature as Ed25519Signature, Signer, Verifier};
use secp256k1::{Secp256k1, Keypair as Secp256k1Keypair, SecretKey as Secp256k1SecretKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::Digest;

/// Supported cryptographic key types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyType {
    /// Ed25519 signature scheme (used by Cardano, Solana, etc.)
    Ed25519,
    /// secp256k1 signature scheme (used by Bitcoin, Ethereum, etc.)
    Secp256k1,
}

impl Default for KeyType {
    fn default() -> Self {
        KeyType::Ed25519
    }
}

impl std::fmt::Display for KeyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyType::Ed25519 => write!(f, "Ed25519"),
            KeyType::Secp256k1 => write!(f, "secp256k1"),
        }
    }
}

/// A cryptographic key pair supporting multiple signature schemes
#[derive(Clone, Debug)]
pub struct KeyPair {
    key_type: KeyType,
    ed25519_signing_key: Option<Ed25519SigningKey>,
    secp256k1_keypair: Option<Secp256k1Keypair>,
}

impl KeyPair {
    /// Generate a new Ed25519 key pair
    pub fn generate_ed25519() -> Result<Self> {
        let mut csprng = OsRng;
        let signing_key = Ed25519SigningKey::generate(&mut csprng);
        
        Ok(Self {
            key_type: KeyType::Ed25519,
            ed25519_signing_key: Some(signing_key),
            secp256k1_keypair: None,
        })
    }

    /// Generate a new secp256k1 key pair
    pub fn generate_secp256k1() -> Result<Self> {
        let secp = Secp256k1::new();
        let mut csprng = OsRng;
        let keypair = Secp256k1Keypair::new(&secp, &mut csprng);
        
        Ok(Self {
            key_type: KeyType::Secp256k1,
            ed25519_signing_key: None,
            secp256k1_keypair: Some(keypair),
        })
    }

    /// Generate a new key pair of the specified type
    pub fn generate(key_type: KeyType) -> Result<Self> {
        match key_type {
            KeyType::Ed25519 => Self::generate_ed25519(),
            KeyType::Secp256k1 => Self::generate_secp256k1(),
        }
    }

    /// Create Ed25519 key pair from seed
    pub fn from_ed25519_seed(seed: &[u8; 32]) -> Result<Self> {
        let signing_key = Ed25519SigningKey::from_bytes(seed);
        
        Ok(Self {
            key_type: KeyType::Ed25519,
            ed25519_signing_key: Some(signing_key),
            secp256k1_keypair: None,
        })
    }

    /// Create secp256k1 key pair from seed
    pub fn from_secp256k1_seed(seed: &[u8; 32]) -> Result<Self> {
        let secp = Secp256k1::new();
        let secret_key = Secp256k1SecretKey::from_slice(seed)
            .map_err(|e| WalletError::invalid_key(format!("Invalid secp256k1 seed: {}", e)))?;
        let keypair = Secp256k1Keypair::from_secret_key(&secp, &secret_key);
        
        Ok(Self {
            key_type: KeyType::Secp256k1,
            ed25519_signing_key: None,
            secp256k1_keypair: Some(keypair),
        })
    }

    /// Get the key type
    pub fn key_type(&self) -> KeyType {
        self.key_type
    }

    /// Get public key bytes
    pub fn public_key_bytes(&self) -> Result<Vec<u8>> {
        match self.key_type {
            KeyType::Ed25519 => {
                let signing_key = self.ed25519_signing_key.as_ref()
                    .ok_or_else(|| WalletError::invalid_key("Ed25519 key not initialized"))?;
                let verifying_key = signing_key.verifying_key();
                Ok(verifying_key.to_bytes().to_vec())
            }
            KeyType::Secp256k1 => {
                let keypair = self.secp256k1_keypair.as_ref()
                    .ok_or_else(|| WalletError::invalid_key("secp256k1 key not initialized"))?;
                Ok(keypair.public_key().serialize().to_vec())
            }
        }
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>> {
        match self.key_type {
            KeyType::Ed25519 => {
                let signing_key = self.ed25519_signing_key.as_ref()
                    .ok_or_else(|| WalletError::invalid_key("Ed25519 key not initialized"))?;
                let signature = signing_key.sign(message);
                Ok(signature.to_bytes().to_vec())
            }
            KeyType::Secp256k1 => {
                let keypair = self.secp256k1_keypair.as_ref()
                    .ok_or_else(|| WalletError::invalid_key("secp256k1 key not initialized"))?;
                let secp = Secp256k1::new();
                let message_hash = secp256k1::Message::from_digest_slice(&sha2::Sha256::digest(message))
                    .map_err(|e| WalletError::cryptographic(format!("Message hashing failed: {}", e)))?;
                let signature = secp.sign_ecdsa(&message_hash, &keypair.secret_key());
                Ok(signature.serialize_compact().to_vec())
            }
        }
    }

    /// Verify a signature
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<bool> {
        match self.key_type {
            KeyType::Ed25519 => {
                let signing_key = self.ed25519_signing_key.as_ref()
                    .ok_or_else(|| WalletError::invalid_key("Ed25519 key not initialized"))?;
                let verifying_key = signing_key.verifying_key();
                
                if signature.len() != 64 {
                    return Ok(false);
                }
                
                let sig_array: [u8; 64] = signature.try_into()
                    .map_err(|_| WalletError::invalid_signature("Invalid Ed25519 signature length"))?;
                let ed25519_signature = Ed25519Signature::from_bytes(&sig_array);
                
                Ok(verifying_key.verify(message, &ed25519_signature).is_ok())
            }
            KeyType::Secp256k1 => {
                let keypair = self.secp256k1_keypair.as_ref()
                    .ok_or_else(|| WalletError::invalid_key("secp256k1 key not initialized"))?;
                let secp = Secp256k1::new();
                let message_hash = secp256k1::Message::from_digest_slice(&sha2::Sha256::digest(message))
                    .map_err(|e| WalletError::cryptographic(format!("Message hashing failed: {}", e)))?;
                
                if signature.len() != 64 {
                    return Ok(false);
                }
                
                let ecdsa_signature = secp256k1::ecdsa::Signature::from_compact(signature)
                    .map_err(|_| WalletError::invalid_signature("Invalid secp256k1 signature format"))?;
                
                Ok(secp.verify_ecdsa(&message_hash, &ecdsa_signature, &keypair.public_key()).is_ok())
            }
        }
    }

    /// Convert to hex string (public key)
    pub fn to_hex(&self) -> Result<String> {
        let public_key = self.public_key_bytes()?;
        Ok(hex::encode(public_key))
    }

    /// Get private key bytes (use with caution)
    pub fn private_key_bytes(&self) -> Result<Vec<u8>> {
        match self.key_type {
            KeyType::Ed25519 => {
                let signing_key = self.ed25519_signing_key.as_ref()
                    .ok_or_else(|| WalletError::invalid_key("Ed25519 key not initialized"))?;
                Ok(signing_key.to_bytes().to_vec())
            }
            KeyType::Secp256k1 => {
                let keypair = self.secp256k1_keypair.as_ref()
                    .ok_or_else(|| WalletError::invalid_key("secp256k1 key not initialized"))?;
                Ok(keypair.secret_key().secret_bytes().to_vec())
            }
        }
    }
}

impl Drop for KeyPair {
    fn drop(&mut self) {
        // Ed25519SigningKey and secp256k1::Keypair handle their own zeroization
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ed25519_keypair_generation() {
        let keypair = KeyPair::generate_ed25519().unwrap();
        assert_eq!(keypair.key_type(), KeyType::Ed25519);
        
        let public_key = keypair.public_key_bytes().unwrap();
        assert_eq!(public_key.len(), 32);
    }

    #[test]
    fn test_secp256k1_keypair_generation() {
        let keypair = KeyPair::generate_secp256k1().unwrap();
        assert_eq!(keypair.key_type(), KeyType::Secp256k1);
        
        let public_key = keypair.public_key_bytes().unwrap();
        assert_eq!(public_key.len(), 33); // Compressed public key
    }

    #[test]
    fn test_ed25519_sign_verify() {
        let keypair = KeyPair::generate_ed25519().unwrap();
        let message = b"Hello, PolyTorus!";
        
        let signature = keypair.sign(message).unwrap();
        assert_eq!(signature.len(), 64);
        
        let is_valid = keypair.verify(message, &signature).unwrap();
        assert!(is_valid);
        
        // Test with different message
        let is_invalid = keypair.verify(b"Different message", &signature).unwrap();
        assert!(!is_invalid);
    }

    #[test]
    fn test_secp256k1_sign_verify() {
        let keypair = KeyPair::generate_secp256k1().unwrap();
        let message = b"Hello, PolyTorus!";
        
        let signature = keypair.sign(message).unwrap();
        assert_eq!(signature.len(), 64);
        
        let is_valid = keypair.verify(message, &signature).unwrap();
        assert!(is_valid);
        
        // Test with different message
        let is_invalid = keypair.verify(b"Different message", &signature).unwrap();
        assert!(!is_invalid);
    }

    #[test]
    fn test_from_seed() {
        let seed = [42u8; 32];
        
        let keypair1 = KeyPair::from_ed25519_seed(&seed).unwrap();
        let keypair2 = KeyPair::from_ed25519_seed(&seed).unwrap();
        
        // Same seed should produce same public key
        assert_eq!(keypair1.public_key_bytes().unwrap(), keypair2.public_key_bytes().unwrap());
    }

    #[test]
    fn test_hex_conversion() {
        let keypair = KeyPair::generate_ed25519().unwrap();
        let hex = keypair.to_hex().unwrap();
        
        assert!(!hex.is_empty());
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

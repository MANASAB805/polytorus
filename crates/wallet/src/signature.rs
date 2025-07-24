//! Signature types and verification utilities

use crate::error::{Result, WalletError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ed25519_dalek::Verifier;

/// Supported signature schemes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignatureScheme {
    /// Ed25519 signature scheme
    Ed25519,
    /// secp256k1 ECDSA signature scheme
    Secp256k1,
}

impl std::fmt::Display for SignatureScheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignatureScheme::Ed25519 => write!(f, "Ed25519"),
            SignatureScheme::Secp256k1 => write!(f, "secp256k1"),
        }
    }
}

/// A cryptographic signature with metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    /// The signature bytes
    pub bytes: Vec<u8>,
    /// The signature scheme used
    pub scheme: SignatureScheme,
    /// Optional recovery ID for secp256k1
    pub recovery_id: Option<u8>,
    /// Hash of the signed message (for verification)
    pub message_hash: Option<Vec<u8>>,
}

impl Signature {
    /// Create a new signature
    pub fn new(bytes: Vec<u8>, scheme: SignatureScheme) -> Self {
        Self {
            bytes,
            scheme,
            recovery_id: None,
            message_hash: None,
        }
    }

    /// Create a signature with message hash
    pub fn with_message_hash(mut self, message_hash: Vec<u8>) -> Self {
        self.message_hash = Some(message_hash);
        self
    }

    /// Create a secp256k1 signature with recovery ID
    pub fn with_recovery_id(mut self, recovery_id: u8) -> Self {
        self.recovery_id = Some(recovery_id);
        self
    }

    /// Get the signature as hex string
    pub fn to_hex(&self) -> String {
        hex::encode(&self.bytes)
    }

    /// Get the signature bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Create signature from hex string
    pub fn from_hex(hex_str: &str, scheme: SignatureScheme) -> Result<Self> {
        let bytes = hex::decode(hex_str)?;
        Ok(Self::new(bytes, scheme))
    }

    /// Verify this signature against a message and public key
    pub fn verify(&self, message: &[u8], public_key: &[u8]) -> Result<bool> {
        match self.scheme {
            SignatureScheme::Ed25519 => {
                self.verify_ed25519(message, public_key)
            }
            SignatureScheme::Secp256k1 => {
                self.verify_secp256k1(message, public_key)
            }
        }
    }

    /// Verify Ed25519 signature
    fn verify_ed25519(&self, message: &[u8], public_key: &[u8]) -> Result<bool> {
        if self.bytes.len() != 64 {
            return Ok(false);
        }
        
        if public_key.len() != 32 {
            return Ok(false);
        }

        let public_key_array: [u8; 32] = public_key.try_into()
            .map_err(|_| WalletError::invalid_key("Invalid Ed25519 public key length"))?;
        let signature_array: [u8; 64] = self.bytes.as_slice().try_into()
            .map_err(|_| WalletError::invalid_signature("Invalid Ed25519 signature length"))?;

        let ed25519_public_key = ed25519_dalek::VerifyingKey::from_bytes(&public_key_array)
            .map_err(|e| WalletError::invalid_key(format!("Invalid Ed25519 public key: {}", e)))?;
        let ed25519_signature = ed25519_dalek::Signature::from_bytes(&signature_array);

        Ok(ed25519_public_key.verify(message, &ed25519_signature).is_ok())
    }

    /// Verify secp256k1 signature
    fn verify_secp256k1(&self, message: &[u8], public_key: &[u8]) -> Result<bool> {
        if self.bytes.len() != 64 {
            return Ok(false);
        }

        let secp = secp256k1::Secp256k1::new();
        
        let public_key = secp256k1::PublicKey::from_slice(public_key)
            .map_err(|e| WalletError::invalid_key(format!("Invalid secp256k1 public key: {}", e)))?;
        
        let signature = secp256k1::ecdsa::Signature::from_compact(&self.bytes)
            .map_err(|e| WalletError::invalid_signature(format!("Invalid secp256k1 signature: {}", e)))?;
        
        let message_hash = secp256k1::Message::from_digest_slice(&Sha256::digest(message))
            .map_err(|e| WalletError::cryptographic(format!("Message hashing failed: {}", e)))?;

        Ok(secp.verify_ecdsa(&message_hash, &signature, &public_key).is_ok())
    }

    /// Check if signature format is valid for the scheme
    pub fn is_valid_format(&self) -> bool {
        match self.scheme {
            SignatureScheme::Ed25519 => self.bytes.len() == 64,
            SignatureScheme::Secp256k1 => self.bytes.len() == 64,
        }
    }

    /// Get signature length for a scheme
    pub fn signature_length(scheme: SignatureScheme) -> usize {
        match scheme {
            SignatureScheme::Ed25519 => 64,
            SignatureScheme::Secp256k1 => 64,
        }
    }
}

/// Signature builder for creating signatures with custom parameters
pub struct SignatureBuilder {
    #[allow(dead_code)]
    scheme: SignatureScheme,
    recovery_id: Option<u8>,
    include_message_hash: bool,
}

impl SignatureBuilder {
    /// Create a new signature builder
    pub fn new(scheme: SignatureScheme) -> Self {
        Self {
            scheme,
            recovery_id: None,
            include_message_hash: false,
        }
    }

    /// Set recovery ID for secp256k1 signatures
    pub fn with_recovery_id(mut self, recovery_id: u8) -> Self {
        self.recovery_id = Some(recovery_id);
        self
    }

    /// Include message hash in the signature
    pub fn with_message_hash(mut self) -> Self {
        self.include_message_hash = true;
        self
    }
}

/// Batch signature verification for performance
pub struct BatchVerifier {
    signatures: Vec<(Signature, Vec<u8>, Vec<u8>)>, // (signature, message, public_key)
}

impl BatchVerifier {
    /// Create a new batch verifier
    pub fn new() -> Self {
        Self {
            signatures: Vec::new(),
        }
    }

    /// Add a signature to the batch
    pub fn add(&mut self, signature: Signature, message: Vec<u8>, public_key: Vec<u8>) {
        self.signatures.push((signature, message, public_key));
    }

    /// Verify all signatures in the batch
    pub fn verify_all(&self) -> Result<Vec<bool>> {
        let mut results = Vec::new();
        
        for (signature, message, public_key) in &self.signatures {
            let is_valid = signature.verify(message, public_key)?;
            results.push(is_valid);
        }
        
        Ok(results)
    }

    /// Verify all signatures and return only if all are valid
    pub fn verify_all_or_none(&self) -> Result<bool> {
        let results = self.verify_all()?;
        Ok(results.iter().all(|&valid| valid))
    }

    /// Get the number of signatures in the batch
    pub fn len(&self) -> usize {
        self.signatures.len()
    }

    /// Check if the batch is empty
    pub fn is_empty(&self) -> bool {
        self.signatures.is_empty()
    }
}

impl Default for BatchVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ed25519_signature() {
        // Direct test without keypair dependency
        let signature = Signature::new(vec![0u8; 64], SignatureScheme::Ed25519);
        assert_eq!(signature.scheme, SignatureScheme::Ed25519);
        assert!(signature.is_valid_format());
    }

    #[test]
    fn test_secp256k1_signature() {
        // Direct test without keypair dependency
        let signature = Signature::new(vec![0u8; 64], SignatureScheme::Secp256k1);
        assert_eq!(signature.scheme, SignatureScheme::Secp256k1);
        assert!(signature.is_valid_format());
    }

    #[test]
    fn test_signature_hex_conversion() {
        let signature = Signature::new(vec![0x12, 0x34, 0x56, 0x78], SignatureScheme::Ed25519);
        let hex = signature.to_hex();
        let recovered = Signature::from_hex(&hex, SignatureScheme::Ed25519).unwrap();
        
        assert_eq!(signature.bytes, recovered.bytes);
        assert_eq!(signature.scheme, recovered.scheme);
    }

    #[test]
    fn test_batch_verifier() {
        let mut verifier = BatchVerifier::new();
        
        // Add some dummy signatures for testing
        for i in 0..3 {
            let signature = Signature::new(vec![i as u8; 64], SignatureScheme::Ed25519);
            let message = format!("test message {}", i).into_bytes();
            let public_key = vec![i as u8; 32];
            verifier.add(signature, message, public_key);
        }
        
        assert_eq!(verifier.len(), 3);
        assert!(!verifier.is_empty());
    }
}

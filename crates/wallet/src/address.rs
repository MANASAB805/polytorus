use blake3::Hasher;
use crate::encoding::{encode_hex, encode_base58};
use crate::error::WalletError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum AddressFormat {
    Hex,
    Base58,
    Bech32,
    Blake3,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Address {
    pub format: AddressFormat,
    pub value: String,
}

impl Address {
    pub fn from_public_key(public_key: &[u8], format: AddressFormat) -> Result<Address, WalletError> {
        let value = match format {
            AddressFormat::Hex => encode_hex(public_key),
            AddressFormat::Base58 => encode_base58(public_key),
            AddressFormat::Blake3 => {
                let mut hasher = Hasher::new();
                hasher.update(public_key);
                let hash = hasher.finalize();
                encode_hex(hash.as_bytes())
            },
            AddressFormat::Bech32 => {
                // Simplified bech32 - just use hex with prefix
                format!("bc1{}", encode_hex(public_key))
            },
        };
        
        Ok(Address { format, value })
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_creation() {
        let public_key = &[1, 2, 3, 4, 5];
        
        let hex_addr = Address::from_public_key(public_key, AddressFormat::Hex).unwrap();
        assert_eq!(hex_addr.format, AddressFormat::Hex);
        assert_eq!(hex_addr.value, "0102030405");
        
        let base58_addr = Address::from_public_key(public_key, AddressFormat::Base58).unwrap();
        assert_eq!(base58_addr.format, AddressFormat::Base58);
    }
}

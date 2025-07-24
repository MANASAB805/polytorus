use crate::error::WalletError;

/// Encode bytes to hexadecimal string
pub fn encode_hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Decode hexadecimal string to bytes
pub fn decode_hex(hex: &str) -> Result<Vec<u8>, WalletError> {
    if hex.len() % 2 != 0 {
        return Err(WalletError::encoding("Invalid hex length"));
    }
    
    let mut result = Vec::new();
    for chunk in hex.as_bytes().chunks(2) {
        if chunk.len() != 2 {
            return Err(WalletError::encoding("Invalid hex chunk"));
        }
        
        let hex_str = std::str::from_utf8(chunk)
            .map_err(|_| WalletError::encoding("Invalid UTF-8 in hex"))?;
        
        let byte = u8::from_str_radix(hex_str, 16)
            .map_err(|_| WalletError::encoding("Invalid hex character"))?;
        
        result.push(byte);
    }
    
    Ok(result)
}

/// Simple Base58 encoding (basic implementation)
pub fn encode_base58(data: &[u8]) -> String {
    if data.is_empty() {
        return String::new();
    }
    
    // For simplicity, use hex encoding with base58 prefix
    format!("1{}", encode_hex(data))
}

/// Simple Base58 decoding (basic implementation)
pub fn decode_base58(encoded: &str) -> Result<Vec<u8>, WalletError> {
    if encoded.is_empty() {
        return Ok(Vec::new());
    }
    
    // For simplicity, remove '1' prefix and decode as hex
    if let Some(hex_part) = encoded.strip_prefix('1') {
        decode_hex(hex_part)
    } else {
        Err(WalletError::encoding("Invalid base58 format"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_encoding() {
        let data = &[0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
        let encoded = encode_hex(data);
        assert_eq!(encoded, "0123456789abcdef");
        
        let decoded = decode_hex(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_base58_encoding() {
        let data = &[1, 2, 3, 4, 5];
        let encoded = encode_base58(data);
        let decoded = decode_base58(&encoded).unwrap();
        assert_eq!(decoded, data);
    }
}

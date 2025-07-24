use polytorus_wallet::{Wallet, AddressFormat, WalletError};

fn main() -> Result<(), WalletError> {
    println!("=== Address Formats Example ===\n");

    // Test with Ed25519 wallet
    println!("Creating Ed25519 wallet...");
    let mut ed25519_wallet = Wallet::new_ed25519()?;
    
    println!("\n=== Ed25519 Address Formats ===");
    demonstrate_address_formats(&mut ed25519_wallet)?;
    
    println!("\n" + "=".repeat(50).as_str());
    
    // Test with secp256k1 wallet
    println!("\nCreating secp256k1 wallet...");
    let mut secp256k1_wallet = Wallet::new_secp256k1()?;
    
    println!("\n=== secp256k1 Address Formats ===");
    demonstrate_address_formats(&mut secp256k1_wallet)?;
    
    println!("\n" + "=".repeat(50).as_str());
    
    // Demonstrate address validation
    println!("\n=== Address Format Validation ===");
    
    let test_addresses = vec![
        ("", "Empty string"),
        ("1234567890abcdef", "Valid hex (short)"),
        ("1234567890abcdef1234567890abcdef12345678", "Valid hex (20 bytes)"),
        ("invalid_hex", "Invalid hex characters"),
        ("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa", "Bitcoin address format"),
        ("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4", "Bech32 format"),
    ];
    
    for (addr, description) in test_addresses {
        println!("\nTesting: {} ({})", addr, description);
        
        for format in [
            AddressFormat::Hex,
            AddressFormat::Base58,
            AddressFormat::Base58Check,
            AddressFormat::Bech32,
            AddressFormat::Blake3Hex,
        ] {
            match validate_address_format(addr, format) {
                Ok(valid) => println!("  {:?}: {}", format, if valid { "✅ Valid" } else { "❌ Invalid" }),
                Err(e) => println!("  {:?}: ❌ Error - {}", format, e),
            }
        }
    }
    
    println!("\n✅ Address format demonstration completed!");
    
    Ok(())
}

fn demonstrate_address_formats(wallet: &mut Wallet) -> Result<(), WalletError> {
    let formats = [
        (AddressFormat::Hex, "Hexadecimal"),
        (AddressFormat::Base58, "Base58"),
        (AddressFormat::Base58Check, "Base58Check"),
        (AddressFormat::Bech32, "Bech32"),
        (AddressFormat::Blake3Hex, "Blake3 + Hex"),
    ];
    
    println!("Key type: {:?}", wallet.key_type());
    
    for (format, name) in &formats {
        match wallet.get_address(*format) {
            Ok(address) => {
                println!("{:12}: {}", name, address);
                
                // Show address length and characteristics
                let char_count = address.len();
                let byte_estimate = match format {
                    AddressFormat::Hex | AddressFormat::Blake3Hex => char_count / 2,
                    AddressFormat::Base58 | AddressFormat::Base58Check => (char_count * 3) / 4, // Rough estimate
                    AddressFormat::Bech32 => char_count, // Variable, but typically longer
                };
                println!("             └─ {} chars, ~{} bytes", char_count, byte_estimate);
            }
            Err(e) => {
                println!("{:12}: ❌ Error - {}", name, e);
            }
        }
    }
    
    Ok(())
}

fn validate_address_format(address: &str, format: AddressFormat) -> Result<bool, WalletError> {
    use polytorus_wallet::encoding::*;
    
    if address.is_empty() {
        return Ok(false);
    }
    
    let is_valid = match format {
        AddressFormat::Hex => {
            // Check if it's valid hex and reasonable length
            if address.len() % 2 != 0 {
                false
            } else {
                hex::decode(address).is_ok() && address.len() >= 8 && address.len() <= 80
            }
        }
        AddressFormat::Base58 => {
            bs58::decode(address).into_vec().is_ok()
        }
        AddressFormat::Base58Check => {
            // For this example, we'll use a simple base58 check
            bs58::decode(address).with_check(None).into_vec().is_ok()
        }
        AddressFormat::Bech32 => {
            // Simple bech32 validation - check if it has the right structure
            address.contains('1') && address.len() > 8
        }
        AddressFormat::Blake3Hex => {
            // Blake3 produces 32-byte hashes, so hex should be 64 characters
            address.len() == 64 && hex::decode(address).is_ok()
        }
    };
    
    Ok(is_valid)
}

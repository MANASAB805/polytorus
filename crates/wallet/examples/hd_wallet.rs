use polytorus_wallet::{HdWallet, WalletError, KeyType, coin_types};

fn main() -> Result<(), WalletError> {
    println!("=== HD Wallet and BIP39 Example ===\n");

    // Create a new HD wallet with random mnemonic
    println!("Creating HD wallet with random mnemonic...");
    let hd_wallet = HdWallet::new(KeyType::Ed25519)?;
    
    println!("Mnemonic phrase:");
    println!("{}", hd_wallet.get_mnemonic().phrase());
    println!();
    
    // Derive wallets using custom paths
    println!("=== Custom Derivation Paths ===");
    
    let paths = [
        "m/44'/0'/0'/0/0",  // First receiving address
        "m/44'/0'/0'/0/1",  // Second receiving address
        "m/44'/0'/0'/1/0",  // First change address
        "m/44'/60'/0'/0/0", // Ethereum-style path
    ];
    
    for path in &paths {
        println!("\nDeriving wallet at path: {}", path);
        
        // Try Ed25519
        match hd_wallet.derive_wallet(path, KeyType::Ed25519) {
            Ok(mut wallet) => {
                let address = wallet.default_address()?;
                println!("  Ed25519 address: {}", address);
            }
            Err(e) => println!("  Ed25519 failed: {}", e),
        }
        
        // Try secp256k1
        match hd_wallet.derive_wallet(path, KeyType::Secp256k1) {
            Ok(mut wallet) => {
                let address = wallet.default_address()?;
                println!("  secp256k1 address: {}", address);
            }
            Err(e) => println!("  secp256k1 failed: {}", e),
        }
    }
    
    println!("\n{}", "=".repeat(50));
    
    // Use BIP44 standard derivation
    println!("\n=== BIP44 Standard Derivation ===");
    
    let coin_types_to_test = [
        (coin_types::BITCOIN, "Bitcoin"),
        (coin_types::ETHEREUM, "Ethereum"),
        (coin_types::CARDANO, "Cardano"),
        (coin_types::SOLANA, "Solana"),
        (coin_types::POLYTORUS, "PolyTorus"),
    ];
    
    for (coin_type, name) in &coin_types_to_test {
        println!("\n{} (coin type: {}):", name, coin_type);
        
        // Generate first few receiving addresses
        for i in 0..3 {
            match hd_wallet.derive_receiving_wallet(*coin_type, 0, i, KeyType::Ed25519) {
                Ok(mut wallet) => {
                    let address = wallet.default_address()?;
                    println!("  Address {}: {}", i, address);
                }
                Err(e) => println!("  Address {} failed: {}", i, e),
            }
        }
    }
    
    println!("\n{}", "=".repeat(50));
    
    // Demonstrate wallet recovery from mnemonic
    println!("\n=== Wallet Recovery ===");
    
    let original_mnemonic = hd_wallet.get_mnemonic().phrase();
    println!("Original mnemonic: {}", original_mnemonic);
    
    // Create new HD wallet from the same mnemonic
    let recovered_wallet = HdWallet::from_mnemonic(&original_mnemonic)?;
    println!("Recovered wallet from mnemonic");
    
    // Verify that derived addresses are the same
    let mut original_wallet = hd_wallet.derive_receiving_wallet(
        coin_types::POLYTORUS, 0, 0, KeyType::Ed25519
    )?;
    let mut recovered_derived = recovered_wallet.derive_receiving_wallet(
        coin_types::POLYTORUS, 0, 0, KeyType::Ed25519
    )?;
    
    let original_addr = original_wallet.default_address()?;
    let recovered_addr = recovered_derived.default_address()?;
    
    println!("Original address:  {}", original_addr);
    println!("Recovered address: {}", recovered_addr);
    println!("Addresses match: {}", original_addr == recovered_addr);
    
    // Test signing consistency
    let message = b"Test message for signature consistency";
    let original_sig = original_wallet.sign(message)?;
    let recovered_sig = recovered_derived.sign(message)?;
    
    // Both wallets should produce the same signature
    println!("Signatures match: {}", original_sig.as_bytes() == recovered_sig.as_bytes());
    
    // Both signatures should be valid
    let original_valid = original_wallet.verify(message, &original_sig)?;
    let recovered_valid = recovered_derived.verify(message, &recovered_sig)?;
    let cross_valid = original_wallet.verify(message, &recovered_sig)?;
    
    println!("Original signature valid: {}", original_valid);
    println!("Recovered signature valid: {}", recovered_valid);
    println!("Cross-verification valid: {}", cross_valid);
    
    println!("\n✅ HD wallet operations completed successfully!");
    
    Ok(())
}

use polytorus_wallet::{Wallet, WalletError};

fn main() -> Result<(), WalletError> {
    println!("=== Basic Wallet Example ===\n");

    // Create a new Ed25519 wallet
    println!("Creating Ed25519 wallet...");
    let mut ed25519_wallet = Wallet::new_ed25519()?;
    ed25519_wallet.set_label("My Ed25519 Wallet");
    
    println!("Ed25519 wallet created!");
    println!("Label: {:?}", ed25519_wallet.label());
    println!("Key type: {:?}", ed25519_wallet.key_type());
    
    // Get the default address
    let address = ed25519_wallet.default_address()?;
    println!("Default address: {}", address);
    
    // Sign a message
    let message = b"Hello, PolyTorus blockchain!";
    println!("\nSigning message: {:?}", std::str::from_utf8(message).unwrap());
    let signature = ed25519_wallet.sign(message)?;
    println!("Signature created (length: {} bytes)", signature.as_bytes().len());
    
    // Verify the signature
    let is_valid = ed25519_wallet.verify(message, &signature)?;
    println!("Signature valid: {}", is_valid);
    
    // Try to verify with wrong message
    let wrong_message = b"Wrong message";
    let is_invalid = ed25519_wallet.verify(wrong_message, &signature)?;
    println!("Signature valid for wrong message: {}", is_invalid);
    
    println!("\n" + "=".repeat(50).as_str());
    
    // Create a secp256k1 wallet
    println!("\nCreating secp256k1 wallet...");
    let mut secp256k1_wallet = Wallet::new_secp256k1()?;
    secp256k1_wallet.set_label("My Bitcoin-compatible Wallet");
    
    println!("secp256k1 wallet created!");
    println!("Label: {:?}", secp256k1_wallet.label());
    println!("Key type: {:?}", secp256k1_wallet.key_type());
    
    let secp_address = secp256k1_wallet.default_address()?;
    println!("Default address: {}", secp_address);
    
    // Sign the same message with secp256k1
    println!("\nSigning same message with secp256k1...");
    let secp_signature = secp256k1_wallet.sign(message)?;
    println!("Signature created (length: {} bytes)", secp_signature.as_bytes().len());
    
    let secp_valid = secp256k1_wallet.verify(message, &secp_signature)?;
    println!("Signature valid: {}", secp_valid);
    
    // Cross-verification should fail
    println!("\n=== Cross-verification Test ===");
    match ed25519_wallet.verify(message, &secp_signature) {
        Ok(valid) => println!("Cross-verification (should be false): {}", valid),
        Err(e) => println!("Cross-verification failed as expected: {}", e),
    }
    
    println!("\n✅ Basic wallet operations completed successfully!");
    
    Ok(())
}

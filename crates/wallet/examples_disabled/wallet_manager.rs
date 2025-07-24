use polytorus_wallet::{Wallet, WalletManager, KeyType, WalletError};

fn main() -> Result<(), WalletError> {
    println!("=== Wallet Manager Example ===\n");

    let mut manager = WalletManager::new();
    
    // Create several wallets
    println!("Creating multiple wallets...");
    
    let wallets_to_create = [
        ("Main Ed25519 Wallet", KeyType::Ed25519),
        ("Trading secp256k1 Wallet", KeyType::Secp256k1),
        ("Validator Ed25519 Wallet", KeyType::Ed25519),
        ("DeFi secp256k1 Wallet", KeyType::Secp256k1),
        ("Cold Storage Ed25519", KeyType::Ed25519),
    ];
    
    for (label, key_type) in &wallets_to_create {
        let wallet = match key_type {
            KeyType::Ed25519 => Wallet::new_ed25519()?,
            KeyType::Secp256k1 => Wallet::new_secp256k1()?,
        };
        
        let wallet_with_label = wallet.with_label(label);
        let address = wallet_with_label.default_address()?;
        
        println!("Created {}: {}", label, address);
        manager.add_wallet(wallet_with_label);
    }
    
    println!("\nTotal wallets in manager: {}", manager.wallet_count());
    
    println!("\n" + "=".repeat(50).as_str());
    
    // List all wallets
    println!("\n=== Wallet Listing ===");
    for (index, wallet) in manager.wallets().iter().enumerate() {
        let label = wallet.label().unwrap_or("Unnamed");
        let key_type = wallet.key_type();
        let address = wallet.default_address()?;
        let is_active = manager.active_wallet_index() == Some(index);
        
        println!("{}[{}] {} ({:?})", 
            if is_active { "🔹 " } else { "  " },
            index, 
            label, 
            key_type
        );
        println!("    Address: {}", address);
    }
    
    println!("\n" + "=".repeat(50).as_str());
    
    // Demonstrate wallet finding
    println!("\n=== Finding Wallets ===");
    
    let search_labels = ["Main Ed25519 Wallet", "NonExistent Wallet", "Trading secp256k1 Wallet"];
    
    for search_label in &search_labels {
        match manager.find_by_label(search_label) {
            Some((index, wallet)) => {
                println!("✅ Found '{}' at index {}", search_label, index);
                println!("   Key type: {:?}", wallet.key_type());
                println!("   Address: {}", wallet.default_address()?);
            }
            None => {
                println!("❌ Wallet '{}' not found", search_label);
            }
        }
    }
    
    println!("\n" + "=".repeat(50).as_str());
    
    // Demonstrate setting active wallet
    println!("\n=== Active Wallet Management ===");
    
    println!("Current active wallet: {:?}", 
        manager.active_wallet()
            .and_then(|w| w.label())
            .unwrap_or("None")
    );
    
    // Set different wallets as active
    for index in [2, 0, 4] {
        if manager.set_active_wallet(index).is_ok() {
            let wallet = manager.active_wallet().unwrap();
            let label = wallet.label().unwrap_or("Unnamed");
            println!("Set active wallet to index {}: {}", index, label);
        }
    }
    
    // Try to set invalid index
    match manager.set_active_wallet(999) {
        Ok(_) => println!("❌ Unexpected success setting invalid index"),
        Err(e) => println!("✅ Expected error for invalid index: {}", e),
    }
    
    println!("\n" + "=".repeat(50).as_str());
    
    // Demonstrate wallet operations through manager
    println!("\n=== Operations with Active Wallet ===");
    
    if let Some(active_wallet) = manager.active_wallet_mut() {
        let label = active_wallet.label().unwrap_or("Unnamed");
        println!("Using active wallet: {}", label);
        
        let message = b"Message signed by wallet manager";
        println!("Signing message: {:?}", std::str::from_utf8(message).unwrap());
        
        let signature = active_wallet.sign(message)?;
        println!("Signature created (length: {} bytes)", signature.as_bytes().len());
        
        let is_valid = active_wallet.verify(message, &signature)?;
        println!("Signature verification: {}", is_valid);
    }
    
    println!("\n" + "=".repeat(50).as_str());
    
    // Demonstrate wallet filtering
    println!("\n=== Wallet Filtering ===");
    
    println!("Ed25519 wallets:");
    for (index, wallet) in manager.wallets().iter().enumerate() {
        if wallet.key_type() == KeyType::Ed25519 {
            let label = wallet.label().unwrap_or("Unnamed");
            println!("  [{}] {}", index, label);
        }
    }
    
    println!("\nsecp256k1 wallets:");
    for (index, wallet) in manager.wallets().iter().enumerate() {
        if wallet.key_type() == KeyType::Secp256k1 {
            let label = wallet.label().unwrap_or("Unnamed");
            println!("  [{}] {}", index, label);
        }
    }
    
    println!("\n" + "=".repeat(50).as_str());
    
    // Demonstrate wallet removal
    println!("\n=== Wallet Removal ===");
    
    let original_count = manager.wallet_count();
    println!("Original wallet count: {}", original_count);
    
    // Remove wallet at index 1
    if let Ok(removed_wallet) = manager.remove_wallet(1) {
        let label = removed_wallet.label().unwrap_or("Unnamed");
        println!("Removed wallet: {}", label);
        println!("New wallet count: {}", manager.wallet_count());
    }
    
    // Try to remove invalid index
    match manager.remove_wallet(999) {
        Ok(_) => println!("❌ Unexpected success removing invalid index"),
        Err(e) => println!("✅ Expected error for invalid index: {}", e),
    }
    
    println!("\n✅ Wallet manager operations completed successfully!");
    
    Ok(())
}

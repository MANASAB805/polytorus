use polytorus_wallet::{Wallet, Signature, KeyType, WalletError};

fn main() -> Result<(), WalletError> {
    println!("=== Multi-Signature Schemes Example ===\n");

    // Create wallets with different signature schemes
    let mut ed25519_wallet = Wallet::new_ed25519()?.with_label("Ed25519 Wallet");
    let mut secp256k1_wallet = Wallet::new_secp256k1()?.with_label("secp256k1 Wallet");
    
    println!("Created wallets:");
    println!("  Ed25519: {}", ed25519_wallet.default_address()?);
    println!("  secp256k1: {}", secp256k1_wallet.default_address()?);
    
    println!("\n" + "=".repeat(50).as_str());
    
    // Test messages
    let messages = [
        b"Hello, PolyTorus!".as_slice(),
        b"Multi-signature test message".as_slice(),
        b"".as_slice(), // Empty message
        b"0123456789".repeat(100).join(b"").as_slice(), // Long message
    ];
    
    for (i, message) in messages.iter().enumerate() {
        println!("\n=== Test Message {} ===", i + 1);
        
        let msg_display = if message.is_empty() {
            "(empty)".to_string()
        } else if message.len() > 50 {
            format!("{}... ({} bytes)", 
                std::str::from_utf8(&message[..50]).unwrap_or("(binary)"),
                message.len()
            )
        } else {
            std::str::from_utf8(message).unwrap_or("(binary)").to_string()
        };
        
        println!("Message: {}", msg_display);
        
        // Sign with Ed25519
        println!("\n--- Ed25519 Signature ---");
        let ed25519_sig = ed25519_wallet.sign(message)?;
        println!("Signature length: {} bytes", ed25519_sig.as_bytes().len());
        
        let ed25519_valid = ed25519_wallet.verify(message, &ed25519_sig)?;
        println!("Self-verification: {}", ed25519_valid);
        
        // Sign with secp256k1
        println!("\n--- secp256k1 Signature ---");
        let secp256k1_sig = secp256k1_wallet.sign(message)?;
        println!("Signature length: {} bytes", secp256k1_sig.as_bytes().len());
        
        let secp256k1_valid = secp256k1_wallet.verify(message, &secp256k1_sig)?;
        println!("Self-verification: {}", secp256k1_valid);
        
        // Cross-verification tests (should fail)
        println!("\n--- Cross-Verification Tests ---");
        
        match ed25519_wallet.verify(message, &secp256k1_sig) {
            Ok(valid) => println!("Ed25519 verifying secp256k1 sig: {} (should be false)", valid),
            Err(e) => println!("Ed25519 verifying secp256k1 sig: Error (expected) - {}", e),
        }
        
        match secp256k1_wallet.verify(message, &ed25519_sig) {
            Ok(valid) => println!("secp256k1 verifying Ed25519 sig: {} (should be false)", valid),
            Err(e) => println!("secp256k1 verifying Ed25519 sig: Error (expected) - {}", e),
        }
    }
    
    println!("\n" + "=".repeat(50).as_str());
    
    // Demonstrate signature properties
    println!("\n=== Signature Properties ===");
    
    let test_message = b"Signature properties test";
    
    // Create multiple signatures from the same wallet
    println!("\nDeterministic signatures test:");
    let sig1 = ed25519_wallet.sign(test_message)?;
    let sig2 = ed25519_wallet.sign(test_message)?;
    
    println!("Ed25519 signature 1: {}", hex::encode(sig1.as_bytes()));
    println!("Ed25519 signature 2: {}", hex::encode(sig2.as_bytes()));
    println!("Ed25519 signatures identical: {}", sig1.as_bytes() == sig2.as_bytes());
    
    let sig3 = secp256k1_wallet.sign(test_message)?;
    let sig4 = secp256k1_wallet.sign(test_message)?;
    
    println!("secp256k1 signature 1: {}", hex::encode(sig3.as_bytes()));
    println!("secp256k1 signature 2: {}", hex::encode(sig4.as_bytes()));
    println!("secp256k1 signatures identical: {}", sig3.as_bytes() == sig4.as_bytes());
    
    println!("\n" + "=".repeat(50).as_str());
    
    // Batch verification demonstration
    println!("\n=== Batch Verification Simulation ===");
    
    // Create multiple message-signature pairs
    let batch_messages = [
        b"Transaction 1".as_slice(),
        b"Transaction 2".as_slice(),
        b"Transaction 3".as_slice(),
        b"Transaction 4".as_slice(),
        b"Transaction 5".as_slice(),
    ];
    
    println!("Creating batch of signatures...");
    
    let mut ed25519_pairs = Vec::new();
    let mut secp256k1_pairs = Vec::new();
    
    for (i, message) in batch_messages.iter().enumerate() {
        let ed25519_sig = ed25519_wallet.sign(message)?;
        let secp256k1_sig = secp256k1_wallet.sign(message)?;
        
        ed25519_pairs.push((*message, ed25519_sig));
        secp256k1_pairs.push((*message, secp256k1_sig));
        
        println!("  Created signatures for message {}", i + 1);
    }
    
    // Verify all signatures
    println!("\nVerifying Ed25519 batch:");
    let mut ed25519_all_valid = true;
    for (i, (message, signature)) in ed25519_pairs.iter().enumerate() {
        let valid = ed25519_wallet.verify(message, signature)?;
        println!("  Message {}: {}", i + 1, if valid { "✅" } else { "❌" });
        ed25519_all_valid &= valid;
    }
    println!("All Ed25519 signatures valid: {}", ed25519_all_valid);
    
    println!("\nVerifying secp256k1 batch:");
    let mut secp256k1_all_valid = true;
    for (i, (message, signature)) in secp256k1_pairs.iter().enumerate() {
        let valid = secp256k1_wallet.verify(message, signature)?;
        println!("  Message {}: {}", i + 1, if valid { "✅" } else { "❌" });
        secp256k1_all_valid &= valid;
    }
    println!("All secp256k1 signatures valid: {}", secp256k1_all_valid);
    
    println!("\n" + "=".repeat(50).as_str());
    
    // Performance comparison (simple timing)
    println!("\n=== Performance Comparison ===");
    
    let perf_message = b"Performance test message";
    let iterations = 100;
    
    // Ed25519 performance
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let sig = ed25519_wallet.sign(perf_message)?;
        ed25519_wallet.verify(perf_message, &sig)?;
    }
    let ed25519_duration = start.elapsed();
    
    // secp256k1 performance
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let sig = secp256k1_wallet.sign(perf_message)?;
        secp256k1_wallet.verify(perf_message, &sig)?;
    }
    let secp256k1_duration = start.elapsed();
    
    println!("Performance test ({} iterations):", iterations);
    println!("  Ed25519:   {:?} ({:.2}μs per operation)", 
        ed25519_duration, 
        ed25519_duration.as_micros() as f64 / iterations as f64
    );
    println!("  secp256k1: {:?} ({:.2}μs per operation)", 
        secp256k1_duration,
        secp256k1_duration.as_micros() as f64 / iterations as f64
    );
    
    let faster = if ed25519_duration < secp256k1_duration {
        "Ed25519"
    } else {
        "secp256k1"
    };
    println!("  {} is faster for this test", faster);
    
    println!("\n✅ Multi-signature scheme testing completed!");
    
    Ok(())
}

//! Simple Database Connection Test
//!
//! This example tests the basic database connectivity and operations.

use anyhow::Result;
use polytorus::smart_contract::{
    database_storage::{
        DatabaseContractStorage, DatabaseStorageConfig, PostgresConfig, RedisConfig,
    },
    unified_engine::{
        ContractExecutionRecord, ContractStateStorage, ContractType, UnifiedContractMetadata,
    },
};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("🔍 Testing Database Connectivity");
    println!("================================");

    // Create test configuration
    let config = DatabaseStorageConfig {
        postgres: Some(PostgresConfig {
            host: "localhost".to_string(),
            port: 5433, // Docker mapped port
            database: "polytorus_test".to_string(),
            username: "polytorus_test".to_string(),
            password: "test_password_123".to_string(),
            schema: "smart_contracts".to_string(),
            max_connections: 10,
        }),
        redis: Some(RedisConfig {
            url: "redis://localhost:6380".to_string(), // Docker mapped port
            password: Some("test_redis_password_123".to_string()),
            database: 0,
            max_connections: 10,
            key_prefix: "polytorus:test:contracts:".to_string(),
            ttl_seconds: Some(300), // 5 minutes for testing
        }),
        fallback_to_memory: true, // Allow fallback during testing
        connection_timeout_secs: 10,
        max_connections: 20,
        use_ssl: false,
    };

    println!("📡 Attempting to connect to databases...");

    // Initialize storage
    let storage = match DatabaseContractStorage::new(config).await {
        Ok(storage) => {
            println!("✅ Database storage initialized successfully");
            storage
        }
        Err(e) => {
            println!("❌ Failed to initialize database storage: {e}");
            return Err(e);
        }
    };

    // Check connectivity status
    println!("\n🔍 Checking database connectivity...");
    let status = storage.check_connectivity().await?;
    println!(
        "PostgreSQL connected: {}",
        if status.postgres_connected {
            "✅ Yes"
        } else {
            "❌ No"
        }
    );
    println!(
        "Redis connected: {}",
        if status.redis_connected {
            "✅ Yes"
        } else {
            "❌ No"
        }
    );
    println!(
        "Fallback available: {}",
        if status.fallback_available {
            "✅ Yes"
        } else {
            "❌ No"
        }
    );

    if !status.postgres_connected && !status.redis_connected && !status.fallback_available {
        println!("❌ No storage backend available!");
        return Err(anyhow::anyhow!("No storage backend available"));
    }

    // Test basic operations
    println!("\n📝 Testing basic contract operations...");

    // Create test metadata
    let metadata = UnifiedContractMetadata {
        address: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
        name: "TestContract".to_string(),
        description: "A test contract for database connectivity".to_string(),
        contract_type: ContractType::Wasm {
            bytecode: vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00], // WASM header
            abi: Some(r#"{"functions": [{"name": "test", "inputs": []}]}"#.to_string()),
        },
        deployment_tx: "0xtest_deployment_hash".to_string(),
        deployment_time: 1640995200,
        owner: "0xtest_owner".to_string(),
        is_active: true,
    };

    // Store metadata
    println!("  📄 Storing contract metadata...");
    storage.store_contract_metadata(&metadata)?;
    println!("  ✅ Contract metadata stored");

    // Retrieve metadata
    println!("  📄 Retrieving contract metadata...");
    let retrieved = storage.get_contract_metadata(&metadata.address)?;
    match retrieved {
        Some(meta) => {
            println!("  ✅ Retrieved contract: {}", meta.name);
            println!("     Address: {}", meta.address);
            println!("     Owner: {}", meta.owner);
        }
        None => {
            println!("  ❌ Failed to retrieve contract metadata");
            return Err(anyhow::anyhow!("Failed to retrieve contract metadata"));
        }
    }

    // Test state operations
    println!("  💾 Testing contract state operations...");
    storage.set_contract_state(&metadata.address, "balance", &1000u64.to_le_bytes())?;
    storage.set_contract_state(&metadata.address, "name", b"TestToken")?;
    println!("  ✅ Contract state stored");

    // Retrieve state
    if let Some(balance_bytes) = storage.get_contract_state(&metadata.address, "balance")? {
        let balance = u64::from_le_bytes(balance_bytes.try_into().unwrap());
        println!("  ✅ Retrieved balance: {balance}");
    } else {
        println!("  ❌ Failed to retrieve balance");
    }

    if let Some(name_bytes) = storage.get_contract_state(&metadata.address, "name")? {
        let name = String::from_utf8(name_bytes).unwrap();
        println!("  ✅ Retrieved name: {name}");
    } else {
        println!("  ❌ Failed to retrieve name");
    }

    // Test execution history
    println!("  📝 Testing execution history...");
    let execution = ContractExecutionRecord {
        execution_id: "test_exec_001".to_string(),
        contract_address: metadata.address.clone(),
        function_name: "test_function".to_string(),
        caller: "0xtest_caller".to_string(),
        timestamp: 1640995200,
        gas_used: 21000,
        success: true,
        error_message: None,
    };

    storage.store_execution(&execution)?;
    println!("  ✅ Execution record stored");

    let history = storage.get_execution_history(&metadata.address)?;
    println!(
        "  ✅ Retrieved execution history: {} entries",
        history.len()
    );

    // Get performance statistics
    println!("\n📊 Performance Statistics:");
    let stats = storage.get_stats().await;
    println!("  PostgreSQL connections: {}", stats.postgres_connections);
    println!("  Redis connections: {}", stats.redis_connections);
    println!("  Total queries: {}", stats.total_queries);
    println!("  Failed queries: {}", stats.failed_queries);
    println!("  Cache hits: {}", stats.cache_hits);
    println!("  Cache misses: {}", stats.cache_misses);

    // Calculate cache hit ratio
    let total_cache_requests = stats.cache_hits + stats.cache_misses;
    if total_cache_requests > 0 {
        let hit_ratio = (stats.cache_hits as f64 / total_cache_requests as f64) * 100.0;
        println!("  Cache hit ratio: {hit_ratio:.1}%");
    }

    // Get database information
    println!("\n💾 Database Information:");
    let info = storage.get_database_info().await?;
    println!("  PostgreSQL size: {} bytes", info.postgres_size_bytes);
    println!(
        "  Redis memory usage: {} bytes",
        info.redis_memory_usage_bytes
    );
    println!(
        "  Memory fallback entries: {}",
        info.memory_fallback_entries
    );
    println!("  Total contracts: {}", info.total_contracts);
    println!("  Total state entries: {}", info.total_state_entries);
    println!("  Total executions: {}", info.total_executions);

    // List all contracts
    let contracts = storage.list_contracts()?;
    println!("  Total contracts in storage: {}", contracts.len());

    println!("\n🎉 Database connectivity test completed successfully!");
    println!("✅ All basic operations are working correctly");

    Ok(())
}

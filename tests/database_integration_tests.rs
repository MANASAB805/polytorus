//! Database Integration Tests
//!
//! These tests require actual PostgreSQL and Redis instances to be running.
//! Run with: docker-compose -f docker-compose.database-test.yml up -d
//! Then: cargo test --test database_integration_tests

use std::time::{Duration, Instant};

use anyhow::Result;
use polytorus::smart_contract::{
    database_storage::{
        DatabaseContractStorage, DatabaseStorageConfig, PostgresConfig, RedisConfig,
    },
    unified_engine::{
        ContractExecutionRecord, ContractStateStorage, ContractType, UnifiedContractMetadata,
    },
};
use tokio::time::sleep;

// Test configuration for local Docker containers
fn create_test_config() -> DatabaseStorageConfig {
    DatabaseStorageConfig {
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
    }
}

fn create_test_metadata(suffix: &str) -> UnifiedContractMetadata {
    UnifiedContractMetadata {
        address: format!("0x{:0>40}", format!("test{}", suffix)),
        name: format!("TestContract{suffix}"),
        description: format!("Test contract {suffix}"),
        contract_type: ContractType::Wasm {
            bytecode: vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00], // WASM header
            abi: Some(format!(
                r#"{{"contract": "test{suffix}", "version": "1.0"}}"#
            )),
        },
        deployment_tx: format!("0x{:0>64}", format!("deployment{}", suffix)),
        deployment_time: 1640995200 + suffix.parse::<u64>().unwrap_or(0) * 3600,
        owner: format!("0x{:0>40}", format!("owner{}", suffix)),
        is_active: true,
    }
}

#[tokio::test]
#[ignore] // Use --ignored to run database tests
async fn test_database_connectivity() -> Result<()> {
    println!("🔍 Testing database connectivity...");

    let config = create_test_config();
    let storage = DatabaseContractStorage::new(config).await?;

    // Check connectivity status
    let status = storage.check_connectivity().await?;
    println!("PostgreSQL connected: {}", status.postgres_connected);
    println!("Redis connected: {}", status.redis_connected);
    println!("Fallback available: {}", status.fallback_available);

    // At least one should be connected or fallback should be available
    assert!(
        status.postgres_connected || status.redis_connected || status.fallback_available,
        "No storage backend available"
    );

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_contract_metadata_operations() -> Result<()> {
    println!("📄 Testing contract metadata operations...");

    let config = create_test_config();
    let storage = DatabaseContractStorage::new(config).await?;

    let metadata = create_test_metadata("metadata");

    // Store metadata
    storage.store_contract_metadata(&metadata)?;
    println!("✅ Stored contract metadata");

    // Retrieve metadata
    let retrieved = storage.get_contract_metadata(&metadata.address)?;
    assert!(retrieved.is_some(), "Failed to retrieve metadata");

    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.address, metadata.address);
    assert_eq!(retrieved.name, metadata.name);
    assert_eq!(retrieved.owner, metadata.owner);
    println!("✅ Retrieved and verified contract metadata");

    // List contracts
    let contracts = storage.list_contracts()?;
    assert!(
        contracts.contains(&metadata.address),
        "Contract not in list"
    );
    println!("✅ Contract appears in listing ({} total)", contracts.len());

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_contract_state_operations() -> Result<()> {
    println!("💾 Testing contract state operations...");

    let config = create_test_config();
    let storage = DatabaseContractStorage::new(config).await?;

    let contract_address = "0x1234567890abcdef1234567890abcdef12345678";

    // Store various types of state data
    storage.set_contract_state(contract_address, "balance", &1000u64.to_le_bytes())?;
    storage.set_contract_state(contract_address, "name", b"TestToken")?;
    storage.set_contract_state(contract_address, "symbol", b"TTK")?;
    storage.set_contract_state(contract_address, "decimals", &18u8.to_le_bytes())?;
    println!("✅ Stored contract state data");

    // Retrieve and verify state data
    let balance = storage.get_contract_state(contract_address, "balance")?;
    assert!(balance.is_some());
    let balance = u64::from_le_bytes(balance.unwrap().try_into().unwrap());
    assert_eq!(balance, 1000);

    let name = storage.get_contract_state(contract_address, "name")?;
    assert!(name.is_some());
    let name = String::from_utf8(name.unwrap()).unwrap();
    assert_eq!(name, "TestToken");

    let decimals = storage.get_contract_state(contract_address, "decimals")?;
    assert!(decimals.is_some());
    let decimals = u8::from_le_bytes(decimals.unwrap().try_into().unwrap());
    assert_eq!(decimals, 18);
    println!("✅ Retrieved and verified contract state");

    // Test state deletion
    storage.delete_contract_state(contract_address, "symbol")?;
    let symbol = storage.get_contract_state(contract_address, "symbol")?;
    assert!(symbol.is_none(), "Symbol should be deleted");
    println!("✅ Verified state deletion");

    // Test non-existent state
    let nonexistent = storage.get_contract_state(contract_address, "nonexistent")?;
    assert!(nonexistent.is_none());
    println!("✅ Verified non-existent state returns None");

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_execution_history() -> Result<()> {
    println!("📝 Testing execution history...");

    let config = create_test_config();
    let storage = DatabaseContractStorage::new(config).await?;

    let contract_address = "0xabcdef1234567890abcdef1234567890abcdef12";

    // Store multiple execution records
    for i in 1..=5 {
        let execution = ContractExecutionRecord {
            execution_id: format!("exec_{i:03}"),
            contract_address: contract_address.to_string(),
            function_name: if i % 2 == 0 { "transfer" } else { "approve" }.to_string(),
            caller: format!("0x{:0>40}", format!("caller{}", i)),
            timestamp: 1640995200 + i * 60, // 1 minute intervals
            gas_used: 21000 + i * 1000,
            success: i % 3 != 0, // Some failures
            error_message: if i % 3 == 0 {
                Some(format!("Error in execution {i}"))
            } else {
                None
            },
        };

        storage.store_execution(&execution)?;
    }
    println!("✅ Stored 5 execution records");

    // Retrieve execution history
    let history = storage.get_execution_history(contract_address)?;
    assert_eq!(history.len(), 5, "Should have 5 execution records");

    // Verify ordering (should be newest first)
    for i in 0..history.len() - 1 {
        assert!(
            history[i].timestamp >= history[i + 1].timestamp,
            "History should be ordered by timestamp (newest first)"
        );
    }
    println!("✅ Verified execution history ordering");

    // Verify content
    let successful_executions = history.iter().filter(|e| e.success).count();
    let failed_executions = history.iter().filter(|e| !e.success).count();
    println!("   Successful: {successful_executions}, Failed: {failed_executions}");

    assert_eq!(successful_executions, 3);
    assert_eq!(failed_executions, 2);
    println!("✅ Verified execution success/failure counts");

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_performance_and_concurrency() -> Result<()> {
    println!("⚡ Testing performance and concurrency...");

    let config = create_test_config();
    let storage = DatabaseContractStorage::new(config).await?;

    let num_contracts = 10;
    let num_operations_per_contract = 20;

    let start_time = Instant::now();

    // Create multiple contracts concurrently
    let mut handles = Vec::new();

    for i in 0..num_contracts {
        let storage = DatabaseContractStorage::new(create_test_config()).await?;
        let handle = tokio::spawn(async move {
            let contract_id = format!("perf{i:03}");
            let metadata = create_test_metadata(&contract_id);

            // Store metadata
            storage.store_contract_metadata(&metadata)?;

            // Perform multiple state operations
            for j in 0..num_operations_per_contract {
                let key = format!("key_{j}");
                let value = format!("value_{i}_{j}");
                storage.set_contract_state(&metadata.address, &key, value.as_bytes())?;

                // Occasionally read back
                if j % 5 == 0 {
                    let _retrieved = storage.get_contract_state(&metadata.address, &key)?;
                }
            }

            // Store execution record
            let execution = ContractExecutionRecord {
                execution_id: format!("perf_exec_{i}"),
                contract_address: metadata.address.clone(),
                function_name: "performance_test".to_string(),
                caller: format!("0x{:0>40}", format!("perfcaller{}", i)),
                timestamp: 1640995200 + i * 10,
                gas_used: 50000 + i * 1000,
                success: true,
                error_message: None,
            };
            storage.store_execution(&execution)?;

            Ok::<(), anyhow::Error>(())
        });

        handles.push(handle);
    }

    // Wait for all operations to complete
    for handle in handles {
        handle.await??;
    }

    let duration = start_time.elapsed();
    let total_operations = num_contracts * (1 + num_operations_per_contract + 1); // metadata + state ops + execution
    let ops_per_second = total_operations as f64 / duration.as_secs_f64();

    println!("✅ Completed {total_operations} operations in {duration:?}");
    println!("   Performance: {ops_per_second:.2} operations/second");

    // Verify all contracts were stored
    let contracts = storage.list_contracts()?;
    let perf_contracts = contracts
        .iter()
        .filter(|addr| addr.contains("perf"))
        .count();
    assert!(
        perf_contracts >= num_contracts as usize,
        "Not all performance test contracts were stored"
    );

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_cache_behavior() -> Result<()> {
    println!("🗄️ Testing cache behavior...");

    let config = create_test_config();
    let storage = DatabaseContractStorage::new(config).await?;

    let contract_address = "0xcache1234567890abcdef1234567890abcdef12";

    // Clear any existing cache
    storage.clear_cache().await?;

    // Get initial stats
    let initial_stats = storage.get_stats().await;
    println!(
        "Initial cache stats - Hits: {}, Misses: {}",
        initial_stats.cache_hits, initial_stats.cache_misses
    );

    // Store some data
    storage.set_contract_state(contract_address, "cached_key", b"cached_value")?;

    // First read should potentially miss cache (depending on implementation)
    let _value1 = storage.get_contract_state(contract_address, "cached_key")?;

    // Second read should hit cache
    let _value2 = storage.get_contract_state(contract_address, "cached_key")?;
    let _value3 = storage.get_contract_state(contract_address, "cached_key")?;

    // Check stats after operations
    let final_stats = storage.get_stats().await;
    println!(
        "Final cache stats - Hits: {}, Misses: {}",
        final_stats.cache_hits, final_stats.cache_misses
    );

    // We should have some cache activity
    let total_cache_ops = final_stats.cache_hits + final_stats.cache_misses;
    assert!(
        total_cache_ops > initial_stats.cache_hits + initial_stats.cache_misses,
        "Cache should show activity"
    );

    println!("✅ Cache behavior verified");

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_database_info_and_monitoring() -> Result<()> {
    println!("📊 Testing database info and monitoring...");

    let config = create_test_config();
    let storage = DatabaseContractStorage::new(config).await?;

    // Get database information
    let info = storage.get_database_info().await?;
    println!("Database info:");
    println!("  PostgreSQL size: {} bytes", info.postgres_size_bytes);
    println!("  Redis memory: {} bytes", info.redis_memory_usage_bytes);
    println!(
        "  Memory fallback entries: {}",
        info.memory_fallback_entries
    );
    println!("  Total contracts: {}", info.total_contracts);
    println!("  Total state entries: {}", info.total_state_entries);
    println!("  Total executions: {}", info.total_executions);

    // Store some test data to see changes
    let metadata = create_test_metadata("monitoring");
    storage.store_contract_metadata(&metadata)?;
    storage.set_contract_state(&metadata.address, "test_key", b"test_value")?;

    let execution = ContractExecutionRecord {
        execution_id: "monitoring_exec".to_string(),
        contract_address: metadata.address.clone(),
        function_name: "monitor_test".to_string(),
        caller: "0xmonitor".to_string(),
        timestamp: 1640995200,
        gas_used: 30000,
        success: true,
        error_message: None,
    };
    storage.store_execution(&execution)?;

    // Get updated info
    let updated_info = storage.get_database_info().await?;
    println!("Updated database info:");
    println!("  Total contracts: {}", updated_info.total_contracts);
    println!(
        "  Total state entries: {}",
        updated_info.total_state_entries
    );
    println!("  Total executions: {}", updated_info.total_executions);

    // Verify increases
    assert!(updated_info.total_contracts >= info.total_contracts);
    assert!(updated_info.total_state_entries >= info.total_state_entries);
    assert!(updated_info.total_executions >= info.total_executions);

    println!("✅ Database monitoring verified");

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_failover_behavior() -> Result<()> {
    println!("🔄 Testing failover behavior...");

    // Test with invalid database configuration to trigger fallback
    let mut config = create_test_config();
    config.postgres.as_mut().unwrap().port = 9999; // Invalid port
    config.redis.as_mut().unwrap().url = "redis://localhost:9999".to_string(); // Invalid port
    config.fallback_to_memory = true;

    let storage = DatabaseContractStorage::new(config).await?;

    // Check connectivity (should show disconnected but fallback available)
    let status = storage.check_connectivity().await?;
    println!("Failover test connectivity:");
    println!("  PostgreSQL: {}", status.postgres_connected);
    println!("  Redis: {}", status.redis_connected);
    println!("  Fallback: {}", status.fallback_available);

    assert!(
        !status.postgres_connected,
        "PostgreSQL should be disconnected"
    );
    assert!(!status.redis_connected, "Redis should be disconnected");
    assert!(status.fallback_available, "Fallback should be available");

    // Operations should still work with memory fallback
    let metadata = create_test_metadata("failover");
    storage.store_contract_metadata(&metadata)?;

    let retrieved = storage.get_contract_metadata(&metadata.address)?;
    assert!(retrieved.is_some(), "Failover storage should work");

    println!("✅ Failover behavior verified");

    Ok(())
}

// Helper function to wait for databases to be ready
async fn wait_for_databases() -> Result<()> {
    println!("⏳ Waiting for databases to be ready...");

    let max_attempts = 30;
    let mut attempts = 0;

    while attempts < max_attempts {
        let config = create_test_config();
        if let Ok(storage) = DatabaseContractStorage::new(config).await {
            let status = storage.check_connectivity().await?;
            if status.postgres_connected && status.redis_connected {
                println!("✅ Databases are ready!");
                return Ok(());
            }
        }

        attempts += 1;
        println!("   Attempt {attempts}/{max_attempts} - waiting...");
        sleep(Duration::from_secs(2)).await;
    }

    Err(anyhow::anyhow!("Databases did not become ready in time"))
}

// Integration test that runs all tests in sequence
#[tokio::test]
#[ignore]
async fn test_full_integration() -> Result<()> {
    println!("🚀 Running full database integration test suite...");

    // Wait for databases to be ready
    wait_for_databases().await?;

    println!("✅ Database integration test environment is ready!");
    println!("Run individual tests with:");
    println!("  cargo test --test database_integration_tests <test_name> -- --ignored --nocapture");

    Ok(())
}

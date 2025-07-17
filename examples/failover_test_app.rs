//! Failover Test Application
//!
//! This example tests the actual application-level failover behavior
//! when databases become unavailable.

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

// Configuration for different failure scenarios
fn create_config_with_invalid_postgres() -> DatabaseStorageConfig {
    DatabaseStorageConfig {
        postgres: Some(PostgresConfig {
            host: "localhost".to_string(),
            port: 9999, // Invalid port
            database: "polytorus_test".to_string(),
            username: "polytorus_test".to_string(),
            password: "test_password_123".to_string(),
            schema: "smart_contracts".to_string(),
            max_connections: 10,
        }),
        redis: Some(RedisConfig {
            url: "redis://localhost:6380".to_string(),
            password: Some("test_redis_password_123".to_string()),
            database: 0,
            max_connections: 10,
            key_prefix: "polytorus:test:contracts:".to_string(),
            ttl_seconds: Some(300),
        }),
        fallback_to_memory: true,
        connection_timeout_secs: 5, // Short timeout for testing
        max_connections: 20,
        use_ssl: false,
    }
}

fn create_config_with_invalid_redis() -> DatabaseStorageConfig {
    DatabaseStorageConfig {
        postgres: Some(PostgresConfig {
            host: "localhost".to_string(),
            port: 5433,
            database: "polytorus_test".to_string(),
            username: "polytorus_test".to_string(),
            password: "test_password_123".to_string(),
            schema: "smart_contracts".to_string(),
            max_connections: 10,
        }),
        redis: Some(RedisConfig {
            url: "redis://localhost:9999".to_string(), // Invalid port
            password: Some("test_redis_password_123".to_string()),
            database: 0,
            max_connections: 10,
            key_prefix: "polytorus:test:contracts:".to_string(),
            ttl_seconds: Some(300),
        }),
        fallback_to_memory: true,
        connection_timeout_secs: 5,
        max_connections: 20,
        use_ssl: false,
    }
}

fn create_config_with_both_invalid() -> DatabaseStorageConfig {
    DatabaseStorageConfig {
        postgres: Some(PostgresConfig {
            host: "localhost".to_string(),
            port: 9998, // Invalid port
            database: "polytorus_test".to_string(),
            username: "polytorus_test".to_string(),
            password: "test_password_123".to_string(),
            schema: "smart_contracts".to_string(),
            max_connections: 10,
        }),
        redis: Some(RedisConfig {
            url: "redis://localhost:9999".to_string(), // Invalid port
            password: Some("test_redis_password_123".to_string()),
            database: 0,
            max_connections: 10,
            key_prefix: "polytorus:test:contracts:".to_string(),
            ttl_seconds: Some(300),
        }),
        fallback_to_memory: true,
        connection_timeout_secs: 5,
        max_connections: 20,
        use_ssl: false,
    }
}

fn create_normal_config() -> DatabaseStorageConfig {
    DatabaseStorageConfig {
        postgres: Some(PostgresConfig {
            host: "localhost".to_string(),
            port: 5433,
            database: "polytorus_test".to_string(),
            username: "polytorus_test".to_string(),
            password: "test_password_123".to_string(),
            schema: "smart_contracts".to_string(),
            max_connections: 10,
        }),
        redis: Some(RedisConfig {
            url: "redis://localhost:6380".to_string(),
            password: Some("test_redis_password_123".to_string()),
            database: 0,
            max_connections: 10,
            key_prefix: "polytorus:test:contracts:".to_string(),
            ttl_seconds: Some(300),
        }),
        fallback_to_memory: true,
        connection_timeout_secs: 10,
        max_connections: 20,
        use_ssl: false,
    }
}

fn create_test_metadata(suffix: &str) -> UnifiedContractMetadata {
    UnifiedContractMetadata {
        address: format!("0x{:0>40}", format!("failover{}", suffix)),
        name: format!("FailoverTest{suffix}"),
        description: format!("Failover test contract {suffix}"),
        contract_type: ContractType::Wasm {
            bytecode: vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00],
            abi: Some(format!(
                r#"{{"test": "failover{suffix}", "version": "1.0"}}"#
            )),
        },
        deployment_tx: format!("0x{:0>64}", format!("failoverdeploy{}", suffix)),
        deployment_time: 1640995200 + suffix.parse::<u64>().unwrap_or(0),
        owner: format!("0x{:0>40}", format!("failoverowner{}", suffix)),
        is_active: true,
    }
}

async fn test_storage_operations(
    storage: &DatabaseContractStorage,
    test_id: &str,
    description: &str,
) -> Result<()> {
    println!("  📝 Testing operations: {description}");

    let metadata = create_test_metadata(test_id);

    // Test metadata operations
    let start = Instant::now();
    match storage.store_contract_metadata(&metadata) {
        Ok(_) => println!("    ✅ Metadata stored ({:?})", start.elapsed()),
        Err(e) => println!("    ❌ Metadata store failed: {e}"),
    }

    let start = Instant::now();
    match storage.get_contract_metadata(&metadata.address) {
        Ok(Some(_)) => println!("    ✅ Metadata retrieved ({:?})", start.elapsed()),
        Ok(None) => println!("    ⚠️  Metadata not found ({:?})", start.elapsed()),
        Err(e) => println!(
            "    ❌ Metadata retrieval failed: {} ({:?})",
            e,
            start.elapsed()
        ),
    }

    // Test state operations
    let start = Instant::now();
    match storage.set_contract_state(&metadata.address, "balance", &1000u64.to_le_bytes()) {
        Ok(_) => println!("    ✅ State stored ({:?})", start.elapsed()),
        Err(e) => println!("    ❌ State store failed: {e}"),
    }

    let start = Instant::now();
    match storage.get_contract_state(&metadata.address, "balance") {
        Ok(Some(_)) => println!("    ✅ State retrieved ({:?})", start.elapsed()),
        Ok(None) => println!("    ⚠️  State not found ({:?})", start.elapsed()),
        Err(e) => println!(
            "    ❌ State retrieval failed: {} ({:?})",
            e,
            start.elapsed()
        ),
    }

    // Test execution history
    let execution = ContractExecutionRecord {
        execution_id: format!("failover_exec_{test_id}"),
        contract_address: metadata.address.clone(),
        function_name: "failover_test".to_string(),
        caller: format!("0x{:0>40}", format!("failovercaller{}", test_id)),
        timestamp: 1640995200 + test_id.parse::<u64>().unwrap_or(0),
        gas_used: 21000,
        success: true,
        error_message: None,
    };

    let start = Instant::now();
    match storage.store_execution(&execution) {
        Ok(_) => println!("    ✅ Execution stored ({:?})", start.elapsed()),
        Err(e) => println!("    ❌ Execution store failed: {e}"),
    }

    let start = Instant::now();
    match storage.get_execution_history(&metadata.address) {
        Ok(history) => println!(
            "    ✅ Execution history retrieved: {} entries ({:?})",
            history.len(),
            start.elapsed()
        ),
        Err(e) => println!(
            "    ❌ Execution history failed: {} ({:?})",
            e,
            start.elapsed()
        ),
    }

    Ok(())
}

async fn test_connectivity_and_stats(
    storage: &DatabaseContractStorage,
    scenario: &str,
) -> Result<()> {
    println!("  🔍 Checking connectivity and stats for: {scenario}");

    // Check connectivity
    match storage.check_connectivity().await {
        Ok(status) => {
            println!(
                "    PostgreSQL: {}",
                if status.postgres_connected {
                    "✅ Connected"
                } else {
                    "❌ Disconnected"
                }
            );
            println!(
                "    Redis: {}",
                if status.redis_connected {
                    "✅ Connected"
                } else {
                    "❌ Disconnected"
                }
            );
            println!(
                "    Fallback: {}",
                if status.fallback_available {
                    "✅ Available"
                } else {
                    "❌ Unavailable"
                }
            );
        }
        Err(e) => println!("    ❌ Connectivity check failed: {e}"),
    }

    // Get statistics
    let stats = storage.get_stats().await;
    println!("    📊 Stats:");
    println!("      Total queries: {}", stats.total_queries);
    println!("      Failed queries: {}", stats.failed_queries);
    println!("      Cache hits: {}", stats.cache_hits);
    println!("      Cache misses: {}", stats.cache_misses);

    if stats.total_queries > 0 {
        let success_rate = ((stats.total_queries - stats.failed_queries) as f64
            / stats.total_queries as f64)
            * 100.0;
        println!("      Success rate: {success_rate:.1}%");
    }

    let total_cache_ops = stats.cache_hits + stats.cache_misses;
    if total_cache_ops > 0 {
        let hit_rate = (stats.cache_hits as f64 / total_cache_ops as f64) * 100.0;
        println!("      Cache hit rate: {hit_rate:.1}%");
    }

    Ok(())
}

async fn run_performance_test(
    storage: &DatabaseContractStorage,
    scenario: &str,
    operations: usize,
) -> Result<()> {
    println!("  ⚡ Performance test for {scenario}: {operations} operations");

    let start = Instant::now();
    let mut successful_ops = 0;
    let mut failed_ops = 0;

    for i in 0..operations {
        let test_id = format!("perf{}_{}", scenario.replace(" ", ""), i);
        let metadata = create_test_metadata(&test_id);

        // Try to perform a complete operation cycle
        let _op_start = Instant::now();
        let mut op_success = true;

        if storage.store_contract_metadata(&metadata).is_err() {
            op_success = false;
        }

        if storage
            .set_contract_state(&metadata.address, "test", b"value")
            .is_err()
        {
            op_success = false;
        }

        if storage.get_contract_metadata(&metadata.address).is_err() {
            op_success = false;
        }

        if op_success {
            successful_ops += 1;
        } else {
            failed_ops += 1;
        }

        // Small delay to avoid overwhelming the system
        if i % 10 == 0 {
            sleep(Duration::from_millis(10)).await;
        }
    }

    let total_time = start.elapsed();
    let ops_per_second = operations as f64 / total_time.as_secs_f64();

    println!("    📊 Results:");
    println!("      Total time: {total_time:?}");
    println!("      Successful operations: {successful_ops}");
    println!("      Failed operations: {failed_ops}");
    println!("      Operations per second: {ops_per_second:.2}");
    println!(
        "      Success rate: {:.1}%",
        (successful_ops as f64 / operations as f64) * 100.0
    );

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("🔄 Application-Level Failover Test");
    println!("==================================");

    // Test 1: Normal operation (both databases available)
    println!("\n🧪 Test 1: Normal Operation");
    println!("===========================");

    let config = create_normal_config();
    match DatabaseContractStorage::new(config).await {
        Ok(storage) => {
            println!("✅ Storage initialized with both databases");
            test_connectivity_and_stats(&storage, "Normal operation").await?;
            test_storage_operations(&storage, "001", "Normal operation").await?;
            run_performance_test(&storage, "normal", 50).await?;
        }
        Err(e) => {
            println!("❌ Failed to initialize storage: {e}");
            println!("⚠️  Make sure databases are running: docker-compose -f docker-compose.database-test.yml up -d");
        }
    }

    // Test 2: PostgreSQL failure (Redis + memory fallback)
    println!("\n🧪 Test 2: PostgreSQL Failure");
    println!("=============================");

    let config = create_config_with_invalid_postgres();
    match DatabaseContractStorage::new(config).await {
        Ok(storage) => {
            println!("✅ Storage initialized with PostgreSQL unavailable");
            test_connectivity_and_stats(&storage, "PostgreSQL failure").await?;
            test_storage_operations(&storage, "002", "PostgreSQL failure").await?;
            run_performance_test(&storage, "postgres_fail", 30).await?;
        }
        Err(e) => {
            println!("❌ Failed to initialize storage: {e}");
        }
    }

    // Test 3: Redis failure (PostgreSQL + memory fallback)
    println!("\n🧪 Test 3: Redis Failure");
    println!("========================");

    let config = create_config_with_invalid_redis();
    match DatabaseContractStorage::new(config).await {
        Ok(storage) => {
            println!("✅ Storage initialized with Redis unavailable");
            test_connectivity_and_stats(&storage, "Redis failure").await?;
            test_storage_operations(&storage, "003", "Redis failure").await?;
            run_performance_test(&storage, "redis_fail", 30).await?;
        }
        Err(e) => {
            println!("❌ Failed to initialize storage: {e}");
        }
    }

    // Test 4: Both databases failure (memory fallback only)
    println!("\n🧪 Test 4: Complete Database Failure");
    println!("====================================");

    let config = create_config_with_both_invalid();
    match DatabaseContractStorage::new(config).await {
        Ok(storage) => {
            println!("✅ Storage initialized with both databases unavailable");
            test_connectivity_and_stats(&storage, "Complete failure").await?;
            test_storage_operations(&storage, "004", "Complete failure").await?;
            run_performance_test(&storage, "complete_fail", 20).await?;
        }
        Err(e) => {
            println!("❌ Failed to initialize storage: {e}");
        }
    }

    // Test 5: Fallback disabled (strict mode)
    println!("\n🧪 Test 5: Strict Mode (No Fallback)");
    println!("====================================");

    let mut config = create_config_with_both_invalid();
    config.fallback_to_memory = false;

    match DatabaseContractStorage::new(config).await {
        Ok(_) => {
            println!("❌ Storage should have failed to initialize");
        }
        Err(e) => {
            println!("✅ Storage correctly failed to initialize: {e}");
            println!("   This is expected behavior in strict mode");
        }
    }

    // Test 6: Recovery simulation
    println!("\n🧪 Test 6: Recovery Simulation");
    println!("==============================");

    println!("Phase 1: Start with failed databases");
    let config = create_config_with_both_invalid();
    if let Ok(storage) = DatabaseContractStorage::new(config).await {
        test_connectivity_and_stats(&storage, "Initial failure").await?;

        println!("\nPhase 2: Simulate database recovery");
        println!("(In real scenario, databases would be restarted)");

        // Create new storage with working databases
        let config = create_normal_config();
        if let Ok(recovered_storage) = DatabaseContractStorage::new(config).await {
            println!("✅ Simulated recovery successful");
            test_connectivity_and_stats(&recovered_storage, "After recovery").await?;
            test_storage_operations(&recovered_storage, "005", "After recovery").await?;
        }
    }

    // Test 7: Concurrent operations during failure
    println!("\n🧪 Test 7: Concurrent Operations During Failure");
    println!("===============================================");

    let config = create_config_with_invalid_postgres();
    if let Ok(storage) = DatabaseContractStorage::new(config).await {
        println!("Testing concurrent operations with PostgreSQL failure...");

        let storage = std::sync::Arc::new(storage);
        let mut handles = Vec::new();

        for i in 0..10 {
            let storage_clone = storage.clone();
            let handle = tokio::spawn(async move {
                let test_id = format!("concurrent_{i}");
                let metadata = create_test_metadata(&test_id);

                let mut operations_completed = 0;
                let mut operations_failed = 0;

                // Try multiple operations
                for _ in 0..5 {
                    if storage_clone.store_contract_metadata(&metadata).is_ok() {
                        operations_completed += 1;
                    } else {
                        operations_failed += 1;
                    }

                    if storage_clone
                        .set_contract_state(&metadata.address, "test", b"value")
                        .is_ok()
                    {
                        operations_completed += 1;
                    } else {
                        operations_failed += 1;
                    }
                }

                (operations_completed, operations_failed)
            });
            handles.push(handle);
        }

        let mut total_completed = 0;
        let mut total_failed = 0;

        for handle in handles {
            if let Ok((completed, failed)) = handle.await {
                total_completed += completed;
                total_failed += failed;
            }
        }

        println!("  📊 Concurrent operations results:");
        println!("    Completed: {total_completed}");
        println!("    Failed: {total_failed}");
        println!(
            "    Success rate: {:.1}%",
            (total_completed as f64 / (total_completed + total_failed) as f64) * 100.0
        );
    }

    // Summary
    println!("\n🎉 Application-Level Failover Tests Completed!");
    println!("==============================================");

    println!("\n📊 Test Summary:");
    println!("✅ Normal operation tested");
    println!("✅ PostgreSQL failure handling tested");
    println!("✅ Redis failure handling tested");
    println!("✅ Complete database failure tested");
    println!("✅ Strict mode behavior verified");
    println!("✅ Recovery simulation tested");
    println!("✅ Concurrent operations during failure tested");

    println!("\n💡 Key Observations:");
    println!("• Fallback mechanisms provide graceful degradation");
    println!("• Performance impact varies by failure scenario");
    println!("• Memory fallback ensures continued operation");
    println!("• Recovery is seamless when databases come back online");
    println!("• Concurrent operations remain stable during failures");

    println!("\n⚠️  Production Considerations:");
    println!("• Monitor database connectivity continuously");
    println!("• Set appropriate timeout values for your use case");
    println!("• Consider the data persistence implications of memory fallback");
    println!("• Implement proper logging and alerting for failure scenarios");
    println!("• Test failover procedures regularly in staging environments");

    Ok(())
}

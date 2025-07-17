//! Database Storage Demo
//!
//! This example demonstrates the advanced database storage capabilities for smart contracts.
//! It shows PostgreSQL and Redis integration with fallback to in-memory storage.
//!
//! Usage:
//! ```bash
//! # Start databases (optional - demo works with memory fallback)
//! docker-compose -f docker-compose.database-test.yml up -d
//!
//! # Run the demo
//! cargo run --example database_storage_demo
//! ```

use std::time::Instant;

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
    println!("🚀 Database Storage Demo for Polytorus Smart Contracts");
    println!("======================================================");

    // Demo configurations
    let configs = vec![
        ("Memory Only", create_memory_only_config()),
        ("Full Database", create_full_database_config()),
        ("Postgres Only", create_postgres_only_config()),
        ("Redis Only", create_redis_only_config()),
    ];

    for (name, config) in configs {
        println!("\n📋 Testing Configuration: {name}");
        println!("----------------------------------------");

        match test_storage_configuration(config).await {
            Ok(_) => println!("✅ {name} configuration test passed"),
            Err(e) => println!("❌ {name} configuration test failed: {e}"),
        }
    }

    println!("\n🎯 Running Performance Benchmark");
    println!("=================================");
    run_performance_benchmark().await?;

    println!("\n📊 Database Monitoring Demo");
    println!("===========================");
    demonstrate_monitoring().await?;

    println!("\n🔄 Failover Behavior Demo");
    println!("=========================");
    demonstrate_failover().await?;

    println!("\n✅ Database Storage Demo Complete!");
    Ok(())
}

fn create_memory_only_config() -> DatabaseStorageConfig {
    DatabaseStorageConfig {
        postgres: None,
        redis: None,
        fallback_to_memory: true,
        connection_timeout_secs: 5,
        max_connections: 10,
        use_ssl: false,
    }
}

fn create_full_database_config() -> DatabaseStorageConfig {
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
            key_prefix: "polytorus:demo:contracts:".to_string(),
            ttl_seconds: Some(300), // 5 minutes for demo
        }),
        fallback_to_memory: true,
        connection_timeout_secs: 5,
        max_connections: 20,
        use_ssl: false,
    }
}

fn create_postgres_only_config() -> DatabaseStorageConfig {
    let mut config = create_full_database_config();
    config.redis = None;
    config
}

fn create_redis_only_config() -> DatabaseStorageConfig {
    let mut config = create_full_database_config();
    config.postgres = None;
    config
}

async fn test_storage_configuration(config: DatabaseStorageConfig) -> Result<()> {
    let storage = DatabaseContractStorage::new(config).await?;

    // Check connectivity
    let status = storage.check_connectivity().await?;
    println!(
        "   Connectivity - PostgreSQL: {}, Redis: {}, Fallback: {}",
        status.postgres_connected, status.redis_connected, status.fallback_available
    );

    // Create sample contract metadata
    let metadata = create_sample_metadata("demo_contract");

    // Test contract metadata operations
    storage.store_contract_metadata(&metadata)?;
    let retrieved = storage.get_contract_metadata(&metadata.address)?;
    assert!(retrieved.is_some(), "Failed to retrieve metadata");
    println!("   ✅ Contract metadata operations working");

    // Test contract state operations
    storage.set_contract_state(&metadata.address, "balance", &1000u64.to_le_bytes())?;
    storage.set_contract_state(&metadata.address, "name", b"DemoToken")?;

    let balance = storage.get_contract_state(&metadata.address, "balance")?;
    assert!(balance.is_some());
    let balance_value = u64::from_le_bytes(balance.unwrap().try_into().unwrap());
    assert_eq!(balance_value, 1000);
    println!("   ✅ Contract state operations working");

    // Test execution history
    let execution = ContractExecutionRecord {
        execution_id: "demo_exec_001".to_string(),
        contract_address: metadata.address.clone(),
        function_name: "transfer".to_string(),
        caller: "0xdemo_caller".to_string(),
        timestamp: chrono::Utc::now().timestamp() as u64,
        gas_used: 21000,
        success: true,
        error_message: None,
    };

    storage.store_execution(&execution)?;
    let history = storage.get_execution_history(&metadata.address)?;
    assert!(!history.is_empty(), "Execution history should not be empty");
    println!("   ✅ Execution history operations working");

    // Get statistics
    let stats = storage.get_stats().await;
    println!(
        "   📊 Stats - Queries: {}, Cache hits: {}, Cache misses: {}",
        stats.total_queries, stats.cache_hits, stats.cache_misses
    );

    Ok(())
}

async fn run_performance_benchmark() -> Result<()> {
    let storage = DatabaseContractStorage::new(create_memory_only_config()).await?;

    let num_contracts = 50;
    let num_operations_per_contract = 10;

    println!(
        "   Benchmarking {num_contracts} contracts with {num_operations_per_contract} operations each"
    );

    let start_time = Instant::now();

    // Create contracts and perform operations
    for i in 0..num_contracts {
        let metadata = create_sample_metadata(&format!("bench_{i:03}"));
        storage.store_contract_metadata(&metadata)?;

        // Perform state operations
        for j in 0..num_operations_per_contract {
            let key = format!("key_{j}");
            let value = format!("value_{i}_{j}");
            storage.set_contract_state(&metadata.address, &key, value.as_bytes())?;
        }

        // Store execution record
        let execution = ContractExecutionRecord {
            execution_id: format!("bench_exec_{i}"),
            contract_address: metadata.address,
            function_name: "benchmark_function".to_string(),
            caller: format!("0xbench_caller_{i:03}"),
            timestamp: chrono::Utc::now().timestamp() as u64,
            gas_used: 50000 + i * 1000,
            success: true,
            error_message: None,
        };
        storage.store_execution(&execution)?;
    }

    let duration = start_time.elapsed();
    let total_operations = num_contracts * (1 + num_operations_per_contract + 1);
    let ops_per_second = total_operations as f64 / duration.as_secs_f64();

    println!("   ⚡ Performance Results:");
    println!("      Total operations: {total_operations}");
    println!("      Duration: {duration:?}");
    println!("      Operations/second: {ops_per_second:.2}");

    // Verify results
    let contracts = storage.list_contracts()?;
    let bench_contracts = contracts
        .iter()
        .filter(|addr| addr.contains("bench"))
        .count();
    println!("      Verified contracts: {bench_contracts}/{num_contracts}");

    Ok(())
}

async fn demonstrate_monitoring() -> Result<()> {
    let storage = DatabaseContractStorage::new(create_memory_only_config()).await?;

    // Initial state
    let initial_info = storage.get_database_info().await?;
    println!("   📊 Initial Database Info:");
    println!(
        "      Memory entries: {}",
        initial_info.memory_fallback_entries
    );
    println!("      Total contracts: {}", initial_info.total_contracts);
    println!(
        "      Total state entries: {}",
        initial_info.total_state_entries
    );

    // Add some data
    let metadata = create_sample_metadata("monitoring_test");
    storage.store_contract_metadata(&metadata)?;

    for i in 0..5 {
        storage.set_contract_state(
            &metadata.address,
            &format!("monitor_key_{i}"),
            format!("monitor_value_{i}").as_bytes(),
        )?;
    }

    // Check updated state
    let updated_info = storage.get_database_info().await?;
    println!("   📊 Updated Database Info:");
    println!(
        "      Memory entries: {}",
        updated_info.memory_fallback_entries
    );
    println!("      Total contracts: {}", updated_info.total_contracts);
    println!(
        "      Total state entries: {}",
        updated_info.total_state_entries
    );

    // Show statistics
    let stats = storage.get_stats().await;
    println!("   📈 Performance Statistics:");
    println!("      Total queries: {}", stats.total_queries);
    println!("      Failed queries: {}", stats.failed_queries);
    println!("      Cache hits: {}", stats.cache_hits);
    println!("      Cache misses: {}", stats.cache_misses);

    Ok(())
}

async fn demonstrate_failover() -> Result<()> {
    println!("   🔄 Testing failover with invalid database configuration...");

    // Create config with invalid database connections
    let mut config = create_full_database_config();
    config.postgres.as_mut().unwrap().port = 9999; // Invalid port
    config.redis.as_mut().unwrap().url = "redis://localhost:9999".to_string(); // Invalid port
    config.fallback_to_memory = true;

    let storage = DatabaseContractStorage::new(config).await?;

    // Check connectivity (should show disconnected but fallback available)
    let status = storage.check_connectivity().await?;
    println!("   📡 Failover Status:");
    println!("      PostgreSQL connected: {}", status.postgres_connected);
    println!("      Redis connected: {}", status.redis_connected);
    println!("      Fallback available: {}", status.fallback_available);

    // Operations should still work with memory fallback
    let metadata = create_sample_metadata("failover_test");
    storage.store_contract_metadata(&metadata)?;

    let retrieved = storage.get_contract_metadata(&metadata.address)?;
    assert!(retrieved.is_some(), "Failover storage should work");

    println!("   ✅ Failover behavior working correctly");

    Ok(())
}

fn create_sample_metadata(suffix: &str) -> UnifiedContractMetadata {
    UnifiedContractMetadata {
        address: format!("0x{:0>40}", format!("demo{}", suffix)),
        name: format!("DemoContract_{suffix}"),
        description: format!("Demo contract for testing: {suffix}"),
        contract_type: ContractType::Wasm {
            bytecode: vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00], // WASM magic
            abi: Some(format!(
                r#"{{"contract": "demo_{suffix}", "version": "1.0.0"}}"#
            )),
        },
        deployment_tx: format!("0x{:0>64}", format!("deployment_{}", suffix)),
        deployment_time: chrono::Utc::now().timestamp() as u64,
        owner: format!("0x{:0>40}", format!("owner_{}", suffix)),
        is_active: true,
    }
}

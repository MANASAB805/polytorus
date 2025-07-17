//! Database Storage Benchmarks
//!
//! These benchmarks measure the performance of different storage backends.
//! Run with: cargo bench --bench database_storage_bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use polytorus::smart_contract::{
    database_storage::{
        DatabaseContractStorage, DatabaseStorageConfig, PostgresConfig, RedisConfig,
    },
    unified_contract_storage::UnifiedContractStorage,
    unified_engine::{
        ContractExecutionRecord, ContractStateStorage, ContractType, UnifiedContractMetadata,
    },
};
use tokio::runtime::Runtime;

// Create test configurations
fn create_database_config() -> DatabaseStorageConfig {
    DatabaseStorageConfig {
        postgres: Some(PostgresConfig {
            host: "localhost".to_string(),
            port: 5433,
            database: "polytorus_test".to_string(),
            username: "polytorus_test".to_string(),
            password: "test_password_123".to_string(),
            schema: "smart_contracts".to_string(),
            max_connections: 20,
        }),
        redis: Some(RedisConfig {
            url: "redis://localhost:6380".to_string(),
            password: Some("test_redis_password_123".to_string()),
            database: 1, // Use different database for benchmarks
            max_connections: 20,
            key_prefix: "polytorus:bench:contracts:".to_string(),
            ttl_seconds: Some(3600),
        }),
        fallback_to_memory: true,
        connection_timeout_secs: 10,
        max_connections: 40,
        use_ssl: false,
    }
}

fn create_test_metadata(id: usize) -> UnifiedContractMetadata {
    UnifiedContractMetadata {
        address: format!("0x{:0>40}", format!("bench{:06}", id)),
        name: format!("BenchContract{id:06}"),
        description: format!("Benchmark contract {id}"),
        contract_type: ContractType::Wasm {
            bytecode: vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00],
            abi: Some(format!(
                r#"{{"contract": "bench{id:06}", "version": "1.0"}}"#
            )),
        },
        deployment_tx: format!("0x{:0>64}", format!("benchdeploy{:06}", id)),
        deployment_time: 1640995200 + id as u64,
        owner: format!("0x{:0>40}", format!("benchowner{:06}", id)),
        is_active: true,
    }
}

fn create_test_execution(contract_id: usize, exec_id: usize) -> ContractExecutionRecord {
    ContractExecutionRecord {
        execution_id: format!("bench_exec_{contract_id}_{exec_id}"),
        contract_address: format!("0x{:0>40}", format!("bench{:06}", contract_id)),
        function_name: "benchmark_function".to_string(),
        caller: format!("0x{:0>40}", format!("benchcaller{:06}", exec_id)),
        timestamp: 1640995200 + (contract_id * 1000 + exec_id) as u64,
        gas_used: 21000 + (exec_id * 1000) as u64,
        success: exec_id % 10 != 0, // 10% failure rate
        error_message: if exec_id % 10 == 0 {
            Some("Benchmark error".to_string())
        } else {
            None
        },
    }
}

// Benchmark metadata operations
fn bench_metadata_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("metadata_operations");
    group.throughput(Throughput::Elements(1));

    // In-memory storage
    group.bench_function("in_memory_store_metadata", |b| {
        let storage = UnifiedContractStorage::new_sync_memory();
        b.iter(|| {
            let metadata = create_test_metadata(black_box(0));
            storage.store_contract_metadata(&metadata).unwrap();
        });
    });

    group.bench_function("in_memory_get_metadata", |b| {
        let storage = UnifiedContractStorage::new_sync_memory();
        let metadata = create_test_metadata(0);
        storage.store_contract_metadata(&metadata).unwrap();

        b.iter(|| {
            storage
                .get_contract_metadata(black_box(&metadata.address))
                .unwrap();
        });
    });

    // Sled storage
    group.bench_function("sled_store_metadata", |b| {
        let storage = UnifiedContractStorage::new_sync_memory();
        b.iter(|| {
            let metadata = create_test_metadata(black_box(0));
            storage.store_contract_metadata(&metadata).unwrap();
        });
    });

    group.bench_function("sled_get_metadata", |b| {
        let storage = UnifiedContractStorage::new_sync_memory();
        let metadata = create_test_metadata(0);
        storage.store_contract_metadata(&metadata).unwrap();

        b.iter(|| {
            storage
                .get_contract_metadata(black_box(&metadata.address))
                .unwrap();
        });
    });

    // Database storage (if available)
    if let Ok(storage) =
        rt.block_on(async { DatabaseContractStorage::new(create_database_config()).await })
    {
        group.bench_function("database_store_metadata", |b| {
            b.iter(|| {
                let metadata = create_test_metadata(black_box(0));
                storage.store_contract_metadata(&metadata).unwrap();
            });
        });

        group.bench_function("database_get_metadata", |b| {
            let metadata = create_test_metadata(0);
            storage.store_contract_metadata(&metadata).unwrap();

            b.iter(|| {
                storage
                    .get_contract_metadata(black_box(&metadata.address))
                    .unwrap();
            });
        });
    }

    group.finish();
}

// Benchmark state operations
fn bench_state_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("state_operations");
    group.throughput(Throughput::Elements(1));

    let contract_address = "0x1234567890abcdef1234567890abcdef12345678";
    let test_value = b"benchmark_test_value_1234567890";

    // In-memory storage
    group.bench_function("in_memory_set_state", |b| {
        let storage = UnifiedContractStorage::new_sync_memory();
        b.iter(|| {
            storage
                .set_contract_state(
                    black_box(contract_address),
                    black_box("bench_key"),
                    black_box(test_value),
                )
                .unwrap();
        });
    });

    group.bench_function("in_memory_get_state", |b| {
        let storage = UnifiedContractStorage::new_sync_memory();
        storage
            .set_contract_state(contract_address, "bench_key", test_value)
            .unwrap();

        b.iter(|| {
            storage
                .get_contract_state(black_box(contract_address), black_box("bench_key"))
                .unwrap();
        });
    });

    // Sled storage
    group.bench_function("sled_set_state", |b| {
        let storage = UnifiedContractStorage::new_sync_memory();
        b.iter(|| {
            storage
                .set_contract_state(
                    black_box(contract_address),
                    black_box("bench_key"),
                    black_box(test_value),
                )
                .unwrap();
        });
    });

    group.bench_function("sled_get_state", |b| {
        let storage = UnifiedContractStorage::new_sync_memory();
        storage
            .set_contract_state(contract_address, "bench_key", test_value)
            .unwrap();

        b.iter(|| {
            storage
                .get_contract_state(black_box(contract_address), black_box("bench_key"))
                .unwrap();
        });
    });

    // Database storage (if available)
    if let Ok(storage) =
        rt.block_on(async { DatabaseContractStorage::new(create_database_config()).await })
    {
        group.bench_function("database_set_state", |b| {
            b.iter(|| {
                storage
                    .set_contract_state(
                        black_box(contract_address),
                        black_box("bench_key"),
                        black_box(test_value),
                    )
                    .unwrap();
            });
        });

        group.bench_function("database_get_state", |b| {
            storage
                .set_contract_state(contract_address, "bench_key", test_value)
                .unwrap();

            b.iter(|| {
                storage
                    .get_contract_state(black_box(contract_address), black_box("bench_key"))
                    .unwrap();
            });
        });
    }

    group.finish();
}

// Benchmark execution history operations
fn bench_execution_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("execution_operations");
    group.throughput(Throughput::Elements(1));

    // In-memory storage
    group.bench_function("in_memory_store_execution", |b| {
        let storage = UnifiedContractStorage::new_sync_memory();
        b.iter(|| {
            let execution = create_test_execution(black_box(0), black_box(0));
            storage.store_execution(&execution).unwrap();
        });
    });

    group.bench_function("in_memory_get_history", |b| {
        let storage = UnifiedContractStorage::new_sync_memory();
        for i in 0..10 {
            let execution = create_test_execution(0, i);
            storage.store_execution(&execution).unwrap();
        }

        b.iter(|| {
            storage
                .get_execution_history(black_box(
                    "0x0000000000000000000000000000000000000000000000000000000000bench000000",
                ))
                .unwrap();
        });
    });

    // Database storage (if available)
    if let Ok(storage) =
        rt.block_on(async { DatabaseContractStorage::new(create_database_config()).await })
    {
        group.bench_function("database_store_execution", |b| {
            b.iter(|| {
                let execution = create_test_execution(black_box(0), black_box(0));
                storage.store_execution(&execution).unwrap();
            });
        });

        group.bench_function("database_get_history", |b| {
            for i in 0..10 {
                let execution = create_test_execution(0, i);
                storage.store_execution(&execution).unwrap();
            }

            b.iter(|| {
                storage
                    .get_execution_history(black_box(
                        "0x0000000000000000000000000000000000000000000000000000000000bench000000",
                    ))
                    .unwrap();
            });
        });
    }

    group.finish();
}

// Benchmark bulk operations
fn bench_bulk_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("bulk_operations");

    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        // In-memory bulk metadata storage
        group.bench_with_input(
            BenchmarkId::new("in_memory_bulk_metadata", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let storage = UnifiedContractStorage::new_sync_memory();
                    for i in 0..size {
                        let metadata = create_test_metadata(black_box(i));
                        storage.store_contract_metadata(&metadata).unwrap();
                    }
                });
            },
        );

        // Database bulk metadata storage (if available)
        if let Ok(storage) =
            rt.block_on(async { DatabaseContractStorage::new(create_database_config()).await })
        {
            group.bench_with_input(
                BenchmarkId::new("database_bulk_metadata", size),
                size,
                |b, &size| {
                    b.iter(|| {
                        for i in 0..size {
                            let metadata = create_test_metadata(black_box(i));
                            storage.store_contract_metadata(&metadata).unwrap();
                        }
                    });
                },
            );
        }
    }

    group.finish();
}

// Benchmark concurrent operations
fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent_operations");

    for concurrency in [1, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements(*concurrency as u64));

        // Database concurrent operations (if available)
        if rt
            .block_on(async { DatabaseContractStorage::new(create_database_config()).await })
            .is_ok()
        {
            group.bench_with_input(
                BenchmarkId::new("database_concurrent_metadata", concurrency),
                concurrency,
                |b, &concurrency| {
                    b.iter(|| {
                        rt.block_on(async {
                            let mut handles = Vec::new();

                            for i in 0..concurrency {
                                let storage =
                                    DatabaseContractStorage::new(create_database_config())
                                        .await
                                        .unwrap();
                                let handle = tokio::spawn(async move {
                                    let metadata = create_test_metadata(black_box(i));
                                    storage.store_contract_metadata(&metadata).unwrap();
                                    storage.get_contract_metadata(&metadata.address).unwrap();
                                });
                                handles.push(handle);
                            }

                            for handle in handles {
                                handle.await.unwrap();
                            }
                        });
                    });
                },
            );
        }
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_metadata_operations,
    bench_state_operations,
    bench_execution_operations,
    bench_bulk_operations,
    bench_concurrent_operations
);
criterion_main!(benches);

//! P2P Network Essential Performance Benchmarks
//!
//! Core performance benchmarks for P2P network operations.
//! Focuses on the most critical operations without redundancy.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use std::time::Duration;

use p2p_network::{P2PConfig, WebRTCP2PNetwork};
use traits::{TxInput, TxOutput, UtxoId, UtxoTransaction};

/// Initialize minimal logging for benchmarks
fn init_logging() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Error)
        .is_test(true)
        .try_init();
}

/// Create minimal P2P configuration
fn create_config(node_id: &str, port: u16) -> P2PConfig {
    P2PConfig {
        node_id: node_id.to_string(),
        listen_addr: format!("127.0.0.1:{}", port).parse().unwrap(),
        bootstrap_peers: vec![], // No network connections
        stun_servers: vec![],    // No external dependencies
        max_peers: 5,
        connection_timeout: 1,
        keep_alive_interval: 30,
        debug_mode: false,
    }
}

/// Create test transaction
fn create_transaction(id: u64) -> UtxoTransaction {
    UtxoTransaction {
        hash: format!("tx_{}", id),
        inputs: vec![TxInput {
            utxo_id: UtxoId {
                tx_hash: format!("input_{}", id),
                output_index: 0,
            },
            redeemer: b"redeemer".to_vec(),
            signature: b"signature".to_vec(),
        }],
        outputs: vec![TxOutput {
            value: 1000 + id,
            script: vec![],
            datum: Some(b"data".to_vec()),
            datum_hash: Some("hash".to_string()),
        }],
        fee: 100,
        validity_range: Some((0, 10000)),
        script_witness: vec![],
        auxiliary_data: None,
    }
}

/// Benchmark core P2P operations
fn benchmark_core_operations(c: &mut Criterion) {
    init_logging();

    let mut group = c.benchmark_group("core_operations");
    group.sample_size(20);
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_secs(1));

    // Network creation
    group.bench_function("network_creation", |b| {
        b.iter(|| {
            let config = create_config("bench_node", 8100);
            let network = WebRTCP2PNetwork::new(config).unwrap();
            black_box(network);
        });
    });

    // Transaction creation
    group.bench_function("transaction_creation", |b| {
        b.iter(|| {
            let tx = create_transaction(black_box(rand::random()));
            black_box(tx);
        });
    });

    // Serialization
    let tx = create_transaction(12345);
    group.bench_function("transaction_serialize", |b| {
        b.iter(|| {
            let serialized = bincode::serialize(&tx).unwrap();
            black_box(serialized);
        });
    });

    // Deserialization
    let serialized = bincode::serialize(&tx).unwrap();
    group.bench_function("transaction_deserialize", |b| {
        b.iter(|| {
            let deserialized: UtxoTransaction = bincode::deserialize(&serialized).unwrap();
            black_box(deserialized);
        });
    });

    group.finish();
}

/// Benchmark batch processing
fn benchmark_batch_processing(c: &mut Criterion) {
    init_logging();

    let mut group = c.benchmark_group("batch_processing");
    group.sample_size(15);
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_secs(1));

    for batch_size in [10, 50].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));

        // Batch transaction creation
        group.bench_with_input(
            format!("create_batch_{}", batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    let transactions: Vec<_> = (0..batch_size)
                        .map(|i| create_transaction(i as u64))
                        .collect();
                    black_box(transactions);
                });
            },
        );

        // Batch serialization
        group.bench_with_input(
            format!("serialize_batch_{}", batch_size),
            batch_size,
            |b, &batch_size| {
                let transactions: Vec<_> = (0..batch_size)
                    .map(|i| create_transaction(i as u64))
                    .collect();

                b.iter(|| {
                    let serialized: Vec<_> = transactions
                        .iter()
                        .map(|tx| bincode::serialize(tx).unwrap())
                        .collect();
                    black_box(serialized);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark network statistics
fn benchmark_network_stats(c: &mut Criterion) {
    init_logging();

    let config = create_config("stats_node", 8105);
    let network = WebRTCP2PNetwork::new(config).unwrap();

    let mut group = c.benchmark_group("network_stats");
    group.sample_size(30);
    group.warm_up_time(Duration::from_millis(100));
    group.measurement_time(Duration::from_millis(500));

    group.bench_function("get_stats", |b| {
        b.iter(|| {
            let stats = network.get_network_stats();
            black_box(stats);
        });
    });

    group.bench_function("get_peers", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let peers = network.get_connected_peers().await;
                black_box(peers);
            });
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_core_operations,
    benchmark_batch_processing,
    benchmark_network_stats
);

criterion_main!(benches);

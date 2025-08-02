//! Realistic P2P Network Performance Benchmarks
//!
//! Benchmarks that simulate real-world P2P network scenarios with proper
//! connection establishment timing and network latency considerations.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use std::time::Duration;
use tokio::runtime::Runtime;

use p2p_network::{P2PConfig, WebRTCP2PNetwork};
use traits::{P2PNetworkLayer, TxInput, TxOutput, UtxoId, UtxoTransaction};

/// Initialize minimal logging for realistic benchmarks
fn init_logging() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Error)
        .is_test(true)
        .try_init();
}

/// Create realistic network configuration
fn create_realistic_config(node_id: &str, port: u16, bootstrap_port: Option<u16>) -> P2PConfig {
    P2PConfig {
        node_id: node_id.to_string(),
        listen_addr: format!("127.0.0.1:{}", port).parse().unwrap(),
        bootstrap_peers: if let Some(bp) = bootstrap_port {
            vec![format!("127.0.0.1:{}", bp)]
        } else {
            vec![]
        },
        stun_servers: vec![], // No STUN for faster benchmarks
        max_peers: 10,
        connection_timeout: 30,
        keep_alive_interval: 30,
        debug_mode: false,
    }
}

/// Create test transaction for realistic load
fn create_realistic_transaction(id: u64, value: u64) -> UtxoTransaction {
    UtxoTransaction {
        hash: format!("realistic_tx_{:08x}", id),
        inputs: vec![TxInput {
            utxo_id: UtxoId {
                tx_hash: format!("input_{:08x}", id / 2),
                output_index: (id % 3) as u32,
            },
            redeemer: vec![0u8; 64],  // Realistic size
            signature: vec![0u8; 64], // ECDSA signature size
        }],
        outputs: vec![TxOutput {
            value,
            script: vec![0u8; 25],      // Typical P2PKH script size
            datum: Some(vec![0u8; 32]), // 32-byte datum
            datum_hash: Some(format!("datum_{:08x}", id)),
        }],
        fee: value / 100, // 1% fee
        validity_range: Some((id * 1000, (id + 100) * 1000)),
        script_witness: vec![vec![0u8; 128]], // Witness data
        auxiliary_data: None,
    }
}

/// Benchmark network initialization performance
fn benchmark_network_initialization(c: &mut Criterion) {
    init_logging();

    let mut group = c.benchmark_group("network_initialization");
    group.sample_size(50);
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_secs(2));

    group.bench_function("create_and_configure", |b| {
        b.iter(|| {
            let config = create_realistic_config("bench_node", 9000, None);
            let network = WebRTCP2PNetwork::new(config).unwrap();
            black_box(network);
        });
    });

    group.finish();
}

/// Benchmark transaction processing throughput
fn benchmark_transaction_throughput(c: &mut Criterion) {
    init_logging();

    let rt = Runtime::new().unwrap();
    let config = create_realistic_config("throughput_node", 9001, None);
    let network = WebRTCP2PNetwork::new(config).unwrap();

    let mut group = c.benchmark_group("transaction_throughput");
    group.sample_size(30);
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_secs(3));

    // Single transaction processing
    group.bench_function("single_transaction", |b| {
        b.iter(|| {
            rt.block_on(async {
                let tx = create_realistic_transaction(rand::random(), 100000);
                let result = network.broadcast_transaction(&tx).await;
                let _ = black_box(result);
            });
        });
    });

    // Batch transaction processing
    for batch_size in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            format!("batch_{}_transactions", batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    rt.block_on(async {
                        let transactions: Vec<_> = (0..batch_size)
                            .map(|i| {
                                create_realistic_transaction(i as u64, 100000 + i as u64 * 1000)
                            })
                            .collect();

                        let mut results = Vec::new();
                        for tx in &transactions {
                            let result = network.broadcast_transaction(tx).await;
                            results.push(result);
                        }
                        let _ = black_box(results);
                    });
                });
            },
        );
    }

    group.finish();
}

/// Benchmark network statistics collection
fn benchmark_network_statistics(c: &mut Criterion) {
    init_logging();

    let rt = Runtime::new().unwrap();
    let config = create_realistic_config("stats_node", 9002, None);
    let network = WebRTCP2PNetwork::new(config).unwrap();

    let mut group = c.benchmark_group("network_statistics");
    group.sample_size(100);
    group.warm_up_time(Duration::from_millis(100));
    group.measurement_time(Duration::from_millis(800));

    group.bench_function("get_network_stats", |b| {
        b.iter(|| {
            let stats = network.get_network_stats();
            black_box(stats);
        });
    });

    group.bench_function("get_connected_peers", |b| {
        b.iter(|| {
            rt.block_on(async {
                let peers = network.get_connected_peers().await;
                black_box(peers);
            });
        });
    });

    group.bench_function("request_blockchain_data", |b| {
        b.iter(|| {
            rt.block_on(async {
                let result = network
                    .request_blockchain_data(
                        "transaction".to_string(),
                        format!("hash_{}", rand::random::<u32>()),
                    )
                    .await;
                let _ = black_box(result);
            });
        });
    });

    group.finish();
}

/// Benchmark transaction serialization/deserialization
fn benchmark_transaction_serialization(c: &mut Criterion) {
    init_logging();

    let mut group = c.benchmark_group("transaction_serialization");
    group.sample_size(100);
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_secs(1));

    // Create realistic transactions of different sizes
    let small_tx = create_realistic_transaction(1, 1000);
    let medium_tx = UtxoTransaction {
        inputs: vec![small_tx.inputs[0].clone(); 3],
        outputs: vec![small_tx.outputs[0].clone(); 5],
        ..small_tx.clone()
    };
    let large_tx = UtxoTransaction {
        inputs: vec![small_tx.inputs[0].clone(); 10],
        outputs: vec![small_tx.outputs[0].clone(); 15],
        script_witness: vec![vec![0u8; 256]; 10],
        ..small_tx.clone()
    };

    group.bench_function("serialize_small_tx", |b| {
        b.iter(|| {
            let serialized = bincode::serialize(&small_tx).unwrap();
            black_box(serialized);
        });
    });

    group.bench_function("serialize_medium_tx", |b| {
        b.iter(|| {
            let serialized = bincode::serialize(&medium_tx).unwrap();
            black_box(serialized);
        });
    });

    group.bench_function("serialize_large_tx", |b| {
        b.iter(|| {
            let serialized = bincode::serialize(&large_tx).unwrap();
            black_box(serialized);
        });
    });

    // Deserialization benchmarks
    let small_serialized = bincode::serialize(&small_tx).unwrap();
    let medium_serialized = bincode::serialize(&medium_tx).unwrap();
    let large_serialized = bincode::serialize(&large_tx).unwrap();

    group.bench_function("deserialize_small_tx", |b| {
        b.iter(|| {
            let tx: UtxoTransaction = bincode::deserialize(&small_serialized).unwrap();
            black_box(tx);
        });
    });

    group.bench_function("deserialize_medium_tx", |b| {
        b.iter(|| {
            let tx: UtxoTransaction = bincode::deserialize(&medium_serialized).unwrap();
            black_box(tx);
        });
    });

    group.bench_function("deserialize_large_tx", |b| {
        b.iter(|| {
            let tx: UtxoTransaction = bincode::deserialize(&large_serialized).unwrap();
            black_box(tx);
        });
    });

    group.finish();
}

/// Benchmark concurrent operations
fn benchmark_concurrent_operations(c: &mut Criterion) {
    init_logging();

    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent_operations");
    group.sample_size(20);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));

    group.bench_function("concurrent_network_creation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let configs: Vec<_> = (0..5)
                    .map(|i| create_realistic_config(&format!("node_{}", i), 9100 + i, None))
                    .collect();

                let networks: Vec<_> = configs
                    .into_iter()
                    .map(WebRTCP2PNetwork::new)
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();

                black_box(networks);
            });
        });
    });

    group.bench_function("concurrent_broadcasts", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = create_realistic_config("concurrent_node", 9200, None);
                let network = WebRTCP2PNetwork::new(config).unwrap();

                let transactions: Vec<_> = (0..10)
                    .map(|i| create_realistic_transaction(i, 10000 + i * 1000))
                    .collect();

                let mut handles = Vec::new();
                for tx in transactions {
                    let net = network.clone();
                    let handle = tokio::spawn(async move { net.broadcast_transaction(&tx).await });
                    handles.push(handle);
                }

                let results = futures::future::join_all(handles).await;
                let _ = black_box(results);
            });
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_network_initialization,
    benchmark_transaction_throughput,
    benchmark_network_statistics,
    benchmark_transaction_serialization,
    benchmark_concurrent_operations
);

criterion_main!(benches);

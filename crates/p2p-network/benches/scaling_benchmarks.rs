//! P2P Network Scaling Performance Benchmarks
//!
//! Benchmarks that measure how the P2P network performs under various scaling scenarios:
//! - Increasing number of peers
//! - Increasing transaction throughput
//! - Increasing message sizes
//! - Network partitioning and recovery

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, PlotConfiguration, Throughput};
use std::time::Duration;
use tokio::runtime::Runtime;

use p2p_network::{P2PConfig, WebRTCP2PNetwork};
use traits::{P2PNetworkLayer, TxInput, TxOutput, UtxoId, UtxoTransaction};

/// Initialize minimal logging
fn init_logging() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Error)
        .is_test(true)
        .try_init();
}

/// Create scaling test configuration
fn create_scaling_config(node_id: &str, port: u16, max_peers: usize) -> P2PConfig {
    P2PConfig {
        node_id: node_id.to_string(),
        listen_addr: format!("127.0.0.1:{}", port).parse().unwrap(),
        bootstrap_peers: vec![],
        stun_servers: vec![], // Local testing for consistent results
        max_peers,
        connection_timeout: 10,
        keep_alive_interval: 30,
        debug_mode: false,
    }
}

/// Create test transaction with configurable size
fn create_sized_transaction(id: u64, num_inputs: usize, num_outputs: usize) -> UtxoTransaction {
    let inputs: Vec<_> = (0..num_inputs)
        .map(|i| TxInput {
            utxo_id: UtxoId {
                tx_hash: format!("input_{}_{}", id, i),
                output_index: i as u32,
            },
            redeemer: vec![0u8; 64],
            signature: vec![0u8; 64],
        })
        .collect();

    let outputs: Vec<_> = (0..num_outputs)
        .map(|i| TxOutput {
            value: 1000 + (i as u64),
            script: vec![0u8; 25],
            datum: Some(vec![0u8; 32]),
            datum_hash: Some(format!("datum_{}_{}", id, i)),
        })
        .collect();

    UtxoTransaction {
        hash: format!("scaled_tx_{:08x}", id),
        inputs,
        outputs,
        fee: 100,
        validity_range: Some((id * 1000, (id + 100) * 1000)),
        script_witness: vec![vec![0u8; 128]],
        auxiliary_data: None,
    }
}

/// Benchmark scaling with increasing number of network instances
fn benchmark_peer_scaling(c: &mut Criterion) {
    init_logging();
    
    let mut group = c.benchmark_group("peer_scaling");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(5));
    
    // Test with different numbers of peers
    for num_peers in [2, 5, 10, 20].iter() {
        group.throughput(Throughput::Elements(*num_peers as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_peers),
            num_peers,
            |b, &num_peers| {
                b.iter(|| {
                    let rt = Runtime::new().unwrap();
                    rt.block_on(async {
                        // Create network instances
                        let mut networks = Vec::new();
                        for i in 0..num_peers {
                            let config = create_scaling_config(
                                &format!("peer_{}", i),
                                9500 + i as u16,
                                num_peers * 2, // Allow connections to all peers
                            );
                            let network = WebRTCP2PNetwork::new(config).unwrap();
                            networks.push(network);
                        }
                        
                        // Simulate network activity
                        let tx = create_sized_transaction(1, 2, 3);
                        for network in &networks {
                            let _ = network.broadcast_transaction(&tx).await;
                        }
                        
                        black_box(networks);
                    });
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark transaction throughput scaling
fn benchmark_transaction_throughput_scaling(c: &mut Criterion) {
    init_logging();
    
    let rt = Runtime::new().unwrap();
    let config = create_scaling_config("throughput_node", 9600, 100);
    let network = WebRTCP2PNetwork::new(config).unwrap();
    
    let mut group = c.benchmark_group("transaction_throughput_scaling");
    group.sample_size(15);
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_secs(3));
    group.plot_config(PlotConfiguration::default().summary_scale(criterion::AxisScale::Logarithmic));
    
    // Test with increasing batch sizes
    for batch_size in [1, 10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    rt.block_on(async {
                        let transactions: Vec<_> = (0..batch_size)
                            .map(|i| create_sized_transaction(i as u64, 1, 2))
                            .collect();
                        
                        let start = std::time::Instant::now();
                        for tx in &transactions {
                            let _ = network.broadcast_transaction(tx).await;
                        }
                        let elapsed = start.elapsed();
                        
                        black_box((transactions, elapsed));
                    });
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark message size scaling
fn benchmark_message_size_scaling(c: &mut Criterion) {
    init_logging();
    
    let rt = Runtime::new().unwrap();
    let config = create_scaling_config("size_node", 9700, 50);
    let network = WebRTCP2PNetwork::new(config).unwrap();
    
    let mut group = c.benchmark_group("message_size_scaling");
    group.sample_size(20);
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_secs(2));
    
    // Test with different transaction sizes (inputs/outputs)
    let sizes = vec![(1, 1), (5, 5), (10, 10), (25, 25), (50, 50)];
    
    for (num_inputs, num_outputs) in sizes {
        let param = format!("{}in_{}out", num_inputs, num_outputs);
        group.throughput(Throughput::Elements((num_inputs + num_outputs) as u64));
        group.bench_with_input(
            BenchmarkId::new("transaction_size", &param),
            &(num_inputs, num_outputs),
            |b, &(num_inputs, num_outputs)| {
                b.iter(|| {
                    rt.block_on(async {
                        let tx = create_sized_transaction(1, num_inputs, num_outputs);
                        let serialized = bincode::serialize(&tx).unwrap();
                        let size = serialized.len();
                        
                        let result = network.broadcast_transaction(&tx).await;
                        let _ = black_box((result, size));
                    });
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark concurrent operations scaling
fn benchmark_concurrent_operations_scaling(c: &mut Criterion) {
    init_logging();
    
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_operations_scaling");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));
    
    // Test with different levels of concurrency
    for concurrency in [1, 5, 10, 25, 50].iter() {
        group.throughput(Throughput::Elements(*concurrency as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            concurrency,
            |b, &concurrency| {
                b.iter(|| {
                    rt.block_on(async {
                        let config = create_scaling_config("concurrent_node", 9800, 100);
                        let network = WebRTCP2PNetwork::new(config).unwrap();
                        
                        // Create transactions
                        let transactions: Vec<_> = (0..concurrency)
                            .map(|i| create_sized_transaction(i as u64, 2, 3))
                            .collect();
                        
                        // Spawn concurrent broadcasts
                        let mut handles = Vec::new();
                        for tx in transactions {
                            let net = network.clone();
                            let handle = tokio::spawn(async move {
                                net.broadcast_transaction(&tx).await
                            });
                            handles.push(handle);
                        }
                        
                        // Wait for all to complete
                        let results = futures::future::join_all(handles).await;
                        black_box(results);
                    });
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark network partitioning and recovery
fn benchmark_network_resilience(c: &mut Criterion) {
    init_logging();
    
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("network_resilience");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(1000));
    group.measurement_time(Duration::from_secs(5));
    
    group.bench_function("partition_recovery", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Create 3 partitions of networks
                let mut partitions = Vec::new();
                for partition in 0..3 {
                    let mut networks = Vec::new();
                    for i in 0..3 {
                        let config = create_scaling_config(
                            &format!("part{}_node{}", partition, i),
                            9900 + partition * 10 + i as u16,
                            20,
                        );
                        let network = WebRTCP2PNetwork::new(config).unwrap();
                        networks.push(network);
                    }
                    partitions.push(networks);
                }
                
                // Simulate partition healing by broadcasting across partitions
                let tx = create_sized_transaction(1, 5, 5);
                let mut broadcast_count = 0;
                
                for partition in &partitions {
                    for network in partition {
                        let _ = network.broadcast_transaction(&tx).await;
                        broadcast_count += 1;
                    }
                }
                
                black_box((partitions, broadcast_count));
            });
        });
    });
    
    group.finish();
}

/// Benchmark memory usage scaling
fn benchmark_memory_scaling(c: &mut Criterion) {
    init_logging();
    
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("memory_scaling");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_secs(2));
    
    // Test memory usage with increasing number of stored transactions
    for num_transactions in [100, 500, 1000, 5000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_transactions),
            num_transactions,
            |b, &num_transactions| {
                b.iter(|| {
                    rt.block_on(async {
                        let config = create_scaling_config("memory_node", 10000, 50);
                        let network = WebRTCP2PNetwork::new(config).unwrap();
                        
                        // Create and broadcast many transactions
                        let mut total_size = 0;
                        for i in 0..num_transactions {
                            let tx = create_sized_transaction(i as u64, 2, 3);
                            let serialized = bincode::serialize(&tx).unwrap();
                            total_size += serialized.len();
                            let _ = network.broadcast_transaction(&tx).await;
                        }
                        
                        // Get network statistics
                        let stats = network.get_network_stats();
                        
                        black_box((stats, total_size));
                    });
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_peer_scaling,
    benchmark_transaction_throughput_scaling,
    benchmark_message_size_scaling,
    benchmark_concurrent_operations_scaling,
    benchmark_network_resilience,
    benchmark_memory_scaling
);

criterion_main!(benches);
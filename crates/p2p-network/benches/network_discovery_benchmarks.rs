//! Network Discovery and Joining Benchmarks
//!
//! Benchmarks that measure how efficiently new nodes can discover
//! and join existing P2P networks.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::{Duration, Instant};
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

/// Create test configuration for discovery benchmarks
fn create_discovery_config(node_id: &str, port: u16, bootstrap_peers: Vec<String>) -> P2PConfig {
    P2PConfig {
        node_id: node_id.to_string(),
        listen_addr: format!("127.0.0.1:{}", port).parse().unwrap(),
        bootstrap_peers,
        stun_servers: vec![], // Local testing only
        max_peers: 20,
        connection_timeout: 5,
        keep_alive_interval: 30,
        debug_mode: false,
    }
}

/// Create test transaction
fn create_test_transaction(id: u64) -> UtxoTransaction {
    UtxoTransaction {
        hash: format!("discovery_tx_{}", id),
        inputs: vec![TxInput {
            utxo_id: UtxoId {
                tx_hash: format!("input_{}", id),
                output_index: 0,
            },
            redeemer: vec![0u8; 32],
            signature: vec![0u8; 64],
        }],
        outputs: vec![TxOutput {
            value: 1000,
            script: vec![0u8; 25],
            datum: Some(vec![0u8; 32]),
            datum_hash: Some(format!("hash_{}", id)),
        }],
        fee: 10,
        validity_range: Some((0, 10000)),
        script_witness: vec![],
        auxiliary_data: None,
    }
}

/// Benchmark network bootstrap time
fn benchmark_network_bootstrap(c: &mut Criterion) {
    init_logging();
    
    let mut group = c.benchmark_group("network_bootstrap");
    group.sample_size(20);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));
    
    group.bench_function("single_node_bootstrap", |b| {
        b.iter(|| {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let config = create_discovery_config("bootstrap", 11000, vec![]);
                let network = WebRTCP2PNetwork::new(config).unwrap();
                
                let start = Instant::now();
                let net_clone = network.clone();
                tokio::spawn(async move {
                    let _ = net_clone.start().await;
                });
                
                // Wait for network to be ready
                tokio::time::sleep(Duration::from_millis(50)).await;
                let bootstrap_time = start.elapsed();
                
                network.shutdown().await.unwrap();
                black_box(bootstrap_time);
            });
        });
    });
    
    group.finish();
}

/// Benchmark peer discovery efficiency
fn benchmark_peer_discovery(c: &mut Criterion) {
    init_logging();
    
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("peer_discovery");
    group.sample_size(15);
    group.warm_up_time(Duration::from_millis(1000));
    group.measurement_time(Duration::from_secs(5));
    
    // Test discovery with different network sizes
    for network_size in [2, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_peers", network_size)),
            network_size,
            |b, &network_size| {
                b.iter(|| {
                    rt.block_on(async {
                        // Create initial network
                        let mut networks = Vec::new();
                        
                        // Bootstrap node
                        let config = create_discovery_config("peer_0", 11100, vec![]);
                        let network = WebRTCP2PNetwork::new(config).unwrap();
                        let net_clone = network.clone();
                        tokio::spawn(async move {
                            let _ = net_clone.start().await;
                        });
                        networks.push(network);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        
                        // Add peers
                        for i in 1..network_size {
                            let config = create_discovery_config(
                                &format!("peer_{}", i),
                                11100 + i as u16,
                                vec!["127.0.0.1:11100".to_string()],
                            );
                            let network = WebRTCP2PNetwork::new(config).unwrap();
                            let net_clone = network.clone();
                            tokio::spawn(async move {
                                let _ = net_clone.start().await;
                            });
                            networks.push(network);
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                        
                        // Measure discovery time for new node
                        let new_config = create_discovery_config(
                            "discoverer",
                            11100 + network_size as u16,
                            vec!["127.0.0.1:11100".to_string()],
                        );
                        let new_network = WebRTCP2PNetwork::new(new_config).unwrap();
                        
                        let discovery_start = Instant::now();
                        let net_clone = new_network.clone();
                        tokio::spawn(async move {
                            let _ = net_clone.start().await;
                        });
                        
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        
                        let peers = new_network.get_connected_peers().await;
                        let discovery_time = discovery_start.elapsed();
                        
                        // Cleanup
                        new_network.shutdown().await.unwrap();
                        for net in networks {
                            net.shutdown().await.unwrap();
                        }
                        
                        black_box((peers.len(), discovery_time));
                    });
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark network join latency
fn benchmark_network_join_latency(c: &mut Criterion) {
    init_logging();
    
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("network_join_latency");
    group.sample_size(20);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));
    
    group.bench_function("join_existing_network", |b| {
        // Setup persistent network
        let bootstrap = rt.block_on(async {
            let config = create_discovery_config("persistent_bootstrap", 11200, vec![]);
            let network = WebRTCP2PNetwork::new(config).unwrap();
            let net_clone = network.clone();
            tokio::spawn(async move {
                let _ = net_clone.start().await;
            });
            tokio::time::sleep(Duration::from_millis(200)).await;
            network
        });
        
        let mut node_counter = 0;
        
        b.iter(|| {
            rt.block_on(async {
                node_counter += 1;
                let config = create_discovery_config(
                    &format!("joiner_{}", node_counter),
                    11300 + node_counter as u16,
                    vec!["127.0.0.1:11200".to_string()],
                );
                
                let network = WebRTCP2PNetwork::new(config).unwrap();
                
                let join_start = Instant::now();
                let net_clone = network.clone();
                tokio::spawn(async move {
                    let _ = net_clone.start().await;
                });
                
                // Wait for network to stabilize
                tokio::time::sleep(Duration::from_millis(100)).await;
                
                // Verify can broadcast
                let tx = create_test_transaction(node_counter);
                let broadcast_result = network.broadcast_transaction(&tx).await;
                let join_time = join_start.elapsed();
                
                network.shutdown().await.unwrap();
                
                black_box((broadcast_result.is_ok(), join_time));
            });
        });
        
        // Cleanup
        rt.block_on(async {
            bootstrap.shutdown().await.unwrap();
        });
    });
    
    group.finish();
}

/// Benchmark network propagation speed
fn benchmark_network_propagation(c: &mut Criterion) {
    init_logging();
    
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("network_propagation");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(1000));
    group.measurement_time(Duration::from_secs(5));
    
    group.bench_function("transaction_propagation", |b| {
        // Setup network
        let networks = rt.block_on(async {
            let mut nets = Vec::new();
            
            // Create 5-node network
            for i in 0..5 {
                let config = create_discovery_config(
                    &format!("prop_node_{}", i),
                    11400 + i,
                    if i == 0 { vec![] } else { vec!["127.0.0.1:11400".to_string()] },
                );
                let network = WebRTCP2PNetwork::new(config).unwrap();
                let net_clone = network.clone();
                tokio::spawn(async move {
                    let _ = net_clone.start().await;
                });
                nets.push(network);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            
            nets
        });
        
        let mut tx_counter = 0;
        
        b.iter(|| {
            rt.block_on(async {
                tx_counter += 1;
                let tx = create_test_transaction(tx_counter);
                
                let propagation_start = Instant::now();
                
                // Broadcast from first node
                let _ = networks[0].broadcast_transaction(&tx).await;
                
                // Measure propagation through network
                let mut propagation_times = Vec::new();
                for (i, network) in networks.iter().enumerate().skip(1) {
                    let stats = network.get_network_stats();
                    propagation_times.push((i, propagation_start.elapsed(), stats.messages_received));
                }
                
                black_box(propagation_times);
            });
        });
        
        // Cleanup
        rt.block_on(async {
            for net in networks {
                net.shutdown().await.unwrap();
            }
        });
    });
    
    group.finish();
}

/// Benchmark discovery under load
fn benchmark_discovery_under_load(c: &mut Criterion) {
    init_logging();
    
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("discovery_under_load");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(1000));
    group.measurement_time(Duration::from_secs(5));
    
    group.bench_function("join_busy_network", |b| {
        // Setup busy network
        let (networks, _load_task) = rt.block_on(async {
            let mut nets = Vec::new();
            
            // Create initial network
            for i in 0..3 {
                let config = create_discovery_config(
                    &format!("busy_node_{}", i),
                    11500 + i,
                    if i == 0 { vec![] } else { vec!["127.0.0.1:11500".to_string()] },
                );
                let network = WebRTCP2PNetwork::new(config).unwrap();
                let net_clone = network.clone();
                tokio::spawn(async move {
                    let _ = net_clone.start().await;
                });
                nets.push(network);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            
            // Generate load
            let nets_clone = nets.clone();
            let load_task = tokio::spawn(async move {
                let mut counter = 0;
                loop {
                    counter += 1;
                    let tx = create_test_transaction(10000 + counter);
                    let _ = nets_clone[(counter % 3) as usize].broadcast_transaction(&tx).await;
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            });
            
            (nets, load_task)
        });
        
        let mut joiner_counter = 0;
        
        b.iter(|| {
            rt.block_on(async {
                joiner_counter += 1;
                let config = create_discovery_config(
                    &format!("busy_joiner_{}", joiner_counter),
                    11600 + joiner_counter as u16,
                    vec!["127.0.0.1:11500".to_string()],
                );
                
                let network = WebRTCP2PNetwork::new(config).unwrap();
                
                let join_start = Instant::now();
                let net_clone = network.clone();
                tokio::spawn(async move {
                    let _ = net_clone.start().await;
                });
                
                tokio::time::sleep(Duration::from_millis(200)).await;
                
                let peers = network.get_connected_peers().await;
                let stats = network.get_network_stats();
                let join_time = join_start.elapsed();
                
                network.shutdown().await.unwrap();
                
                black_box((peers.len(), stats.messages_received, join_time));
            });
        });
        
        // Cleanup
        rt.block_on(async {
            for net in networks {
                net.shutdown().await.unwrap();
            }
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_network_bootstrap,
    benchmark_peer_discovery,
    benchmark_network_join_latency,
    benchmark_network_propagation,
    benchmark_discovery_under_load
);

criterion_main!(benches);
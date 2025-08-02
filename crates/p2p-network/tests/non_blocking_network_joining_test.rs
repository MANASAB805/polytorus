//! Non-blocking Network Joining Test
//!
//! Tests that verify new nodes can successfully join an existing network
//! without using blocking start() method.

use anyhow::Result;
use log::info;

use p2p_network::{P2PConfig, WebRTCP2PNetwork};
use traits::{P2PNetworkLayer, TxInput, TxOutput, UtxoId, UtxoTransaction};

/// Initialize test logging with detailed output
fn init_test_logging() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();
}

/// Create a test transaction with unique ID
fn create_test_transaction(id: u64, from_node: &str) -> UtxoTransaction {
    UtxoTransaction {
        hash: format!("tx_{}_from_{}", id, from_node),
        inputs: vec![TxInput {
            utxo_id: UtxoId {
                tx_hash: format!("input_{}", id),
                output_index: 0,
            },
            redeemer: b"test_redeemer".to_vec(),
            signature: b"test_signature".to_vec(),
        }],
        outputs: vec![TxOutput {
            value: 1000 + id,
            script: vec![],
            datum: Some(format!("data_from_{}", from_node).into_bytes()),
            datum_hash: Some(format!("hash_{}", id)),
        }],
        fee: 100,
        validity_range: Some((0, 10000)),
        script_witness: vec![],
        auxiliary_data: None,
    }
}

#[tokio::test]
async fn test_non_blocking_network_joining_setup() -> Result<()> {
    init_test_logging();
    info!("=== Testing non-blocking network joining setup ===");

    // Create initial "existing" network node
    let existing_config = P2PConfig {
        node_id: "existing_node".to_string(),
        listen_addr: "127.0.0.1:11001".parse().unwrap(),
        bootstrap_peers: vec![],
        stun_servers: vec![], // No STUN for non-blocking test
        max_peers: 10,
        connection_timeout: 5,
        keep_alive_interval: 30,
        debug_mode: false,
    };

    let existing_network = WebRTCP2PNetwork::new(existing_config)?;
    info!("Created existing network node");

    // Test existing network functionality
    let existing_stats = existing_network.get_network_stats();
    info!("Existing network stats: connections={}, total={}", 
          existing_stats.active_connections, existing_stats.total_connections);

    // Create new node that wants to join
    let joining_config = P2PConfig {
        node_id: "joining_node".to_string(),
        listen_addr: "127.0.0.1:11002".parse().unwrap(),
        bootstrap_peers: vec!["127.0.0.1:11001".to_string()], // Bootstrap to existing
        stun_servers: vec![],
        max_peers: 10,
        connection_timeout: 5,
        keep_alive_interval: 30,
        debug_mode: false,
    };

    let joining_network = WebRTCP2PNetwork::new(joining_config)?;
    info!("Created joining network node");

    // Test joining network functionality
    let joining_stats = joining_network.get_network_stats();
    info!("Joining network stats: connections={}, total={}", 
          joining_stats.active_connections, joining_stats.total_connections);

    // Test both networks can handle transactions
    let tx_existing = create_test_transaction(1, "existing_node");
    let tx_joining = create_test_transaction(2, "joining_node");

    let result_existing = existing_network.broadcast_transaction(&tx_existing).await;
    let result_joining = joining_network.broadcast_transaction(&tx_joining).await;

    assert!(result_existing.is_ok(), "Existing network should handle transactions");
    assert!(result_joining.is_ok(), "Joining network should handle transactions");
    
    info!("Both networks can handle transactions");

    // Test discovery capabilities
    let discovered_existing = existing_network.get_discovered_peers().await;
    let discovered_joining = joining_network.get_discovered_peers().await;

    info!("Existing network discovered {} peers", discovered_existing.len());
    info!("Joining network discovered {} peers", discovered_joining.len());

    // Test peer connection capabilities (without actual connections)
    let connected_existing = existing_network.get_connected_peers().await;
    let connected_joining = joining_network.get_connected_peers().await;

    info!("Existing network connected to {} peers", connected_existing.len());
    info!("Joining network connected to {} peers", connected_joining.len());

    // Test data request capabilities
    let data_request_existing = existing_network.request_blockchain_data(
        "transaction".to_string(),
        tx_joining.hash.clone()
    ).await;
    
    let data_request_joining = joining_network.request_blockchain_data(
        "transaction".to_string(),
        tx_existing.hash.clone()
    ).await;

    info!("Data request results - Existing: {:?}, Joining: {:?}", 
          data_request_existing.is_ok(), data_request_joining.is_ok());

    // Cleanup
    existing_network.shutdown().await?;
    joining_network.shutdown().await?;
    
    info!("Non-blocking network joining setup test completed");
    Ok(())
}

#[tokio::test]
async fn test_multiple_nodes_joining_sequence() -> Result<()> {
    init_test_logging();
    info!("Testing multiple nodes joining in sequence");

    // Create bootstrap node
    let bootstrap_config = P2PConfig {
        node_id: "bootstrap".to_string(),
        listen_addr: "127.0.0.1:11010".parse().unwrap(),
        bootstrap_peers: vec![],
        stun_servers: vec![],
        max_peers: 20,
        connection_timeout: 5,
        keep_alive_interval: 30,
        debug_mode: false,
    };

    let bootstrap_network = WebRTCP2PNetwork::new(bootstrap_config)?;
    let mut networks = vec![bootstrap_network];
    
    info!("Created bootstrap network");

    // Create multiple joining nodes
    for i in 1..=5 {
        let config = P2PConfig {
            node_id: format!("joining_node_{}", i),
            listen_addr: format!("127.0.0.1:{}", 11010 + i).parse().unwrap(),
            bootstrap_peers: vec!["127.0.0.1:11010".to_string()],
            stun_servers: vec![],
            max_peers: 20,
            connection_timeout: 5,
            keep_alive_interval: 30,
            debug_mode: false,
        };
        
        let network = WebRTCP2PNetwork::new(config)?;
        networks.push(network);
        info!("Created joining node {}", i);
    }

    // Test all networks can handle transactions
    let mut successful_broadcasts = 0;
    for (i, network) in networks.iter().enumerate() {
        let tx = create_test_transaction(100 + i as u64, &format!("node_{}", i));
        if network.broadcast_transaction(&tx).await.is_ok() {
            successful_broadcasts += 1;
        }
    }
    
    assert_eq!(successful_broadcasts, 6, "All networks should handle transactions");
    info!("All {} networks can handle transactions", successful_broadcasts);

    // Test adaptive capabilities on all networks
    let tx_adaptive = create_test_transaction(200, "adaptive_test");
    let mut adaptive_results = 0;
    
    for (i, network) in networks.iter().enumerate() {
        if network.adaptive_broadcast_transaction(&tx_adaptive).await.is_ok() {
            adaptive_results += 1;
        }
        
        let adaptive_stats = network.get_adaptive_network_stats().await;
        info!("Network {} adaptive stats - Discovered: {}, DHT: {}, Efficiency: {:.2}", 
              i, adaptive_stats.discovered_peers_count, 
              adaptive_stats.dht_nodes_count, adaptive_stats.discovery_efficiency);
    }
    
    assert_eq!(adaptive_results, 6, "All networks should support adaptive broadcast");

    // Test network statistics across all nodes
    for (i, network) in networks.iter().enumerate() {
        let stats = network.get_network_stats();
        let discovered = network.get_discovered_peers().await;
        let connected = network.get_connected_peers().await;
        
        info!("Network {} final stats - Messages: {}, Discovered: {}, Connected: {}", 
              i, stats.messages_sent, discovered.len(), connected.len());
    }

    // Cleanup all networks
    for (i, network) in networks.into_iter().enumerate() {
        network.shutdown().await?;
        info!("Network {} cleanup complete", i);
    }
    
    info!("Multiple nodes joining sequence test completed");
    Ok(())
}

#[tokio::test]
async fn test_network_joining_with_failures() -> Result<()> {
    init_test_logging();
    info!("Testing network joining with simulated failures");

    // Create initial network
    let mut networks = Vec::new();
    
    for i in 0..3 {
        let config = P2PConfig {
            node_id: format!("failure_test_node_{}", i),
            listen_addr: format!("127.0.0.1:{}", 11020 + i).parse().unwrap(),
            bootstrap_peers: if i == 0 {
                vec![]
            } else {
                vec!["127.0.0.1:11020".to_string()]
            },
            stun_servers: vec![],
            max_peers: 10,
            connection_timeout: 5,
            keep_alive_interval: 30,
            debug_mode: false,
        };
        
        let network = WebRTCP2PNetwork::new(config)?;
        networks.push(network);
    }
    
    info!("Created {} networks for failure testing", networks.len());

    // Test all networks initially work
    for (i, network) in networks.iter().enumerate() {
        let tx = create_test_transaction(300 + i as u64, &format!("failure_node_{}", i));
        let result = network.broadcast_transaction(&tx).await;
        assert!(result.is_ok(), "Network {} should be functional initially", i);
    }

    // Simulate failure by shutting down bootstrap node
    info!("Simulating bootstrap node failure");
    networks[0].shutdown().await?;
    let failed_node = networks.remove(0);
    drop(failed_node);

    // Test remaining networks can still function
    for (i, network) in networks.iter().enumerate() {
        let tx = create_test_transaction(400 + i as u64, &format!("surviving_node_{}", i));
        let result = network.broadcast_transaction(&tx).await;
        assert!(result.is_ok(), "Surviving network {} should still work", i);
        
        let stats = network.get_network_stats();
        info!("Surviving network {} stats: messages={}, connections={}", 
              i, stats.messages_sent, stats.total_connections);
    }

    // Add new node to replace failed bootstrap
    info!("Adding replacement node");
    let replacement_config = P2PConfig {
        node_id: "replacement_node".to_string(),
        listen_addr: "127.0.0.1:11030".parse().unwrap(),
        bootstrap_peers: vec![
            "127.0.0.1:11021".to_string(),
            "127.0.0.1:11022".to_string(),
        ],
        stun_servers: vec![],
        max_peers: 10,
        connection_timeout: 5,
        keep_alive_interval: 30,
        debug_mode: false,
    };
    
    let replacement_network = WebRTCP2PNetwork::new(replacement_config)?;
    networks.push(replacement_network);

    // Test all networks (including replacement) work
    let mut working_networks = 0;
    for (i, network) in networks.iter().enumerate() {
        let tx = create_test_transaction(500 + i as u64, &format!("final_node_{}", i));
        if network.broadcast_transaction(&tx).await.is_ok() {
            working_networks += 1;
        }
        
        let discovered = network.get_discovered_peers().await;
        let adaptive_stats = network.get_adaptive_network_stats().await;
        info!("Final network {} - Discovered: {}, DHT: {}, Working: {}", 
              i, discovered.len(), adaptive_stats.dht_nodes_count, 
              working_networks <= i + 1);
    }
    
    assert_eq!(working_networks, networks.len(), "All remaining networks should work");
    info!("Network recovery successful: {}/{} networks working", 
          working_networks, networks.len());

    // Cleanup
    for (i, network) in networks.into_iter().enumerate() {
        network.shutdown().await?;
        info!("Final cleanup network {} complete", i);
    }
    
    info!("Network joining with failures test completed");
    Ok(())
}
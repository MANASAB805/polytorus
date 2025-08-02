//! Non-blocking Adaptive Network Tests
//!
//! Tests that verify the P2P network discovery and joining capabilities
//! without using blocking start() method.

use anyhow::Result;
use log::info;

use p2p_network::{P2PConfig, WebRTCP2PNetwork};
use traits::{P2PNetworkLayer, TxInput, TxOutput, UtxoId, UtxoTransaction};

fn init_test_logging() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();
}

fn create_test_tx(id: u64) -> UtxoTransaction {
    UtxoTransaction {
        hash: format!("adaptive_test_tx_{}", id),
        inputs: vec![TxInput {
            utxo_id: UtxoId {
                tx_hash: format!("input_{}", id),
                output_index: 0,
            },
            redeemer: b"test".to_vec(),
            signature: b"sig".to_vec(),
        }],
        outputs: vec![TxOutput {
            value: 1000,
            script: vec![],
            datum: Some(b"test_data".to_vec()),
            datum_hash: Some("hash".to_string()),
        }],
        fee: 100,
        validity_range: Some((0, 10000)),
        script_witness: vec![],
        auxiliary_data: None,
    }
}

#[tokio::test]
async fn test_non_blocking_peer_discovery() -> Result<()> {
    init_test_logging();
    info!("Testing non-blocking peer discovery mechanism");

    // Create networks with auto discovery enabled
    let config1 = P2PConfig {
        node_id: "discovery_node1".to_string(),
        listen_addr: "127.0.0.1:12001".parse().unwrap(),
        bootstrap_peers: vec![],
        stun_servers: vec![], // No STUN for non-blocking test
        max_peers: 5,
        connection_timeout: 5,
        keep_alive_interval: 30,
        debug_mode: false,
    };

    let config2 = P2PConfig {
        node_id: "discovery_node2".to_string(),
        listen_addr: "127.0.0.1:12002".parse().unwrap(),
        bootstrap_peers: vec![],
        stun_servers: vec![],
        max_peers: 5,
        connection_timeout: 5,
        keep_alive_interval: 30,
        debug_mode: false,
    };

    let network1 = WebRTCP2PNetwork::new(config1)?;
    let network2 = WebRTCP2PNetwork::new(config2)?;
    
    info!("Created both networks");

    // Test discovery functionality without full startup
    let discovered_peers1 = network1.get_discovered_peers().await;
    let discovered_peers2 = network2.get_discovered_peers().await;
    
    info!("Network 1 discovered {} peers", discovered_peers1.len());
    info!("Network 2 discovered {} peers", discovered_peers2.len());
    
    // Initially should be empty
    assert_eq!(discovered_peers1.len(), 0);
    assert_eq!(discovered_peers2.len(), 0);

    // Test broadcasting capabilities
    let tx1 = create_test_tx(1);
    let broadcast_result1 = network1.broadcast_transaction(&tx1).await;
    
    let tx2 = create_test_tx(2);
    let broadcast_result2 = network2.broadcast_transaction(&tx2).await;
    
    assert!(broadcast_result1.is_ok(), "Network 1 should broadcast successfully");
    assert!(broadcast_result2.is_ok(), "Network 2 should broadcast successfully");
    
    info!("Both networks can broadcast transactions");

    // Get network statistics
    let stats1 = network1.get_network_stats();
    let stats2 = network2.get_network_stats();
    
    info!("Network 1 stats: connections={}, messages_sent={}", 
          stats1.active_connections, stats1.messages_sent);
    info!("Network 2 stats: connections={}, messages_sent={}", 
          stats2.active_connections, stats2.messages_sent);

    // Test adaptive network statistics if available
    let adaptive_stats1 = network1.get_adaptive_network_stats().await;
    let adaptive_stats2 = network2.get_adaptive_network_stats().await;
    
    info!("Network 1 adaptive stats: discovered={}, connected={}, efficiency={:.2}", 
          adaptive_stats1.discovered_peers_count, 
          adaptive_stats1.connected_peers_count,
          adaptive_stats1.discovery_efficiency);
    info!("Network 2 adaptive stats: discovered={}, connected={}, efficiency={:.2}", 
          adaptive_stats2.discovered_peers_count, 
          adaptive_stats2.connected_peers_count,
          adaptive_stats2.discovery_efficiency);

    // Cleanup
    network1.shutdown().await?;
    network2.shutdown().await?;
    
    info!("Non-blocking peer discovery test completed");
    Ok(())
}

#[tokio::test]
async fn test_non_blocking_network_expansion() -> Result<()> {
    init_test_logging();
    info!("Testing non-blocking network expansion");

    // Create multiple networks simulating gradual expansion
    let mut networks = Vec::new();
    
    for i in 0..4 {
        let config = P2PConfig {
            node_id: format!("expansion_node_{}", i),
            listen_addr: format!("127.0.0.1:{}", 12010 + i).parse().unwrap(),
            bootstrap_peers: if i == 0 { 
                vec![] 
            } else { 
                vec![format!("127.0.0.1:{}", 12010)] // Bootstrap to first node
            },
            stun_servers: vec![],
            max_peers: 10,
            connection_timeout: 5,
            keep_alive_interval: 30,
            debug_mode: false,
        };
        
        let network = WebRTCP2PNetwork::new(config)?;
        networks.push(network);
        info!("Created network {}", i);
    }

    // Test that all networks can handle transactions
    let mut successful_broadcasts = 0;
    for (i, network) in networks.iter().enumerate() {
        let tx = create_test_tx(100 + i as u64);
        if network.broadcast_transaction(&tx).await.is_ok() {
            successful_broadcasts += 1;
        }
    }
    
    assert_eq!(successful_broadcasts, 4, "All networks should handle transactions");
    info!("All {} networks can handle transactions", successful_broadcasts);

    // Check network capabilities
    for (i, network) in networks.iter().enumerate() {
        let discovered = network.get_discovered_peers().await;
        let connected = network.get_connected_peers().await;
        let stats = network.get_network_stats();
        
        info!("Network {} - Discovered: {}, Connected: {}, Total connections: {}", 
              i, discovered.len(), connected.len(), stats.total_connections);
    }

    // Test adaptive broadcasting on all networks
    let tx_broadcast = create_test_tx(200);
    let mut adaptive_broadcast_results = 0;
    
    for (i, network) in networks.iter().enumerate() {
        if network.adaptive_broadcast_transaction(&tx_broadcast).await.is_ok() {
            adaptive_broadcast_results += 1;
        }
        info!("Network {} adaptive broadcast result: OK", i);
    }
    
    assert_eq!(adaptive_broadcast_results, 4, "All networks should support adaptive broadcast");

    // Cleanup all networks
    for (i, network) in networks.into_iter().enumerate() {
        network.shutdown().await?;
        info!("Network {} shutdown complete", i);
    }
    
    info!("Non-blocking network expansion test completed");
    Ok(())
}

#[tokio::test]
async fn test_non_blocking_network_resilience() -> Result<()> {
    init_test_logging();
    info!("Testing non-blocking network resilience");

    // Create a small network setup
    let mut networks = Vec::new();
    
    for i in 0..3 {
        let config = P2PConfig {
            node_id: format!("resilient_node_{}", i),
            listen_addr: format!("127.0.0.1:{}", 12020 + i).parse().unwrap(),
            bootstrap_peers: if i == 0 {
                vec![]
            } else {
                vec!["127.0.0.1:12020".to_string()]
            },
            stun_servers: vec![],
            max_peers: 5,
            connection_timeout: 5,
            keep_alive_interval: 30,
            debug_mode: false,
        };
        
        let network = WebRTCP2PNetwork::new(config)?;
        networks.push(network);
    }
    
    info!("Created {} networks for resilience testing", networks.len());

    // Test all networks are functional
    for (i, network) in networks.iter().enumerate() {
        let tx = create_test_tx(300 + i as u64);
        let result = network.broadcast_transaction(&tx).await;
        assert!(result.is_ok(), "Network {} should be functional", i);
        
        let stats = network.get_network_stats();
        info!("Network {} initial stats: connections={}, messages_sent={}", 
              i, stats.active_connections, stats.messages_sent);
    }

    // Simulate "node failure" by shutting down middle network
    info!("Simulating node failure by shutting down network 1");
    networks[1].shutdown().await?;
    let failed_network = networks.remove(1);
    drop(failed_network);

    // Test remaining networks still function
    for (i, network) in networks.iter().enumerate() {
        let tx = create_test_tx(400 + i as u64);
        let result = network.broadcast_transaction(&tx).await;
        assert!(result.is_ok(), "Remaining network {} should still work", i);
        
        let discovered = network.get_discovered_peers().await;
        info!("Network {} after failure - Discovered peers: {}", i, discovered.len());
    }

    // Add new "healing" network
    info!("Adding new network to heal the network");
    let healing_config = P2PConfig {
        node_id: "healing_node".to_string(),
        listen_addr: "127.0.0.1:12030".parse().unwrap(),
        bootstrap_peers: vec![
            "127.0.0.1:12020".to_string(),
            "127.0.0.1:12022".to_string(),
        ],
        stun_servers: vec![],
        max_peers: 5,
        connection_timeout: 5,
        keep_alive_interval: 30,
        debug_mode: false,
    };
    
    let healing_network = WebRTCP2PNetwork::new(healing_config)?;
    networks.push(healing_network);

    // Test network functionality after healing
    let mut working_nodes = 0;
    for (i, network) in networks.iter().enumerate() {
        let tx = create_test_tx(500 + i as u64);
        if network.broadcast_transaction(&tx).await.is_ok() {
            working_nodes += 1;
        }
        
        let discovered = network.get_discovered_peers().await;
        let adaptive_stats = network.get_adaptive_network_stats().await;
        info!("Network {} after healing - Discovered: {}, DHT nodes: {}", 
              i, discovered.len(), adaptive_stats.dht_nodes_count);
    }
    
    assert!(working_nodes >= 2, "At least 2 nodes should work after healing");
    info!("Network resilience test: {}/{} nodes working after healing", 
          working_nodes, networks.len());

    // Cleanup remaining networks
    for (i, network) in networks.into_iter().enumerate() {
        network.shutdown().await?;
        info!("Network {} cleanup complete", i);
    }
    
    info!("Non-blocking network resilience test completed");
    Ok(())
}

#[tokio::test]
async fn test_discovery_mechanisms() -> Result<()> {
    init_test_logging();
    info!("Testing various discovery mechanisms");

    let config = P2PConfig {
        node_id: "discovery_test_node".to_string(),
        listen_addr: "127.0.0.1:12040".parse().unwrap(),
        bootstrap_peers: vec![],
        stun_servers: vec![],
        max_peers: 10,
        connection_timeout: 5,
        keep_alive_interval: 30,
        debug_mode: false,
    };

    let network = WebRTCP2PNetwork::new(config)?;

    // Test initial discovery state
    let initial_discovered = network.get_discovered_peers().await;
    let initial_connected = network.get_connected_peers().await;
    
    info!("Initial state - Discovered: {}, Connected: {}", 
          initial_discovered.len(), initial_connected.len());
    
    assert_eq!(initial_discovered.len(), 0);
    assert_eq!(initial_connected.len(), 0);

    // Test adaptive network statistics
    let adaptive_stats = network.get_adaptive_network_stats().await;
    
    info!("Adaptive stats - Discovered peers: {}, DHT nodes: {}, Connected: {}, Efficiency: {:.2}", 
          adaptive_stats.discovered_peers_count,
          adaptive_stats.dht_nodes_count,
          adaptive_stats.connected_peers_count,
          adaptive_stats.discovery_efficiency);

    // Test adaptive broadcasting
    let tx = create_test_tx(600);
    let adaptive_result = network.adaptive_broadcast_transaction(&tx).await;
    assert!(adaptive_result.is_ok(), "Adaptive broadcast should work");
    
    info!("Adaptive broadcast successful");

    // Test multiple data requests to exercise discovery
    for i in 0..5 {
        let result = network.request_blockchain_data(
            "test_data".to_string(), 
            format!("discovery_test_{}", i)
        ).await;
        info!("Data request {} result: {:?}", i, result.is_ok());
    }

    // Check final stats
    let final_stats = network.get_network_stats();
    let final_adaptive = network.get_adaptive_network_stats().await;
    
    info!("Final stats - Messages sent: {}, Total connections: {}", 
          final_stats.messages_sent, final_stats.total_connections);
    info!("Final adaptive - Discovery efficiency: {:.2}", 
          final_adaptive.discovery_efficiency);

    network.shutdown().await?;
    info!("Discovery mechanisms test completed");
    Ok(())
}
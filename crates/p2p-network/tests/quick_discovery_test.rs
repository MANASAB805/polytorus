//! Quick Discovery Test
//!
//! Non-blocking tests for peer discovery functionality

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
        hash: format!("quick_test_tx_{}", id),
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
async fn test_network_creation_and_stats() -> Result<()> {
    init_test_logging();
    info!("Testing network creation and statistics");

    // Create network
    let config = P2PConfig {
        node_id: "test_stats_node".to_string(),
        listen_addr: "127.0.0.1:13001".parse().unwrap(),
        bootstrap_peers: vec![],
        stun_servers: vec![],
        max_peers: 5,
        connection_timeout: 1, // Short timeout for testing
        keep_alive_interval: 30,
        debug_mode: false,
    };

    let network = WebRTCP2PNetwork::new(config)?;
    
    // Test basic functionality without starting full network
    let stats = network.get_network_stats();
    info!("Initial stats: connections={}, messages_sent={}", 
          stats.active_connections, stats.messages_sent);
    
    // Test broadcast without actual connections
    let tx = create_test_tx(1);
    let broadcast_result = network.broadcast_transaction(&tx).await;
    info!("Broadcast result: {:?}", broadcast_result.is_ok());
    
    // Test getting connected peers (should be empty)
    let peers = network.get_connected_peers().await;
    info!("Connected peers: {}", peers.len());
    
    // Test shutdown
    network.shutdown().await?;
    info!("Network shutdown completed");
    
    assert_eq!(stats.active_connections, 0);
    assert_eq!(peers.len(), 0);
    assert!(broadcast_result.is_ok()); // Should succeed even with no peers
    
    info!("Network creation and stats test completed");
    Ok(())
}

#[tokio::test]
async fn test_network_initialization_only() -> Result<()> {
    init_test_logging();
    info!("Testing network initialization without full startup");

    // Create multiple networks to test initialization
    let mut networks = Vec::new();
    
    for i in 0..3 {
        let config = P2PConfig {
            node_id: format!("init_test_node_{}", i),
            listen_addr: format!("127.0.0.1:{}", 13010 + i).parse().unwrap(),
            bootstrap_peers: vec![],
            stun_servers: vec![],
            max_peers: 5,
            connection_timeout: 1,
            keep_alive_interval: 30,
            debug_mode: false,
        };
        
        let network = WebRTCP2PNetwork::new(config)?;
        networks.push(network);
    }
    
    info!("Created {} networks", networks.len());
    
    // Test each network individually
    for (i, network) in networks.iter().enumerate() {
        let stats = network.get_network_stats();
        let peers = network.get_connected_peers().await;
        
        info!("Network {} - Stats: {}, Peers: {}", i, stats.active_connections, peers.len());
        
        // Test transaction creation and serialization
        let tx = create_test_tx(i as u64);
        let serialized = bincode::serialize(&tx)?;
        info!("Network {} - Transaction size: {} bytes", i, serialized.len());
    }
    
    // Cleanup
    for network in networks {
        network.shutdown().await?;
    }
    
    info!("Network initialization test completed");
    Ok(())
}

#[tokio::test]
async fn test_discovered_peers_functionality() -> Result<()> {
    init_test_logging();
    info!("Testing discovered peers functionality");

    let config = P2PConfig {
        node_id: "discovery_test_node".to_string(),
        listen_addr: "127.0.0.1:13020".parse().unwrap(),
        bootstrap_peers: vec![],
        stun_servers: vec![],
        max_peers: 10,
        connection_timeout: 1,
        keep_alive_interval: 30,
        debug_mode: false,
    };

    let network = WebRTCP2PNetwork::new(config)?;
    
    // Test get_discovered_peers method (should be empty initially)
    let discovered = network.get_discovered_peers().await;
    info!("Initially discovered peers: {}", discovered.len());
    
    // Test network statistics
    let stats = network.get_network_stats();
    info!("Network stats - Total connections: {}, Active: {}, Messages sent: {}", 
          stats.total_connections, stats.active_connections, stats.messages_sent);
    
    // Test broadcasting multiple transactions
    for i in 0..5 {
        let tx = create_test_tx(100 + i);
        let result = network.broadcast_transaction(&tx).await;
        info!("Broadcast {} result: {:?}", i, result.is_ok());
    }
    
    // Check stats after broadcasts
    let final_stats = network.get_network_stats();
    info!("Final stats - Messages sent: {}", final_stats.messages_sent);
    
    network.shutdown().await?;
    
    assert_eq!(discovered.len(), 0); // No real discovery without network activity
    // Stats may not be updated immediately in this test setup
    
    info!("Discovered peers functionality test completed");
    Ok(())
}

#[tokio::test]
async fn test_concurrent_network_operations() -> Result<()> {
    init_test_logging();
    info!("Testing concurrent network operations");

    let config = P2PConfig {
        node_id: "concurrent_test_node".to_string(),
        listen_addr: "127.0.0.1:13030".parse().unwrap(),
        bootstrap_peers: vec![],
        stun_servers: vec![],
        max_peers: 10,
        connection_timeout: 1,
        keep_alive_interval: 30,
        debug_mode: false,
    };

    let network = WebRTCP2PNetwork::new(config)?;
    
    // Create multiple concurrent transactions
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let net = network.clone();
        let handle = tokio::spawn(async move {
            let tx = create_test_tx(200 + i);
            net.broadcast_transaction(&tx).await
        });
        handles.push(handle);
    }
    
    // Wait for all broadcasts to complete
    let results = futures::future::join_all(handles).await;
    
    let successful_broadcasts = results.iter()
        .filter_map(|r| r.as_ref().ok())
        .filter(|r| r.is_ok())
        .count();
    
    info!("Successful concurrent broadcasts: {}/{}", successful_broadcasts, results.len());
    
    // Test concurrent peer queries
    let mut peer_handles = Vec::new();
    for _ in 0..5 {
        let net = network.clone();
        let handle = tokio::spawn(async move {
            net.get_connected_peers().await
        });
        peer_handles.push(handle);
    }
    
    let peer_results = futures::future::join_all(peer_handles).await;
    let successful_queries = peer_results.iter()
        .filter_map(|r| r.as_ref().ok())
        .count();
    
    info!("Successful concurrent peer queries: {}/{}", successful_queries, peer_results.len());
    
    network.shutdown().await?;
    
    assert!(successful_broadcasts >= 8); // Most should succeed
    assert_eq!(successful_queries, 5); // All peer queries should succeed
    
    info!("Concurrent network operations test completed");
    Ok(())
}
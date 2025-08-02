//! Non-blocking peer communication functionality tests
//!
//! Tests for peer-to-peer communication capabilities without using blocking start() method.

use anyhow::Result;
use log::info;

use p2p_network::{P2PConfig, WebRTCP2PNetwork};
use traits::{P2PNetworkLayer, TxInput, TxOutput, UtxoId, UtxoTransaction};

/// Initialize test logging
fn init_test_logging() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();
}

/// Create a test transaction with unique properties
fn create_test_transaction(id: u64, from: &str, to: &str, amount: u64) -> UtxoTransaction {
    UtxoTransaction {
        hash: format!("peer_tx_{}_{}_to_{}", id, from, to),
        inputs: vec![TxInput {
            utxo_id: UtxoId {
                tx_hash: format!("input_{}_{}", from, id),
                output_index: 0,
            },
            redeemer: format!("redeemer_{}", from).into_bytes(),
            signature: format!("sig_{}_{}", from, id).into_bytes(),
        }],
        outputs: vec![TxOutput {
            value: amount,
            script: vec![],
            datum: Some(format!("payment_from_{}_to_{}", from, to).into_bytes()),
            datum_hash: Some(format!("datum_hash_{}_{}", from, id)),
        }],
        fee: 100,
        validity_range: Some((0, 10000)),
        script_witness: vec![],
        auxiliary_data: None,
    }
}

#[tokio::test]
async fn test_non_blocking_peer_communication_setup() -> Result<()> {
    init_test_logging();
    info!("Testing non-blocking peer communication setup");

    // Create two peer networks
    let config_peer1 = P2PConfig {
        node_id: "peer_1".to_string(),
        listen_addr: "127.0.0.1:12100".parse().unwrap(),
        bootstrap_peers: vec![],
        stun_servers: vec![], // No STUN for non-blocking test
        max_peers: 5,
        connection_timeout: 5,
        keep_alive_interval: 30,
        debug_mode: false,
    };

    let config_peer2 = P2PConfig {
        node_id: "peer_2".to_string(),
        listen_addr: "127.0.0.1:12101".parse().unwrap(),
        bootstrap_peers: vec!["127.0.0.1:12100".to_string()],
        stun_servers: vec![],
        max_peers: 5,
        connection_timeout: 5,
        keep_alive_interval: 30,
        debug_mode: false,
    };

    let peer1 = WebRTCP2PNetwork::new(config_peer1)?;
    let peer2 = WebRTCP2PNetwork::new(config_peer2)?;
    
    info!("Created two peer networks");

    // Test basic peer functionality
    let stats1 = peer1.get_network_stats();
    let stats2 = peer2.get_network_stats();
    
    info!("Peer 1 stats: connections={}, total={}", 
          stats1.active_connections, stats1.total_connections);
    info!("Peer 2 stats: connections={}, total={}", 
          stats2.active_connections, stats2.total_connections);

    // Test transaction exchange simulation
    let tx1_to_2 = create_test_transaction(1, "peer1", "peer2", 1000);
    let tx2_to_1 = create_test_transaction(2, "peer2", "peer1", 500);

    let result1 = peer1.broadcast_transaction(&tx1_to_2).await;
    let result2 = peer2.broadcast_transaction(&tx2_to_1).await;

    assert!(result1.is_ok(), "Peer 1 should broadcast successfully");
    assert!(result2.is_ok(), "Peer 2 should broadcast successfully");
    
    info!("Both peers can handle transaction broadcasting");

    // Test data requests between peers
    let data_request_1 = peer1.request_blockchain_data(
        "transaction".to_string(),
        tx2_to_1.hash.clone()
    ).await;
    
    let data_request_2 = peer2.request_blockchain_data(
        "transaction".to_string(),
        tx1_to_2.hash.clone()
    ).await;

    info!("Data request results - Peer1: {:?}, Peer2: {:?}", 
          data_request_1.is_ok(), data_request_2.is_ok());

    // Test peer discovery capabilities
    let discovered1 = peer1.get_discovered_peers().await;
    let discovered2 = peer2.get_discovered_peers().await;
    let connected1 = peer1.get_connected_peers().await;
    let connected2 = peer2.get_connected_peers().await;

    info!("Peer 1 - Discovered: {}, Connected: {}", discovered1.len(), connected1.len());
    info!("Peer 2 - Discovered: {}, Connected: {}", discovered2.len(), connected2.len());

    // Test adaptive features
    let adaptive_stats1 = peer1.get_adaptive_network_stats().await;
    let adaptive_stats2 = peer2.get_adaptive_network_stats().await;

    info!("Peer 1 adaptive - Discovered: {}, DHT: {}, Efficiency: {:.2}", 
          adaptive_stats1.discovered_peers_count, 
          adaptive_stats1.dht_nodes_count,
          adaptive_stats1.discovery_efficiency);
    info!("Peer 2 adaptive - Discovered: {}, DHT: {}, Efficiency: {:.2}", 
          adaptive_stats2.discovered_peers_count, 
          adaptive_stats2.dht_nodes_count,
          adaptive_stats2.discovery_efficiency);

    // Cleanup
    peer1.shutdown().await?;
    peer2.shutdown().await?;
    
    info!("Non-blocking peer communication setup test completed");
    Ok(())
}

#[tokio::test]
async fn test_multi_peer_network_simulation() -> Result<()> {
    init_test_logging();
    info!("Testing multi-peer network simulation");

    // Create multiple peer networks
    let mut peers = Vec::new();
    
    for i in 0..5 {
        let config = P2PConfig {
            node_id: format!("multi_peer_{}", i),
            listen_addr: format!("127.0.0.1:{}", 12110 + i).parse().unwrap(),
            bootstrap_peers: if i == 0 {
                vec![]
            } else {
                vec!["127.0.0.1:12110".to_string()] // All bootstrap to first peer
            },
            stun_servers: vec![],
            max_peers: 10,
            connection_timeout: 5,
            keep_alive_interval: 30,
            debug_mode: false,
        };
        
        let peer = WebRTCP2PNetwork::new(config)?;
        peers.push(peer);
        info!("Created multi-peer network {}", i);
    }

    // Test transaction broadcasting from each peer
    let mut successful_broadcasts = 0;
    for (i, peer) in peers.iter().enumerate() {
        let tx = create_test_transaction(
            100 + i as u64, 
            &format!("peer_{}", i), 
            &format!("peer_{}", (i + 1) % peers.len()), 
            1000 + i as u64 * 100
        );
        
        if peer.broadcast_transaction(&tx).await.is_ok() {
            successful_broadcasts += 1;
        }
    }
    
    assert_eq!(successful_broadcasts, 5, "All peers should broadcast successfully");
    info!("All {} peers can broadcast transactions", successful_broadcasts);

    // Test adaptive broadcasting on all peers
    let global_tx = create_test_transaction(200, "global", "all", 5000);
    let mut adaptive_broadcasts = 0;
    
    for (i, peer) in peers.iter().enumerate() {
        if peer.adaptive_broadcast_transaction(&global_tx).await.is_ok() {
            adaptive_broadcasts += 1;
        }
        
        let adaptive_stats = peer.get_adaptive_network_stats().await;
        info!("Peer {} adaptive stats - Discovered: {}, Connected: {}, DHT: {}", 
              i, adaptive_stats.discovered_peers_count, 
              adaptive_stats.connected_peers_count,
              adaptive_stats.dht_nodes_count);
    }
    
    assert_eq!(adaptive_broadcasts, 5, "All peers should support adaptive broadcast");

    // Test cross-peer data requests
    for (i, peer) in peers.iter().enumerate() {
        let target_peer = (i + 2) % peers.len();
        let request_result = peer.request_blockchain_data(
            "peer_data".to_string(),
            format!("data_from_peer_{}", target_peer)
        ).await;
        
        info!("Peer {} data request to peer {}: {:?}", 
              i, target_peer, request_result.is_ok());
    }

    // Test network statistics across all peers
    for (i, peer) in peers.iter().enumerate() {
        let stats = peer.get_network_stats();
        let discovered = peer.get_discovered_peers().await;
        let connected = peer.get_connected_peers().await;
        
        info!("Peer {} final stats - Messages: {}, Discovered: {}, Connected: {}", 
              i, stats.messages_sent, discovered.len(), connected.len());
    }

    // Cleanup all peers
    for (i, peer) in peers.into_iter().enumerate() {
        peer.shutdown().await?;
        info!("Peer {} cleanup complete", i);
    }
    
    info!("Multi-peer network simulation test completed");
    Ok(())
}

#[tokio::test]
async fn test_peer_connection_resilience() -> Result<()> {
    init_test_logging();
    info!("Testing peer connection resilience");

    // Create network of peers
    let mut peers = Vec::new();
    
    for i in 0..4 {
        let config = P2PConfig {
            node_id: format!("resilient_peer_{}", i),
            listen_addr: format!("127.0.0.1:{}", 12120 + i).parse().unwrap(),
            bootstrap_peers: match i {
                0 => vec![],
                1 => vec!["127.0.0.1:12120".to_string()],
                2 => vec!["127.0.0.1:12120".to_string(), "127.0.0.1:12121".to_string()],
                _ => vec!["127.0.0.1:12120".to_string(), "127.0.0.1:12122".to_string()],
            },
            stun_servers: vec![],
            max_peers: 8,
            connection_timeout: 5,
            keep_alive_interval: 30,
            debug_mode: false,
        };
        
        let peer = WebRTCP2PNetwork::new(config)?;
        peers.push(peer);
    }
    
    info!("Created {} resilient peers", peers.len());

    // Test all peers are functional
    for (i, peer) in peers.iter().enumerate() {
        let tx = create_test_transaction(300 + i as u64, &format!("resilient_{}", i), "network", 1000);
        let result = peer.broadcast_transaction(&tx).await;
        assert!(result.is_ok(), "Peer {} should be functional", i);
        
        let stats = peer.get_network_stats();
        info!("Peer {} initial stats: messages={}, connections={}", 
              i, stats.messages_sent, stats.total_connections);
    }

    // Simulate peer failure by removing middle peer
    info!("Simulating peer failure (removing peer 1)");
    peers[1].shutdown().await?;
    let failed_peer = peers.remove(1);
    drop(failed_peer);

    // Test remaining peers still function
    for (i, peer) in peers.iter().enumerate() {
        let tx = create_test_transaction(400 + i as u64, &format!("surviving_{}", i), "network", 1500);
        let result = peer.broadcast_transaction(&tx).await;
        assert!(result.is_ok(), "Surviving peer {} should still work", i);
        
        let discovered = peer.get_discovered_peers().await;
        let adaptive_stats = peer.get_adaptive_network_stats().await;
        info!("Surviving peer {} - Discovered: {}, DHT: {}, Efficiency: {:.2}", 
              i, discovered.len(), adaptive_stats.dht_nodes_count, 
              adaptive_stats.discovery_efficiency);
    }

    // Add recovery peer
    info!("Adding recovery peer");
    let recovery_config = P2PConfig {
        node_id: "recovery_peer".to_string(),
        listen_addr: "127.0.0.1:12130".parse().unwrap(),
        bootstrap_peers: vec![
            "127.0.0.1:12120".to_string(),
            "127.0.0.1:12122".to_string(),
            "127.0.0.1:12123".to_string(),
        ],
        stun_servers: vec![],
        max_peers: 8,
        connection_timeout: 5,
        keep_alive_interval: 30,
        debug_mode: false,
    };
    
    let recovery_peer = WebRTCP2PNetwork::new(recovery_config)?;
    peers.push(recovery_peer);

    // Test network recovery
    let mut recovered_peers = 0;
    for (i, peer) in peers.iter().enumerate() {
        let tx = create_test_transaction(500 + i as u64, &format!("recovered_{}", i), "network", 2000);
        if peer.broadcast_transaction(&tx).await.is_ok() {
            recovered_peers += 1;
        }
        
        let stats = peer.get_network_stats();
        let adaptive_stats = peer.get_adaptive_network_stats().await;
        info!("Recovery peer {} - Messages: {}, DHT nodes: {}, Discovered: {}", 
              i, stats.messages_sent, adaptive_stats.dht_nodes_count, 
              adaptive_stats.discovered_peers_count);
    }
    
    assert_eq!(recovered_peers, peers.len(), "All remaining peers should work after recovery");
    info!("Network recovery successful: {}/{} peers functional", 
          recovered_peers, peers.len());

    // Final cleanup
    for (i, peer) in peers.into_iter().enumerate() {
        peer.shutdown().await?;
        info!("Recovery cleanup peer {} complete", i);
    }
    
    info!("Peer connection resilience test completed");
    Ok(())
}

#[tokio::test]
async fn test_peer_broadcast_patterns() -> Result<()> {
    init_test_logging();
    info!("Testing various peer broadcast patterns");

    // Create small peer network
    let mut peers = Vec::new();
    
    for i in 0..3 {
        let config = P2PConfig {
            node_id: format!("broadcast_peer_{}", i),
            listen_addr: format!("127.0.0.1:{}", 12140 + i).parse().unwrap(),
            bootstrap_peers: if i == 0 {
                vec![]
            } else {
                vec!["127.0.0.1:12140".to_string()]
            },
            stun_servers: vec![],
            max_peers: 5,
            connection_timeout: 5,
            keep_alive_interval: 30,
            debug_mode: false,
        };
        
        let peer = WebRTCP2PNetwork::new(config)?;
        peers.push(peer);
    }

    info!("Created {} peers for broadcast testing", peers.len());

    // Test one-to-many broadcast pattern
    info!("Testing one-to-many broadcast pattern");
    let broadcast_tx = create_test_transaction(600, "broadcaster", "everyone", 10000);
    
    for (i, peer) in peers.iter().enumerate() {
        let result = peer.broadcast_transaction(&broadcast_tx).await;
        assert!(result.is_ok(), "Peer {} should broadcast successfully", i);
    }

    // Test many-to-one data request pattern
    info!("Testing many-to-one data request pattern");
    for (i, peer) in peers.iter().enumerate() {
        let request_result = peer.request_blockchain_data(
            "broadcast_data".to_string(),
            format!("shared_data_item_{}", i)
        ).await;
        info!("Peer {} data request result: {:?}", i, request_result.is_ok());
    }

    // Test concurrent broadcast pattern
    info!("Testing concurrent broadcast pattern");
    let mut handles = Vec::new();
    
    for (i, peer) in peers.iter().enumerate() {
        let peer_clone = peer.clone();
        let handle = tokio::spawn(async move {
            let mut results = Vec::new();
            for j in 0..3 {
                let tx = create_test_transaction(
                    700 + j, 
                    &format!("concurrent_peer_{}", i), 
                    &format!("target_{}", j), 
                    1000 + j * 100
                );
                let result = peer_clone.broadcast_transaction(&tx).await;
                results.push(result.is_ok());
            }
            results
        });
        handles.push(handle);
    }

    let concurrent_results = futures::future::join_all(handles).await;
    
    for (i, result) in concurrent_results.iter().enumerate() {
        match result {
            Ok(broadcasts) => {
                let successful = broadcasts.iter().filter(|&&x| x).count();
                info!("Peer {} concurrent broadcasts: {}/3 successful", i, successful);
                assert_eq!(successful, 3, "All concurrent broadcasts should succeed for peer {}", i);
            }
            Err(e) => {
                panic!("Peer {} concurrent broadcast task failed: {:?}", i, e);
            }
        }
    }

    // Test adaptive broadcast pattern
    info!("Testing adaptive broadcast pattern");
    let adaptive_tx = create_test_transaction(800, "adaptive", "smart_routing", 5000);
    
    for (i, peer) in peers.iter().enumerate() {
        let result = peer.adaptive_broadcast_transaction(&adaptive_tx).await;
        assert!(result.is_ok(), "Peer {} adaptive broadcast should succeed", i);
        
        let adaptive_stats = peer.get_adaptive_network_stats().await;
        info!("Peer {} adaptive broadcast stats - Efficiency: {:.2}, DHT: {}", 
              i, adaptive_stats.discovery_efficiency, adaptive_stats.dht_nodes_count);
    }

    // Final statistics
    for (i, peer) in peers.iter().enumerate() {
        let stats = peer.get_network_stats();
        info!("Peer {} final broadcast stats - Total messages: {}, Connections: {}", 
              i, stats.messages_sent, stats.total_connections);
    }

    // Cleanup
    for (i, peer) in peers.into_iter().enumerate() {
        peer.shutdown().await?;
        info!("Broadcast test peer {} cleanup complete", i);
    }
    
    info!("Peer broadcast patterns test completed");
    Ok(())
}
//! Non-blocking WebRTC P2P Network Integration Tests
//!
//! This module contains non-blocking tests for the WebRTC P2P network
//! implementation, testing communication functionality without hanging.

use anyhow::Result;
use log::info;

use p2p_network::{P2PConfig, WebRTCP2PNetwork};
use traits::{P2PNetworkLayer, TxInput, TxOutput, UtxoBlock, UtxoId, UtxoTransaction};

/// Initialize test logging
fn init_test_logging() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();
}

/// Create a test UTXO transaction
fn create_test_transaction(from: &str, to: &str, amount: u64) -> UtxoTransaction {
    UtxoTransaction {
        hash: format!("tx_{}_{}_{}_{}", from, to, amount, uuid::Uuid::new_v4()),
        inputs: vec![TxInput {
            utxo_id: UtxoId {
                tx_hash: "genesis_tx".to_string(),
                output_index: 0,
            },
            redeemer: b"test_redeemer".to_vec(),
            signature: format!("sig_{}", from).into_bytes(),
        }],
        outputs: vec![TxOutput {
            value: amount,
            script: vec![],
            datum: Some(format!("Payment to {}", to).into_bytes()),
            datum_hash: Some(format!("datum_hash_{}", to)),
        }],
        fee: 1000,
        validity_range: Some((0, 1000)),
        script_witness: vec![],
        auxiliary_data: None,
    }
}

/// Create a test UTXO block
fn create_test_block(number: u64, transactions: Vec<UtxoTransaction>) -> UtxoBlock {
    UtxoBlock {
        hash: format!("block_{}", number),
        parent_hash: if number == 0 {
            "genesis".to_string()
        } else {
            format!("block_{}", number - 1)
        },
        number,
        timestamp: chrono::Utc::now().timestamp() as u64,
        slot: number,
        transactions,
        utxo_set_hash: format!("utxo_set_hash_{}", number),
        transaction_root: format!("tx_root_{}", number),
        validator: "test_validator".to_string(),
        proof: vec![0, 1, 2, 3], // Mock proof
    }
}

#[tokio::test]
async fn test_non_blocking_p2p_setup() -> Result<()> {
    init_test_logging();
    info!("Testing non-blocking P2P network setup and basic operations");

    // Create two network instances
    let config1 = P2PConfig {
        node_id: "node_1".to_string(),
        listen_addr: "127.0.0.1:8081".parse().unwrap(),
        bootstrap_peers: vec![],
        stun_servers: vec![], // No STUN for non-blocking test
        max_peers: 10,
        connection_timeout: 5,
        keep_alive_interval: 10,
        debug_mode: false,
    };

    let config2 = P2PConfig {
        node_id: "node_2".to_string(),
        listen_addr: "127.0.0.1:8082".parse().unwrap(),
        bootstrap_peers: vec![],
        stun_servers: vec![], // No STUN for non-blocking test
        max_peers: 10,
        connection_timeout: 5,
        keep_alive_interval: 10,
        debug_mode: false,
    };

    let network1 = WebRTCP2PNetwork::new(config1)?;
    let network2 = WebRTCP2PNetwork::new(config2)?;

    // Test network creation and configuration
    let stats1 = network1.get_network_stats();
    let stats2 = network2.get_network_stats();

    assert_eq!(stats1.active_connections, 0);
    assert_eq!(stats2.active_connections, 0);

    info!("Network1 initial stats: {:?}", stats1);
    info!("Network2 initial stats: {:?}", stats2);

    // Test transaction broadcasting (should work even without connections)
    let tx = create_test_transaction("alice", "bob", 1000);
    let broadcast_result1 = network1.broadcast_transaction(&tx).await;
    let broadcast_result2 = network2.broadcast_transaction(&tx).await;

    assert!(broadcast_result1.is_ok());
    assert!(broadcast_result2.is_ok());
    info!("Transaction broadcast successful on both networks");

    // Test block broadcasting
    let block = create_test_block(1, vec![tx.clone()]);
    let block_result1 = network1.broadcast_block(&block).await;
    let block_result2 = network2.broadcast_block(&block).await;

    assert!(block_result1.is_ok());
    assert!(block_result2.is_ok());
    info!("Block broadcast successful on both networks");

    // Test data requests
    let data_request1 = network1
        .request_blockchain_data("transaction".to_string(), tx.hash.clone())
        .await;
    let data_request2 = network2
        .request_blockchain_data("block".to_string(), block.hash.clone())
        .await;

    info!(
        "Data request results - Network1: {:?}, Network2: {:?}",
        data_request1.is_ok(),
        data_request2.is_ok()
    );

    // Test peer queries
    let peers1 = network1.get_connected_peers().await;
    let peers2 = network2.get_connected_peers().await;

    assert_eq!(peers1.len(), 0); // No connections without start()
    assert_eq!(peers2.len(), 0);
    info!(
        "Peer queries successful - Network1: {} peers, Network2: {} peers",
        peers1.len(),
        peers2.len()
    );

    // Test discovery functionality
    let discovered1 = network1.get_discovered_peers().await;
    let discovered2 = network2.get_discovered_peers().await;

    info!(
        "Discovery results - Network1: {} discovered, Network2: {} discovered",
        discovered1.len(),
        discovered2.len()
    );

    // Graceful shutdown
    network1.shutdown().await?;
    network2.shutdown().await?;

    info!("Non-blocking P2P setup test completed successfully");
    Ok(())
}

#[tokio::test]
async fn test_network_configuration_validation() -> Result<()> {
    init_test_logging();
    info!("Testing network configuration validation");

    // Test various configurations
    let configs = vec![
        P2PConfig {
            node_id: "minimal_node".to_string(),
            listen_addr: "127.0.0.1:9001".parse().unwrap(),
            bootstrap_peers: vec![],
            stun_servers: vec![],
            max_peers: 1,
            connection_timeout: 1,
            keep_alive_interval: 5,
            debug_mode: false,
        },
        P2PConfig {
            node_id: "max_node".to_string(),
            listen_addr: "127.0.0.1:9002".parse().unwrap(),
            bootstrap_peers: vec!["127.0.0.1:9001".to_string()],
            stun_servers: vec!["stun:test.example.com:19302".to_string()],
            max_peers: 100,
            connection_timeout: 300,
            keep_alive_interval: 60,
            debug_mode: true,
        },
    ];

    let mut networks = Vec::new();

    for (i, config) in configs.into_iter().enumerate() {
        let network = WebRTCP2PNetwork::new(config)?;

        // Test basic functionality
        let stats = network.get_network_stats();
        let peers = network.get_connected_peers().await;

        info!("Network {} - Stats: {:?}, Peers: {}", i, stats, peers.len());

        // Test transaction handling
        let tx = create_test_transaction("user1", "user2", 100 * (i as u64 + 1));
        let result = network.broadcast_transaction(&tx).await;
        assert!(result.is_ok(), "Network {} should handle transactions", i);

        networks.push(network);
    }

    // Cleanup all networks
    for (i, network) in networks.into_iter().enumerate() {
        network.shutdown().await?;
        info!("Network {} shutdown complete", i);
    }

    info!("Network configuration validation test completed");
    Ok(())
}

#[tokio::test]
async fn test_multiple_transactions_and_blocks() -> Result<()> {
    init_test_logging();
    info!("Testing multiple transactions and blocks handling");

    let config = P2PConfig {
        node_id: "multi_test_node".to_string(),
        listen_addr: "127.0.0.1:9010".parse().unwrap(),
        bootstrap_peers: vec![],
        stun_servers: vec![],
        max_peers: 10,
        connection_timeout: 5,
        keep_alive_interval: 30,
        debug_mode: false,
    };

    let network = WebRTCP2PNetwork::new(config)?;

    // Create multiple transactions
    let transactions: Vec<UtxoTransaction> = (0..10)
        .map(|i| {
            create_test_transaction(
                &format!("user_{}", i),
                &format!("user_{}", i + 1),
                1000 + i * 100,
            )
        })
        .collect();

    // Broadcast all transactions
    let mut successful_broadcasts = 0;
    for (i, tx) in transactions.iter().enumerate() {
        let result = network.broadcast_transaction(tx).await;
        if result.is_ok() {
            successful_broadcasts += 1;
        }
        info!("Transaction {} broadcast result: {:?}", i, result.is_ok());
    }

    assert_eq!(
        successful_broadcasts, 10,
        "All transactions should broadcast successfully"
    );

    // Create multiple blocks with the transactions
    let blocks: Vec<UtxoBlock> = (0..5)
        .map(|i| {
            let block_txs = transactions[i * 2..i * 2 + 2].to_vec();
            create_test_block(i as u64, block_txs)
        })
        .collect();

    // Broadcast all blocks
    let mut successful_block_broadcasts = 0;
    for (i, block) in blocks.iter().enumerate() {
        let result = network.broadcast_block(block).await;
        if result.is_ok() {
            successful_block_broadcasts += 1;
        }
        info!("Block {} broadcast result: {:?}", i, result.is_ok());
    }

    assert_eq!(
        successful_block_broadcasts, 5,
        "All blocks should broadcast successfully"
    );

    // Test data requests for all items
    for (i, tx) in transactions.iter().enumerate() {
        let result = network
            .request_blockchain_data("transaction".to_string(), tx.hash.clone())
            .await;
        info!(
            "Transaction {} data request result: {:?}",
            i,
            result.is_ok()
        );
    }

    for (i, block) in blocks.iter().enumerate() {
        let result = network
            .request_blockchain_data("block".to_string(), block.hash.clone())
            .await;
        info!("Block {} data request result: {:?}", i, result.is_ok());
    }

    // Check final stats
    let final_stats = network.get_network_stats();
    info!("Final stats after multiple operations: {:?}", final_stats);

    network.shutdown().await?;
    info!("Multiple transactions and blocks test completed");
    Ok(())
}

#[tokio::test]
async fn test_concurrent_network_operations() -> Result<()> {
    init_test_logging();
    info!("Testing concurrent network operations across multiple networks");

    // Create multiple networks
    let mut networks = Vec::new();
    let mut configs = Vec::new();

    for i in 0..3 {
        let config = P2PConfig {
            node_id: format!("concurrent_node_{}", i),
            listen_addr: format!("127.0.0.1:{}", 9020 + i).parse().unwrap(),
            bootstrap_peers: vec![],
            stun_servers: vec![],
            max_peers: 5,
            connection_timeout: 5,
            keep_alive_interval: 30,
            debug_mode: false,
        };

        let network = WebRTCP2PNetwork::new(config.clone())?;
        networks.push(network);
        configs.push(config);
    }

    // Concurrent transaction broadcasting
    let mut handles = Vec::new();

    for (i, network) in networks.iter().enumerate() {
        let net = network.clone();
        let handle = tokio::spawn(async move {
            let mut results = Vec::new();
            for j in 0..5 {
                let tx = create_test_transaction(
                    &format!("user_{}_{}", i, j),
                    &format!("user_{}_{}", i, j + 1),
                    1000 + j * 100,
                );
                let result = net.broadcast_transaction(&tx).await;
                results.push(result.is_ok());
            }
            results
        });
        handles.push(handle);
    }

    // Wait for all concurrent operations
    let results = futures::future::join_all(handles).await;

    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(broadcasts) => {
                let successful = broadcasts.iter().filter(|&&x| x).count();
                info!("Network {} - Successful broadcasts: {}/5", i, successful);
                assert_eq!(
                    successful, 5,
                    "All broadcasts should succeed for network {}",
                    i
                );
            }
            Err(e) => {
                panic!("Network {} task failed: {:?}", i, e);
            }
        }
    }

    // Concurrent data requests
    let mut request_handles = Vec::new();

    for (i, network) in networks.iter().enumerate() {
        let net = network.clone();
        let handle = tokio::spawn(async move {
            let mut results = Vec::new();
            for j in 0..3 {
                let result = net
                    .request_blockchain_data(
                        "test_data".to_string(),
                        format!("test_id_{}_{}", i, j),
                    )
                    .await;
                results.push(result.is_ok());
            }
            results
        });
        request_handles.push(handle);
    }

    let request_results = futures::future::join_all(request_handles).await;

    for (i, result) in request_results.iter().enumerate() {
        match result {
            Ok(requests) => {
                info!(
                    "Network {} - Data requests completed: {}",
                    i,
                    requests.len()
                );
            }
            Err(e) => {
                panic!("Network {} request task failed: {:?}", i, e);
            }
        }
    }

    // Cleanup all networks
    for (i, network) in networks.into_iter().enumerate() {
        network.shutdown().await?;
        info!("Network {} shutdown complete", i);
    }

    info!("Concurrent network operations test completed");
    Ok(())
}

//! Peer connection functionality tests
//!
//! Tests for peer-specific functionality including connection state management,
//! latency tracking, ping/pong handling, and handshake operations.

use std::sync::Arc;
use tokio::sync::broadcast;
use anyhow::Result;
use log::info;

use p2p_network::P2PMessage;
use webrtc::{
    api::APIBuilder,
    peer_connection::{
        configuration::RTCConfiguration,
        peer_connection_state::RTCPeerConnectionState,
        RTCPeerConnection,
    },
    ice_transport::ice_server::RTCIceServer,
};

/// Initialize test logging
fn init_test_logging() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();
}

/// Create a test WebRTC peer connection
async fn create_test_rtc_peer() -> Result<Arc<RTCPeerConnection>> {
    let api = APIBuilder::new().build();
    
    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_string()],
            ..Default::default()
        }],
        ..Default::default()
    };
    
    let peer = api.new_peer_connection(config).await?;
    Ok(Arc::new(peer))
}

#[tokio::test]
async fn test_peer_connection_creation() -> Result<()> {
    init_test_logging();
    info!("🧪 Testing peer connection creation");

    let rtc_peer = create_test_rtc_peer().await?;
    let (message_tx, _) = broadcast::channel(100);
    
    // Create PeerConnection using the new method
    use p2p_network::PeerConnection;
    let peer = PeerConnection::new(
        "test_peer_1".to_string(),
        "test_node_1".to_string(),
        rtc_peer,
        message_tx,
    )?;
    
    // Test connection state
    let state = peer.get_connection_state();
    assert!(matches!(state, RTCPeerConnectionState::New));
    
    // Test connection check
    let is_connected = peer.is_connected();
    assert!(!is_connected); // Should be false for new connection
    
    info!("✅ Peer connection creation test passed");
    Ok(())
}

#[tokio::test]
async fn test_peer_latency_tracking() -> Result<()> {
    init_test_logging();
    info!("🧪 Testing peer latency tracking");

    let rtc_peer = create_test_rtc_peer().await?;
    let (message_tx, _) = broadcast::channel(100);
    
    use p2p_network::PeerConnection;
    let peer = PeerConnection::new(
        "test_peer_2".to_string(),
        "test_node_2".to_string(),
        rtc_peer,
        message_tx,
    )?;
    
    // Test updating latency
    peer.update_latency(50);  // Good latency
    peer.update_latency(200); // Acceptable latency
    peer.update_latency(800); // Poor latency
    
    // Latency updates should affect reputation score
    // (We can't directly verify this without accessing the peer info,
    // but the method calls should not panic)
    
    info!("✅ Peer latency tracking test passed");
    Ok(())
}

#[tokio::test]
async fn test_peer_ping_pong_handling() -> Result<()> {
    init_test_logging();
    info!("🧪 Testing peer ping/pong handling");

    let rtc_peer = create_test_rtc_peer().await?;
    let (message_tx, _message_rx) = broadcast::channel(100);
    
    use p2p_network::PeerConnection;
    let peer = PeerConnection::new(
        "test_peer_3".to_string(),
        "test_node_3".to_string(),
        rtc_peer,
        message_tx,
    )?;
    
    // Test handling ping (this would normally send a pong response)
    let timestamp = chrono::Utc::now().timestamp() as u64;
    let nonce = 12345;
    
    // This will fail since no data channel is established, but tests the API
    let ping_result = peer.handle_ping(timestamp, nonce).await;
    // Expected to fail due to no data channel
    assert!(ping_result.is_err());
    
    // Test handling pong
    peer.handle_pong(timestamp, nonce);
    // This should not panic and should update latency
    
    info!("✅ Peer ping/pong handling test passed");
    Ok(())
}

#[tokio::test]
async fn test_peer_handshake() -> Result<()> {
    init_test_logging();
    info!("🧪 Testing peer handshake");

    let rtc_peer = create_test_rtc_peer().await?;
    let (message_tx, _) = broadcast::channel(100);
    
    use p2p_network::PeerConnection;
    let peer = PeerConnection::new(
        "test_peer_4".to_string(),
        "test_node_4".to_string(),
        rtc_peer,
        message_tx,
    )?;
    
    // Test performing handshake
    let version = "1.0.0".to_string();
    let handshake_result = peer.perform_handshake(version).await;
    
    // Expected to fail since no data channel is established
    assert!(handshake_result.is_err());
    
    info!("✅ Peer handshake test passed");
    Ok(())
}

#[tokio::test]
async fn test_peer_disconnection() -> Result<()> {
    init_test_logging();
    info!("🧪 Testing peer disconnection");

    let rtc_peer = create_test_rtc_peer().await?;
    let (message_tx, _) = broadcast::channel(100);
    
    use p2p_network::PeerConnection;
    let peer = PeerConnection::new(
        "test_peer_5".to_string(),
        "test_node_5".to_string(),
        rtc_peer,
        message_tx,
    )?;
    
    // Test disconnection
    let disconnect_result = peer.disconnect().await;
    assert!(disconnect_result.is_ok());
    
    info!("✅ Peer disconnection test passed");
    Ok(())
}

#[tokio::test] 
async fn test_peer_message_sending() -> Result<()> {
    init_test_logging();
    info!("🧪 Testing peer message sending");

    let rtc_peer = create_test_rtc_peer().await?;
    let (message_tx, _) = broadcast::channel(100);
    
    use p2p_network::PeerConnection;
    let peer = PeerConnection::new(
        "test_peer_6".to_string(),
        "test_node_6".to_string(),
        rtc_peer,
        message_tx,
    )?;
    
    // Test sending different message types
    let ping_msg = P2PMessage::Ping {
        timestamp: chrono::Utc::now().timestamp() as u64,
        nonce: 12345,
    };
    
    let handshake_msg = P2PMessage::Handshake {
        node_id: "test_node".to_string(),
        version: "1.0.0".to_string(),
        timestamp: chrono::Utc::now().timestamp() as u64,
    };
    
    // These will fail due to no data channel, but test the API
    let ping_result = peer.send_message(ping_msg).await;
    assert!(ping_result.is_err());
    
    let handshake_result = peer.send_message(handshake_msg).await;
    assert!(handshake_result.is_err());
    
    info!("✅ Peer message sending test passed");
    Ok(())
}
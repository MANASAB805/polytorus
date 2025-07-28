//! Peer connection implementation for WebRTC P2P networking

use std::{
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use bytes::Bytes;
use log::{debug, error, info, warn};
use tokio::{
    sync::{broadcast, RwLock},
    time::timeout,
};
use webrtc::peer_connection::{peer_connection_state::RTCPeerConnectionState, RTCPeerConnection};

use crate::{P2PMessage, PeerInfo};

impl super::PeerConnection {
    /// Create a new peer connection
    pub fn new(
        id: String,
        node_id: String,
        rtc_peer: Arc<RTCPeerConnection>,
        message_tx: broadcast::Sender<(String, P2PMessage)>,
    ) -> Result<Self> {
        let peer_info = PeerInfo {
            id: id.clone(),
            node_id: node_id.clone(),
            connection_state: "new".to_string(),
            connected_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            last_seen: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            bytes_sent: 0,
            bytes_received: 0,
            latency_ms: None,
            reputation_score: 1.0,
        };

        Ok(Self {
            id,
            node_id,
            rtc_peer,
            data_channel: Arc::new(RwLock::new(None)),
            info: Arc::new(Mutex::new(peer_info)),
            message_tx,
        })
    }

    /// Send a message to this peer
    pub async fn send_message(&self, message: P2PMessage) -> Result<()> {
        let data_channel = {
            let dc_lock = self.data_channel.read().await;
            dc_lock
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Data channel not available for peer: {}", self.id))?
                .clone()
        };

        // Serialize message
        let serialized = bincode::serialize(&message).context("Failed to serialize P2P message")?;

        // Send message with timeout
        let send_result = timeout(
            std::time::Duration::from_secs(10),
            data_channel.send(&Bytes::from(serialized.clone())),
        )
        .await;

        match send_result {
            Ok(Ok(_)) => {
                // Update peer stats
                {
                    let mut info = self.info.lock().unwrap();
                    info.bytes_sent += serialized.len() as u64;
                    info.last_seen = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    // Increase reputation for successful sends
                    info.reputation_score = (info.reputation_score + 0.01).min(2.0);
                }
                debug!("📤 Sent message to peer {}: {:?}", self.id, message);
                Ok(())
            }
            Ok(Err(e)) => {
                // Decrease reputation for failed sends
                {
                    let mut info = self.info.lock().unwrap();
                    info.reputation_score = (info.reputation_score - 0.1).max(0.0);
                }
                error!("❌ Failed to send message to peer {}: {}", self.id, e);
                Err(anyhow::anyhow!("Send failed: {}", e))
            }
            Err(_) => {
                // Decrease reputation for timeouts
                {
                    let mut info = self.info.lock().unwrap();
                    info.reputation_score = (info.reputation_score - 0.2).max(0.0);
                }
                error!("⏰ Timeout sending message to peer: {}", self.id);
                Err(anyhow::anyhow!("Send timeout"))
            }
        }
    }

    /// Disconnect this peer connection
    pub async fn disconnect(&self) -> Result<()> {
        info!("🔌 Disconnecting peer: {}", self.id);

        // Close data channel if available
        {
            let dc_lock = self.data_channel.read().await;
            if let Some(data_channel) = dc_lock.as_ref() {
                if let Err(e) = data_channel.close().await {
                    warn!("Error closing data channel for peer {}: {}", self.id, e);
                }
            }
        }

        // Close peer connection
        if let Err(e) = self.rtc_peer.close().await {
            warn!("Error closing peer connection for {}: {}", self.id, e);
        }

        info!("✅ Peer {} disconnected successfully", self.id);
        Ok(())
    }

    /// Get current connection state
    pub fn get_connection_state(&self) -> RTCPeerConnectionState {
        self.rtc_peer.connection_state()
    }

    /// Check if peer connection is active
    pub fn is_connected(&self) -> bool {
        matches!(
            self.rtc_peer.connection_state(),
            RTCPeerConnectionState::Connected
        )
    }

    /// Update latency measurement
    pub fn update_latency(&self, latency_ms: u64) {
        let mut info = self.info.lock().unwrap();
        info.latency_ms = Some(latency_ms);

        // Adjust reputation based on latency
        let latency_factor = if latency_ms < 100 {
            1.05 // Good latency
        } else if latency_ms < 500 {
            1.0 // Acceptable latency
        } else {
            0.95 // Poor latency
        };

        info.reputation_score = (info.reputation_score * latency_factor).min(2.0).max(0.0);
    }

    /// Handle ping message and respond with pong
    pub async fn handle_ping(&self, timestamp: u64, nonce: u64) -> Result<()> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let pong_message = P2PMessage::Pong {
            timestamp: current_time,
            nonce,
        };

        self.send_message(pong_message).await?;

        // Calculate latency if this is a response to our ping
        let latency_ms = (current_time.saturating_sub(timestamp)) * 1000;
        self.update_latency(latency_ms);

        Ok(())
    }

    /// Handle pong message and update latency
    pub fn handle_pong(&self, timestamp: u64, _nonce: u64) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let latency_ms = (current_time.saturating_sub(timestamp)) * 1000;
        self.update_latency(latency_ms);
    }

    /// Perform handshake with peer
    pub async fn perform_handshake(&self, version: String) -> Result<()> {
        let handshake_message = P2PMessage::Handshake {
            node_id: self.node_id.clone(),
            version,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        self.send_message(handshake_message).await?;
        info!("🤝 Sent handshake to peer: {}", self.id);
        Ok(())
    }
}

impl Clone for super::NetworkStats {
    fn clone(&self) -> Self {
        Self {
            total_connections: self.total_connections,
            active_connections: self.active_connections,
            messages_sent: self.messages_sent,
            messages_received: self.messages_received,
            bytes_sent: self.bytes_sent,
            bytes_received: self.bytes_received,
            connection_errors: self.connection_errors,
            last_updated: self.last_updated,
        }
    }
}

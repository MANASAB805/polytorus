//! WebRTC signaling server implementation for P2P connections
//!
//! This module provides a signaling server that facilitates WebRTC connection
//! establishment between peers by exchanging SDP offers/answers and ICE candidates.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use anyhow::{Result, Context};
use log::{info, warn, error, debug};
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, RwLock},
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
};
use uuid::Uuid;

/// Signaling message types for WebRTC negotiation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalingMessage {
    /// Register a new peer with the signaling server
    Register {
        peer_id: String,
        node_id: String,
    },
    /// SDP offer from initiating peer
    Offer {
        from: String,
        to: String,
        sdp: String,
    },
    /// SDP answer from responding peer
    Answer {
        from: String,
        to: String,
        sdp: String,
    },
    /// ICE candidate exchange
    IceCandidate {
        from: String,
        to: String,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    },
    /// List available peers
    ListPeers,
    /// Peer list response
    PeerList {
        peers: Vec<PeerDescriptor>,
    },
    /// Error response
    Error {
        message: String,
    },
    /// Connection established confirmation
    Connected {
        peer_id: String,
    },
    /// Peer disconnection notification
    Disconnected {
        peer_id: String,
    },
}

/// Peer descriptor for signaling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerDescriptor {
    pub peer_id: String,
    pub node_id: String,
    pub connected_at: u64,
    pub status: String,
}

/// Connected peer information
#[derive(Debug, Clone)]
struct ConnectedPeer {
    peer_id: String,
    node_id: String,
    sender: broadcast::Sender<SignalingMessage>,
    connected_at: std::time::SystemTime,
}

/// WebRTC signaling server
pub struct SignalingServer {
    /// Server listening address
    listen_addr: SocketAddr,
    /// Connected peers
    peers: Arc<RwLock<HashMap<String, ConnectedPeer>>>,
    /// Message broadcast channel
    broadcast_tx: broadcast::Sender<(String, SignalingMessage)>,
    /// Server statistics
    stats: Arc<Mutex<SignalingStats>>,
}

/// Signaling server statistics
#[derive(Debug, Default)]
pub struct SignalingStats {
    total_connections: u64,
    active_connections: u64,
    messages_relayed: u64,
    offers_processed: u64,
    answers_processed: u64,
    ice_candidates_processed: u64,
    errors: u64,
}

impl SignalingServer {
    /// Create a new signaling server
    pub fn new(listen_addr: SocketAddr) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);

        Self {
            listen_addr,
            peers: Arc::new(RwLock::new(HashMap::new())),
            broadcast_tx,
            stats: Arc::new(Mutex::new(SignalingStats::default())),
        }
    }

    /// Start the signaling server
    pub async fn start(&self) -> Result<()> {
        let listener = TcpListener::bind(self.listen_addr).await
            .context("Failed to bind signaling server")?;

        info!("🔗 Signaling server listening on: {}", self.listen_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("📞 New signaling connection from: {}", addr);
                    let peers = Arc::clone(&self.peers);
                    let stats = Arc::clone(&self.stats);
                    let broadcast_tx = self.broadcast_tx.clone();

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_peer_connection(
                            stream, addr, peers, stats, broadcast_tx
                        ).await {
                            error!("❌ Error handling peer connection {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("❌ Failed to accept connection: {}", e);
                }
            }
        }
    }

    /// Handle individual peer connection
    async fn handle_peer_connection(
        stream: TcpStream,
        addr: SocketAddr,
        peers: Arc<RwLock<HashMap<String, ConnectedPeer>>>,
        stats: Arc<Mutex<SignalingStats>>,
        broadcast_tx: broadcast::Sender<(String, SignalingMessage)>,
    ) -> Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut buf_reader = BufReader::new(reader);
        let mut line = String::new();
        let peer_id = Uuid::new_v4().to_string();

        // Create peer-specific message channel
        let (peer_tx, mut peer_rx) = broadcast::channel(100);

        // Spawn task to handle outgoing messages to this peer
        let peer_id_out = peer_id.clone();
        tokio::spawn(async move {
            while let Ok(message) = peer_rx.recv().await {
                let json = match serde_json::to_string(&message) {
                    Ok(json) => json,
                    Err(e) => {
                        error!("❌ Failed to serialize message: {}", e);
                        continue;
                    }
                };

                if let Err(e) = writer.write_all(format!("{}\n", json).as_bytes()).await {
                    error!("❌ Failed to send message to peer {}: {}", peer_id_out, e);
                    break;
                }
            }
        });

        // Update stats
        {
            let mut stats = stats.lock().unwrap();
            stats.total_connections += 1;
            stats.active_connections += 1;
        }

        // Process incoming messages
        loop {
            line.clear();
            match buf_reader.read_line(&mut line).await {
                Ok(0) => {
                    // Connection closed
                    info!("📴 Peer {} disconnected", peer_id);
                    break;
                }
                Ok(_) => {
                    let message: SignalingMessage = match serde_json::from_str(line.trim()) {
                        Ok(msg) => msg,
                        Err(e) => {
                            error!("❌ Invalid message from {}: {}", addr, e);
                            let error_msg = SignalingMessage::Error {
                                message: format!("Invalid message format: {}", e),
                            };
                            if let Err(e) = peer_tx.send(error_msg) {
                                error!("Failed to send error message: {}", e);
                            }
                            continue;
                        }
                    };

                    debug!("📨 Received signaling message from {}: {:?}", addr, message);

                    if let Err(e) = Self::process_signaling_message(
                        message,
                        &peer_id,
                        &peers,
                        &stats,
                        &peer_tx,
                        &broadcast_tx,
                    ).await {
                        error!("❌ Error processing message from {}: {}", addr, e);
                    }
                }
                Err(e) => {
                    error!("❌ Error reading from {}: {}", addr, e);
                    break;
                }
            }
        }

        // Clean up peer on disconnect
        {
            let mut peers = peers.write().await;
            peers.remove(&peer_id);
        }

        // Update stats
        {
            let mut stats = stats.lock().unwrap();
            stats.active_connections = stats.active_connections.saturating_sub(1);
        }

        // Notify other peers about disconnection
        let disconnect_msg = SignalingMessage::Disconnected {
            peer_id: peer_id.clone(),
        };
        if let Err(e) = broadcast_tx.send((peer_id, disconnect_msg)) {
            warn!("Failed to broadcast disconnect message: {}", e);
        }

        Ok(())
    }

    /// Process incoming signaling message
    async fn process_signaling_message(
        message: SignalingMessage,
        peer_id: &str,
        peers: &Arc<RwLock<HashMap<String, ConnectedPeer>>>,
        stats: &Arc<Mutex<SignalingStats>>,
        peer_tx: &broadcast::Sender<SignalingMessage>,
        _broadcast_tx: &broadcast::Sender<(String, SignalingMessage)>,
    ) -> Result<()> {
        match message {
            SignalingMessage::Register { peer_id: reg_peer_id, node_id } => {
                info!("📝 Registering peer: {} (node: {})", reg_peer_id, node_id);

                let connected_peer = ConnectedPeer {
                    peer_id: reg_peer_id.clone(),
                    node_id,
                    sender: peer_tx.clone(),
                    connected_at: std::time::SystemTime::now(),
                };

                {
                    let mut peers = peers.write().await;
                    peers.insert(reg_peer_id.clone(), connected_peer);
                }

                // Send confirmation
                let connected_msg = SignalingMessage::Connected {
                    peer_id: reg_peer_id,
                };
                peer_tx.send(connected_msg)?;
            }

            SignalingMessage::ListPeers => {
                let peer_list = {
                    let peers = peers.read().await;
                    peers.values()
                        .map(|p| PeerDescriptor {
                            peer_id: p.peer_id.clone(),
                            node_id: p.node_id.clone(),
                            connected_at: p.connected_at
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                            status: "connected".to_string(),
                        })
                        .collect()
                };

                let response = SignalingMessage::PeerList {
                    peers: peer_list,
                };
                peer_tx.send(response)?;
            }

            SignalingMessage::Offer { ref from, ref to, sdp: _ } => {
                info!("📋 Relaying offer from {} to {}", from, to);
                let target_id = to.clone();
                Self::relay_message_to_peer(&target_id, message, peers).await?;

                {
                    let mut stats = stats.lock().unwrap();
                    stats.offers_processed += 1;
                    stats.messages_relayed += 1;
                }
            }

            SignalingMessage::Answer { ref from, ref to, sdp: _ } => {
                info!("📝 Relaying answer from {} to {}", from, to);
                let target_id = to.clone();
                Self::relay_message_to_peer(&target_id, message, peers).await?;

                {
                    let mut stats = stats.lock().unwrap();
                    stats.answers_processed += 1;
                    stats.messages_relayed += 1;
                }
            }

            SignalingMessage::IceCandidate { ref from, ref to, .. } => {
                debug!("🧊 Relaying ICE candidate from {} to {}", from, to);
                let target_id = to.clone();
                Self::relay_message_to_peer(&target_id, message, peers).await?;

                {
                    let mut stats = stats.lock().unwrap();
                    stats.ice_candidates_processed += 1;
                    stats.messages_relayed += 1;
                }
            }

            _ => {
                warn!("❓ Unhandled signaling message type from {}", peer_id);
            }
        }

        Ok(())
    }

    /// Relay message to specific peer
    async fn relay_message_to_peer(
        target_peer_id: &str,
        message: SignalingMessage,
        peers: &Arc<RwLock<HashMap<String, ConnectedPeer>>>,
    ) -> Result<()> {
        let peers = peers.read().await;
        if let Some(target_peer) = peers.get(target_peer_id) {
            target_peer.sender.send(message)
                .context("Failed to send message to target peer")?;
            debug!("📤 Message relayed to peer: {}", target_peer_id);
        } else {
            warn!("🔍 Target peer not found: {}", target_peer_id);
            return Err(anyhow::anyhow!("Target peer not found: {}", target_peer_id));
        }

        Ok(())
    }

    /// Get server statistics
    pub fn get_stats(&self) -> SignalingStats {
        let stats = self.stats.lock().unwrap();
        SignalingStats {
            total_connections: stats.total_connections,
            active_connections: stats.active_connections,
            messages_relayed: stats.messages_relayed,
            offers_processed: stats.offers_processed,
            answers_processed: stats.answers_processed,
            ice_candidates_processed: stats.ice_candidates_processed,
            errors: stats.errors,
        }
    }

    /// Get list of connected peers
    pub async fn get_connected_peers(&self) -> Vec<PeerDescriptor> {
        let peers = self.peers.read().await;
        peers.values()
            .map(|p| PeerDescriptor {
                peer_id: p.peer_id.clone(),
                node_id: p.node_id.clone(),
                connected_at: p.connected_at
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                status: "connected".to_string(),
            })
            .collect()
    }
}
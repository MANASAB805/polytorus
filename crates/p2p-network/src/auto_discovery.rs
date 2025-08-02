//! Auto P2P Discovery
//!
//! Simplified but effective peer discovery mechanism that automatically discovers
//! peers in the P2P network without breaking current functionality.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime},
};

use anyhow::Result;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use tokio::{
    net::UdpSocket,
    sync::RwLock,
    time::{interval, sleep},
};

/// Simple discovery message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SimpleDiscoveryMessage {
    /// Announce node presence
    Announce {
        node_id: String,
        address: SocketAddr,
        port: u16,
    },
    /// Request peer list
    PeerRequest { node_id: String },
    /// Respond with known peers
    PeerResponse {
        peers: Vec<SimplePeerInfo>,
    },
}

/// Simple peer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimplePeerInfo {
    pub node_id: String,
    pub address: SocketAddr,
    pub last_seen: u64,
}

/// Auto discovery service
pub struct AutoDiscovery {
    node_id: String,
    listen_port: u16,
    socket: Option<Arc<UdpSocket>>,
    known_peers: Arc<RwLock<HashMap<String, SimplePeerInfo>>>,
    discovery_ports: Vec<u16>,
}

impl AutoDiscovery {
    /// Create new auto discovery
    pub fn new(node_id: String, listen_port: u16) -> Self {
        Self {
            node_id,
            listen_port,
            socket: None,
            known_peers: Arc::new(RwLock::new(HashMap::new())),
            discovery_ports: vec![9000, 9001, 9002, 9010, 9020, 9100], // Common discovery ports
        }
    }
    
    /// Start discovery service
    pub async fn start(&mut self) -> Result<()> {
        // Try to bind discovery socket
        let discovery_addr = format!("0.0.0.0:{}", self.listen_port + 1000); // Offset for discovery
        
        match UdpSocket::bind(&discovery_addr).await {
            Ok(socket) => {
                info!("Auto discovery started on {}", discovery_addr);
                self.socket = Some(Arc::new(socket));
                
                // Start announcement task
                self.start_announcement_task().await;
                
                // Start peer discovery task
                self.start_peer_discovery_task().await;
                
                // Start listening task
                self.start_listening_task().await;
                
                Ok(())
            }
            Err(e) => {
                warn!("Failed to start auto discovery: {}", e);
                Ok(()) // Don't fail the entire network
            }
        }
    }
    
    /// Start announcement task
    async fn start_announcement_task(&self) {
        if let Some(socket) = &self.socket {
            let socket = socket.clone();
            let node_id = self.node_id.clone();
            let listen_port = self.listen_port;
            let discovery_ports = self.discovery_ports.clone();
            
            tokio::spawn(async move {
                let mut interval = interval(Duration::from_secs(15)); // Announce every 15 seconds
                
                loop {
                    interval.tick().await;
                    
                    let announce = SimpleDiscoveryMessage::Announce {
                        node_id: node_id.clone(),
                        address: format!("127.0.0.1:{}", listen_port).parse().unwrap(),
                        port: listen_port,
                    };
                    
                    if let Ok(data) = bincode::serialize(&announce) {
                        // Broadcast to discovery ports
                        for port in &discovery_ports {
                            let target = format!("127.0.0.1:{}", port + 1000);
                            if let Ok(addr) = target.parse::<SocketAddr>() {
                                let _ = socket.send_to(&data, addr).await;
                            }
                        }
                        
                        // Also try broadcast
                        let broadcast_addr = format!("255.255.255.255:{}", listen_port + 1000);
                        if let Ok(addr) = broadcast_addr.parse::<SocketAddr>() {
                            let _ = socket.send_to(&data, addr).await;
                        }
                    }
                }
            });
        }
    }
    
    /// Start peer discovery task
    async fn start_peer_discovery_task(&self) {
        if let Some(socket) = &self.socket {
            let socket = socket.clone();
            let node_id = self.node_id.clone();
            let discovery_ports = self.discovery_ports.clone();
            
            tokio::spawn(async move {
                let mut interval = interval(Duration::from_secs(30)); // Discover every 30 seconds
                
                loop {
                    interval.tick().await;
                    
                    let request = SimpleDiscoveryMessage::PeerRequest {
                        node_id: node_id.clone(),
                    };
                    
                    if let Ok(data) = bincode::serialize(&request) {
                        // Send discovery requests
                        for port in &discovery_ports {
                            let target = format!("127.0.0.1:{}", port + 1000);
                            if let Ok(addr) = target.parse::<SocketAddr>() {
                                let _ = socket.send_to(&data, addr).await;
                            }
                        }
                    }
                }
            });
        }
    }
    
    /// Start listening task
    async fn start_listening_task(&self) {
        if let Some(socket) = &self.socket {
            let socket = socket.clone();
            let known_peers = self.known_peers.clone();
            let node_id = self.node_id.clone();
            
            tokio::spawn(async move {
                let mut buffer = [0u8; 1024];
                
                loop {
                    match socket.recv_from(&mut buffer).await {
                        Ok((len, from)) => {
                            if let Ok(message) = bincode::deserialize::<SimpleDiscoveryMessage>(&buffer[..len]) {
                                match message {
                                    SimpleDiscoveryMessage::Announce { node_id: peer_id, address, port: _ } => {
                                        if peer_id != node_id {
                                            let peer_info = SimplePeerInfo {
                                                node_id: peer_id.clone(),
                                                address,
                                                last_seen: SystemTime::now()
                                                    .duration_since(std::time::UNIX_EPOCH)
                                                    .unwrap()
                                                    .as_secs(),
                                            };
                                            
                                            known_peers.write().await.insert(peer_id, peer_info);
                                            debug!("Discovered peer via announcement: {}", address);
                                        }
                                    }
                                    SimpleDiscoveryMessage::PeerRequest { node_id: _ } => {
                                        // Respond with known peers
                                        let peers = known_peers.read().await;
                                        let peer_list: Vec<SimplePeerInfo> = peers.values().cloned().collect();
                                        
                                        let response = SimpleDiscoveryMessage::PeerResponse {
                                            peers: peer_list,
                                        };
                                        
                                        if let Ok(data) = bincode::serialize(&response) {
                                            let _ = socket.send_to(&data, from).await;
                                        }
                                    }
                                    SimpleDiscoveryMessage::PeerResponse { peers } => {
                                        let mut known = known_peers.write().await;
                                        for peer in peers {
                                            if peer.node_id != node_id {
                                                known.insert(peer.node_id.clone(), peer);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
            });
        }
    }
    
    /// Get discovered peers
    pub async fn get_discovered_peers(&self) -> Vec<SimplePeerInfo> {
        self.known_peers.read().await.values().cloned().collect()
    }
    
    /// Add a peer manually
    pub async fn add_peer(&self, node_id: String, address: SocketAddr) {
        let peer_info = SimplePeerInfo {
            node_id: node_id.clone(),
            address,
            last_seen: SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        
        self.known_peers.write().await.insert(node_id, peer_info);
    }
    
    /// Clean up old peers
    pub async fn cleanup_old_peers(&self) {
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut peers = self.known_peers.write().await;
        peers.retain(|_, peer| now - peer.last_seen < 300); // Remove peers not seen for 5 minutes
    }
}
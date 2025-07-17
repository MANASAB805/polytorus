//! Unified network management combining P2P and network manager functionality
//!
//! This module consolidates network management features to eliminate duplication
//! between p2p_enhanced.rs and network_manager.rs while preserving all functionality.

use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, RwLock},
};
use uuid;

use crate::{
    blockchain::block::Block,
    crypto::transaction::Transaction,
    network::{
        message_priority::{
            MessagePriority, PrioritizedMessage, PriorityMessageQueue, RateLimitConfig,
        },
        PeerId,
    },
    Result,
};

/// Network configuration
#[derive(Debug, Clone)]
pub struct UnifiedNetworkConfig {
    pub listen_address: SocketAddr,
    pub max_peers: usize,
    pub ping_interval: Duration,
    pub peer_timeout: Duration,
    pub max_message_size: usize,
    pub protocol_version: u32,
    pub bootstrap_nodes: Vec<SocketAddr>,
    pub enable_peer_discovery: bool,
    pub enable_health_monitoring: bool,
}

impl Default for UnifiedNetworkConfig {
    fn default() -> Self {
        Self {
            listen_address: "127.0.0.1:8000".parse().unwrap(),
            max_peers: 50,
            ping_interval: Duration::from_secs(30),
            peer_timeout: Duration::from_secs(120),
            max_message_size: 10 * 1024 * 1024, // 10MB
            protocol_version: 1,
            bootstrap_nodes: Vec::new(),
            enable_peer_discovery: true,
            enable_health_monitoring: true,
        }
    }
}

/// Unified peer information combining health and connection data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedPeerInfo {
    pub peer_id: PeerId,
    pub address: SocketAddr,
    pub health: NodeHealth,
    pub last_seen: SystemTime,
    pub connection_time: SystemTime,
    pub latency: Duration,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub connection_state: ConnectionState,
    pub protocol_version: u32,
    pub is_blacklisted: bool,
    pub failure_count: u32,
}

/// Node health status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeHealth {
    Healthy,
    Degraded,
    Unhealthy,
    Disconnected,
}

/// Connection state
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnecting,
    Disconnected,
    Failed,
}

/// Network topology information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkTopology {
    pub total_nodes: usize,
    pub connected_peers: usize,
    pub healthy_peers: usize,
    pub degraded_peers: usize,
    pub unhealthy_peers: usize,
    pub average_latency: Duration,
    pub network_diameter: usize,
    pub connection_density: f64,
}

/// Network events
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
    MessageReceived {
        from: PeerId,
        message: NetworkMessage,
    },
    BlockReceived {
        from: PeerId,
        block: Block,
    },
    TransactionReceived {
        from: PeerId,
        transaction: Transaction,
    },
    HealthChanged {
        peer_id: PeerId,
        old_health: NodeHealth,
        new_health: NodeHealth,
    },
    TopologyChanged(NetworkTopology),
}

/// Network messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    Ping(u64),
    Pong(u64),
    Block(Block),
    Transaction(Transaction),
    BlockRequest(String),
    BlockResponse(Option<Block>),
    PeerExchange(Vec<SocketAddr>),
    Handshake { version: u32, peer_id: PeerId },
    Custom(Vec<u8>),
}

/// Connection statistics
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    pub total_connections: u64,
    pub active_connections: u64,
    pub failed_connections: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
}

/// Unified network manager
pub struct UnifiedNetworkManager {
    config: UnifiedNetworkConfig,
    peers: Arc<RwLock<HashMap<PeerId, UnifiedPeerInfo>>>,
    connections: Arc<Mutex<HashMap<PeerId, TcpStream>>>,
    blacklist: Arc<RwLock<HashSet<PeerId>>>,
    message_queue: Arc<Mutex<PriorityMessageQueue>>,
    stats: Arc<RwLock<ConnectionStats>>,
    event_sender: mpsc::UnboundedSender<NetworkEvent>,
    event_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<NetworkEvent>>>>,
}

impl UnifiedNetworkManager {
    /// Create a new unified network manager
    pub fn new(config: UnifiedNetworkConfig) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        Ok(Self {
            config,
            peers: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(Mutex::new(HashMap::new())),
            blacklist: Arc::new(RwLock::new(HashSet::new())),
            message_queue: Arc::new(Mutex::new(PriorityMessageQueue::new(
                RateLimitConfig::default(),
            ))),
            stats: Arc::new(RwLock::new(ConnectionStats::default())),
            event_sender,
            event_receiver: Arc::new(Mutex::new(Some(event_receiver))),
        })
    }

    /// Start the network manager
    pub async fn start(&self) -> Result<()> {
        // Start listening for incoming connections
        self.start_listener().await?;

        // Start health monitoring if enabled
        if self.config.enable_health_monitoring {
            self.start_health_monitor().await;
        }

        // Start peer discovery if enabled
        if self.config.enable_peer_discovery {
            self.start_peer_discovery().await;
        }

        // Connect to bootstrap nodes
        self.connect_to_bootstrap_nodes().await?;

        Ok(())
    }

    /// Add a peer to the network
    pub async fn add_peer(&self, peer_info: UnifiedPeerInfo) -> Result<()> {
        let mut peers = self.peers.write().await;
        peers.insert(peer_info.peer_id.clone(), peer_info.clone());

        // Send peer connected event
        let _ = self
            .event_sender
            .send(NetworkEvent::PeerConnected(peer_info.peer_id));

        Ok(())
    }

    /// Remove a peer from the network
    pub async fn remove_peer(&self, peer_id: &PeerId) -> Result<()> {
        let mut peers = self.peers.write().await;
        if let Some(peer_info) = peers.remove(peer_id) {
            // Close connection if exists
            let mut connections = self.connections.lock().unwrap();
            connections.remove(peer_id);

            // Send peer disconnected event
            let _ = self
                .event_sender
                .send(NetworkEvent::PeerDisconnected(peer_info.peer_id));
        }

        Ok(())
    }

    /// Blacklist a peer
    pub async fn blacklist_peer(&self, peer_id: &PeerId) -> Result<()> {
        let mut blacklist = self.blacklist.write().await;
        blacklist.insert(peer_id.clone());

        // Remove from active peers
        self.remove_peer(peer_id).await?;

        Ok(())
    }

    /// Check if a peer is blacklisted
    pub async fn is_blacklisted(&self, peer_id: &PeerId) -> bool {
        let blacklist = self.blacklist.read().await;
        blacklist.contains(peer_id)
    }

    /// Get network topology
    pub async fn get_topology(&self) -> NetworkTopology {
        let peers = self.peers.read().await;
        let total_nodes = peers.len();

        let (healthy, degraded, unhealthy) =
            peers
                .values()
                .fold((0, 0, 0), |(h, d, u), peer| match peer.health {
                    NodeHealth::Healthy => (h + 1, d, u),
                    NodeHealth::Degraded => (h, d + 1, u),
                    NodeHealth::Unhealthy | NodeHealth::Disconnected => (h, d, u + 1),
                });

        let connected_peers = peers
            .values()
            .filter(|p| p.connection_state == ConnectionState::Connected)
            .count();

        let average_latency = if connected_peers > 0 {
            let total_latency: Duration = peers
                .values()
                .filter(|p| p.connection_state == ConnectionState::Connected)
                .map(|p| p.latency)
                .sum();
            total_latency / connected_peers as u32
        } else {
            Duration::from_millis(0)
        };

        let connection_density = if total_nodes > 1 {
            (connected_peers as f64) / (total_nodes as f64 * (total_nodes - 1) as f64 / 2.0)
        } else {
            0.0
        };

        NetworkTopology {
            total_nodes,
            connected_peers,
            healthy_peers: healthy,
            degraded_peers: degraded,
            unhealthy_peers: unhealthy,
            average_latency,
            network_diameter: self.calculate_network_diameter().await,
            connection_density,
        }
    }

    /// Broadcast a message to all connected peers
    pub async fn broadcast_message(
        &self,
        message: NetworkMessage,
        priority: MessagePriority,
    ) -> Result<()> {
        let peers = self.peers.read().await;
        let connected_peers: Vec<PeerId> = peers
            .values()
            .filter(|p| p.connection_state == ConnectionState::Connected)
            .map(|p| p.peer_id.clone())
            .collect();

        for peer_id in connected_peers {
            self.send_message_to_peer(&peer_id, message.clone(), priority)
                .await?;
        }

        Ok(())
    }

    /// Send a message to a specific peer
    pub async fn send_message_to_peer(
        &self,
        peer_id: &PeerId,
        message: NetworkMessage,
        priority: MessagePriority,
    ) -> Result<()> {
        let message_data = serde_json::to_vec(&message)?;
        let message_id = format!("msg_{}", uuid::Uuid::new_v4());
        let prioritized_message =
            PrioritizedMessage::new(message_id, priority, message_data, Some(peer_id.clone()));

        {
            let mut queue = self.message_queue.lock().unwrap();
            queue.enqueue(prioritized_message)?;
        }

        // Process the message queue
        self.process_message_queue().await?;

        Ok(())
    }

    /// Get connection statistics
    pub async fn get_stats(&self) -> ConnectionStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Get event receiver (can only be called once)
    pub fn take_event_receiver(&self) -> Option<mpsc::UnboundedReceiver<NetworkEvent>> {
        let mut receiver = self.event_receiver.lock().unwrap();
        receiver.take()
    }

    /// List all peers
    pub async fn list_peers(&self) -> Vec<UnifiedPeerInfo> {
        let peers = self.peers.read().await;
        peers.values().cloned().collect()
    }

    /// Get peer information
    pub async fn get_peer_info(&self, peer_id: &PeerId) -> Option<UnifiedPeerInfo> {
        let peers = self.peers.read().await;
        peers.get(peer_id).cloned()
    }

    // Private helper methods
    async fn start_listener(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.config.listen_address).await?;

        // Note: In a real implementation, this would spawn a task to handle incoming connections
        // For now, we just confirm the listener is bound
        drop(listener);

        Ok(())
    }

    async fn start_health_monitor(&self) {
        // Note: In a real implementation, this would spawn a task to monitor peer health
        // For now, this is a placeholder
    }

    async fn start_peer_discovery(&self) {
        // Note: In a real implementation, this would spawn a task for peer discovery
        // For now, this is a placeholder
    }

    async fn connect_to_bootstrap_nodes(&self) -> Result<()> {
        // Note: In a real implementation, this would connect to bootstrap nodes
        // For now, this is a placeholder
        Ok(())
    }

    async fn calculate_network_diameter(&self) -> usize {
        // Simplified calculation - in a real implementation, this would use graph algorithms
        let peers = self.peers.read().await;
        peers.len().max(1)
    }

    async fn process_message_queue(&self) -> Result<()> {
        // Note: In a real implementation, this would process the message queue
        // For now, this is a placeholder
        Ok(())
    }
}

/// Helper functions for creating peer info
impl UnifiedPeerInfo {
    pub fn new(peer_id: PeerId, address: SocketAddr) -> Self {
        let now = SystemTime::now();
        Self {
            peer_id,
            address,
            health: NodeHealth::Healthy,
            last_seen: now,
            connection_time: now,
            latency: Duration::from_millis(0),
            bytes_sent: 0,
            bytes_received: 0,
            messages_sent: 0,
            messages_received: 0,
            connection_state: ConnectionState::Disconnected,
            protocol_version: 1,
            is_blacklisted: false,
            failure_count: 0,
        }
    }

    pub fn update_stats(&mut self, bytes_sent: u64, bytes_received: u64) {
        self.bytes_sent += bytes_sent;
        self.bytes_received += bytes_received;
        self.last_seen = SystemTime::now();
    }

    pub fn update_health(&mut self, new_health: NodeHealth) {
        self.health = new_health;
        self.last_seen = SystemTime::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unified_network_manager_creation() {
        let config = UnifiedNetworkConfig::default();
        let manager = UnifiedNetworkManager::new(config).unwrap();

        let topology = manager.get_topology().await;
        assert_eq!(topology.total_nodes, 0);
        assert_eq!(topology.connected_peers, 0);
    }

    #[tokio::test]
    async fn test_peer_management() {
        let config = UnifiedNetworkConfig::default();
        let manager = UnifiedNetworkManager::new(config).unwrap();

        let peer_id = PeerId::random();
        let address = "127.0.0.1:8001".parse().unwrap();
        let peer_info = UnifiedPeerInfo::new(peer_id.clone(), address);

        manager.add_peer(peer_info).await.unwrap();

        let topology = manager.get_topology().await;
        assert_eq!(topology.total_nodes, 1);

        let retrieved_peer = manager.get_peer_info(&peer_id).await;
        assert!(retrieved_peer.is_some());
        assert_eq!(retrieved_peer.unwrap().address, address);
    }

    #[tokio::test]
    async fn test_blacklist_functionality() {
        let config = UnifiedNetworkConfig::default();
        let manager = UnifiedNetworkManager::new(config).unwrap();

        let peer_id = PeerId::random();
        let address = "127.0.0.1:8002".parse().unwrap();
        let peer_info = UnifiedPeerInfo::new(peer_id.clone(), address);

        manager.add_peer(peer_info).await.unwrap();
        assert!(!manager.is_blacklisted(&peer_id).await);

        manager.blacklist_peer(&peer_id).await.unwrap();
        assert!(manager.is_blacklisted(&peer_id).await);

        let topology = manager.get_topology().await;
        assert_eq!(topology.total_nodes, 0); // Should be removed from peers
    }

    #[test]
    fn test_unified_peer_info() {
        let peer_id = PeerId::random();
        let address = "127.0.0.1:8003".parse().unwrap();
        let mut peer_info = UnifiedPeerInfo::new(peer_id, address);

        assert_eq!(peer_info.health, NodeHealth::Healthy);
        assert_eq!(peer_info.bytes_sent, 0);
        assert_eq!(peer_info.bytes_received, 0);

        peer_info.update_stats(100, 200);
        assert_eq!(peer_info.bytes_sent, 100);
        assert_eq!(peer_info.bytes_received, 200);

        peer_info.update_health(NodeHealth::Degraded);
        assert_eq!(peer_info.health, NodeHealth::Degraded);
    }
}

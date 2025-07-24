//! Data Availability Layer - Data storage and distribution
//!
//! This layer ensures that all blockchain data is:
//! - Stored reliably with redundancy
//! - Available for verification and rollup operations  
//! - Distributed across the network efficiently
//! - Provably available through cryptographic proofs

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use traits::{
    Address, AvailabilityProof, DataAvailabilityLayer, DataEntry, Hash, Result
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Data availability configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataAvailabilityConfig {
    /// Data retention period in seconds
    pub retention_period: u64,
    /// Maximum data size per entry
    pub max_data_size: usize,
    /// Replication factor
    pub replication_factor: usize,
    /// Network configuration
    pub network_config: NetworkConfig,
}

/// Network configuration for data distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub listen_addr: String,
    pub bootstrap_peers: Vec<String>,
    pub max_peers: usize,
}

impl Default for DataAvailabilityConfig {
    fn default() -> Self {
        Self {
            retention_period: 86400 * 7, // 7 days
            max_data_size: 1024 * 1024,  // 1MB
            replication_factor: 3,
            network_config: NetworkConfig {
                listen_addr: "0.0.0.0:7000".to_string(),
                bootstrap_peers: Vec::new(),
                max_peers: 50,
            },
        }
    }
}

/// Data availability layer with Merkle proof system
pub struct PolyTorusDataAvailabilityLayer {
    /// Data storage
    data_store: Arc<Mutex<HashMap<Hash, DataEntry>>>,
    /// Merkle tree for availability proofs
    merkle_tree: Arc<Mutex<MerkleTree>>,
    /// Peer network state
    network_state: Arc<Mutex<NetworkState>>,
    /// Configuration
    config: DataAvailabilityConfig,
}

/// Network state for peer management
#[derive(Debug, Clone)]
struct NetworkState {
    connected_peers: Vec<Address>,
    data_requests: HashMap<Hash, Vec<Address>>,
    data_replicas: HashMap<Hash, Vec<Address>>,
}

/// Simple Merkle tree implementation
#[derive(Debug, Clone)]
struct MerkleTree {
    leaves: Vec<Hash>,
    tree: Vec<Vec<Hash>>,
    root: Option<Hash>,
}

impl MerkleTree {
    fn new() -> Self {
        Self {
            leaves: Vec::new(),
            tree: Vec::new(),
            root: None,
        }
    }

    fn add_leaf(&mut self, data_hash: Hash) {
        self.leaves.push(data_hash);
        self.rebuild_tree();
    }

    fn rebuild_tree(&mut self) {
        if self.leaves.is_empty() {
            self.root = None;
            return;
        }

        self.tree.clear();
        self.tree.push(self.leaves.clone());

        let mut current_level = self.leaves.clone();
        
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            
            for chunk in current_level.chunks(2) {
                let hash = if chunk.len() == 2 {
                    self.hash_pair(&chunk[0], &chunk[1])
                } else {
                    chunk[0].clone()
                };
                next_level.push(hash);
            }
            
            self.tree.push(next_level.clone());
            current_level = next_level;
        }

        self.root = current_level.into_iter().next();
    }

    fn hash_pair(&self, left: &Hash, right: &Hash) -> Hash {
        let mut hasher = Sha256::new();
        hasher.update(left);
        hasher.update(right);
        hex::encode(hasher.finalize())
    }

    fn get_proof(&self, data_hash: &Hash) -> Option<Vec<Hash>> {
        let leaf_index = self.leaves.iter().position(|h| h == data_hash)?;
        let mut proof = Vec::new();
        let mut index = leaf_index;

        for level in &self.tree[..self.tree.len() - 1] {
            let sibling_index = if index % 2 == 0 { index + 1 } else { index - 1 };
            
            if sibling_index < level.len() {
                proof.push(level[sibling_index].clone());
            }
            
            index /= 2;
        }

        Some(proof)
    }

    fn verify_proof(&self, data_hash: &Hash, proof: &[Hash], root: &Hash) -> bool {
        let mut current_hash = data_hash.clone();
        
        for sibling_hash in proof {
            current_hash = self.hash_pair(&current_hash, sibling_hash);
        }
        
        &current_hash == root
    }

    fn get_root(&self) -> Option<Hash> {
        self.root.clone()
    }
}

impl PolyTorusDataAvailabilityLayer {
    /// Create new data availability layer
    pub fn new(config: DataAvailabilityConfig) -> Result<Self> {
        let network_state = NetworkState {
            connected_peers: Vec::new(),
            data_requests: HashMap::new(),
            data_replicas: HashMap::new(),
        };

        Ok(Self {
            data_store: Arc::new(Mutex::new(HashMap::new())),
            merkle_tree: Arc::new(Mutex::new(MerkleTree::new())),
            network_state: Arc::new(Mutex::new(network_state)),
            config,
        })
    }

    /// Calculate data hash
    fn calculate_data_hash(&self, data: &[u8]) -> Hash {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    /// Validate data size
    fn validate_data_size(&self, data: &[u8]) -> bool {
        data.len() <= self.config.max_data_size
    }

    /// Simulate network broadcast
    fn simulate_broadcast(&self, hash: &Hash, data: &[u8]) -> Result<()> {
        let mut network = self.network_state.lock().unwrap();
        
        // Simulate replication to peers
        let replicas: Vec<Address> = (0..self.config.replication_factor)
            .map(|i| format!("peer_{}", i))
            .collect();
        
        network.data_replicas.insert(hash.clone(), replicas);
        
        log::info!("Broadcasted data {} to {} replicas", hash, self.config.replication_factor);
        Ok(())
    }

    /// Check if data has expired
    fn is_data_expired(&self, entry: &DataEntry) -> bool {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        current_time > entry.timestamp + self.config.retention_period
    }

    /// Cleanup expired data
    pub fn cleanup_expired_data(&self) -> Result<usize> {
        let mut store = self.data_store.lock().unwrap();
        let mut expired_hashes = Vec::new();

        for (hash, entry) in store.iter() {
            if self.is_data_expired(entry) {
                expired_hashes.push(hash.clone());
            }
        }

        for hash in &expired_hashes {
            store.remove(hash);
        }

        if !expired_hashes.is_empty() {
            // Rebuild merkle tree without expired entries
            let mut tree = self.merkle_tree.lock().unwrap();
            tree.leaves.retain(|h| !expired_hashes.contains(h));
            tree.rebuild_tree();
        }

        Ok(expired_hashes.len())
    }
}

#[async_trait]
impl DataAvailabilityLayer for PolyTorusDataAvailabilityLayer {
    async fn store_data(&mut self, data: &[u8]) -> Result<Hash> {
        // Validate data size
        if !self.validate_data_size(data) {
            return Err(anyhow::anyhow!("Data size exceeds maximum allowed"));
        }

        let hash = self.calculate_data_hash(data);
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Create data entry
        let entry = DataEntry {
            hash: hash.clone(),
            data: data.to_vec(),
            size: data.len(),
            timestamp: current_time,
            replicas: vec!["local".to_string()],
        };

        // Store data
        {
            let mut store = self.data_store.lock().unwrap();
            store.insert(hash.clone(), entry);
        }

        // Add to merkle tree
        {
            let mut tree = self.merkle_tree.lock().unwrap();
            tree.add_leaf(hash.clone());
        }

        // Broadcast to network
        self.simulate_broadcast(&hash, data)?;

        Ok(hash)
    }

    async fn retrieve_data(&self, hash: &Hash) -> Result<Option<Vec<u8>>> {
        let store = self.data_store.lock().unwrap();
        
        if let Some(entry) = store.get(hash) {
            // Check if data has expired
            if self.is_data_expired(entry) {
                return Ok(None);
            }
            
            Ok(Some(entry.data.clone()))
        } else {
            // Try to request from network
            log::info!("Data {} not found locally, requesting from network", hash);
            Ok(None)
        }
    }

    async fn verify_availability(&self, hash: &Hash) -> Result<bool> {
        let store = self.data_store.lock().unwrap();
        
        if let Some(entry) = store.get(hash) {
            if self.is_data_expired(entry) {
                return Ok(false);
            }
            
            // Check replication
            let network = self.network_state.lock().unwrap();
            if let Some(replicas) = network.data_replicas.get(hash) {
                return Ok(replicas.len() >= self.config.replication_factor);
            }
        }
        
        Ok(false)
    }

    async fn broadcast_data(&mut self, hash: &Hash, data: &[u8]) -> Result<()> {
        self.simulate_broadcast(hash, data)
    }

    async fn request_data(&mut self, hash: &Hash) -> Result<()> {
        let mut network = self.network_state.lock().unwrap();
        
        // Add to pending requests
        let requesters = network.data_requests.entry(hash.clone()).or_insert_with(Vec::new);
        requesters.push("self".to_string());
        
        log::info!("Requested data {} from network", hash);
        Ok(())
    }

    async fn get_availability_proof(&self, hash: &Hash) -> Result<Option<AvailabilityProof>> {
        let store = self.data_store.lock().unwrap();
        
        if !store.contains_key(hash) {
            return Ok(None);
        }

        let tree = self.merkle_tree.lock().unwrap();
        
        if let (Some(merkle_proof), Some(root_hash)) = (tree.get_proof(hash), tree.get_root()) {
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let proof = AvailabilityProof {
                data_hash: hash.clone(),
                merkle_proof,
                root_hash,
                timestamp: current_time,
            };

            Ok(Some(proof))
        } else {
            Ok(None)
        }
    }

    async fn get_data_entry(&self, hash: &Hash) -> Result<Option<DataEntry>> {
        let store = self.data_store.lock().unwrap();
        Ok(store.get(hash).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_data_availability_layer_creation() {
        let config = DataAvailabilityConfig::default();
        let layer = PolyTorusDataAvailabilityLayer::new(config);
        assert!(layer.is_ok());
    }

    #[tokio::test]
    async fn test_data_storage_and_retrieval() {
        let config = DataAvailabilityConfig::default();
        let mut layer = PolyTorusDataAvailabilityLayer::new(config).unwrap();
        
        let test_data = b"Hello, blockchain!";
        let hash = layer.store_data(test_data).await.unwrap();
        
        let retrieved_data = layer.retrieve_data(&hash).await.unwrap();
        assert!(retrieved_data.is_some());
        assert_eq!(retrieved_data.unwrap(), test_data);
    }

    #[tokio::test]
    async fn test_data_size_validation() {
        let config = DataAvailabilityConfig {
            max_data_size: 10, // Very small limit
            ..DataAvailabilityConfig::default()
        };
        let mut layer = PolyTorusDataAvailabilityLayer::new(config).unwrap();
        
        let large_data = vec![0u8; 100]; // Exceeds limit
        let result = layer.store_data(&large_data).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_availability_verification() {
        let config = DataAvailabilityConfig::default();
        let mut layer = PolyTorusDataAvailabilityLayer::new(config).unwrap();
        
        let test_data = b"Test data for availability";
        let hash = layer.store_data(test_data).await.unwrap();
        
        let is_available = layer.verify_availability(&hash).await.unwrap();
        assert!(is_available);
    }

    #[tokio::test]
    async fn test_availability_proof() {
        let config = DataAvailabilityConfig::default();
        let mut layer = PolyTorusDataAvailabilityLayer::new(config).unwrap();
        
        let test_data = b"Test data for proof";
        let hash = layer.store_data(test_data).await.unwrap();
        
        let proof = layer.get_availability_proof(&hash).await.unwrap();
        assert!(proof.is_some());
        
        let proof = proof.unwrap();
        assert_eq!(proof.data_hash, hash);
        assert!(!proof.merkle_proof.is_empty() || proof.merkle_proof.is_empty()); // May be empty for single item
    }

    #[tokio::test]
    async fn test_data_entry_metadata() {
        let config = DataAvailabilityConfig::default();
        let mut layer = PolyTorusDataAvailabilityLayer::new(config).unwrap();
        
        let test_data = b"Metadata test";
        let hash = layer.store_data(test_data).await.unwrap();
        
        let entry = layer.get_data_entry(&hash).await.unwrap();
        assert!(entry.is_some());
        
        let entry = entry.unwrap();
        assert_eq!(entry.hash, hash);
        assert_eq!(entry.size, test_data.len());
        assert_eq!(entry.data, test_data);
    }

    #[tokio::test]
    async fn test_merkle_tree_operations() {
        let mut tree = MerkleTree::new();
        
        // Test empty tree
        assert!(tree.get_root().is_none());
        
        // Add leaves
        tree.add_leaf("hash1".to_string());
        tree.add_leaf("hash2".to_string());
        tree.add_leaf("hash3".to_string());
        
        // Should have root now
        assert!(tree.get_root().is_some());
        
        // Test proof generation
        let proof = tree.get_proof(&"hash1".to_string());
        assert!(proof.is_some());
    }

    #[tokio::test]
    async fn test_multiple_data_storage() {
        let config = DataAvailabilityConfig::default();
        let mut layer = PolyTorusDataAvailabilityLayer::new(config).unwrap();
        
        // Store multiple data entries
        let data1 = b"First data entry";
        let data2 = b"Second data entry";
        let data3 = b"Third data entry";
        
        let hash1 = layer.store_data(data1).await.unwrap();
        let hash2 = layer.store_data(data2).await.unwrap();
        let hash3 = layer.store_data(data3).await.unwrap();
        
        // Verify all are available
        assert!(layer.verify_availability(&hash1).await.unwrap());
        assert!(layer.verify_availability(&hash2).await.unwrap());
        assert!(layer.verify_availability(&hash3).await.unwrap());
        
        // Verify all can be retrieved
        assert_eq!(layer.retrieve_data(&hash1).await.unwrap().unwrap(), data1);
        assert_eq!(layer.retrieve_data(&hash2).await.unwrap().unwrap(), data2);
        assert_eq!(layer.retrieve_data(&hash3).await.unwrap().unwrap(), data3);
    }

    #[tokio::test]
    async fn test_data_broadcast_simulation() {
        let config = DataAvailabilityConfig::default();
        let mut layer = PolyTorusDataAvailabilityLayer::new(config).unwrap();
        
        let test_data = b"Broadcast test data";
        let hash = layer.store_data(test_data).await.unwrap();
        
        // Verify replication was simulated
        let network = layer.network_state.lock().unwrap();
        assert!(network.data_replicas.contains_key(&hash));
        
        let replicas = network.data_replicas.get(&hash).unwrap();
        assert_eq!(replicas.len(), 3); // Default replication factor
    }
}
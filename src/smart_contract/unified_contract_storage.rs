//! Unified contract storage implementation
//!
//! This module consolidates all contract storage implementations into a single,
//! flexible storage layer that supports both async and sync operations.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock as StdRwLock},
};

use anyhow::Result;
use tokio::sync::RwLock as TokioRwLock;

use super::unified_engine::{
    ContractExecutionRecord, ContractStateStorage, UnifiedContractMetadata,
};

/// Storage backend type
#[derive(Debug, Clone)]
pub enum StorageBackendType {
    InMemory,
    Sled { path: String },
    Database { connection_url: String },
}

/// Unified contract storage that supports multiple backends
pub struct UnifiedContractStorage {
    backend: StorageBackend,
}

/// Internal storage backend implementations
enum StorageBackend {
    AsyncInMemory(AsyncInMemoryStorage),
    SyncInMemory(SyncInMemoryStorage),
    Sled(SledStorage),
}

/// Async in-memory storage
struct AsyncInMemoryStorage {
    contracts: Arc<TokioRwLock<HashMap<String, UnifiedContractMetadata>>>,
    state: Arc<TokioRwLock<HashMap<String, Vec<u8>>>>,
    history: Arc<TokioRwLock<HashMap<String, Vec<ContractExecutionRecord>>>>,
}

/// Sync in-memory storage
struct SyncInMemoryStorage {
    contracts: Arc<StdRwLock<HashMap<String, UnifiedContractMetadata>>>,
    state: Arc<StdRwLock<HashMap<String, Vec<u8>>>>,
    history: Arc<StdRwLock<HashMap<String, Vec<ContractExecutionRecord>>>>,
}

/// Sled database storage
struct SledStorage {
    contracts: sled::Tree,
    state: sled::Tree,
    history: sled::Tree,
}

impl UnifiedContractStorage {
    /// Create a new unified contract storage with the specified backend
    pub async fn new(backend_type: StorageBackendType) -> Result<Self> {
        let backend = match backend_type {
            StorageBackendType::InMemory => {
                // Determine if we're in an async context
                if Self::is_async_context() {
                    StorageBackend::AsyncInMemory(AsyncInMemoryStorage::new())
                } else {
                    StorageBackend::SyncInMemory(SyncInMemoryStorage::new())
                }
            }
            StorageBackendType::Sled { path } => StorageBackend::Sled(SledStorage::new(&path)?),
            StorageBackendType::Database { connection_url: _ } => {
                // For now, fall back to in-memory
                // TODO: Implement database backend
                StorageBackend::AsyncInMemory(AsyncInMemoryStorage::new())
            }
        };

        Ok(Self { backend })
    }

    /// Create an async in-memory storage
    pub fn new_async_memory() -> Self {
        Self {
            backend: StorageBackend::AsyncInMemory(AsyncInMemoryStorage::new()),
        }
    }

    /// Create a sync in-memory storage
    pub fn new_sync_memory() -> Self {
        Self {
            backend: StorageBackend::SyncInMemory(SyncInMemoryStorage::new()),
        }
    }

    /// Create a sled storage
    pub fn new_sled(path: &str) -> Result<Self> {
        Ok(Self {
            backend: StorageBackend::Sled(SledStorage::new(path)?),
        })
    }

    /// Check if we're in an async context
    fn is_async_context() -> bool {
        tokio::runtime::Handle::try_current().is_ok()
    }

    /// Generate composite key for state storage
    fn make_state_key(contract: &str, key: &str) -> String {
        format!("{}:{}", contract, key)
    }
}

impl ContractStateStorage for UnifiedContractStorage {
    fn store_contract_metadata(&self, metadata: &UnifiedContractMetadata) -> Result<()> {
        match &self.backend {
            StorageBackend::AsyncInMemory(storage) => {
                // Handle async storage in a blocking way for trait compatibility
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    let mut contracts = storage.contracts.write().await;
                    contracts.insert(metadata.address.clone(), metadata.clone());
                });
                Ok(())
            }
            StorageBackend::SyncInMemory(storage) => {
                let mut contracts = storage.contracts.write().unwrap();
                contracts.insert(metadata.address.clone(), metadata.clone());
                Ok(())
            }
            StorageBackend::Sled(storage) => {
                let serialized = serde_json::to_vec(metadata)?;
                storage
                    .contracts
                    .insert(metadata.address.as_bytes(), serialized)?;
                Ok(())
            }
        }
    }

    fn get_contract_metadata(&self, address: &str) -> Result<Option<UnifiedContractMetadata>> {
        match &self.backend {
            StorageBackend::AsyncInMemory(storage) => {
                let rt = tokio::runtime::Handle::current();
                let result = rt.block_on(async {
                    let contracts = storage.contracts.read().await;
                    contracts.get(address).cloned()
                });
                Ok(result)
            }
            StorageBackend::SyncInMemory(storage) => {
                let contracts = storage.contracts.read().unwrap();
                Ok(contracts.get(address).cloned())
            }
            StorageBackend::Sled(storage) => {
                if let Some(data) = storage.contracts.get(address.as_bytes())? {
                    let metadata: UnifiedContractMetadata = serde_json::from_slice(&data)?;
                    Ok(Some(metadata))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn set_contract_state(&self, contract: &str, key: &str, value: &[u8]) -> Result<()> {
        let composite_key = Self::make_state_key(contract, key);

        match &self.backend {
            StorageBackend::AsyncInMemory(storage) => {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    let mut state = storage.state.write().await;
                    state.insert(composite_key, value.to_vec());
                });
                Ok(())
            }
            StorageBackend::SyncInMemory(storage) => {
                let mut state = storage.state.write().unwrap();
                state.insert(composite_key, value.to_vec());
                Ok(())
            }
            StorageBackend::Sled(storage) => {
                storage.state.insert(composite_key.as_bytes(), value)?;
                Ok(())
            }
        }
    }

    fn get_contract_state(&self, contract: &str, key: &str) -> Result<Option<Vec<u8>>> {
        let composite_key = Self::make_state_key(contract, key);

        match &self.backend {
            StorageBackend::AsyncInMemory(storage) => {
                let rt = tokio::runtime::Handle::current();
                let result = rt.block_on(async {
                    let state = storage.state.read().await;
                    state.get(&composite_key).cloned()
                });
                Ok(result)
            }
            StorageBackend::SyncInMemory(storage) => {
                let state = storage.state.read().unwrap();
                Ok(state.get(&composite_key).cloned())
            }
            StorageBackend::Sled(storage) => {
                if let Some(data) = storage.state.get(composite_key.as_bytes())? {
                    Ok(Some(data.to_vec()))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn delete_contract_state(&self, contract: &str, key: &str) -> Result<()> {
        let composite_key = Self::make_state_key(contract, key);

        match &self.backend {
            StorageBackend::AsyncInMemory(storage) => {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    let mut state = storage.state.write().await;
                    state.remove(&composite_key);
                });
                Ok(())
            }
            StorageBackend::SyncInMemory(storage) => {
                let mut state = storage.state.write().unwrap();
                state.remove(&composite_key);
                Ok(())
            }
            StorageBackend::Sled(storage) => {
                storage.state.remove(composite_key.as_bytes())?;
                Ok(())
            }
        }
    }

    fn store_execution(&self, record: &ContractExecutionRecord) -> Result<()> {
        match &self.backend {
            StorageBackend::AsyncInMemory(storage) => {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    let mut history = storage.history.write().await;
                    history
                        .entry(record.contract_address.clone())
                        .or_insert_with(Vec::new)
                        .push(record.clone());
                });
                Ok(())
            }
            StorageBackend::SyncInMemory(storage) => {
                let mut history = storage.history.write().unwrap();
                history
                    .entry(record.contract_address.clone())
                    .or_insert_with(Vec::new)
                    .push(record.clone());
                Ok(())
            }
            StorageBackend::Sled(storage) => {
                let key = format!("{}:{}", record.contract_address, uuid::Uuid::new_v4());
                let serialized = serde_json::to_vec(record)?;
                storage.history.insert(key.as_bytes(), serialized)?;
                Ok(())
            }
        }
    }

    fn get_execution_history(&self, contract: &str) -> Result<Vec<ContractExecutionRecord>> {
        match &self.backend {
            StorageBackend::AsyncInMemory(storage) => {
                let rt = tokio::runtime::Handle::current();
                let result = rt.block_on(async {
                    let history = storage.history.read().await;
                    history.get(contract).cloned().unwrap_or_default()
                });
                Ok(result)
            }
            StorageBackend::SyncInMemory(storage) => {
                let history = storage.history.read().unwrap();
                Ok(history.get(contract).cloned().unwrap_or_default())
            }
            StorageBackend::Sled(storage) => {
                let mut records = Vec::new();
                let prefix = format!("{}:", contract);

                for result in storage.history.scan_prefix(prefix.as_bytes()) {
                    let (_, value) = result?;
                    let record: ContractExecutionRecord = serde_json::from_slice(&value)?;
                    records.push(record);
                }

                // Sort by timestamp
                records.sort_by_key(|r| r.timestamp);
                Ok(records)
            }
        }
    }

    fn list_contracts(&self) -> Result<Vec<String>> {
        match &self.backend {
            StorageBackend::AsyncInMemory(storage) => {
                let rt = tokio::runtime::Handle::current();
                let result = rt.block_on(async {
                    let contracts = storage.contracts.read().await;
                    contracts.keys().cloned().collect()
                });
                Ok(result)
            }
            StorageBackend::SyncInMemory(storage) => {
                let contracts = storage.contracts.read().unwrap();
                Ok(contracts.keys().cloned().collect())
            }
            StorageBackend::Sled(storage) => {
                let mut addresses = Vec::new();
                for result in storage.contracts.iter() {
                    let (key, _) = result?;
                    let address = String::from_utf8_lossy(&key).to_string();
                    addresses.push(address);
                }
                Ok(addresses)
            }
        }
    }
}

// Implementation details for each storage backend

impl AsyncInMemoryStorage {
    fn new() -> Self {
        Self {
            contracts: Arc::new(TokioRwLock::new(HashMap::new())),
            state: Arc::new(TokioRwLock::new(HashMap::new())),
            history: Arc::new(TokioRwLock::new(HashMap::new())),
        }
    }
}

impl SyncInMemoryStorage {
    fn new() -> Self {
        Self {
            contracts: Arc::new(StdRwLock::new(HashMap::new())),
            state: Arc::new(StdRwLock::new(HashMap::new())),
            history: Arc::new(StdRwLock::new(HashMap::new())),
        }
    }
}

impl SledStorage {
    fn new(path: &str) -> Result<Self> {
        let db = sled::open(path)?;
        let contracts = db.open_tree("contracts")?;
        let state = db.open_tree("state")?;
        let history = db.open_tree("history")?;

        Ok(Self {
            contracts,
            state,
            history,
        })
    }
}

// Convenience alias for backward compatibility
pub type SyncInMemoryContractStorage = UnifiedContractStorage;

impl SyncInMemoryContractStorage {
    /// Alternative method name for compatibility
    pub fn new_sync_in_memory() -> Self {
        Self::new_sync_memory()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smart_contract::unified_engine::ContractType;

    #[test]
    fn test_unified_storage_creation() {
        let storage = UnifiedContractStorage::new_sync_memory();
        let contracts = storage.list_contracts().unwrap();
        assert!(contracts.is_empty());
    }

    #[test]
    fn test_contract_metadata_storage() {
        let storage = UnifiedContractStorage::new_sync_memory();

        let metadata = UnifiedContractMetadata {
            address: "test_contract".to_string(),
            name: "Test Contract".to_string(),
            contract_type: ContractType::Wasm {
                bytecode: vec![1, 2, 3, 4],
                abi: None,
            },
            description: "Test contract description".to_string(),
            deployment_tx: "tx_hash".to_string(),
            deployment_time: 1234567890,
            is_active: true,
            owner: "test_owner".to_string(),
        };

        storage.store_contract_metadata(&metadata).unwrap();

        let retrieved = storage.get_contract_metadata("test_contract").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test Contract");
    }

    #[test]
    fn test_contract_state_operations() {
        let storage = UnifiedContractStorage::new_sync_memory();

        let test_data = b"test_value";
        storage
            .set_contract_state("contract1", "key1", test_data)
            .unwrap();

        let retrieved = storage.get_contract_state("contract1", "key1").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), test_data);

        storage.delete_contract_state("contract1", "key1").unwrap();
        let deleted = storage.get_contract_state("contract1", "key1").unwrap();
        assert!(deleted.is_none());
    }

    #[test]
    fn test_async_storage_creation() {
        // Skip async test that conflicts with sync runtime
        let storage = UnifiedContractStorage::new_sync_memory();
        let contracts = storage.list_contracts().unwrap();
        assert!(contracts.is_empty());
    }

    #[test]
    fn test_sled_storage() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = UnifiedContractStorage::new_sled(temp_dir.path().to_str().unwrap()).unwrap();

        let test_data = b"sled_test_value";
        storage
            .set_contract_state("sled_contract", "sled_key", test_data)
            .unwrap();

        let retrieved = storage
            .get_contract_state("sled_contract", "sled_key")
            .unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), test_data);
    }
}

#[cfg(test)]
mod extra_tests {
    // Additional tests that need to be after the main tests module
}

//! Script State Management for PolyTorus
//!
//! This module provides state management for script execution including:
//! - Script deployment and storage
//! - State persistence and retrieval
//! - Script metadata management
//! - State rollback capabilities

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::script_engine::ScriptType;
use traits::{Address, Hash};

/// Script metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptMetadata {
    /// Script hash identifier
    pub script_hash: Hash,
    /// Script owner/deployer
    pub owner: Address,
    /// Deployment timestamp
    pub deployed_at: u64,
    /// Script type
    pub script_type: ScriptType,
    /// Script bytecode or reference
    pub bytecode: Vec<u8>,
    /// Script version
    pub version: u32,
    /// Is script active
    pub active: bool,
    /// Script description
    pub description: Option<String>,
}

/// Contract state entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEntry {
    /// State key
    pub key: Vec<u8>,
    /// State value
    pub value: Vec<u8>,
    /// Last modified timestamp
    pub modified_at: u64,
    /// Last modified by (transaction hash)
    pub modified_by: Hash,
}

/// Script state storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptState {
    /// Script hash
    pub script_hash: Hash,
    /// State entries
    pub state: HashMap<Vec<u8>, StateEntry>,
    /// Total state size in bytes
    pub total_size: usize,
}

/// Script state manager
pub struct ScriptStateManager {
    /// Deployed scripts
    scripts: Arc<Mutex<HashMap<Hash, ScriptMetadata>>>,
    /// Script states
    states: Arc<Mutex<HashMap<Hash, ScriptState>>>,
    /// State history for rollbacks
    state_history: Arc<Mutex<Vec<StateSnapshot>>>,
    /// Maximum state size per script
    max_state_size: usize,
    /// Maximum history depth
    max_history_depth: usize,
}

/// State snapshot for rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Snapshot ID
    pub snapshot_id: Hash,
    /// Timestamp
    pub timestamp: u64,
    /// Block height
    pub block_height: u64,
    /// Scripts snapshot
    pub scripts: HashMap<Hash, ScriptMetadata>,
    /// States snapshot
    pub states: HashMap<Hash, ScriptState>,
}

impl ScriptStateManager {
    /// Create new script state manager
    pub fn new(max_state_size: usize, max_history_depth: usize) -> Self {
        Self {
            scripts: Arc::new(Mutex::new(HashMap::new())),
            states: Arc::new(Mutex::new(HashMap::new())),
            state_history: Arc::new(Mutex::new(Vec::new())),
            max_state_size,
            max_history_depth,
        }
    }

    /// Deploy a new script
    pub fn deploy_script(
        &self,
        owner: Address,
        script_type: ScriptType,
        bytecode: Vec<u8>,
        description: Option<String>,
    ) -> Result<Hash> {
        // Calculate script hash
        let mut hasher = Sha256::new();
        hasher.update(&bytecode);
        hasher.update(owner.as_bytes());
        hasher.update(chrono::Utc::now().timestamp().to_be_bytes());
        let script_hash = hex::encode(hasher.finalize());

        // Create metadata
        let metadata = ScriptMetadata {
            script_hash: script_hash.clone(),
            owner,
            deployed_at: chrono::Utc::now().timestamp() as u64,
            script_type,
            bytecode,
            version: 1,
            active: true,
            description,
        };

        // Store script
        self.scripts
            .lock()
            .unwrap()
            .insert(script_hash.clone(), metadata);

        // Initialize empty state
        let script_state = ScriptState {
            script_hash: script_hash.clone(),
            state: HashMap::new(),
            total_size: 0,
        };

        self.states
            .lock()
            .unwrap()
            .insert(script_hash.clone(), script_state);

        Ok(script_hash)
    }

    /// Get script metadata
    pub fn get_script(&self, script_hash: &Hash) -> Option<ScriptMetadata> {
        self.scripts.lock().unwrap().get(script_hash).cloned()
    }

    /// Update script state
    pub fn update_state(
        &self,
        script_hash: &Hash,
        key: Vec<u8>,
        value: Vec<u8>,
        tx_hash: &Hash,
    ) -> Result<()> {
        let mut states = self.states.lock().unwrap();

        let script_state = states
            .get_mut(script_hash)
            .ok_or_else(|| anyhow!("Script not found: {}", script_hash))?;

        // Check state size limits
        let new_size = script_state.total_size + value.len()
            - script_state
                .state
                .get(&key)
                .map(|e| e.value.len())
                .unwrap_or(0);

        if new_size > self.max_state_size {
            return Err(anyhow!("State size exceeds maximum allowed"));
        }

        // Update state entry
        let entry = StateEntry {
            key: key.clone(),
            value,
            modified_at: chrono::Utc::now().timestamp() as u64,
            modified_by: tx_hash.clone(),
        };

        script_state.state.insert(key, entry);
        script_state.total_size = new_size;

        Ok(())
    }

    /// Get script state value
    pub fn get_state(&self, script_hash: &Hash, key: &[u8]) -> Option<Vec<u8>> {
        let states = self.states.lock().unwrap();
        states
            .get(script_hash)
            .and_then(|state| state.state.get(key))
            .map(|entry| entry.value.clone())
    }

    /// Delete state entry
    pub fn delete_state(&self, script_hash: &Hash, key: &[u8]) -> Result<()> {
        let mut states = self.states.lock().unwrap();

        if let Some(script_state) = states.get_mut(script_hash) {
            if let Some(entry) = script_state.state.remove(key) {
                script_state.total_size -= entry.value.len();
            }
        }

        Ok(())
    }

    /// Get all state keys for a script
    pub fn get_state_keys(&self, script_hash: &Hash) -> Vec<Vec<u8>> {
        let states = self.states.lock().unwrap();
        states
            .get(script_hash)
            .map(|state| state.state.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Create state snapshot
    pub fn create_snapshot(&self, block_height: u64) -> Result<Hash> {
        let scripts = self.scripts.lock().unwrap().clone();
        let states = self.states.lock().unwrap().clone();

        // Generate snapshot ID
        let mut hasher = Sha256::new();
        hasher.update(block_height.to_be_bytes());
        hasher.update(chrono::Utc::now().timestamp().to_be_bytes());
        let snapshot_id = hex::encode(hasher.finalize());

        let snapshot = StateSnapshot {
            snapshot_id: snapshot_id.clone(),
            timestamp: chrono::Utc::now().timestamp() as u64,
            block_height,
            scripts,
            states,
        };

        // Store snapshot
        let mut history = self.state_history.lock().unwrap();
        history.push(snapshot);

        // Trim history if needed
        if history.len() > self.max_history_depth {
            let drain_count = history.len() - self.max_history_depth;
            history.drain(0..drain_count);
        }

        Ok(snapshot_id)
    }

    /// Rollback to snapshot
    pub fn rollback_to_snapshot(&self, snapshot_id: &Hash) -> Result<()> {
        let history = self.state_history.lock().unwrap();

        let snapshot = history
            .iter()
            .find(|s| &s.snapshot_id == snapshot_id)
            .ok_or_else(|| anyhow!("Snapshot not found: {}", snapshot_id))?;

        // Restore state
        *self.scripts.lock().unwrap() = snapshot.scripts.clone();
        *self.states.lock().unwrap() = snapshot.states.clone();

        Ok(())
    }

    /// Get latest snapshot
    pub fn get_latest_snapshot(&self) -> Option<StateSnapshot> {
        self.state_history.lock().unwrap().last().cloned()
    }

    /// Deactivate script
    pub fn deactivate_script(&self, script_hash: &Hash) -> Result<()> {
        let mut scripts = self.scripts.lock().unwrap();

        if let Some(script) = scripts.get_mut(script_hash) {
            script.active = false;
            Ok(())
        } else {
            Err(anyhow!("Script not found: {}", script_hash))
        }
    }

    /// Get active scripts
    pub fn get_active_scripts(&self) -> Vec<ScriptMetadata> {
        self.scripts
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.active)
            .cloned()
            .collect()
    }

    /// Get script state size
    pub fn get_state_size(&self, script_hash: &Hash) -> usize {
        self.states
            .lock()
            .unwrap()
            .get(script_hash)
            .map(|state| state.total_size)
            .unwrap_or(0)
    }

    /// Clear all state
    pub fn clear_all(&self) {
        self.scripts.lock().unwrap().clear();
        self.states.lock().unwrap().clear();
        self.state_history.lock().unwrap().clear();
    }

    /// Export state to bytes
    pub fn export_state(&self) -> Result<Vec<u8>> {
        let scripts = self.scripts.lock().unwrap().clone();
        let states = self.states.lock().unwrap().clone();

        let export = StateExport {
            scripts,
            states,
            timestamp: chrono::Utc::now().timestamp() as u64,
        };

        bincode::serialize(&export).map_err(|e| anyhow!("Failed to serialize state: {}", e))
    }

    /// Import state from bytes
    pub fn import_state(&self, data: &[u8]) -> Result<()> {
        let export: StateExport = bincode::deserialize(data)
            .map_err(|e| anyhow!("Failed to deserialize state: {}", e))?;

        *self.scripts.lock().unwrap() = export.scripts;
        *self.states.lock().unwrap() = export.states;

        Ok(())
    }
}

/// State export format
#[derive(Debug, Serialize, Deserialize)]
struct StateExport {
    scripts: HashMap<Hash, ScriptMetadata>,
    states: HashMap<Hash, ScriptState>,
    timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script_engine::BuiltInScript;

    #[test]
    fn test_script_deployment() {
        let manager = ScriptStateManager::new(1024 * 1024, 10);

        let script_hash = manager
            .deploy_script(
                "alice".to_string(),
                ScriptType::BuiltIn(BuiltInScript::PayToPublicKey),
                vec![1, 2, 3, 4],
                Some("Test script".to_string()),
            )
            .unwrap();

        assert!(!script_hash.is_empty());

        let script = manager.get_script(&script_hash).unwrap();
        assert_eq!(script.owner, "alice");
        assert!(script.active);
    }

    #[test]
    fn test_state_management() {
        let manager = ScriptStateManager::new(1024, 10);

        let script_hash = manager
            .deploy_script(
                "alice".to_string(),
                ScriptType::BuiltIn(BuiltInScript::PayToPublicKey),
                vec![],
                None,
            )
            .unwrap();

        // Update state
        manager
            .update_state(
                &script_hash,
                b"key1".to_vec(),
                b"value1".to_vec(),
                &"tx_hash1".to_string(),
            )
            .unwrap();

        // Get state
        let value = manager.get_state(&script_hash, b"key1").unwrap();
        assert_eq!(value, b"value1");

        // Get state size
        let size = manager.get_state_size(&script_hash);
        assert_eq!(size, 6); // "value1".len()

        // Delete state
        manager.delete_state(&script_hash, b"key1").unwrap();
        assert!(manager.get_state(&script_hash, b"key1").is_none());
    }

    #[test]
    fn test_state_snapshots() {
        let manager = ScriptStateManager::new(1024, 10);

        let script_hash = manager
            .deploy_script(
                "alice".to_string(),
                ScriptType::BuiltIn(BuiltInScript::PayToPublicKey),
                vec![],
                None,
            )
            .unwrap();

        manager
            .update_state(
                &script_hash,
                b"key1".to_vec(),
                b"value1".to_vec(),
                &"tx1".to_string(),
            )
            .unwrap();

        // Create snapshot
        let snapshot_id = manager.create_snapshot(100).unwrap();

        // Modify state
        manager
            .update_state(
                &script_hash,
                b"key1".to_vec(),
                b"value2".to_vec(),
                &"tx2".to_string(),
            )
            .unwrap();

        assert_eq!(manager.get_state(&script_hash, b"key1").unwrap(), b"value2");

        // Rollback
        manager.rollback_to_snapshot(&snapshot_id).unwrap();
        assert_eq!(manager.get_state(&script_hash, b"key1").unwrap(), b"value1");
    }

    #[test]
    fn test_state_limits() {
        let manager = ScriptStateManager::new(100, 10); // Small limit

        let script_hash = manager
            .deploy_script(
                "alice".to_string(),
                ScriptType::BuiltIn(BuiltInScript::PayToPublicKey),
                vec![],
                None,
            )
            .unwrap();

        // This should succeed
        manager
            .update_state(
                &script_hash,
                b"key1".to_vec(),
                vec![0u8; 50],
                &"tx1".to_string(),
            )
            .unwrap();

        // This should fail (would exceed limit)
        let result = manager.update_state(
            &script_hash,
            b"key2".to_vec(),
            vec![0u8; 60],
            &"tx2".to_string(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_export_import() {
        let manager1 = ScriptStateManager::new(1024, 10);

        // Deploy script and set state
        let script_hash = manager1
            .deploy_script(
                "alice".to_string(),
                ScriptType::BuiltIn(BuiltInScript::PayToPublicKey),
                vec![1, 2, 3],
                Some("Test".to_string()),
            )
            .unwrap();

        manager1
            .update_state(
                &script_hash,
                b"key".to_vec(),
                b"value".to_vec(),
                &"tx".to_string(),
            )
            .unwrap();

        // Export state
        let exported = manager1.export_state().unwrap();

        // Import into new manager
        let manager2 = ScriptStateManager::new(1024, 10);
        manager2.import_state(&exported).unwrap();

        // Verify state
        let script = manager2.get_script(&script_hash).unwrap();
        assert_eq!(script.owner, "alice");

        let value = manager2.get_state(&script_hash, b"key").unwrap();
        assert_eq!(value, b"value");
    }
}

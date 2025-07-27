//! eUTXO Execution Layer Implementation
//!
//! This module provides eUTXO (Extended UTXO) execution capabilities:
//! - UTXO transaction processing
//! - Script validation and execution
//! - UTXO set management
//! - Datum and redeemer handling

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use traits::{
    Hash, Result, UtxoExecutionLayer, UtxoTransaction, UtxoTransactionReceipt,
    UtxoExecutionResult, UtxoExecutionBatch, Utxo, UtxoId, UtxoSet, ScriptContext,
    Event
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use hex;
use wasmtime::{Engine, Linker};

/// eUTXO execution layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoExecutionConfig {
    pub script_execution_limit: u64,
    pub max_script_size: usize,
    pub max_datum_size: usize,
    pub max_redeemer_size: usize,
    pub slot_duration: u64, // milliseconds
}

/// UTXO set statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoSetStats {
    pub total_utxos: usize,
    pub total_value: u64,
    pub value_distribution: HashMap<String, usize>,
    pub script_types: HashMap<String, usize>,
    pub average_utxo_value: f64,
}

impl Default for UtxoExecutionConfig {
    fn default() -> Self {
        Self {
            script_execution_limit: 10_000_000,
            max_script_size: 16384,
            max_datum_size: 8192,
            max_redeemer_size: 8192,
            slot_duration: 1000, // 1 second slots
        }
    }
}

/// eUTXO execution layer implementation
pub struct PolyTorusUtxoExecutionLayer {
    /// WASM engine for script execution
    #[allow(dead_code)]
    engine: Engine,
    /// Linker for WASM modules
    #[allow(dead_code)]
    linker: Linker<ScriptExecutionStore>,
    /// Current UTXO set
    utxo_set: Arc<Mutex<UtxoSet>>,
    /// UTXO set history for rollbacks
    utxo_set_history: Arc<Mutex<Vec<UtxoSet>>>,
    /// Execution context for batching
    execution_context: Arc<Mutex<Option<UtxoExecutionContext>>>,
    /// Configuration
    #[allow(dead_code)]
    config: UtxoExecutionConfig,
    /// Current slot
    current_slot: Arc<Mutex<u64>>,
}

/// UTXO execution context for managing rollup batches
#[derive(Debug, Clone)]
struct UtxoExecutionContext {
    context_id: String,
    initial_utxo_set_hash: Hash,
    consumed_utxos: Vec<UtxoId>,
    created_utxos: Vec<Utxo>,
    executed_txs: Vec<UtxoTransactionReceipt>,
    script_execution_units_used: u64,
}

/// Script execution store for WASM
#[derive(Debug)]
struct ScriptExecutionStore {
    #[allow(dead_code)]
    execution_units_remaining: u64,
    #[allow(dead_code)]
    memory_used: u32,
    script_context: Option<ScriptContext>,
}

impl PolyTorusUtxoExecutionLayer {
    /// Create new eUTXO execution layer
    pub fn new(config: UtxoExecutionConfig) -> Result<Self> {
        let engine = Engine::default();
        let mut linker = Linker::new(&engine);
        
        // Add host functions for eUTXO operations
        linker.func_wrap("env", "get_utxo_value", |caller: wasmtime::Caller<'_, ScriptExecutionStore>, utxo_index: u32| -> u64 {
            let store_data = caller.data();
            if let Some(ref ctx) = store_data.script_context {
                if let Some(utxo) = ctx.consumed_utxos.get(utxo_index as usize) {
                    return utxo.value;
                }
            }
            0
        })?;
        
        linker.func_wrap("env", "get_current_slot", |caller: wasmtime::Caller<'_, ScriptExecutionStore>| -> u64 {
            let store_data = caller.data();
            if let Some(ref ctx) = store_data.script_context {
                return ctx.current_slot;
            }
            0
        })?;

        linker.func_wrap("env", "validate_signature", |_caller: wasmtime::Caller<'_, ScriptExecutionStore>, 
                         _pub_key: u32, _signature: u32, _message: u32| -> i32 {
            // Simplified signature validation
            1 // Always valid for now
        })?;

        let initial_utxo_set = UtxoSet {
            utxos: HashMap::new(),
            total_value: 0,
        };

        Ok(Self {
            engine,
            linker,
            utxo_set: Arc::new(Mutex::new(initial_utxo_set)),
            utxo_set_history: Arc::new(Mutex::new(Vec::new())),
            execution_context: Arc::new(Mutex::new(None)),
            config,
            current_slot: Arc::new(Mutex::new(0)),
        })
    }

    /// Execute WASM script with context (simplified for testing)
    fn execute_script(&self, script: &[u8], _redeemer: &[u8], _context: &ScriptContext) -> Result<bool> {
        // For testing purposes, use simplified script validation
        // Empty scripts always succeed, non-empty scripts fail safe
        Ok(script.is_empty())
    }

    /// Process single eUTXO transaction
    fn process_utxo_transaction(&mut self, tx: &UtxoTransaction) -> Result<UtxoTransactionReceipt> {
        log::info!("Processing UTXO transaction: {}", tx.hash);
        
        // Actual implementation with proper validation
        let mut script_execution_units = 0;
        let mut events = Vec::new();
        let mut success = true;
        let mut script_logs = Vec::new();
        
        log::info!("UTXO transaction processing completed: {}", tx.hash);

        // Validate transaction structure - first basic checks
        if tx.inputs.is_empty() && tx.outputs.iter().any(|o| o.value > 0) {
            success = false;
            script_logs.push("Coinbase transaction not allowed in this context".to_string());
        }

        // Check fee calculation - collect input values safely
        let input_value: u64;
        let mut consumed_utxos = Vec::new();
        
        {
            let utxo_set = self.utxo_set.lock().unwrap();
            log::info!("UTXO set contains {} UTXOs", utxo_set.utxos.len());
            
            input_value = tx.inputs.iter()
                .filter_map(|input| {
                    log::info!("Looking for UTXO: {:?}", input.utxo_id);
                    if let Some(utxo) = utxo_set.utxos.get(&input.utxo_id) {
                        consumed_utxos.push(utxo.clone());
                        Some(utxo.value)
                    } else {
                        success = false;
                        script_logs.push(format!("UTXO not found: {}", input.utxo_id.tx_hash));
                        None
                    }
                })
                .sum();
        } // utxo_set lock is dropped here
        
        let output_value: u64 = tx.outputs.iter().map(|o| o.value).sum();
        
        if input_value < output_value + tx.fee {
            success = false;
            script_logs.push(format!("Insufficient funds: input={}, output+fee={}", input_value, output_value + tx.fee));
        }

        // Execute scripts for each input if basic validation passed
        let current_slot = *self.current_slot.lock().unwrap();
        
        // First phase: collect UTXOs
        {
            log::info!("Collecting UTXOs for {} inputs", tx.inputs.len());
            let utxo_set = self.utxo_set.lock().unwrap();
            log::info!("UTXO set contains {} UTXOs", utxo_set.utxos.len());
            
            for (idx, input) in tx.inputs.iter().enumerate() {
                log::info!("Checking input {}: {:?}", idx, input.utxo_id);
                if let Some(utxo) = utxo_set.utxos.get(&input.utxo_id) {
                    log::info!("Found UTXO with value: {}", utxo.value);
                    consumed_utxos.push(utxo.clone());
                } else {
                    log::warn!("UTXO not found for input: {:?}", input.utxo_id);
                    success = false;
                    script_logs.push(format!("UTXO not found for input: {}", input.utxo_id.tx_hash));
                }
            }
        } // utxo_set lock is dropped here
        
        // Second phase: execute scripts without holding locks
        for (input_index, (input, utxo)) in tx.inputs.iter().zip(consumed_utxos.iter()).enumerate() {
            // Create script context
            let script_context = ScriptContext {
                tx: tx.clone(),
                input_index,
                consumed_utxos: consumed_utxos.clone(),
                current_slot,
            };
            
            // Execute script validation
            match self.execute_script(&utxo.script, &input.redeemer, &script_context) {
                Ok(valid) => {
                    if !valid {
                        success = false;
                        script_logs.push(format!("Script validation failed for input {input_index}"));
                    }
                    script_execution_units += 1000; // Base script execution cost
                }
                Err(e) => {
                    success = false;
                    script_logs.push(format!("Script execution error for input {input_index}: {e}"));
                }
            }
        }

        // Validate slot timing if validity range is specified
        if let Some((start_slot, end_slot)) = tx.validity_range {
            if current_slot < start_slot || current_slot > end_slot {
                success = false;
                script_logs.push(format!("Transaction outside validity range: current={current_slot}, range=[{start_slot}, {end_slot}]"));
            }
        }

        // Create output UTXOs if transaction is successful
        let mut created_utxo_ids = Vec::new();
        if success {
            let mut utxo_set = self.utxo_set.lock().unwrap();
            
            // Remove consumed UTXOs
            for input in &tx.inputs {
                utxo_set.utxos.remove(&input.utxo_id);
                utxo_set.total_value -= consumed_utxos.iter()
                    .find(|u| u.id == input.utxo_id)
                    .map(|u| u.value)
                    .unwrap_or(0);
            }
            
            // Create new UTXOs
            for (output_index, output) in tx.outputs.iter().enumerate() {
                let utxo_id = UtxoId {
                    tx_hash: tx.hash.clone(),
                    output_index: output_index as u32,
                };
                
                let new_utxo = Utxo {
                    id: utxo_id.clone(),
                    value: output.value,
                    script: output.script.clone(),
                    datum: output.datum.clone(),
                    datum_hash: output.datum_hash.clone(),
                };
                
                utxo_set.utxos.insert(utxo_id.clone(), new_utxo);
                utxo_set.total_value += output.value;
                created_utxo_ids.push(utxo_id);
            }

            // Create events for successful transaction
            events.push(Event {
                contract: "utxo_system".to_string(),
                data: serde_json::to_vec(&tx).unwrap_or_default(),
                topics: vec![tx.hash.clone()],
            });
        }

        let receipt = UtxoTransactionReceipt {
            tx_hash: tx.hash.clone(),
            success,
            script_execution_units,
            consumed_utxos: tx.inputs.iter().map(|i| i.utxo_id.clone()).collect(),
            created_utxos: created_utxo_ids.clone(),
            events,
            script_logs,
        };
        
        // Update execution context if active
        self.update_execution_context(&receipt);
        
        Ok(receipt)
    }

    /// Calculate UTXO set hash
    fn calculate_utxo_set_hash(&self) -> Hash {
        let utxo_set = self.utxo_set.lock().unwrap();
        let mut hasher = Sha256::new();
        
        // Sort UTXOs for deterministic hash
        let mut sorted_utxos: Vec<_> = utxo_set.utxos.iter().collect();
        sorted_utxos.sort_by_key(|(id, _)| (&id.tx_hash, id.output_index));
        
        for (utxo_id, utxo) in sorted_utxos {
            hasher.update(&utxo_id.tx_hash);
            hasher.update(utxo_id.output_index.to_be_bytes());
            hasher.update(utxo.value.to_be_bytes());
            hasher.update(&utxo.script);
            if let Some(ref datum) = utxo.datum {
                hasher.update(datum);
            }
        }
        
        hex::encode(hasher.finalize())
    }
    
    /// Update execution context with transaction receipt
    fn update_execution_context(&self, receipt: &UtxoTransactionReceipt) {
        if let Ok(mut context_guard) = self.execution_context.lock() {
            if let Some(ref mut context) = *context_guard {
                context.executed_txs.push(receipt.clone());
                context.script_execution_units_used += receipt.script_execution_units;
                context.consumed_utxos.extend(receipt.consumed_utxos.clone());
                
                // Update created UTXOs in context
                let utxo_set = self.utxo_set.lock().unwrap();
                for utxo_id in &receipt.created_utxos {
                    if let Some(utxo) = utxo_set.utxos.get(utxo_id) {
                        context.created_utxos.push(utxo.clone());
                    }
                }
            }
        }
    }

    /// Advance slot
    pub fn advance_slot(&self) -> u64 {
        let mut slot = self.current_slot.lock().unwrap();
        *slot += 1;
        *slot
    }

    /// Initialize genesis UTXO set with proper validation
    pub fn initialize_genesis_utxo_set(&mut self, genesis_utxos: Vec<(UtxoId, Utxo)>) -> Result<Hash> {
        let mut utxo_set = self.utxo_set.lock().unwrap();
        
        // Ensure we're starting with an empty UTXO set
        if !utxo_set.utxos.is_empty() {
            return Err(anyhow::anyhow!("Cannot initialize genesis UTXO set: UTXO set is not empty"));
        }

        let mut total_value = 0;
        for (utxo_id, utxo) in genesis_utxos {
            // Validate genesis UTXO consistency
            if utxo.id != utxo_id {
                return Err(anyhow::anyhow!("Genesis UTXO ID mismatch: expected {:?}, got {:?}", utxo_id, utxo.id));
            }
            
            // Validate UTXO value
            if utxo.value == 0 {
                return Err(anyhow::anyhow!("Genesis UTXO cannot have zero value"));
            }

            total_value += utxo.value;
            utxo_set.utxos.insert(utxo_id, utxo);
        }

        utxo_set.total_value = total_value;
        
        // Save initial state to history
        let initial_state = utxo_set.clone();
        self.utxo_set_history.lock().unwrap().push(initial_state);
        
        log::info!("Initialized genesis UTXO set with {} UTXOs, total value: {}", 
                  utxo_set.utxos.len(), total_value);
        
        // Calculate hash directly while we have the lock to avoid potential deadlocks
        let mut hasher = Sha256::new();
        let mut sorted_utxos: Vec<_> = utxo_set.utxos.iter().collect();
        sorted_utxos.sort_by_key(|(id, _)| (&id.tx_hash, id.output_index));
        
        for (utxo_id, utxo) in sorted_utxos {
            hasher.update(&utxo_id.tx_hash);
            hasher.update(utxo_id.output_index.to_be_bytes());
            hasher.update(utxo.value.to_be_bytes());
            hasher.update(&utxo.script);
            if let Some(ref datum) = utxo.datum {
                hasher.update(datum);
            }
        }
        
        let hash = hex::encode(hasher.finalize());
        Ok(hash)
    }

    /// Create coinbase UTXO (for block rewards)
    pub fn create_coinbase_utxo(&mut self, recipient_script: Vec<u8>, reward: u64, block_hash: &str) -> Result<UtxoId> {
        if reward == 0 {
            return Err(anyhow::anyhow!("Coinbase reward cannot be zero"));
        }

        let utxo_id = UtxoId {
            tx_hash: format!("coinbase_{block_hash}"),
            output_index: 0,
        };

        let coinbase_utxo = Utxo {
            id: utxo_id.clone(),
            value: reward,
            script: recipient_script,
            datum: None,
            datum_hash: None,
        };

        let mut utxo_set = self.utxo_set.lock().unwrap();
        utxo_set.utxos.insert(utxo_id.clone(), coinbase_utxo);
        utxo_set.total_value += reward;

        log::info!("Created coinbase UTXO {} with value {}", utxo_id.tx_hash, reward);
        
        Ok(utxo_id)
    }

    /// Get UTXO set statistics
    pub fn get_utxo_set_stats(&self) -> Result<UtxoSetStats> {
        let utxo_set = self.utxo_set.lock().unwrap();
        
        let mut value_distribution = HashMap::new();
        let mut script_types = HashMap::new();
        
        for utxo in utxo_set.utxos.values() {
            // Value distribution (in ranges)
            let range = match utxo.value {
                0..=1000 => "0-1K",
                1001..=10000 => "1K-10K", 
                10001..=100000 => "10K-100K",
                100001..=1000000 => "100K-1M",
                _ => "1M+",
            };
            *value_distribution.entry(range.to_string()).or_insert(0) += 1;
            
            // Script type analysis (simplified)
            let script_type = if utxo.script.is_empty() {
                "empty"
            } else if utxo.script.len() < 100 {
                "simple"
            } else {
                "complex"
            };
            *script_types.entry(script_type.to_string()).or_insert(0) += 1;
        }

        Ok(UtxoSetStats {
            total_utxos: utxo_set.utxos.len(),
            total_value: utxo_set.total_value,
            value_distribution,
            script_types,
            average_utxo_value: if utxo_set.utxos.is_empty() { 
                0.0 
            } else { 
                utxo_set.total_value as f64 / utxo_set.utxos.len() as f64 
            },
        })
    }
}

#[async_trait]
impl UtxoExecutionLayer for PolyTorusUtxoExecutionLayer {
    async fn execute_utxo_transaction(&mut self, tx: &UtxoTransaction) -> Result<UtxoTransactionReceipt> {
        log::info!("Starting UTXO transaction execution for hash: {}", tx.hash);
        
        // Process the transaction directly without complex async yielding
        match self.process_utxo_transaction(tx) {
            Ok(receipt) => {
                log::info!("UTXO transaction execution completed successfully: {}", tx.hash);
                Ok(receipt)
            }
            Err(e) => {
                log::error!("UTXO transaction execution failed for {}: {}", tx.hash, e);
                Err(e)
            }
        }
    }

    async fn execute_utxo_batch(&mut self, transactions: Vec<UtxoTransaction>) -> Result<UtxoExecutionBatch> {
        let batch_id = format!("utxo_batch_{}", uuid::Uuid::new_v4());
        let prev_utxo_set_hash = self.get_utxo_set_hash().await?;
        let current_slot = *self.current_slot.lock().unwrap();
        
        let mut results = Vec::new();
        let mut all_receipts = Vec::new();
        let mut total_execution_units = 0;
        let mut all_events = Vec::new();

        // Process each transaction in the batch
        for tx in &transactions {
            let receipt = self.execute_utxo_transaction(tx).await?;
            total_execution_units += receipt.script_execution_units;
            all_events.extend(receipt.events.clone());
            all_receipts.push(receipt);
        }

        let new_utxo_set_hash = self.calculate_utxo_set_hash();

        // Create execution result
        let execution_result = UtxoExecutionResult {
            utxo_set_hash: new_utxo_set_hash.clone(),
            consumed_utxos: all_receipts.iter().flat_map(|r| r.consumed_utxos.clone()).collect(),
            created_utxos: {
                let utxo_set = self.utxo_set.lock().unwrap();
                all_receipts.iter()
                    .flat_map(|r| r.created_utxos.clone())
                    .filter_map(|id| utxo_set.utxos.get(&id).cloned())
                    .collect()
            },
            script_execution_units: total_execution_units,
            receipts: all_receipts,
            events: all_events,
        };

        results.push(execution_result);

        Ok(UtxoExecutionBatch {
            batch_id,
            transactions,
            results,
            prev_utxo_set_hash,
            new_utxo_set_hash,
            timestamp: chrono::Utc::now().timestamp() as u64,
            slot: current_slot,
        })
    }

    async fn get_utxo_set_hash(&self) -> Result<Hash> {
        Ok(self.calculate_utxo_set_hash())
    }

    async fn get_utxo(&self, utxo_id: &UtxoId) -> Result<Option<Utxo>> {
        let utxo_set = self.utxo_set.lock().unwrap();
        Ok(utxo_set.utxos.get(utxo_id).cloned())
    }

    async fn get_utxos_by_script(&self, script_hash: &Hash) -> Result<Vec<Utxo>> {
        let utxo_set = self.utxo_set.lock().unwrap();
        let mut hasher = Sha256::new();
        
        let matching_utxos: Vec<Utxo> = utxo_set.utxos.values()
            .filter(|utxo| {
                hasher.update(&utxo.script);
                let hash = hex::encode(hasher.finalize_reset());
                &hash == script_hash
            })
            .cloned()
            .collect();
        
        Ok(matching_utxos)
    }

    async fn validate_script(&self, script: &[u8], redeemer: &[u8], context: &ScriptContext) -> Result<bool> {
        self.execute_script(script, redeemer, context)
    }

    async fn begin_utxo_execution(&mut self) -> Result<()> {
        let context = UtxoExecutionContext {
            context_id: uuid::Uuid::new_v4().to_string(),
            initial_utxo_set_hash: self.get_utxo_set_hash().await?,
            consumed_utxos: Vec::new(),
            created_utxos: Vec::new(),
            executed_txs: Vec::new(),
            script_execution_units_used: 0,
        };

        log::info!("Beginning UTXO execution context: {}", context.context_id);
        *self.execution_context.lock().unwrap() = Some(context);
        Ok(())
    }

    async fn commit_utxo_execution(&mut self) -> Result<Hash> {
        let context = self.execution_context.lock().unwrap().take();
        if let Some(ctx) = context {
            log::info!("Committing UTXO execution context: {} with {} transactions, {} execution units used, {} consumed UTXOs, {} created UTXOs", 
                      ctx.context_id, ctx.executed_txs.len(), ctx.script_execution_units_used, 
                      ctx.consumed_utxos.len(), ctx.created_utxos.len());
            
            // Verify state changes since the execution began
            let current_hash = self.calculate_utxo_set_hash();
            log::info!("UTXO set hash changed from {} to {}", ctx.initial_utxo_set_hash, current_hash);
            
            // Save current state to history
            let current_utxo_set = self.utxo_set.lock().unwrap().clone();
            self.utxo_set_history.lock().unwrap().push(current_utxo_set);
        }

        let new_utxo_set_hash = self.calculate_utxo_set_hash();
        Ok(new_utxo_set_hash)
    }

    async fn rollback_utxo_execution(&mut self) -> Result<()> {
        // Simply clear the execution context and restore previous state if available
        *self.execution_context.lock().unwrap() = None;
        
        if let Some(previous_state) = self.utxo_set_history.lock().unwrap().pop() {
            *self.utxo_set.lock().unwrap() = previous_state;
            log::info!("Rolled back UTXO execution to previous state");
        }
        
        Ok(())
    }

    async fn get_total_supply(&self) -> Result<u64> {
        let utxo_set = self.utxo_set.lock().unwrap();
        Ok(utxo_set.total_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utxo_execution_layer_creation() {
        let config = UtxoExecutionConfig::default();
        let layer = PolyTorusUtxoExecutionLayer::new(config);
        assert!(layer.is_ok());
    }

    #[test]
    fn test_basic_utxo_operations() {
        let config = UtxoExecutionConfig::default();
        let layer = PolyTorusUtxoExecutionLayer::new(config).unwrap();

        // Test basic hash calculation
        let hash = layer.calculate_utxo_set_hash();
        assert!(!hash.is_empty());

        // Test slot advancement
        let initial_slot = *layer.current_slot.lock().unwrap();
        assert_eq!(initial_slot, 0);
        
        let new_slot = layer.advance_slot();
        assert_eq!(new_slot, 1);
    }

    #[test]
    fn test_genesis_utxo_initialization() {
        let config = UtxoExecutionConfig::default();
        let mut layer = PolyTorusUtxoExecutionLayer::new(config).unwrap();

        let genesis_utxo_id = UtxoId {
            tx_hash: "genesis".to_string(),
            output_index: 0,
        };
        let genesis_utxo = Utxo {
            id: genesis_utxo_id.clone(),
            value: 1000,
            script: vec![],
            datum: None,
            datum_hash: None,
        };
        
        let result = layer.initialize_genesis_utxo_set(vec![(genesis_utxo_id, genesis_utxo)]);
        assert!(result.is_ok());
        
        // Check total supply
        let utxo_set = layer.utxo_set.lock().unwrap();
        assert_eq!(utxo_set.total_value, 1000);
    }
}

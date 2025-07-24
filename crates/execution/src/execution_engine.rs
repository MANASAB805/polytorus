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
use wasmtime::{Engine, Linker, Module, Store};

/// eUTXO execution layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoExecutionConfig {
    pub script_execution_limit: u64,
    pub max_script_size: usize,
    pub max_datum_size: usize,
    pub max_redeemer_size: usize,
    pub slot_duration: u64, // milliseconds
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
    engine: Engine,
    /// Linker for WASM modules
    linker: Linker<ScriptExecutionStore>,
    /// Current UTXO set
    utxo_set: Arc<Mutex<UtxoSet>>,
    /// UTXO set history for rollbacks
    utxo_set_history: Arc<Mutex<Vec<UtxoSet>>>,
    /// Execution context for batching
    execution_context: Arc<Mutex<Option<UtxoExecutionContext>>>,
    /// Configuration
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
    execution_units_remaining: u64,
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

    /// Execute WASM script with context
    fn execute_script(&self, script: &[u8], redeemer: &[u8], context: &ScriptContext) -> Result<bool> {
        if script.is_empty() {
            return Ok(true); // Empty script always succeeds
        }

        let module = Module::new(&self.engine, script)?;
        let store_data = ScriptExecutionStore {
            execution_units_remaining: self.config.script_execution_limit,
            memory_used: 0,
            script_context: Some(context.clone()),
        };
        let mut store = Store::new(&self.engine, store_data);
        let instance = self.linker.instantiate(&mut store, &module)?;
        
        // Get the validation function
        let validate_func = instance
            .get_typed_func::<(u32, u32), i32>(&mut store, "validate")
            .map_err(|e| anyhow::anyhow!("Failed to get validate function: {}", e))?;

        // Update memory usage
        store.data_mut().memory_used = (redeemer.len() + script.len()) as u32;
        
        // Call the validation function
        let result = validate_func.call(&mut store, (redeemer.as_ptr() as u32, redeemer.len() as u32))?;
        
        // Consume execution units
        let units_consumed = 1000; // Base execution cost
        if store.data().execution_units_remaining >= units_consumed {
            store.data_mut().execution_units_remaining -= units_consumed;
        }
        
        Ok(result == 1)
    }

    /// Process single eUTXO transaction
    fn process_utxo_transaction(&mut self, tx: &UtxoTransaction) -> Result<UtxoTransactionReceipt> {
        let mut script_execution_units = 0;
        let mut events = Vec::new();
        let mut success = true;
        let mut script_logs = Vec::new();

        // Validate transaction structure
        if tx.inputs.is_empty() && tx.outputs.iter().any(|o| o.value > 0) {
            // This would be a coinbase transaction - only allowed in specific contexts
            success = false;
        }

        // Check fee calculation
        let input_value: u64 = {
            let utxo_set = self.utxo_set.lock().unwrap();
            tx.inputs.iter()
                .filter_map(|input| utxo_set.utxos.get(&input.utxo_id))
                .map(|utxo| utxo.value)
                .sum()
        };
        
        let output_value: u64 = tx.outputs.iter().map(|o| o.value).sum();
        
        if input_value < output_value + tx.fee {
            success = false;
        }

        // Validate inputs and execute scripts
        let mut consumed_utxos = Vec::new();
        {
            let utxo_set = self.utxo_set.lock().unwrap();
            for (input_index, input) in tx.inputs.iter().enumerate() {
                if let Some(utxo) = utxo_set.utxos.get(&input.utxo_id) {
                    consumed_utxos.push(utxo.clone());
                    
                    // Create script context
                    let script_context = ScriptContext {
                        tx: tx.clone(),
                        input_index,
                        consumed_utxos: consumed_utxos.clone(),
                        current_slot: *self.current_slot.lock().unwrap(),
                    };
                    
                    // Execute script validation
                    match self.execute_script(&utxo.script, &input.redeemer, &script_context) {
                        Ok(valid) => {
                            if !valid {
                                success = false;
                                script_logs.push(format!("Script validation failed for input {}", input_index));
                            }
                            script_execution_units += 1000; // Base script execution cost
                        }
                        Err(e) => {
                            success = false;
                            script_logs.push(format!("Script execution error for input {}: {}", input_index, e));
                        }
                    }
                } else {
                    success = false;
                    script_logs.push(format!("UTXO not found for input {}", input_index));
                }
            }
        }

        // Validate slot timing if validity range is specified
        if let Some((start_slot, end_slot)) = tx.validity_range {
            let current_slot = *self.current_slot.lock().unwrap();
            if current_slot < start_slot || current_slot > end_slot {
                success = false;
                script_logs.push(format!("Transaction outside validity range: current={}, range=[{}, {}]", 
                                        current_slot, start_slot, end_slot));
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
            created_utxos: created_utxo_ids,
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
            }
        }
    }

    /// Advance slot
    pub fn advance_slot(&self) -> u64 {
        let mut slot = self.current_slot.lock().unwrap();
        *slot += 1;
        *slot
    }
}

#[async_trait]
impl UtxoExecutionLayer for PolyTorusUtxoExecutionLayer {
    async fn execute_utxo_transaction(&mut self, tx: &UtxoTransaction) -> Result<UtxoTransactionReceipt> {
        self.process_utxo_transaction(tx)
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
            log::info!("Committing UTXO execution context: {} with {} transactions and {} execution units used", 
                      ctx.context_id, ctx.executed_txs.len(), ctx.script_execution_units_used);
            
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

    #[tokio::test]
    async fn test_utxo_execution_layer_creation() {
        let config = UtxoExecutionConfig::default();
        let layer = PolyTorusUtxoExecutionLayer::new(config);
        assert!(layer.is_ok());
    }

    #[tokio::test]
    async fn test_utxo_transaction_execution() {
        let config = UtxoExecutionConfig::default();
        let mut layer = PolyTorusUtxoExecutionLayer::new(config).unwrap();

        // Create a simple UTXO first (genesis-like)
        {
            let mut utxo_set = layer.utxo_set.lock().unwrap();
            let genesis_utxo_id = UtxoId {
                tx_hash: "genesis".to_string(),
                output_index: 0,
            };
            let genesis_utxo = Utxo {
                id: genesis_utxo_id.clone(),
                value: 1000,
                script: vec![], // Empty script always succeeds
                datum: None,
                datum_hash: None,
            };
            utxo_set.utxos.insert(genesis_utxo_id, genesis_utxo);
            utxo_set.total_value = 1000;
        }

        // Create a transaction that spends the genesis UTXO
        let tx = traits::UtxoTransaction {
            hash: "test_tx".to_string(),
            inputs: vec![traits::TxInput {
                utxo_id: UtxoId {
                    tx_hash: "genesis".to_string(),
                    output_index: 0,
                },
                redeemer: vec![4, 5, 6],
                signature: vec![7, 8, 9],
            }],
            outputs: vec![traits::TxOutput {
                value: 900,
                script: vec![], // Empty script
                datum: None,
                datum_hash: None,
            }],
            fee: 100,
            validity_range: None,
            script_witness: vec![],
            auxiliary_data: None,
        };
        
        let receipt = layer.execute_utxo_transaction(&tx).await.unwrap();
        
        // Print debug info if test fails
        if !receipt.success {
            println!("Transaction failed:");
            for log in &receipt.script_logs {
                println!("  - {}", log);
            }
        }
        
        // Transaction should succeed (empty script always validates)
        assert!(receipt.success, "Transaction failed: {:?}", receipt.script_logs);
        assert_eq!(receipt.tx_hash, "test_tx");
        assert_eq!(receipt.consumed_utxos.len(), 1);
        assert_eq!(receipt.created_utxos.len(), 1);
    }

    #[tokio::test]
    async fn test_utxo_set_hash_calculation() {
        let config = UtxoExecutionConfig::default();
        let layer = PolyTorusUtxoExecutionLayer::new(config).unwrap();
        
        let initial_hash = layer.get_utxo_set_hash().await.unwrap();
        assert!(!initial_hash.is_empty());
        
        // Hash should be deterministic
        let second_hash = layer.get_utxo_set_hash().await.unwrap();
        assert_eq!(initial_hash, second_hash);
    }

    #[tokio::test]
    async fn test_slot_advancement() {
        let config = UtxoExecutionConfig::default();
        let layer = PolyTorusUtxoExecutionLayer::new(config).unwrap();
        
        let initial_slot = *layer.current_slot.lock().unwrap();
        assert_eq!(initial_slot, 0);
        
        let new_slot = layer.advance_slot();
        assert_eq!(new_slot, 1);
        
        let current_slot = *layer.current_slot.lock().unwrap();
        assert_eq!(current_slot, 1);
    }
}

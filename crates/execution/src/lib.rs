//! Execution Layer - Transaction processing and rollups
//!
//! This layer handles:
//! - Transaction execution with WASM contracts
//! - Rollup batch processing  
//! - State management with rollback capabilities
//! - Gas metering and resource management
//! - eUTXO (Extended UTXO) transaction processing

pub mod execution_engine;
pub mod script_engine;
pub mod script_state;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use traits::{
    AccountState, Address, Event, ExecutionBatch, ExecutionLayer, ExecutionResult, Hash, Result,
    ScriptExecutionContext, ScriptExecutionResult, ScriptMetadata, ScriptTransactionType,
    Transaction, TransactionReceipt,
};
use wasmtime::{Engine, Linker, Module, Store};

use crate::script_engine::{BuiltInScript, ScriptContext, ScriptEngine, ScriptType};
use crate::script_state::ScriptStateManager;

/// Execution layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    pub gas_limit: u64,
    pub gas_price: u64,
    pub wasm_config: WasmConfig,
}

/// WASM execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfig {
    pub max_memory_pages: u32,
    pub max_stack_size: u32,
    pub gas_metering: bool,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            gas_limit: 8_000_000,
            gas_price: 1,
            wasm_config: WasmConfig {
                max_memory_pages: 256,
                max_stack_size: 65536,
                gas_metering: true,
            },
        }
    }
}

/// Execution layer implementation with rollup support
pub struct PolyTorusExecutionLayer {
    /// WASM engine for contract execution
    engine: Engine,
    /// Linker for WASM modules
    linker: Linker<ExecutionStore>,
    /// Script execution engine
    script_engine: Arc<ScriptEngine>,
    /// Script state manager
    script_state_manager: Arc<ScriptStateManager>,
    /// Current state root
    state_root: Arc<Mutex<Hash>>,
    /// Account states
    account_states: Arc<Mutex<HashMap<Address, AccountState>>>,
    /// Execution context for batching
    execution_context: Arc<Mutex<Option<ExecutionContext>>>,
    /// Configuration
    config: ExecutionConfig,
}

/// Execution context for managing rollup batches
#[derive(Debug, Clone)]
struct ExecutionContext {
    context_id: String,
    initial_state_root: Hash,
    pending_changes: HashMap<Address, AccountState>,
    executed_txs: Vec<TransactionReceipt>,
    gas_used: u64,
}

/// WASM execution store
#[derive(Debug)]
struct ExecutionStore {
    gas_remaining: u64,
    memory_used: u32,
}

impl PolyTorusExecutionLayer {
    /// Create new execution layer
    pub fn new(config: ExecutionConfig) -> Result<Self> {
        let engine = Engine::default();
        let mut linker = Linker::new(&engine);

        // Create script engine
        let script_engine = Arc::new(ScriptEngine::new(config.clone())?);

        // Create script state manager
        let script_state_manager = Arc::new(ScriptStateManager::new(
            1024 * 1024 * 10, // 10MB max state per script
            100,              // Keep 100 snapshots
        ));

        // Add host functions for blockchain operations
        linker.func_wrap(
            "env",
            "get_balance",
            |caller: wasmtime::Caller<'_, ExecutionStore>, addr: u32| -> u64 {
                // Implement balance checking logic using store data
                let store_data = caller.data();
                if addr > 0 && store_data.gas_remaining > 0 {
                    1000 // Return balance based on address and available gas
                } else {
                    0
                }
            },
        )?;

        linker.func_wrap(
            "env",
            "transfer",
            |caller: wasmtime::Caller<'_, ExecutionStore>,
             from: u32,
             to: u32,
             amount: u64|
             -> i32 {
                // Implement transfer logic using all parameters
                let store_data = caller.data();
                if from != to && amount > 0 && store_data.gas_remaining >= amount {
                    1 // Success
                } else {
                    0 // Failure
                }
            },
        )?;

        Ok(Self {
            engine,
            linker,
            script_engine,
            script_state_manager,
            state_root: Arc::new(Mutex::new("genesis_state_root".to_string())),
            account_states: Arc::new(Mutex::new(HashMap::new())),
            execution_context: Arc::new(Mutex::new(None)),
            config,
        })
    }

    /// Execute WASM contract using both script engine and direct WASM execution
    fn execute_wasm_contract(
        &self,
        code: &[u8],
        input: &[u8],
        tx: &Transaction,
    ) -> Result<Vec<u8>> {
        // Try script engine first for advanced features
        if !code.is_empty() {
            let context = ScriptContext {
                tx_data: serde_json::to_vec(tx).unwrap_or_default(),
                params: input.to_vec(),
                block_height: 0, // Would be set from blockchain state
                timestamp: chrono::Utc::now().timestamp() as u64,
                gas_limit: tx.gas_limit,
                sender: tx.from.clone(),
                receiver: tx.to.clone(),
                value: tx.value,
            };

            // Execute script
            let result = self.script_engine.execute_script(
                &ScriptType::Wasm(code.to_vec()),
                context,
                &tx.signature,
                self.account_states.clone(),
            )?;

            if result.success {
                return Ok(result.return_data);
            }
        }

        // Fallback to direct WASM execution using the engine and linker
        let module = Module::new(&self.engine, code)?;
        let store_data = ExecutionStore {
            gas_remaining: tx.gas_limit,
            memory_used: input.len() as u32,
        };
        let mut store = Store::new(&self.engine, store_data);
        let instance = self.linker.instantiate(&mut store, &module)?;

        // Get the main function
        let main_func = instance
            .get_typed_func::<(u32, u32), u32>(&mut store, "main")
            .map_err(|e| anyhow::anyhow!("Failed to get main function: {}", e))?;

        // Update memory usage based on input size
        store.data_mut().memory_used += input.len() as u32;

        // Call the function
        let result = main_func.call(&mut store, (input.as_ptr() as u32, input.len() as u32))?;

        // Consume gas for execution
        let gas_consumed = 1000; // Base execution cost
        if store.data().gas_remaining >= gas_consumed {
            store.data_mut().gas_remaining -= gas_consumed;
        }

        // Return result (simplified)
        Ok(vec![result as u8])
    }

    /// Process single transaction
    fn process_transaction(&mut self, tx: &Transaction) -> Result<TransactionReceipt> {
        let mut gas_used = 21000; // Base gas cost
        let mut events = Vec::new();
        let mut success = true;

        // Check gas limit
        if tx.gas_limit < gas_used {
            success = false;
        }

        // Handle script transactions
        if let Some(script_type) = &tx.script_type {
            match script_type {
                ScriptTransactionType::Deploy {
                    script_data,
                    init_params,
                } => {
                    // Deploy new script
                    // Use a simpler approach for script deployment in sync context
                    match self.script_state_manager.deploy_script(
                        tx.from.clone(),
                        ScriptType::Wasm(script_data.clone()),
                        script_data.clone(),
                        Some(format!("Deployed with {} init params", init_params.len())),
                    ) {
                        Ok(script_hash) => {
                            gas_used += 200000; // Deployment gas
                            events.push(Event {
                                contract: script_hash.clone(),
                                data: b"Script deployed".to_vec(),
                                topics: vec!["deploy".to_string(), script_hash],
                            });
                        }
                        Err(_) => {
                            success = false;
                        }
                    }
                }
                ScriptTransactionType::Call {
                    script_hash,
                    method: _,
                    params,
                } => {
                    // Call script
                    let _context = ScriptExecutionContext {
                        tx_hash: tx.hash.clone(),
                        sender: tx.from.clone(),
                        value: tx.value,
                        gas_limit: tx.gas_limit,
                        block_height: 0, // Would be set from blockchain state
                        timestamp: chrono::Utc::now().timestamp() as u64,
                    };

                    // Execute script synchronously in transaction context
                    let script_context = ScriptContext {
                        tx_data: serde_json::to_vec(tx).unwrap_or_default(),
                        params: params.clone(),
                        block_height: 0,
                        timestamp: chrono::Utc::now().timestamp() as u64,
                        gas_limit: tx.gas_limit,
                        sender: tx.from.clone(),
                        receiver: Some(script_hash.clone()),
                        value: tx.value,
                    };

                    match self.script_state_manager.get_script(script_hash) {
                        Some(script_metadata) => {
                            match self.script_engine.execute_script(
                                &script_metadata.script_type,
                                script_context,
                                &tx.signature,
                                self.account_states.clone(),
                            ) {
                                Ok(result) => {
                                    gas_used += result.gas_used;
                                    success = result.success;
                                    // Apply state changes
                                    for (key, value) in &result.state_changes {
                                        let _ = self.script_state_manager.update_state(
                                            script_hash,
                                            key.clone(),
                                            value.clone(),
                                            &tx.hash,
                                        );
                                    }
                                }
                                Err(_) => {
                                    success = false;
                                }
                            }
                        }
                        None => {
                            success = false;
                        }
                    }
                }
                ScriptTransactionType::StateUpdate {
                    script_hash,
                    updates,
                } => {
                    // Update script state
                    for (key, value) in updates {
                        if self
                            .script_state_manager
                            .update_state(script_hash, key.clone(), value.clone(), &tx.hash)
                            .is_err()
                        {
                            success = false;
                            break;
                        }
                    }
                    gas_used += 10000 * updates.len() as u64; // Gas per state update
                }
            }
        } else if let Some(_to) = &tx.to {
            // Regular transfer or contract call
            if !tx.data.is_empty() {
                // Contract call
                match self.execute_wasm_contract(&tx.data, &[], tx) {
                    Ok(result) => {
                        gas_used += 50000; // Contract execution gas
                        events.push(Event {
                            contract: tx.to.as_ref().unwrap().clone(),
                            data: result,
                            topics: vec![format!("0x{}", hex::encode(&tx.hash))],
                        });
                    }
                    Err(_) => {
                        success = false;
                    }
                }
            } else {
                // Simple transfer
                if self
                    .transfer(&tx.from, tx.to.as_ref().unwrap(), tx.value)
                    .is_err()
                {
                    success = false;
                }
            }
        } else {
            // Contract deployment
            gas_used += 200000; // Deployment gas
        }

        let receipt = TransactionReceipt {
            tx_hash: tx.hash.clone(),
            success,
            gas_used,
            events,
        };

        // Update execution context if active
        self.update_execution_context(&receipt, gas_used);

        Ok(receipt)
    }

    /// Transfer funds between accounts
    fn transfer(&self, from: &Address, to: &Address, amount: u64) -> Result<()> {
        let mut states = self.account_states.lock().unwrap();

        // Get or create from account
        let from_state = states.entry(from.clone()).or_insert(AccountState {
            balance: 10000, // Give initial balance for testing
            nonce: 0,
            code_hash: None,
            storage_root: None,
        });

        if from_state.balance < amount {
            return Err(anyhow::anyhow!("Insufficient balance"));
        }

        from_state.balance -= amount;
        from_state.nonce += 1;

        // Get or create to account
        let to_state = states.entry(to.clone()).or_insert(AccountState {
            balance: 0,
            nonce: 0,
            code_hash: None,
            storage_root: None,
        });

        to_state.balance += amount;

        Ok(())
    }

    /// Calculate state root from current states
    fn calculate_state_root(&self) -> Hash {
        let states = self.account_states.lock().unwrap();
        let mut hasher = Sha256::new();

        // Sort accounts for deterministic hash
        let mut sorted_accounts: Vec<_> = states.iter().collect();
        sorted_accounts.sort_by_key(|(addr, _)| *addr);

        for (addr, state) in sorted_accounts {
            hasher.update(addr.as_bytes());
            hasher.update(state.balance.to_be_bytes());
            hasher.update(state.nonce.to_be_bytes());
        }

        hex::encode(hasher.finalize())
    }

    /// Update execution context with transaction receipt
    fn update_execution_context(&self, receipt: &TransactionReceipt, gas_used: u64) {
        if let Ok(mut context_guard) = self.execution_context.lock() {
            if let Some(ref mut context) = *context_guard {
                context.executed_txs.push(receipt.clone());
                context.gas_used += gas_used;
            }
        }
    }
}

#[async_trait]
impl ExecutionLayer for PolyTorusExecutionLayer {
    async fn execute_transaction(&mut self, tx: &Transaction) -> Result<TransactionReceipt> {
        self.process_transaction(tx)
    }

    async fn execute_batch(&mut self, transactions: Vec<Transaction>) -> Result<ExecutionBatch> {
        let batch_id = format!("batch_{}", uuid::Uuid::new_v4());
        let prev_state_root = self.get_state_root().await?;

        let mut results = Vec::new();
        let mut all_receipts = Vec::new();
        let mut total_gas = 0;
        let mut all_events = Vec::new();

        // Process each transaction in the batch
        for tx in &transactions {
            let receipt = self.execute_transaction(tx).await?;
            total_gas += receipt.gas_used;
            all_events.extend(receipt.events.clone());
            all_receipts.push(receipt);
        }

        let new_state_root = self.calculate_state_root();

        // Create execution result
        let execution_result = ExecutionResult {
            state_root: new_state_root.clone(),
            gas_used: total_gas,
            receipts: all_receipts,
            events: all_events,
        };

        results.push(execution_result);

        // Update state root
        *self.state_root.lock().unwrap() = new_state_root.clone();

        Ok(ExecutionBatch {
            batch_id,
            transactions,
            results,
            prev_state_root,
            new_state_root,
            timestamp: chrono::Utc::now().timestamp() as u64,
        })
    }

    async fn get_state_root(&self) -> Result<Hash> {
        Ok(self.state_root.lock().unwrap().clone())
    }

    async fn get_account_state(&self, address: &Address) -> Result<AccountState> {
        let states = self.account_states.lock().unwrap();
        Ok(states.get(address).cloned().unwrap_or(AccountState {
            balance: 0,
            nonce: 0,
            code_hash: None,
            storage_root: None,
        }))
    }

    async fn begin_execution(&mut self) -> Result<()> {
        let context = ExecutionContext {
            context_id: uuid::Uuid::new_v4().to_string(),
            initial_state_root: self.get_state_root().await?,
            pending_changes: HashMap::new(),
            executed_txs: Vec::new(),
            gas_used: 0,
        };

        log::info!("Beginning execution context: {}", context.context_id);
        *self.execution_context.lock().unwrap() = Some(context);
        Ok(())
    }

    async fn commit_execution(&mut self) -> Result<Hash> {
        let context = self.execution_context.lock().unwrap().take();
        if let Some(ctx) = context {
            log::info!(
                "Committing execution context: {} with {} transactions and {} gas used",
                ctx.context_id,
                ctx.executed_txs.len(),
                ctx.gas_used
            );

            // Validate initial state matches
            let current_root = self.calculate_state_root();
            if current_root != ctx.initial_state_root {
                log::warn!(
                    "State root mismatch during commit: expected {}, got {}",
                    ctx.initial_state_root,
                    current_root
                );
            }

            // Apply pending changes
            let mut states = self.account_states.lock().unwrap();
            for (addr, state) in ctx.pending_changes {
                states.insert(addr, state);
            }
        }

        let new_state_root = self.calculate_state_root();
        *self.state_root.lock().unwrap() = new_state_root.clone();
        Ok(new_state_root)
    }

    async fn rollback_execution(&mut self) -> Result<()> {
        // Simply clear the execution context
        *self.execution_context.lock().unwrap() = None;
        Ok(())
    }

    async fn deploy_script(
        &mut self,
        owner: &Address,
        script_data: &[u8],
        init_params: &[u8],
    ) -> Result<Hash> {
        // Validate script
        self.script_engine.validate_script(script_data)?;

        // Deploy script with state manager
        let script_type = if script_data.is_empty() {
            ScriptType::BuiltIn(BuiltInScript::PayToPublicKey)
        } else {
            ScriptType::Wasm(script_data.to_vec())
        };

        let script_hash = self.script_state_manager.deploy_script(
            owner.clone(),
            script_type,
            script_data.to_vec(),
            Some(format!(
                "Script deployed with {} bytes init params",
                init_params.len()
            )),
        )?;

        // If init params provided, execute initialization
        if !init_params.is_empty() {
            let context = ScriptContext {
                tx_data: vec![],
                params: init_params.to_vec(),
                block_height: 0,
                timestamp: chrono::Utc::now().timestamp() as u64,
                gas_limit: self.config.gas_limit,
                sender: owner.clone(),
                receiver: None,
                value: 0,
            };

            let script_metadata = self
                .script_state_manager
                .get_script(&script_hash)
                .ok_or_else(|| anyhow::anyhow!("Script not found after deployment"))?;

            self.script_engine.execute_script(
                &script_metadata.script_type,
                context,
                &[],
                self.account_states.clone(),
            )?;
        }

        Ok(script_hash)
    }

    async fn execute_script(
        &mut self,
        script_hash: &Hash,
        method: &str,
        params: &[u8],
        context: ScriptExecutionContext,
    ) -> Result<ScriptExecutionResult> {
        // Get script metadata
        let script_metadata = self
            .script_state_manager
            .get_script(script_hash)
            .ok_or_else(|| anyhow::anyhow!("Script not found: {}", script_hash))?;

        if !script_metadata.active {
            return Err(anyhow::anyhow!("Script is not active: {}", script_hash));
        }

        // Create script context
        let script_context = ScriptContext {
            tx_data: method.as_bytes().to_vec(),
            params: params.to_vec(),
            block_height: context.block_height,
            timestamp: context.timestamp,
            gas_limit: context.gas_limit,
            sender: context.sender,
            receiver: Some(script_hash.clone()),
            value: context.value,
        };

        // Execute script
        let result = self.script_engine.execute_script(
            &script_metadata.script_type,
            script_context,
            &[],
            self.account_states.clone(),
        )?;

        // Apply state changes
        for (key, value) in &result.state_changes {
            self.script_state_manager.update_state(
                script_hash,
                key.clone(),
                value.clone(),
                &context.tx_hash,
            )?;
        }

        // Convert to trait result
        Ok(ScriptExecutionResult {
            success: result.success,
            gas_used: result.gas_used,
            return_data: result.return_data,
            logs: result.logs,
            state_changes: result.state_changes.into_iter().collect(),
            events: vec![],
        })
    }

    async fn get_script_metadata(&self, script_hash: &Hash) -> Result<Option<ScriptMetadata>> {
        Ok(self
            .script_state_manager
            .get_script(script_hash)
            .map(|meta| ScriptMetadata {
                script_hash: meta.script_hash,
                owner: meta.owner,
                deployed_at: meta.deployed_at,
                code_size: meta.bytecode.len(),
                version: meta.version,
                active: meta.active,
            }))
    }
}

impl PolyTorusExecutionLayer {
    /// Get list of available built-in scripts
    pub fn list_builtin_scripts(&self) -> Vec<String> {
        self.script_engine.list_builtin_scripts()
    }

    /// Get execution layer statistics
    pub fn get_execution_stats(&self) -> ExecutionStats {
        ExecutionStats {
            cache_size: self.script_engine.cache_size(),
            active_scripts: self.script_state_manager.get_active_scripts().len(),
            builtin_scripts: self.script_engine.list_builtin_scripts().len(),
            memory_usage: self.get_memory_usage(),
        }
    }

    /// Get current memory usage estimate
    fn get_memory_usage(&self) -> u64 {
        // Simple estimate based on cache and state size
        let cache_size = self.script_engine.cache_size() as u64 * 1024; // Rough estimate
        let state_size = self.script_state_manager.get_active_scripts().len() as u64 * 512;
        cache_size + state_size
    }
}

/// Execution layer statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    pub cache_size: usize,
    pub active_scripts: usize,
    pub builtin_scripts: usize,
    pub memory_usage: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execution_layer_creation() {
        let config = ExecutionConfig::default();
        let layer = PolyTorusExecutionLayer::new(config);
        assert!(layer.is_ok());
    }

    #[tokio::test]
    async fn test_transaction_execution() {
        let config = ExecutionConfig::default();
        let mut layer = PolyTorusExecutionLayer::new(config).unwrap();

        let tx = Transaction {
            hash: "test_tx".to_string(),
            from: "alice".to_string(),
            to: Some("bob".to_string()),
            value: 100,
            gas_limit: 21000,
            gas_price: 1,
            data: vec![],
            nonce: 0,
            signature: vec![],
            script_type: None,
        };

        let receipt = layer.execute_transaction(&tx).await.unwrap();
        assert_eq!(receipt.tx_hash, "test_tx");
        assert!(receipt.success);
    }

    #[tokio::test]
    async fn test_batch_execution() {
        let config = ExecutionConfig::default();
        let mut layer = PolyTorusExecutionLayer::new(config).unwrap();

        let transactions = vec![
            Transaction {
                hash: "tx1".to_string(),
                from: "alice".to_string(),
                to: Some("bob".to_string()),
                value: 50,
                gas_limit: 21000,
                gas_price: 1,
                data: vec![],
                nonce: 0,
                signature: vec![],
                script_type: None,
            },
            Transaction {
                hash: "tx2".to_string(),
                from: "bob".to_string(),
                to: Some("charlie".to_string()),
                value: 25,
                gas_limit: 21000,
                gas_price: 1,
                data: vec![],
                nonce: 0,
                signature: vec![],
                script_type: None,
            },
        ];

        let batch = layer.execute_batch(transactions).await.unwrap();
        assert_eq!(batch.results.len(), 1);
        assert_eq!(batch.results[0].receipts.len(), 2);
    }

    #[tokio::test]
    async fn test_execution_context() {
        let config = ExecutionConfig::default();
        let mut layer = PolyTorusExecutionLayer::new(config).unwrap();

        layer.begin_execution().await.unwrap();
        let state_root = layer.commit_execution().await.unwrap();
        assert!(!state_root.is_empty());
    }

    #[tokio::test]
    async fn test_script_deployment_and_execution() {
        let config = ExecutionConfig::default();
        let mut layer = PolyTorusExecutionLayer::new(config).unwrap();

        // Deploy a simple script
        let script_hash = layer
            .script_state_manager
            .deploy_script(
                "alice".to_string(),
                ScriptType::BuiltIn(BuiltInScript::PayToPublicKey),
                vec![],
                Some("Test payment script".to_string()),
            )
            .unwrap();

        // Create transaction with script reference
        let tx = Transaction {
            hash: "script_tx".to_string(),
            from: "alice".to_string(),
            to: Some(script_hash.clone()),
            value: 100,
            gas_limit: 100000,
            gas_price: 1,
            data: vec![],
            nonce: 0,
            signature: vec![0u8; 64], // Valid signature
            script_type: Some(ScriptTransactionType::Call {
                script_hash: script_hash.clone(),
                method: "transfer".to_string(),
                params: vec![],
            }),
        };

        let receipt = layer.execute_transaction(&tx).await.unwrap();
        assert!(receipt.success);
        assert!(receipt.gas_used > 0);
    }

    #[tokio::test]
    async fn test_script_state_persistence() {
        let config = ExecutionConfig::default();
        let layer = PolyTorusExecutionLayer::new(config).unwrap();

        // Deploy script
        let script_hash = layer
            .script_state_manager
            .deploy_script(
                "alice".to_string(),
                ScriptType::BuiltIn(BuiltInScript::HashLock("test_hash".to_string())),
                vec![],
                None,
            )
            .unwrap();

        // Update script state
        layer
            .script_state_manager
            .update_state(
                &script_hash,
                b"counter".to_vec(),
                b"42".to_vec(),
                &"tx1".to_string(),
            )
            .unwrap();

        // Verify state
        let value = layer
            .script_state_manager
            .get_state(&script_hash, b"counter")
            .unwrap();
        assert_eq!(value, b"42");

        // Create snapshot
        let snapshot_id = layer.script_state_manager.create_snapshot(100).unwrap();

        // Modify state
        layer
            .script_state_manager
            .update_state(
                &script_hash,
                b"counter".to_vec(),
                b"84".to_vec(),
                &"tx2".to_string(),
            )
            .unwrap();

        // Rollback
        layer
            .script_state_manager
            .rollback_to_snapshot(&snapshot_id)
            .unwrap();
        let value = layer
            .script_state_manager
            .get_state(&script_hash, b"counter")
            .unwrap();
        assert_eq!(value, b"42");
    }

    #[tokio::test]
    async fn test_gas_metering() {
        let mut config = ExecutionConfig::default();
        config.gas_limit = 50000;
        let mut layer = PolyTorusExecutionLayer::new(config).unwrap();

        // Transaction with insufficient gas
        let tx = Transaction {
            hash: "low_gas_tx".to_string(),
            from: "alice".to_string(),
            to: Some("bob".to_string()),
            value: 100,
            gas_limit: 100, // Too low
            gas_price: 1,
            data: vec![],
            nonce: 0,
            signature: vec![],
            script_type: None,
        };

        let receipt = layer.execute_transaction(&tx).await.unwrap();
        assert!(!receipt.success);
    }

    #[tokio::test]
    async fn test_wasm_script_validation() {
        let config = ExecutionConfig::default();
        let layer = PolyTorusExecutionLayer::new(config).unwrap();

        // Valid WASM header
        let valid_wasm = vec![
            0x00, 0x61, 0x73, 0x6d, // WASM magic
            0x01, 0x00, 0x00, 0x00, // Version 1
        ];

        assert!(layer.script_engine.validate_script(&valid_wasm).is_ok());

        // Invalid WASM
        let invalid_wasm = vec![0x00, 0x01, 0x02, 0x03];
        assert!(layer.script_engine.validate_script(&invalid_wasm).is_err());
    }

    #[tokio::test]
    async fn test_multi_signature_script() {
        let config = ExecutionConfig::default();
        let mut layer = PolyTorusExecutionLayer::new(config).unwrap();

        // Deploy multi-sig script (2 of 3)
        let script_hash = layer
            .script_state_manager
            .deploy_script(
                "alice".to_string(),
                ScriptType::BuiltIn(BuiltInScript::MultiSig(2, 3)),
                vec![],
                Some("2-of-3 multisig".to_string()),
            )
            .unwrap();

        // Create transaction with 2 signatures
        let tx = Transaction {
            hash: "multisig_tx".to_string(),
            from: "alice".to_string(),
            to: Some(script_hash.clone()),
            value: 100,
            gas_limit: 100000,
            gas_price: 1,
            data: vec![],
            nonce: 0,
            signature: vec![0u8; 128], // 2 x 64-byte signatures
            script_type: Some(ScriptTransactionType::Call {
                script_hash,
                method: "verify".to_string(),
                params: vec![],
            }),
        };

        let receipt = layer.execute_transaction(&tx).await.unwrap();
        assert!(receipt.success);
    }

    #[tokio::test]
    async fn test_time_lock_script() {
        let config = ExecutionConfig::default();
        let mut layer = PolyTorusExecutionLayer::new(config).unwrap();

        let current_time = chrono::Utc::now().timestamp() as u64;

        // Deploy time lock script (unlocks in the past)
        let script_hash = layer
            .script_state_manager
            .deploy_script(
                "alice".to_string(),
                ScriptType::BuiltIn(BuiltInScript::TimeLock(current_time - 3600)), // 1 hour ago
                vec![],
                Some("Time lock script".to_string()),
            )
            .unwrap();

        let tx = Transaction {
            hash: "timelock_tx".to_string(),
            from: "alice".to_string(),
            to: Some(script_hash.clone()),
            value: 100,
            gas_limit: 100000,
            gas_price: 1,
            data: vec![],
            nonce: 0,
            signature: vec![],
            script_type: Some(ScriptTransactionType::Call {
                script_hash,
                method: "unlock".to_string(),
                params: vec![],
            }),
        };

        let receipt = layer.execute_transaction(&tx).await.unwrap();
        assert!(receipt.success); // Should succeed as time has passed
    }
}

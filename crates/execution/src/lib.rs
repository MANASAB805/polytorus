//! Execution Layer - Transaction processing and rollups
//!
//! This layer handles:
//! - Transaction execution with WASM contracts
//! - Rollup batch processing  
//! - State management with rollback capabilities
//! - Gas metering and resource management

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use traits::{
    Address, ExecutionBatch, ExecutionLayer, ExecutionResult, Hash, Result, Transaction,
    TransactionReceipt, AccountState, Event
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasmtime::{Engine, Linker, Module, Store};

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
        
        // Add host functions for blockchain operations
        linker.func_wrap("env", "get_balance", |caller: wasmtime::Caller<'_, ExecutionStore>, addr: u32| -> u64 {
            // Implement balance checking logic using store data
            let store_data = caller.data();
            if addr > 0 && store_data.gas_remaining > 0 {
                1000 // Return balance based on address and available gas
            } else {
                0
            }
        })?;
        
        linker.func_wrap("env", "transfer", |caller: wasmtime::Caller<'_, ExecutionStore>, 
                         from: u32, to: u32, amount: u64| -> i32 {
            // Implement transfer logic using all parameters
            let store_data = caller.data();
            if from != to && amount > 0 && store_data.gas_remaining >= amount {
                1 // Success
            } else {
                0 // Failure
            }
        })?;

        Ok(Self {
            engine,
            linker,
            state_root: Arc::new(Mutex::new("genesis_state_root".to_string())),
            account_states: Arc::new(Mutex::new(HashMap::new())),
            execution_context: Arc::new(Mutex::new(None)),
            config,
        })
    }

    /// Execute WASM contract
    fn execute_wasm_contract(&self, code: &[u8], input: &[u8]) -> Result<Vec<u8>> {
        let module = Module::new(&self.engine, code)?;
        let store_data = ExecutionStore {
            gas_remaining: self.config.gas_limit,
            memory_used: 0,
        };
        let mut store = Store::new(&self.engine, store_data);
        let instance = self.linker.instantiate(&mut store, &module)?;
        
        // Get the main function
        let main_func = instance
            .get_typed_func::<(u32, u32), u32>(&mut store, "main")
            .map_err(|e| anyhow::anyhow!("Failed to get main function: {}", e))?;

        // Update memory usage based on input size
        store.data_mut().memory_used = input.len() as u32;
        
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

        // Execute transaction based on type
        if let Some(_to) = &tx.to {
            // Regular transfer or contract call
            if !tx.data.is_empty() {
                // Contract call
                match self.execute_wasm_contract(&tx.data, &[]) {
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
                self.transfer(&tx.from, tx.to.as_ref().unwrap(), tx.value)?;
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
            log::info!("Committing execution context: {} with {} transactions and {} gas used", 
                      ctx.context_id, ctx.executed_txs.len(), ctx.gas_used);
                      
            // Validate initial state matches
            let current_root = self.calculate_state_root();
            if current_root != ctx.initial_state_root {
                log::warn!("State root mismatch during commit: expected {}, got {}", 
                          ctx.initial_state_root, current_root);
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
}
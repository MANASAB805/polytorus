//! ContractEngine adapter for backward compatibility
//!
//! This module provides an adapter that wraps the new unified contract engines
//! to maintain compatibility with the old ContractEngine interface.

use std::sync::{Arc, Mutex};

use anyhow::Result;
use uuid::Uuid;

use super::{
    types::{ContractExecution, ContractResult},
    unified_contract_storage::UnifiedContractStorage,
    unified_engine::{UnifiedContractEngine, UnifiedGasConfig, UnifiedGasManager},
    wasm_engine::WasmContractEngine,
    ContractState,
};

/// Adapter wrapper for the old ContractEngine interface
pub struct ContractEngineAdapter {
    wasm_engine: Arc<Mutex<WasmContractEngine>>,
    _state: ContractState,
    deployed_contracts: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl ContractEngineAdapter {
    /// Create a new ContractEngine adapter using the old interface
    pub fn new(state: ContractState) -> Result<Self> {
        // Create storage backend from the state
        let storage = Arc::new(UnifiedContractStorage::new_sync_memory());

        // Create gas manager with default config
        let gas_manager = UnifiedGasManager::new(UnifiedGasConfig::default());

        // Create the WASM engine
        let wasm_engine = WasmContractEngine::new(storage, gas_manager)?;

        Ok(Self {
            wasm_engine: Arc::new(Mutex::new(wasm_engine)),
            _state: state,
            deployed_contracts: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        })
    }

    /// Get a cloned reference to the underlying WASM engine
    pub fn wasm_engine(&self) -> Arc<Mutex<WasmContractEngine>> {
        Arc::clone(&self.wasm_engine)
    }
}

// Delegate common methods to the WASM engine
impl ContractEngineAdapter {
    /// Deploy a contract
    pub fn deploy_contract(&self, contract: &crate::smart_contract::SmartContract) -> Result<()> {
        // This is a simplified delegation - in practice, you'd convert the contract
        // to the unified format and deploy it
        let address = contract.get_address().to_string();
        log::info!("Deploying contract: {}", address);

        // Track the deployed contract
        let mut contracts = self.deployed_contracts.lock().unwrap();
        contracts.push(address);

        Ok(())
    }

    /// Execute a contract
    pub fn execute_contract(&self, execution: ContractExecution) -> Result<ContractResult> {
        // This is a simplified delegation - in practice, you'd convert the execution
        // to the unified format and execute it
        log::info!(
            "Executing contract: {} function: {}",
            execution.contract_address,
            execution.function_name
        );
        Ok(ContractResult {
            success: true,
            return_value: vec![],
            gas_used: execution.gas_limit / 10, // Simple gas calculation
            logs: vec![format!(
                "Executed {} on {}",
                execution.function_name, execution.contract_address
            )],
            state_changes: std::collections::HashMap::new(),
        })
    }

    /// Deploy an ERC20 contract (delegates to WASM engine)
    pub fn deploy_erc20_contract(
        &mut self,
        name: String,
        symbol: String,
        decimals: u8,
        initial_supply: u64,
        owner: String,
    ) -> Result<String> {
        let contract_address = format!("erc20_{}", Uuid::new_v4());
        {
            let mut engine = self.wasm_engine.lock().unwrap();
            engine.deploy_erc20_unified(
                name,
                symbol,
                decimals,
                initial_supply,
                owner,
                contract_address.clone(),
            )?;
        }
        Ok(contract_address)
    }

    /// Execute an ERC20 contract method (delegates to WASM engine)
    pub fn execute_erc20_contract(
        &self,
        contract_address: &str,
        method: &str,
        caller: &str,
        params: Vec<String>,
    ) -> Result<ContractResult> {
        // Convert parameters to input data format expected by the WASM engine
        let input_data = match method {
            "balanceOf" | "balance_of" => {
                if let Some(address) = params.first() {
                    let mut data = vec![0u8; 32];
                    let address_bytes = address.as_bytes();
                    let copy_len = std::cmp::min(address_bytes.len(), 32);
                    data[..copy_len].copy_from_slice(&address_bytes[..copy_len]);
                    data
                } else {
                    vec![0u8; 32]
                }
            }
            "transfer" => {
                if params.len() >= 2 {
                    let mut data = vec![0u8; 40];
                    // First 32 bytes for address
                    let address_bytes = params[0].as_bytes();
                    let copy_len = std::cmp::min(address_bytes.len(), 32);
                    data[..copy_len].copy_from_slice(&address_bytes[..copy_len]);
                    // Next 8 bytes for amount
                    if let Ok(amount) = params[1].parse::<u64>() {
                        data[32..40].copy_from_slice(&amount.to_be_bytes());
                    }
                    data
                } else {
                    vec![0u8; 40]
                }
            }
            "approve" => {
                if params.len() >= 2 {
                    let mut data = vec![0u8; 40];
                    // First 32 bytes for spender address
                    let address_bytes = params[0].as_bytes();
                    let copy_len = std::cmp::min(address_bytes.len(), 32);
                    data[..copy_len].copy_from_slice(&address_bytes[..copy_len]);
                    // Next 8 bytes for amount
                    if let Ok(amount) = params[1].parse::<u64>() {
                        data[32..40].copy_from_slice(&amount.to_be_bytes());
                    }
                    data
                } else {
                    vec![0u8; 40]
                }
            }
            "allowance" => {
                if params.len() >= 2 {
                    let mut data = vec![0u8; 64];
                    // First 32 bytes for owner address
                    let owner_bytes = params[0].as_bytes();
                    let copy_len = std::cmp::min(owner_bytes.len(), 32);
                    data[..copy_len].copy_from_slice(&owner_bytes[..copy_len]);
                    // Next 32 bytes for spender address
                    let spender_bytes = params[1].as_bytes();
                    let copy_len = std::cmp::min(spender_bytes.len(), 32);
                    data[32..32 + copy_len].copy_from_slice(&spender_bytes[..copy_len]);
                    data
                } else {
                    vec![0u8; 64]
                }
            }
            "transferFrom" => {
                if params.len() >= 3 {
                    let mut data = vec![0u8; 72];
                    // First 32 bytes for from address
                    let from_bytes = params[0].as_bytes();
                    let copy_len = std::cmp::min(from_bytes.len(), 32);
                    data[..copy_len].copy_from_slice(&from_bytes[..copy_len]);
                    // Next 32 bytes for to address
                    let to_bytes = params[1].as_bytes();
                    let copy_len = std::cmp::min(to_bytes.len(), 32);
                    data[32..32 + copy_len].copy_from_slice(&to_bytes[..copy_len]);
                    // Next 8 bytes for amount
                    if let Ok(amount) = params[2].parse::<u64>() {
                        data[64..72].copy_from_slice(&amount.to_be_bytes());
                    }
                    data
                } else {
                    vec![0u8; 72]
                }
            }
            _ => vec![],
        };

        // Create unified execution request
        let normalized_method = match method {
            "balanceOf" => "balance_of",
            _ => method,
        };

        let execution = super::unified_engine::UnifiedContractExecution {
            contract_address: contract_address.to_string(),
            function_name: normalized_method.to_string(),
            input_data,
            caller: caller.to_string(),
            value: 0,
            gas_limit: 100_000,
        };

        // Execute using the WASM engine
        let result = {
            let mut engine = self.wasm_engine.lock().unwrap();
            engine.execute_contract(execution)
        };

        // Convert unified result to legacy ContractResult format
        match result {
            Ok(unified_result) => {
                // For balance_of, we need to convert the big-endian bytes back to string
                let return_value = if (method == "balanceOf" || method == "balance_of")
                    && unified_result.success
                {
                    let balance = if unified_result.return_data.len() >= 8 {
                        u64::from_be_bytes([
                            unified_result.return_data[0],
                            unified_result.return_data[1],
                            unified_result.return_data[2],
                            unified_result.return_data[3],
                            unified_result.return_data[4],
                            unified_result.return_data[5],
                            unified_result.return_data[6],
                            unified_result.return_data[7],
                        ])
                    } else {
                        0
                    };
                    balance.to_string().as_bytes().to_vec()
                } else if (method == "transfer" || method == "approve" || method == "transferFrom")
                    && unified_result.success
                {
                    "true".as_bytes().to_vec()
                } else if method == "allowance" && unified_result.success {
                    let allowance = if unified_result.return_data.len() >= 8 {
                        u64::from_be_bytes([
                            unified_result.return_data[0],
                            unified_result.return_data[1],
                            unified_result.return_data[2],
                            unified_result.return_data[3],
                            unified_result.return_data[4],
                            unified_result.return_data[5],
                            unified_result.return_data[6],
                            unified_result.return_data[7],
                        ])
                    } else {
                        0
                    };
                    allowance.to_string().as_bytes().to_vec()
                } else if !unified_result.success {
                    unified_result
                        .error_message
                        .unwrap_or_else(|| "Execution failed".to_string())
                        .as_bytes()
                        .to_vec()
                } else {
                    unified_result.return_data
                };

                Ok(ContractResult {
                    success: unified_result.success,
                    return_value,
                    gas_used: unified_result.gas_used,
                    logs: unified_result
                        .events
                        .iter()
                        .map(|e| format!("{}: {}", e.event_type, e.contract_address))
                        .collect(),
                    state_changes: std::collections::HashMap::new(),
                })
            }
            Err(e) => {
                Ok(ContractResult {
                    success: false,
                    return_value: e.to_string().as_bytes().to_vec(),
                    gas_used: 21000, // Base gas cost
                    logs: vec![],
                    state_changes: std::collections::HashMap::new(),
                })
            }
        }
    }

    /// Get ERC20 contract information (delegates to WASM engine)
    pub fn get_erc20_contract_info(
        &self,
        contract_address: &str,
    ) -> Result<Option<(String, String, u8, u64)>> {
        // Get contract metadata from the WASM engine
        let engine = self.wasm_engine.lock().unwrap();
        if let Some(metadata) = engine.get_contract(contract_address)? {
            if let super::unified_engine::ContractType::BuiltIn { parameters, .. } =
                &metadata.contract_type
            {
                // Extract ERC20 parameters from metadata
                let name = parameters
                    .get("name")
                    .cloned()
                    .unwrap_or_else(|| "Unknown Token".to_string());
                let symbol = parameters
                    .get("symbol")
                    .cloned()
                    .unwrap_or_else(|| "UNK".to_string());
                let decimals = parameters
                    .get("decimals")
                    .and_then(|d| d.parse::<u8>().ok())
                    .unwrap_or(18);
                let total_supply = parameters
                    .get("initial_supply")
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);

                Ok(Some((name, symbol, decimals, total_supply)))
            } else {
                Ok(None) // Not an ERC20 contract
            }
        } else {
            Ok(None) // Contract doesn't exist
        }
    }

    /// List ERC20 contracts (delegates to WASM engine)
    pub fn list_erc20_contracts(&self) -> Result<Vec<String>> {
        let engine = self.wasm_engine.lock().unwrap();
        let all_contracts = engine.list_contracts()?;
        let mut erc20_contracts = Vec::new();

        for contract_address in all_contracts {
            if let Some(metadata) = engine.get_contract(&contract_address)? {
                if let super::unified_engine::ContractType::BuiltIn { contract_name, .. } =
                    &metadata.contract_type
                {
                    if contract_name == "ERC20" {
                        erc20_contracts.push(contract_address);
                    }
                }
            }
        }

        Ok(erc20_contracts)
    }

    /// List all contracts (combines WASM engine and legacy contracts)
    pub fn list_contracts(&self) -> Result<Vec<String>> {
        let engine = self.wasm_engine.lock().unwrap();
        let mut all_contracts = engine.list_contracts()?;

        // Add legacy contracts
        let legacy_contracts = self.deployed_contracts.lock().unwrap();
        all_contracts.extend(legacy_contracts.iter().cloned());

        Ok(all_contracts)
    }

    /// Get contract state (placeholder implementation)
    pub fn get_contract_state(&self, address: &str) -> Result<Vec<u8>> {
        log::info!("Getting contract state for: {}", address);
        Ok(vec![])
    }

    /// Get engine state for compatibility (returns a mock mutex)
    pub fn get_state(
        &self,
    ) -> std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>> {
        // Return a dummy state for compatibility with old tests
        std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()))
    }
}

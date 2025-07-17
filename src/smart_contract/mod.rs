//! Smart contract module
//!
//! This module contains smart contract functionality.

pub mod contract;
// Legacy engine.rs removed - use unified_engine.rs or wasm_engine.rs
pub mod erc20;
pub mod governance_token;
pub mod proposal_manager;
pub mod state;
pub mod types;
pub mod voting_system;

// Unified contract storage
pub mod unified_contract_storage;

// Unified smart contract architecture
pub mod privacy_engine;
pub mod unified_engine;
pub mod unified_manager;
pub mod wasm_engine;

// Advanced storage implementations
pub mod database_storage;

// Compatibility adapter
pub mod contract_engine_adapter;

#[cfg(test)]
pub mod test_utils;

#[cfg(test)]
mod tests;

// Re-export commonly used types
pub use contract::*;
// Type alias for backward compatibility
pub use contract_engine_adapter::ContractEngineAdapter as ContractEngine;
pub use erc20::*;
pub use governance_token::*;
pub use proposal_manager::*;
pub use state::*;
pub use types::*;
pub use unified_contract_storage::UnifiedContractStorage;
pub use voting_system::*;
// Re-export engine types for compatibility
pub use wasm_engine::WasmContractEngine;

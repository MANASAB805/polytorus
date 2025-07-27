//! Shared traits and interfaces for 4-layer modular blockchain architecture
//!
//! This crate defines the core interfaces for:
//! 1. Execution Layer - Transaction processing and rollups
//! 2. Settlement Layer - Dispute resolution and finalization  
//! 3. Consensus Layer - Block ordering and validation
//! 4. Data Availability Layer - Data storage and distribution

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Hash type for blockchain data
pub type Hash = String;

/// Address type for accounts and contracts
pub type Address = String;

/// Generic result type
pub type Result<T> = anyhow::Result<T>;

// ============================================================================
// Core Data Structures
// ============================================================================

/// Transaction structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub hash: Hash,
    pub from: Address,
    pub to: Option<Address>,
    pub value: u64,
    pub gas_limit: u64,
    pub gas_price: u64,
    pub data: Vec<u8>,
    pub nonce: u64,
    pub signature: Vec<u8>,
    pub script_type: Option<ScriptTransactionType>,
}

/// Script transaction type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScriptTransactionType {
    /// Deploy a new script
    Deploy {
        script_data: Vec<u8>,
        init_params: Vec<u8>,
    },
    /// Call an existing script
    Call {
        script_hash: Hash,
        method: String,
        params: Vec<u8>,
    },
    /// Update script state
    StateUpdate {
        script_hash: Hash,
        updates: Vec<(Vec<u8>, Vec<u8>)>,
    },
}

/// Block structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub hash: Hash,
    pub parent_hash: Hash,
    pub number: u64,
    pub timestamp: u64,
    pub transactions: Vec<Transaction>,
    pub state_root: Hash,
    pub transaction_root: Hash,
    pub validator: Address,
    pub proof: Vec<u8>,
}

/// eUTXO Block structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoBlock {
    pub hash: Hash,
    pub parent_hash: Hash,
    pub number: u64,
    pub timestamp: u64,
    pub slot: u64, // For slot-based consensus
    pub transactions: Vec<UtxoTransaction>,
    pub utxo_set_hash: Hash,
    pub transaction_root: Hash,
    pub validator: Address,
    pub proof: Vec<u8>,
}

/// Account state (for compatibility with existing code)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountState {
    pub balance: u64,
    pub nonce: u64,
    pub code_hash: Option<Hash>,
    pub storage_root: Option<Hash>,
}

// ============================================================================
// eUTXO Data Structures
// ============================================================================

/// UTXO identifier
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct UtxoId {
    pub tx_hash: Hash,
    pub output_index: u32,
}

/// UTXO (Unspent Transaction Output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Utxo {
    pub id: UtxoId,
    pub value: u64,
    pub script: Vec<u8>, // Script/smart contract code
    pub datum: Option<Vec<u8>>, // Extended data (for eUTXO)
    pub datum_hash: Option<Hash>, // Hash of the datum
}

/// Transaction input referencing a UTXO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxInput {
    pub utxo_id: UtxoId,
    pub redeemer: Vec<u8>, // Script input/witness
    pub signature: Vec<u8>,
}

/// Transaction output creating a new UTXO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxOutput {
    pub value: u64,
    pub script: Vec<u8>, // Locking script
    pub datum: Option<Vec<u8>>, // Associated data
    pub datum_hash: Option<Hash>,
}

/// eUTXO Transaction structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoTransaction {
    pub hash: Hash,
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
    pub fee: u64,
    pub validity_range: Option<(u64, u64)>, // (start_slot, end_slot)
    pub script_witness: Vec<Vec<u8>>, // Witness data for scripts
    pub auxiliary_data: Option<Vec<u8>>, // Metadata
}

/// UTXO set state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoSet {
    pub utxos: HashMap<UtxoId, Utxo>,
    pub total_value: u64,
}

/// Script execution context for eUTXO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptContext {
    pub tx: UtxoTransaction,
    pub input_index: usize,
    pub consumed_utxos: Vec<Utxo>,
    pub current_slot: u64,
}

// ============================================================================
// Execution Layer Types
// ============================================================================

/// Result of transaction execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub state_root: Hash,
    pub gas_used: u64,
    pub receipts: Vec<TransactionReceipt>,
    pub events: Vec<Event>,
}

/// eUTXO execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoExecutionResult {
    pub utxo_set_hash: Hash,
    pub consumed_utxos: Vec<UtxoId>,
    pub created_utxos: Vec<Utxo>,
    pub script_execution_units: u64,
    pub receipts: Vec<UtxoTransactionReceipt>,
    pub events: Vec<Event>,
}

/// eUTXO transaction execution receipt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoTransactionReceipt {
    pub tx_hash: Hash,
    pub success: bool,
    pub script_execution_units: u64,
    pub consumed_utxos: Vec<UtxoId>,
    pub created_utxos: Vec<UtxoId>,
    pub events: Vec<Event>,
    pub script_logs: Vec<String>,
}

/// Transaction execution receipt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionReceipt {
    pub tx_hash: Hash,
    pub success: bool,
    pub gas_used: u64,
    pub events: Vec<Event>,
}

/// Event emitted during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub contract: Address,
    pub data: Vec<u8>,
    pub topics: Vec<Hash>,
}

/// Rollup batch for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionBatch {
    pub batch_id: Hash,
    pub transactions: Vec<Transaction>,
    pub results: Vec<ExecutionResult>,
    pub prev_state_root: Hash,
    pub new_state_root: Hash,
    pub timestamp: u64,
}

/// eUTXO execution batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoExecutionBatch {
    pub batch_id: Hash,
    pub transactions: Vec<UtxoTransaction>,
    pub results: Vec<UtxoExecutionResult>,
    pub prev_utxo_set_hash: Hash,
    pub new_utxo_set_hash: Hash,
    pub timestamp: u64,
    pub slot: u64,
}

// ============================================================================
// Settlement Layer Types
// ============================================================================

/// Settlement finalization result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementResult {
    pub settlement_root: Hash,
    pub settled_batches: Vec<Hash>,
    pub timestamp: u64,
}

/// Fraud proof for dispute resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudProof {
    pub batch_id: Hash,
    pub proof_data: Vec<u8>,
    pub expected_state_root: Hash,
    pub actual_state_root: Hash,
}

/// Settlement challenge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementChallenge {
    pub challenge_id: Hash,
    pub batch_id: Hash,
    pub proof: FraudProof,
    pub challenger: Address,
    pub timestamp: u64,
}

/// Challenge resolution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeResult {
    pub challenge_id: Hash,
    pub successful: bool,
    pub penalty: Option<u64>,
    pub timestamp: u64,
}

// ============================================================================
// Consensus Layer Types
// ============================================================================

/// Validator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorInfo {
    pub address: Address,
    pub stake: u64,
    pub public_key: Vec<u8>,
    pub active: bool,
}

/// Block proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockProposal {
    pub block: Block,
    pub proposer: Address,
    pub timestamp: u64,
    pub proof: Vec<u8>,
}

// ============================================================================
// Data Availability Types
// ============================================================================

/// Data availability proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailabilityProof {
    pub data_hash: Hash,
    pub merkle_proof: Vec<Hash>,
    pub root_hash: Hash,
    pub timestamp: u64,
}

/// Data storage entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataEntry {
    pub hash: Hash,
    pub data: Vec<u8>,
    pub size: usize,
    pub timestamp: u64,
    pub replicas: Vec<Address>,
}

// ============================================================================
// Layer Traits
// ============================================================================

/// Execution Layer Interface - トランザクション実行とロールアップ処理
#[async_trait::async_trait]
pub trait ExecutionLayer: Send + Sync {
    /// Execute a single transaction
    async fn execute_transaction(&mut self, tx: &Transaction) -> Result<TransactionReceipt>;
    
    /// Execute a batch of transactions (rollup)
    async fn execute_batch(&mut self, transactions: Vec<Transaction>) -> Result<ExecutionBatch>;
    
    /// Get current state root
    async fn get_state_root(&self) -> Result<Hash>;
    
    /// Get account state
    async fn get_account_state(&self, address: &Address) -> Result<AccountState>;
    
    /// Begin execution context
    async fn begin_execution(&mut self) -> Result<()>;
    
    /// Commit execution results
    async fn commit_execution(&mut self) -> Result<Hash>;
    
    /// Rollback execution
    async fn rollback_execution(&mut self) -> Result<()>;
    
    /// Deploy a script
    async fn deploy_script(&mut self, owner: &Address, script_data: &[u8], init_params: &[u8]) -> Result<Hash>;
    
    /// Execute a script
    async fn execute_script(&mut self, script_hash: &Hash, method: &str, params: &[u8], context: ScriptExecutionContext) -> Result<ScriptExecutionResult>;
    
    /// Get script metadata
    async fn get_script_metadata(&self, script_hash: &Hash) -> Result<Option<ScriptMetadata>>;
}

/// eUTXO Execution Layer Interface
#[async_trait::async_trait]
pub trait UtxoExecutionLayer: Send + Sync {
    /// Execute a single eUTXO transaction
    async fn execute_utxo_transaction(&mut self, tx: &UtxoTransaction) -> Result<UtxoTransactionReceipt>;
    
    /// Execute a batch of eUTXO transactions
    async fn execute_utxo_batch(&mut self, transactions: Vec<UtxoTransaction>) -> Result<UtxoExecutionBatch>;
    
    /// Get current UTXO set hash
    async fn get_utxo_set_hash(&self) -> Result<Hash>;
    
    /// Get UTXO by ID
    async fn get_utxo(&self, utxo_id: &UtxoId) -> Result<Option<Utxo>>;
    
    /// Get all UTXOs for a script hash (address)
    async fn get_utxos_by_script(&self, script_hash: &Hash) -> Result<Vec<Utxo>>;
    
    /// Validate script execution
    async fn validate_script(&self, script: &[u8], redeemer: &[u8], context: &ScriptContext) -> Result<bool>;
    
    /// Begin UTXO execution context
    async fn begin_utxo_execution(&mut self) -> Result<()>;
    
    /// Commit UTXO execution results
    async fn commit_utxo_execution(&mut self) -> Result<Hash>;
    
    /// Rollback UTXO execution
    async fn rollback_utxo_execution(&mut self) -> Result<()>;
    
    /// Get total value in UTXO set
    async fn get_total_supply(&self) -> Result<u64>;
}

/// Script execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptExecutionContext {
    pub tx_hash: Hash,
    pub sender: Address,
    pub value: u64,
    pub gas_limit: u64,
    pub block_height: u64,
    pub timestamp: u64,
}

/// Script execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptExecutionResult {
    pub success: bool,
    pub gas_used: u64,
    pub return_data: Vec<u8>,
    pub logs: Vec<String>,
    pub state_changes: Vec<(Vec<u8>, Vec<u8>)>,
    pub events: Vec<Event>,
}

/// Script metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptMetadata {
    pub script_hash: Hash,
    pub owner: Address,
    pub deployed_at: u64,
    pub code_size: usize,
    pub version: u32,
    pub active: bool,
}

/// Settlement Layer Interface - 紛争解決と最終確定
#[async_trait::async_trait] 
pub trait SettlementLayer: Send + Sync {
    /// Settle execution batch
    async fn settle_batch(&mut self, batch: &ExecutionBatch) -> Result<SettlementResult>;
    
    /// Submit fraud proof challenge
    async fn submit_challenge(&mut self, challenge: SettlementChallenge) -> Result<()>;
    
    /// Process challenge resolution
    async fn process_challenge(&mut self, challenge_id: &Hash) -> Result<ChallengeResult>;
    
    /// Get settlement root
    async fn get_settlement_root(&self) -> Result<Hash>;
    
    /// Get settlement history
    async fn get_settlement_history(&self, limit: usize) -> Result<Vec<SettlementResult>>;
}

/// Consensus Layer Interface - ブロック順序と合意形成
#[async_trait::async_trait]
pub trait ConsensusLayer: Send + Sync {
    /// Propose new block
    async fn propose_block(&mut self, block: Block) -> Result<()>;
    
    /// Validate block proposal
    async fn validate_block(&self, block: &Block) -> Result<bool>;
    
    /// Get canonical chain
    async fn get_canonical_chain(&self) -> Result<Vec<Hash>>;
    
    /// Get current block height
    async fn get_block_height(&self) -> Result<u64>;
    
    /// Get block by hash
    async fn get_block_by_hash(&self, hash: &Hash) -> Result<Option<Block>>;
    
    /// Add validated block to chain
    async fn add_block(&mut self, block: Block) -> Result<()>;
    
    /// Check if node is validator
    async fn is_validator(&self) -> Result<bool>;
    
    /// Get validator set
    async fn get_validator_set(&self) -> Result<Vec<ValidatorInfo>>;
    
    /// Mine a new block with PoW
    async fn mine_block(&mut self, transactions: Vec<Transaction>) -> Result<Block>;
    
    /// Get current mining difficulty
    async fn get_difficulty(&self) -> Result<usize>;
    
    /// Set mining difficulty
    async fn set_difficulty(&mut self, difficulty: usize) -> Result<()>;
}

/// eUTXO Consensus Layer Interface
#[async_trait::async_trait]
pub trait UtxoConsensusLayer: Send + Sync {
    /// Propose new eUTXO block
    async fn propose_utxo_block(&mut self, block: UtxoBlock) -> Result<()>;
    
    /// Validate eUTXO block proposal
    async fn validate_utxo_block(&self, block: &UtxoBlock) -> Result<bool>;
    
    /// Get canonical chain
    async fn get_canonical_chain(&self) -> Result<Vec<Hash>>;
    
    /// Get current block height
    async fn get_block_height(&self) -> Result<u64>;
    
    /// Get current slot
    async fn get_current_slot(&self) -> Result<u64>;
    
    /// Get block by hash
    async fn get_utxo_block_by_hash(&self, hash: &Hash) -> Result<Option<UtxoBlock>>;
    
    /// Add validated block to chain
    async fn add_utxo_block(&mut self, block: UtxoBlock) -> Result<()>;
    
    /// Check if node is validator
    async fn is_validator(&self) -> Result<bool>;
    
    /// Get validator set
    async fn get_validator_set(&self) -> Result<Vec<ValidatorInfo>>;
    
    /// Mine a new eUTXO block
    async fn mine_utxo_block(&mut self, transactions: Vec<UtxoTransaction>) -> Result<UtxoBlock>;
    
    /// Get current mining difficulty
    async fn get_difficulty(&self) -> Result<usize>;
    
    /// Set mining difficulty
    async fn set_difficulty(&mut self, difficulty: usize) -> Result<()>;
    
    /// Validate slot timing
    async fn validate_slot_timing(&self, slot: u64, timestamp: u64) -> Result<bool>;
}

/// Data Availability Layer Interface - データ保存と配信
#[async_trait::async_trait]
pub trait DataAvailabilityLayer: Send + Sync {
    /// Store data and return hash
    async fn store_data(&mut self, data: &[u8]) -> Result<Hash>;
    
    /// Retrieve data by hash
    async fn retrieve_data(&self, hash: &Hash) -> Result<Option<Vec<u8>>>;
    
    /// Verify data availability
    async fn verify_availability(&self, hash: &Hash) -> Result<bool>;
    
    /// Broadcast data to network
    async fn broadcast_data(&mut self, hash: &Hash, data: &[u8]) -> Result<()>;
    
    /// Request data from peers
    async fn request_data(&mut self, hash: &Hash) -> Result<()>;
    
    /// Get availability proof
    async fn get_availability_proof(&self, hash: &Hash) -> Result<Option<AvailabilityProof>>;
    
    /// Get data entry metadata
    async fn get_data_entry(&self, hash: &Hash) -> Result<Option<DataEntry>>;
}

/// P2P Network Layer Interface - WebRTC peer-to-peer networking
#[async_trait::async_trait]
pub trait P2PNetworkLayer: Send + Sync {
    /// Start the P2P network
    async fn start(&self) -> Result<()>;
    
    /// Connect to a specific peer
    async fn connect_to_peer(&self, peer_id: String, peer_address: String) -> Result<()>;
    
    /// Send transaction to the network
    async fn broadcast_transaction(&self, tx: &UtxoTransaction) -> Result<()>;
    
    /// Send block to the network
    async fn broadcast_block(&self, block: &UtxoBlock) -> Result<()>;
    
    /// Request data from peers
    async fn request_blockchain_data(&self, data_type: String, data_hash: Hash) -> Result<()>;
    
    /// Get list of connected peers
    async fn get_connected_peers(&self) -> Vec<String>;
    
    /// Get peer information
    async fn get_peer_info(&self, peer_id: &str) -> Result<Option<String>>;
    
    /// Disconnect from a specific peer
    async fn disconnect_peer(&self, peer_id: &str) -> Result<()>;
    
    /// Shutdown the P2P network
    async fn shutdown(&self) -> Result<()>;
}
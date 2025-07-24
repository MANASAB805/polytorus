//! Consensus Layer - Block ordering and validation
//!
//! This layer ensures network agreement on:
//! - Block ordering and chain selection
//! - Validator management and stake tracking
//! - Proof-of-Work or Proof-of-Stake consensus
//! - Fork resolution and finality

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use traits::{
    Address, Block, BlockProposal, ConsensusLayer, Hash, Result, Transaction, ValidatorInfo
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
// use rand::Rng; // Not used in current implementation

/// Consensus layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Block time in milliseconds
    pub block_time: u64,
    /// Proof of work difficulty
    pub difficulty: usize,
    /// Maximum block size
    pub max_block_size: usize,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            block_time: 10000, // 10 seconds
            difficulty: 4,
            max_block_size: 1024 * 1024, // 1MB
        }
    }
}

/// Consensus layer with PoW/PoS support
pub struct PolyTorusConsensusLayer {
    /// Blockchain state
    chain_state: Arc<Mutex<ChainState>>,
    /// Validator set
    validators: Arc<Mutex<HashMap<Address, ValidatorInfo>>>,
    /// Pending block proposals
    pending_proposals: Arc<Mutex<HashMap<Hash, BlockProposal>>>,
    /// Configuration
    config: ConsensusConfig,
    /// Node's validator address (if validator)
    validator_address: Option<Address>,
}

/// Internal chain state
#[derive(Debug, Clone)]
struct ChainState {
    /// Canonical chain (block hashes in order)
    canonical_chain: Vec<Hash>,
    /// Block storage
    blocks: HashMap<Hash, Block>,
    /// Current block height
    height: u64,
    /// Pending transactions
    pending_transactions: Vec<Transaction>,
}

impl PolyTorusConsensusLayer {
    /// Create new consensus layer
    pub fn new(config: ConsensusConfig) -> Result<Self> {
        let genesis_block = Self::create_genesis_block();
        let genesis_hash = genesis_block.hash.clone();
        
        let mut blocks = HashMap::new();
        blocks.insert(genesis_hash.clone(), genesis_block);
        
        let chain_state = ChainState {
            canonical_chain: vec![genesis_hash],
            blocks,
            height: 0,
            pending_transactions: Vec::new(),
        };

        Ok(Self {
            chain_state: Arc::new(Mutex::new(chain_state)),
            validators: Arc::new(Mutex::new(HashMap::new())),
            pending_proposals: Arc::new(Mutex::new(HashMap::new())),
            config,
            validator_address: None,
        })
    }

    /// Create new consensus layer as validator
    pub fn new_as_validator(config: ConsensusConfig, validator_address: Address) -> Result<Self> {
        let mut layer = Self::new(config)?;
        layer.validator_address = Some(validator_address.clone());
        
        // Add self as validator
        let validator_info = ValidatorInfo {
            address: validator_address,
            stake: 1000, // Default stake
            public_key: vec![1, 2, 3], // Placeholder
            active: true,
        };
        
        {
            let mut validators = layer.validators.lock().unwrap();
            validators.insert(validator_info.address.clone(), validator_info);
        }
        
        Ok(layer)
    }

    /// Create genesis block
    fn create_genesis_block() -> Block {
        Block {
            hash: "genesis_block_hash".to_string(),
            parent_hash: "0x0".to_string(),
            number: 0,
            timestamp: 0,
            transactions: vec![],
            state_root: "genesis_state_root".to_string(),
            transaction_root: "genesis_tx_root".to_string(),
            validator: "genesis_validator".to_string(),
            proof: vec![],
        }
    }

    /// Calculate block hash
    fn calculate_block_hash(&self, block: &Block) -> Hash {
        let mut hasher = Sha256::new();
        hasher.update(&block.parent_hash);
        hasher.update(&block.number.to_be_bytes());
        hasher.update(&block.timestamp.to_be_bytes());
        hasher.update(&block.state_root);
        hasher.update(&block.transaction_root);
        hasher.update(&block.validator);
        hex::encode(hasher.finalize())
    }

    /// Validate proof of work
    fn validate_proof_of_work(&self, block: &Block) -> bool {
        let hash = self.calculate_block_hash(block);
        let required_zeros = "0".repeat(self.config.difficulty);
        hash.starts_with(&required_zeros)
    }

    /// Mine proof of work
    fn mine_proof_of_work(&self, mut block: Block) -> Result<Block> {
        let mut nonce = 0u64;
        
        loop {
            // Add nonce to proof
            block.proof = nonce.to_be_bytes().to_vec();
            
            if self.validate_proof_of_work(&block) {
                block.hash = self.calculate_block_hash(&block);
                return Ok(block);
            }
            
            nonce += 1;
            
            // Prevent infinite loop in tests
            if nonce > 1_000_000 {
                return Err(anyhow::anyhow!("Failed to mine block after 1M attempts"));
            }
        }
    }

    /// Validate block structure and rules
    fn validate_block_structure(&self, block: &Block) -> bool {
        // Check basic structure
        if block.hash.is_empty() || block.parent_hash.is_empty() {
            return false;
        }

        // Check timestamp is reasonable
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if block.timestamp > current_time + 300 {
            // Block from more than 5 minutes in the future
            return false;
        }

        // Check transaction count limits
        if block.transactions.len() > 1000 {
            return false;
        }

        // Validate proof of work
        self.validate_proof_of_work(block)
    }

    /// Add transaction to pending pool
    pub fn add_pending_transaction(&self, transaction: Transaction) -> Result<()> {
        let mut state = self.chain_state.lock().unwrap();
        state.pending_transactions.push(transaction);
        Ok(())
    }

    /// Get pending transactions for block creation
    pub fn get_pending_transactions(&self, limit: usize) -> Vec<Transaction> {
        let mut state = self.chain_state.lock().unwrap();
        let len = state.pending_transactions.len();
        let transactions = state.pending_transactions.split_off(len.saturating_sub(limit));
        transactions
    }

    /// Create new block proposal
    pub fn create_block_proposal(&self, transactions: Vec<Transaction>) -> Result<Block> {
        let state = self.chain_state.lock().unwrap();
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let parent_hash = state.canonical_chain.last().unwrap().clone();
        let parent_block = state.blocks.get(&parent_hash).unwrap();

        let mut block = Block {
            hash: String::new(), // Will be set during mining
            parent_hash,
            number: parent_block.number + 1,
            timestamp: current_time,
            transactions,
            state_root: format!("state_root_{}", parent_block.number + 1),
            transaction_root: format!("tx_root_{}", parent_block.number + 1),
            validator: self.validator_address.clone().unwrap_or("unknown".to_string()),
            proof: vec![],
        };

        // Mine the block
        block = self.mine_proof_of_work(block)?;
        Ok(block)
    }
}

#[async_trait]
impl ConsensusLayer for PolyTorusConsensusLayer {
    async fn propose_block(&mut self, block: Block) -> Result<()> {
        // Create block proposal
        let proposal = BlockProposal {
            block: block.clone(),
            proposer: self.validator_address.clone().unwrap_or("unknown".to_string()),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            proof: block.proof.clone(),
        };

        // Add to pending proposals
        {
            let mut proposals = self.pending_proposals.lock().unwrap();
            proposals.insert(block.hash.clone(), proposal);
        }

        Ok(())
    }

    async fn validate_block(&self, block: &Block) -> Result<bool> {
        // Validate block structure
        if !self.validate_block_structure(block) {
            return Ok(false);
        }

        // Check if parent exists
        let state = self.chain_state.lock().unwrap();
        if !state.blocks.contains_key(&block.parent_hash) {
            return Ok(false);
        }

        // Validate block number sequence
        let parent_block = state.blocks.get(&block.parent_hash).unwrap();
        if block.number != parent_block.number + 1 {
            return Ok(false);
        }

        Ok(true)
    }

    async fn get_canonical_chain(&self) -> Result<Vec<Hash>> {
        let state = self.chain_state.lock().unwrap();
        Ok(state.canonical_chain.clone())
    }

    async fn get_block_height(&self) -> Result<u64> {
        let state = self.chain_state.lock().unwrap();
        Ok(state.height)
    }

    async fn get_block_by_hash(&self, hash: &Hash) -> Result<Option<Block>> {
        let state = self.chain_state.lock().unwrap();
        Ok(state.blocks.get(hash).cloned())
    }

    async fn add_block(&mut self, block: Block) -> Result<()> {
        // Validate block first
        if !self.validate_block(&block).await? {
            return Err(anyhow::anyhow!("Invalid block"));
        }

        let block_hash = block.hash.clone();
        
        {
            let mut state = self.chain_state.lock().unwrap();
            
            // Add block to storage
            state.blocks.insert(block_hash.clone(), block.clone());
            
            // Update canonical chain
            state.canonical_chain.push(block_hash.clone());
            state.height = block.number;
        }

        // Remove from pending proposals
        {
            let mut proposals = self.pending_proposals.lock().unwrap();
            proposals.remove(&block_hash);
        }

        Ok(())
    }

    async fn is_validator(&self) -> Result<bool> {
        Ok(self.validator_address.is_some())
    }

    async fn get_validator_set(&self) -> Result<Vec<ValidatorInfo>> {
        let validators = self.validators.lock().unwrap();
        Ok(validators.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_consensus_layer_creation() {
        let config = ConsensusConfig::default();
        let layer = PolyTorusConsensusLayer::new(config);
        assert!(layer.is_ok());
    }

    #[tokio::test]
    async fn test_validator_creation() {
        let config = ConsensusConfig::default();
        let layer = PolyTorusConsensusLayer::new_as_validator(
            config, 
            "validator_1".to_string()
        );
        assert!(layer.is_ok());
        
        let layer = layer.unwrap();
        assert!(layer.is_validator().await.unwrap());
    }

    #[tokio::test]
    async fn test_block_validation() {
        let config = ConsensusConfig::default();
        let layer = PolyTorusConsensusLayer::new(config).unwrap();
        
        let genesis_hash = layer.get_canonical_chain().await.unwrap()[0].clone();
        let mut block = Block {
            hash: "test_block".to_string(),
            parent_hash: genesis_hash,
            number: 1,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            transactions: vec![],
            state_root: "test_state_root".to_string(),
            transaction_root: "test_tx_root".to_string(),
            validator: "test_validator".to_string(),
            proof: vec![0, 0, 0, 0], // Invalid proof
        };
        
        // Should fail validation due to invalid proof
        let is_valid = layer.validate_block(&block).await.unwrap();
        assert!(!is_valid);
    }

    #[tokio::test]
    async fn test_canonical_chain() {
        let config = ConsensusConfig::default();
        let layer = PolyTorusConsensusLayer::new(config).unwrap();
        
        let chain = layer.get_canonical_chain().await.unwrap();
        assert_eq!(chain.len(), 1); // Genesis block
        assert_eq!(chain[0], "genesis_block_hash");
    }

    #[tokio::test]
    async fn test_block_height() {
        let config = ConsensusConfig::default();
        let layer = PolyTorusConsensusLayer::new(config).unwrap();
        
        let height = layer.get_block_height().await.unwrap();
        assert_eq!(height, 0); // Genesis height
    }

    #[tokio::test]
    async fn test_get_block_by_hash() {
        let config = ConsensusConfig::default();
        let layer = PolyTorusConsensusLayer::new(config).unwrap();
        
        let genesis_block = layer.get_block_by_hash(&"genesis_block_hash".to_string()).await.unwrap();
        assert!(genesis_block.is_some());
        assert_eq!(genesis_block.unwrap().number, 0);
    }

    #[tokio::test]
    async fn test_validator_set() {
        let config = ConsensusConfig::default();
        let layer = PolyTorusConsensusLayer::new_as_validator(
            config, 
            "validator_1".to_string()
        ).unwrap();
        
        let validators = layer.get_validator_set().await.unwrap();
        assert_eq!(validators.len(), 1);
        assert_eq!(validators[0].address, "validator_1");
    }

    #[tokio::test]
    async fn test_block_proposal_creation() {
        let config = ConsensusConfig {
            difficulty: 0, // No difficulty for testing
            ..ConsensusConfig::default()
        };
        let layer = PolyTorusConsensusLayer::new_as_validator(
            config, 
            "validator_1".to_string()
        ).unwrap();
        
        let transactions = vec![
            Transaction {
                hash: "tx1".to_string(),
                from: "alice".to_string(),
                to: Some("bob".to_string()),
                value: 100,
                gas_limit: 21000,
                gas_price: 1,
                data: vec![],
                nonce: 0,
                signature: vec![],
            }
        ];
        
        let block = layer.create_block_proposal(transactions).unwrap();
        assert_eq!(block.number, 1);
        assert_eq!(block.transactions.len(), 1);
        assert!(!block.hash.is_empty());
    }
}
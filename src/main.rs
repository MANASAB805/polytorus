//! PolyTorus - 4-Layer Modular Blockchain Platform
//!
//! This is the main orchestration layer that coordinates between:
//! 1. Execution Layer - Transaction processing and rollups
//! 2. Settlement Layer - Dispute resolution and finalization  
//! 3. Consensus Layer - Block ordering and validation
//! 4. Data Availability Layer - Data storage and distribution

use std::sync::Arc;
use std::path::Path;

use anyhow::Result;
use clap::{Arg, Command};
use log::{error, info};
use tokio::sync::RwLock;
use tokio::fs;

// Import layer implementations
use consensus::{ConsensusConfig, PolyTorusConsensusLayer};
use data_availability::{DataAvailabilityConfig, PolyTorusDataAvailabilityLayer};
use execution::{ExecutionConfig, PolyTorusExecutionLayer};
use settlement::{PolyTorusSettlementLayer, SettlementConfig};
use traits::*;

/// Main blockchain orchestrator
pub struct PolyTorusBlockchain {
    execution_layer: Arc<RwLock<PolyTorusExecutionLayer>>,
    settlement_layer: Arc<RwLock<PolyTorusSettlementLayer>>,
    consensus_layer: Arc<RwLock<PolyTorusConsensusLayer>>,
    data_availability_layer: Arc<RwLock<PolyTorusDataAvailabilityLayer>>,
}

impl PolyTorusBlockchain {
    /// Create new blockchain instance
    pub async fn new() -> Result<Self> {
        Self::new_with_configs(
            ExecutionConfig::default(),
            SettlementConfig::default(),
            ConsensusConfig::default(),
            DataAvailabilityConfig::default(),
        ).await
    }
    
    /// Create new blockchain instance with custom configurations
    pub async fn new_with_configs(
        execution_config: ExecutionConfig,
        settlement_config: SettlementConfig,
        consensus_config: ConsensusConfig,
        data_availability_config: DataAvailabilityConfig,
    ) -> Result<Self> {
        info!("Initializing PolyTorus 4-Layer Blockchain");

        info!("🔧 Initializing Execution Layer");
        let execution_layer = PolyTorusExecutionLayer::new(execution_config)?;

        info!("⚖️  Initializing Settlement Layer");
        let settlement_layer = PolyTorusSettlementLayer::new(settlement_config)?;

        info!("🤝 Initializing Consensus Layer");
        let consensus_layer = PolyTorusConsensusLayer::new(consensus_config)?;

        info!("📦 Initializing Data Availability Layer");
        let data_availability_layer = PolyTorusDataAvailabilityLayer::new(data_availability_config)?;

        Ok(Self {
            execution_layer: Arc::new(RwLock::new(execution_layer)),
            settlement_layer: Arc::new(RwLock::new(settlement_layer)),
            consensus_layer: Arc::new(RwLock::new(consensus_layer)),
            data_availability_layer: Arc::new(RwLock::new(data_availability_layer)),
        })
    }
    
    /// Get consensus layer for direct access (used by CLI)
    pub fn get_consensus_layer(&self) -> &Arc<RwLock<PolyTorusConsensusLayer>> {
        &self.consensus_layer
    }
    
    /// Load blockchain state from disk
    pub async fn load_from_disk() -> Result<Self> {
        let data_dir = Path::new("./blockchain_data");
        
        if !data_dir.exists() {
            info!("No existing blockchain data found, creating new instance");
            return Self::new().await;
        }
        
        info!("Loading blockchain state from disk");
        
        // For now, create new instance and load pending transactions
        let blockchain = Self::new().await?;
        
        // Load pending transactions if they exist
        let pending_tx_path = data_dir.join("pending_transactions.json");
        if pending_tx_path.exists() {
            let tx_data = fs::read_to_string(&pending_tx_path).await?;
            if !tx_data.trim().is_empty() {
                let transactions: Vec<Transaction> = serde_json::from_str(&tx_data)?;
                
                let consensus = blockchain.consensus_layer.write().await;
                for tx in transactions {
                    info!("Restoring pending transaction: {}", tx.hash);
                    consensus.add_pending_transaction(tx)?;
                }
                info!("Restored {} pending transactions", consensus.get_pending_transactions(1000).len());
            }
        }
        
        Ok(blockchain)
    }
    
    /// Save blockchain state to disk
    pub async fn save_to_disk(&self) -> Result<()> {
        let data_dir = Path::new("./blockchain_data");
        fs::create_dir_all(data_dir).await?;
        
        // Save pending transactions
        let consensus = self.consensus_layer.read().await;
        let pending_transactions = consensus.get_pending_transactions(1000);
        
        let pending_tx_path = data_dir.join("pending_transactions.json");
        let tx_json = serde_json::to_string_pretty(&pending_transactions)?;
        fs::write(&pending_tx_path, tx_json).await?;
        
        info!("Saved {} pending transactions to disk", pending_transactions.len());
        Ok(())
    }

    /// Start the blockchain node
    pub async fn start(&self) -> Result<()> {
        info!("🚀 Starting PolyTorus Blockchain Node");

        // In a real implementation, this would start background tasks
        // for each layer to communicate and coordinate
        
        info!("✅ All layers initialized successfully");
        info!("🌐 Blockchain node is ready to accept transactions");

        Ok(())
    }

    /// Process a transaction through all layers
    pub async fn process_transaction(&self, transaction: Transaction) -> Result<()> {
        info!("Processing transaction: {}", transaction.hash);

        // 1. Execute transaction
        let mut execution = self.execution_layer.write().await;
        let receipt = execution.execute_transaction(&transaction).await?;
        info!("✅ Transaction executed: gas_used={}", receipt.gas_used);

        // 2. Store transaction data for availability
        let tx_data = serde_json::to_vec(&transaction)?;
        let mut data_layer = self.data_availability_layer.write().await;
        let data_hash = data_layer.store_data(&tx_data).await?;
        info!("📦 Transaction data stored: {}", data_hash);

        // 3. Add to pending transactions for consensus
        let consensus = self.consensus_layer.read().await;
        consensus.add_pending_transaction(transaction)?;
        info!("🤝 Transaction added to consensus pool");

        // Save state to disk
        self.save_to_disk().await?;

        Ok(())
    }

    /// Create and mine a new block with PoW
    pub async fn create_block(&self) -> Result<()> {
        info!("🔨 Starting PoW mining process");

        // 1. Get pending transactions from consensus layer
        let consensus = self.consensus_layer.read().await;
        let pending_txs = consensus.get_pending_transactions(100);
        drop(consensus);

        if pending_txs.is_empty() {
            info!("No pending transactions, skipping block creation");
            return Ok(());
        }

        info!("Mining block with {} transactions", pending_txs.len());

        // 2. Execute transaction batch
        let mut execution = self.execution_layer.write().await;
        let batch = execution.execute_batch(pending_txs.clone()).await?;
        drop(execution);

        info!("✅ Executed batch: gas_used={}", batch.results.iter().map(|r| r.gas_used).sum::<u64>());

        // 3. Settle the batch
        let mut settlement = self.settlement_layer.write().await;
        let settlement_result = settlement.settle_batch(&batch).await?;
        drop(settlement);

        info!("⚖️  Batch settlement initiated: {}", settlement_result.settlement_root);

        // 4. Mine new block with PoW
        let mut consensus = self.consensus_layer.write().await;
        let block = consensus.mine_block(pending_txs).await?;
        consensus.propose_block(block.clone()).await?;
        drop(consensus);

        info!("🤝 Block proposed: {} (height: {})", block.hash, block.number);

        // Save state to disk after mining
        self.save_to_disk().await?;

        Ok(())
    }

    /// Get blockchain status
    pub async fn get_status(&self) -> Result<BlockchainStatus> {
        let consensus = self.consensus_layer.read().await;
        let height = consensus.get_block_height().await?;
        let chain = consensus.get_canonical_chain().await?;
        drop(consensus);

        let settlement = self.settlement_layer.read().await;
        let settlement_root = settlement.get_settlement_root().await?;
        drop(settlement);

        let execution = self.execution_layer.read().await;
        let state_root = execution.get_state_root().await?;
        drop(execution);

        Ok(BlockchainStatus {
            block_height: height,
            chain_length: chain.len(),
            state_root,
            settlement_root,
        })
    }
}

/// Blockchain status information
#[derive(Debug)]
pub struct BlockchainStatus {
    pub block_height: u64,
    pub chain_length: usize,
    pub state_root: Hash,
    pub settlement_root: Hash,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let matches = Command::new("polytorus")
        .version("0.1.0")
        .about("PolyTorus - 4-Layer Modular Blockchain Platform")
        .subcommand(
            Command::new("start")
                .about("Start the blockchain node")
        )
        .subcommand(
            Command::new("status")
                .about("Get blockchain status")
        )
        .subcommand(
            Command::new("send")
                .about("Send a transaction")
                .arg(Arg::new("from").required(true))
                .arg(Arg::new("to").required(true))
                .arg(Arg::new("amount").required(true))
        )
        .subcommand(
            Command::new("mine")
                .about("Mine a new block")
        )
        .subcommand(
            Command::new("difficulty")
                .about("Get or set mining difficulty")
                .arg(Arg::new("value").help("New difficulty value (optional)"))
        )
        .subcommand(
            Command::new("pending")
                .about("Show pending transactions")
        )
        .get_matches();

    let blockchain = PolyTorusBlockchain::load_from_disk().await?;

    match matches.subcommand() {
        Some(("start", _)) => {
            blockchain.start().await?;
            
            // Keep the node running
            info!("Press Ctrl+C to stop the node");
            tokio::signal::ctrl_c().await?;
            info!("Shutting down blockchain node");
        }
        
        Some(("status", _)) => {
            let status = blockchain.get_status().await?;
            println!("Blockchain Status:");
            println!("  Block Height: {}", status.block_height);
            println!("  Chain Length: {}", status.chain_length);
            println!("  State Root: {}", status.state_root);
            println!("  Settlement Root: {}", status.settlement_root);
        }
        
        Some(("send", sub_matches)) => {
            let from = sub_matches.get_one::<String>("from").unwrap();
            let to = sub_matches.get_one::<String>("to").unwrap();
            let amount: u64 = sub_matches.get_one::<String>("amount").unwrap().parse()?;
            
            let transaction = Transaction {
                hash: format!("tx_{}", uuid::Uuid::new_v4()),
                from: from.clone(),
                to: Some(to.clone()),
                value: amount,
                gas_limit: 21000,
                gas_price: 1,
                data: vec![],
                nonce: 0,
                signature: vec![],
            };
            
            blockchain.process_transaction(transaction).await?;
            println!("Transaction sent successfully");
        }
        
        Some(("mine", _)) => {
            blockchain.create_block().await?;
            println!("Block mined successfully");
        }
        
        Some(("difficulty", sub_matches)) => {
            if let Some(value_str) = sub_matches.get_one::<String>("value") {
                let difficulty: usize = value_str.parse()?;
                let mut consensus = blockchain.get_consensus_layer().write().await;
                consensus.set_difficulty(difficulty).await?;
                println!("Mining difficulty set to {}", difficulty);
            } else {
                let consensus = blockchain.get_consensus_layer().read().await;
                let current_difficulty = consensus.get_difficulty().await?;
                println!("Current mining difficulty: {}", current_difficulty);
            }
        }
        
        Some(("pending", _)) => {
            let consensus = blockchain.get_consensus_layer().read().await;
            let pending_transactions = consensus.get_pending_transactions(100);
            
            if pending_transactions.is_empty() {
                println!("No pending transactions");
            } else {
                println!("Pending transactions ({}):", pending_transactions.len());
                for (i, tx) in pending_transactions.iter().enumerate() {
                    println!("  {}. {} -> {} ({})", 
                             i + 1, 
                             tx.from, 
                             tx.to.as_ref().unwrap_or(&"N/A".to_string()), 
                             tx.value);
                }
            }
        }
        
        _ => {
            error!("No subcommand provided");
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_blockchain_creation() {
        let blockchain = PolyTorusBlockchain::new().await;
        assert!(blockchain.is_ok());
    }

    #[tokio::test]
    async fn test_blockchain_status() {
        let blockchain = PolyTorusBlockchain::new().await.unwrap();
        let status = blockchain.get_status().await.unwrap();
        
        assert_eq!(status.block_height, 0); // Genesis
        assert_eq!(status.chain_length, 1); // Genesis block only
    }

    #[tokio::test]
    async fn test_transaction_processing() {
        let blockchain = PolyTorusBlockchain::new().await.unwrap();
        
        let transaction = Transaction {
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
        
        let result = blockchain.process_transaction(transaction).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_block_creation() {
        // Use test-friendly configuration with no mining difficulty
        let consensus_config = ConsensusConfig {
            difficulty: 0, // No mining difficulty for fast tests
            ..ConsensusConfig::default()
        };
        
        let blockchain = PolyTorusBlockchain::new_with_configs(
            ExecutionConfig::default(),
            SettlementConfig::default(),
            consensus_config,
            DataAvailabilityConfig::default(),
        ).await.unwrap();
        
        // Add some transactions first
        let transaction = Transaction {
            hash: "test_tx_1".to_string(),
            from: "alice".to_string(),
            to: Some("bob".to_string()),
            value: 100,
            gas_limit: 21000,
            gas_price: 1,
            data: vec![],
            nonce: 0,
            signature: vec![],
        };
        
        blockchain.process_transaction(transaction).await.unwrap();
        
        // Now create a block
        let result = blockchain.create_block().await;
        if let Err(e) = &result {
            println!("Block creation failed: {}", e);
        }
        assert!(result.is_ok());
    }
}
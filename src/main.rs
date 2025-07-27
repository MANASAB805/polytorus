use anyhow::Result;
use clap::{Arg, Command};
use log::{info, error};
use std::env;

use execution::execution_engine::{PolyTorusUtxoExecutionLayer, UtxoExecutionConfig};
use consensus::consensus_engine::{PolyTorusUtxoConsensusLayer, UtxoConsensusConfig};
use p2p_network::{WebRTCP2PNetwork, P2PConfig};
use traits::{
    UtxoExecutionLayer, UtxoConsensusLayer, UtxoTransaction, UtxoId,
    TxInput, TxOutput
};

pub struct PolyTorusBlockchain {
    execution_layer: PolyTorusUtxoExecutionLayer,
    consensus_layer: PolyTorusUtxoConsensusLayer,
    p2p_network: WebRTCP2PNetwork,
}

impl PolyTorusBlockchain {
    pub fn new() -> Result<Self> {
        Self::new_with_p2p_config(None)
    }

    pub fn new_with_p2p_config(p2p_config: Option<P2PConfig>) -> Result<Self> {
        let execution_config = UtxoExecutionConfig::default();
        
        // テスト用設定: PoW難易度を0に設定
        let consensus_config = UtxoConsensusConfig {
            difficulty: 0, // 即座にマイニング完了
            slot_time: 100, // 100ms slot time for faster testing
            ..UtxoConsensusConfig::default()
        };
        
        info!("Using test configuration: difficulty={}, slot_time={}ms", 
              consensus_config.difficulty, consensus_config.slot_time);

        let execution_layer = PolyTorusUtxoExecutionLayer::new(execution_config)?;
        let consensus_layer = PolyTorusUtxoConsensusLayer::new_as_validator(
            consensus_config,
            "main_validator".to_string()
        )?;

        // Initialize P2P network with provided or default config
        let p2p_config = p2p_config.unwrap_or_else(|| Self::p2p_config_from_env());
        let p2p_network = WebRTCP2PNetwork::new(p2p_config)?;

        Ok(Self {
            execution_layer,
            consensus_layer,
            p2p_network,
        })
    }

    /// Create P2P configuration from environment variables
    fn p2p_config_from_env() -> P2PConfig {
        let node_id = env::var("NODE_ID").unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
        let listen_port = env::var("LISTEN_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()
            .unwrap_or(8080);
        
        let bootstrap_peers = env::var("BOOTSTRAP_PEERS")
            .map(|peers| peers.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_else(|_| Vec::new());
            
        let debug_mode = env::var("DEBUG_MODE").unwrap_or_else(|_| "false".to_string()) == "true";

        P2PConfig {
            node_id,
            listen_addr: format!("0.0.0.0:{}", listen_port).parse().unwrap(),
            stun_servers: vec![
                "stun:stun.l.google.com:19302".to_string(),
                "stun:stun1.l.google.com:19302".to_string(),
            ],
            bootstrap_peers,
            max_peers: 50,
            connection_timeout: 30,
            keep_alive_interval: 30,
            debug_mode,
        }
    }

    /// Get P2P network reference
    pub fn p2p_network(&self) -> &WebRTCP2PNetwork {
        &self.p2p_network
    }

    /// Start P2P network
    pub async fn start_p2p_network(&self) -> Result<()> {
        self.p2p_network.start().await
    }

    pub async fn initialize_genesis(&mut self) -> Result<UtxoId> {
        info!("Starting genesis UTXO initialization");
        
        let genesis_utxo_id = UtxoId {
            tx_hash: "genesis_tx".to_string(),
            output_index: 0,
        };

        let genesis_utxo = traits::Utxo {
            id: genesis_utxo_id.clone(),
            value: 10_000_000, // 10M units initial supply
            script: vec![], // Empty script = "always true"
            datum: Some(b"Genesis UTXO for PolyTorus".to_vec()),
            datum_hash: Some("genesis_datum_hash".to_string()),
        };

        info!("Calling initialize_genesis_utxo_set");
        self.execution_layer.initialize_genesis_utxo_set(vec![(genesis_utxo_id.clone(), genesis_utxo)])?;
        info!("Genesis UTXO created: {:?}", genesis_utxo_id);
        info!("Genesis initialization completed successfully");
        Ok(genesis_utxo_id)
    }

    pub async fn send_transaction(
        &mut self,
        from: &str,
        to: &str,
        amount: u64,
    ) -> Result<String> {
        // Use the genesis UTXO as the source for all transactions (simplified demo)
        let from_utxo_id = UtxoId {
            tx_hash: "genesis_tx".to_string(),
            output_index: 0,
        };

        let tx_hash = format!("tx_{}_{}_{}_{}", from, to, amount, uuid::Uuid::new_v4());
        let fee = 1000; // Fixed fee
        let genesis_value = 10_000_000; // Match the genesis UTXO value
        
        if amount + fee > genesis_value {
            return Err(anyhow::anyhow!("Insufficient funds: need {} but genesis UTXO has {}", amount + fee, genesis_value));
        }
        
        let change = genesis_value - amount - fee;

        let transaction = UtxoTransaction {
            hash: tx_hash.clone(),
            inputs: vec![TxInput {
                utxo_id: from_utxo_id,
                redeemer: b"signature_redeemer".to_vec(),
                signature: format!("sig_{}", from).into_bytes(),
            }],
            outputs: vec![
                TxOutput {
                    value: amount,
                    script: vec![],
                    datum: Some(format!("Payment to {}", to).into_bytes()),
                    datum_hash: Some(format!("datum_hash_{}", to)),
                },
                TxOutput {
                    value: change,
                    script: vec![],
                    datum: Some(format!("Change for {}", from).into_bytes()),
                    datum_hash: Some(format!("change_datum_hash_{}", from)),
                },
            ],
            fee,
            validity_range: Some((0, 1000)),
            script_witness: vec![b"witness_data".to_vec()],
            auxiliary_data: Some(format!("Transfer from {} to {}", from, to).into_bytes()),
        };

        info!("Executing transaction: {}", tx_hash);
        
        match self.execution_layer.execute_utxo_transaction(&transaction).await {
            Ok(receipt) => {
                info!("Transaction executed successfully: {}", receipt.success);
                
                // Mine a block with this transaction
                info!("Starting block mining for transaction: {}", tx_hash);
                let block = self.consensus_layer.mine_utxo_block(vec![transaction]).await?;
                info!("Block mined successfully: {} (slot {})", block.hash, block.slot);
                
                // Validate and add block
                let is_valid = self.consensus_layer.validate_utxo_block(&block).await?;
                if is_valid {
                    self.consensus_layer.add_utxo_block(block).await?;
                    info!("Block added to chain");
                } else {
                    error!("Block validation failed");
                }
                
                Ok(tx_hash)
            }
            Err(e) => {
                error!("Transaction execution failed: {}", e);
                Err(e)
            }
        }
    }

    pub async fn get_status(&self) -> Result<()> {
        let chain_height = self.consensus_layer.get_block_height().await?;
        let current_slot = self.consensus_layer.get_current_slot().await?;
        let canonical_chain = self.consensus_layer.get_canonical_chain().await?;
        let utxo_set_hash = self.execution_layer.get_utxo_set_hash().await?;
        let total_supply = self.execution_layer.get_total_supply().await?;

        println!("PolyTorus Blockchain Status:");
        println!("============================");
        println!("Chain Height: {}", chain_height);
        println!("Current Slot: {}", current_slot);
        println!("Chain Length: {} blocks", canonical_chain.len());
        println!("UTXO Set Hash: {}", utxo_set_hash);
        println!("Total Supply: {} units", total_supply);

        Ok(())
    }
}

fn main() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async_main())
}

async fn async_main() -> Result<()> {
    // Docker output debugging
    println!("🐳 PolyTorus starting in Docker container...");
    eprintln!("🐳 PolyTorus stderr test...");
    
    // Initialize logging
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    env_logger::init();
    
    println!("🐳 Environment initialized, parsing commands...");

    let matches = Command::new("polytorus")
        .version("0.1.0")
        .author("quantumshiro")
        .about("PolyTorus - 4-Layer Modular Blockchain Platform")
        .subcommand(
            Command::new("start")
                .about("Initialize and start the blockchain node")
        )
        .subcommand(
            Command::new("start-p2p")
                .about("Start the blockchain node with P2P networking")
                .arg(Arg::new("node-id")
                    .long("node-id")
                    .value_name("NODE_ID")
                    .help("Node identifier"))
                .arg(Arg::new("listen-port")
                    .long("listen-port")
                    .value_name("PORT")
                    .help("Port to listen on for P2P connections")
                    .default_value("8080"))
                .arg(Arg::new("bootstrap-peers")
                    .long("bootstrap-peers")
                    .value_name("PEERS")
                    .help("Comma-separated list of bootstrap peer addresses"))
        )
        .subcommand(
            Command::new("send")
                .about("Send a transaction")
                .arg(Arg::new("from")
                    .long("from")
                    .value_name("FROM")
                    .help("Sender address")
                    .required(true))
                .arg(Arg::new("to")
                    .long("to")
                    .value_name("TO")
                    .help("Recipient address")
                    .required(true))
                .arg(Arg::new("amount")
                    .long("amount")
                    .value_name("AMOUNT")
                    .help("Amount to send")
                    .required(true))
        )
        .subcommand(
            Command::new("status")
                .about("Show blockchain status")
        )
        .get_matches();

    match matches.subcommand() {
        Some(("start", _)) => {
            info!("Starting PolyTorus blockchain node...");
            let mut blockchain = PolyTorusBlockchain::new()?;
            let _genesis_id = blockchain.initialize_genesis().await?;
            info!("PolyTorus node started successfully");
            println!("✅ PolyTorus blockchain node started successfully");
            println!("Genesis UTXO initialized with 10,000,000 units");
            
            info!("Start command completed successfully - exiting");
            return Ok(());
        }
        Some(("start-p2p", sub_matches)) => {
            info!("Starting PolyTorus blockchain node with P2P networking...");
            
            // Build P2P configuration from arguments
            let node_id = sub_matches.get_one::<String>("node-id")
                .map(|s| s.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            
            let listen_port = sub_matches.get_one::<String>("listen-port")
                .unwrap()
                .parse::<u16>()
                .unwrap_or(8080);
            
            let bootstrap_peers = sub_matches.get_one::<String>("bootstrap-peers")
                .map(|peers| peers.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_else(|| Vec::new());
            
            let p2p_config = P2PConfig {
                node_id: node_id.clone(),
                listen_addr: format!("0.0.0.0:{}", listen_port).parse().unwrap(),
                stun_servers: vec![
                    "stun:stun.l.google.com:19302".to_string(),
                    "stun:stun1.l.google.com:19302".to_string(),
                ],
                bootstrap_peers: bootstrap_peers.clone(),
                max_peers: 50,
                connection_timeout: 30,
                keep_alive_interval: 30,
                debug_mode: true,
            };
            
            let mut blockchain = PolyTorusBlockchain::new_with_p2p_config(Some(p2p_config))?;
            let _genesis_id = blockchain.initialize_genesis().await?;
            
            println!("🚀 Starting PolyTorus P2P node: {}", node_id);
            println!("📡 Listening on port: {}", listen_port);
            println!("🔗 Bootstrap peers: {:?}", bootstrap_peers);
            
            // Start P2P network
            info!("Starting P2P network...");
            blockchain.start_p2p_network().await?;
        }
        Some(("send", sub_matches)) => {
            let from = sub_matches.get_one::<String>("from").unwrap();
            let to = sub_matches.get_one::<String>("to").unwrap();
            let amount: u64 = sub_matches.get_one::<String>("amount").unwrap().parse()?;

            info!("Sending transaction: {} -> {} ({})", from, to, amount);
            let mut blockchain = PolyTorusBlockchain::new()?;
            let _genesis_id = blockchain.initialize_genesis().await?;
            
            match blockchain.send_transaction(from, to, amount).await {
                Ok(tx_hash) => {
                    println!("✅ Transaction sent successfully");
                    println!("Transaction Hash: {}", tx_hash);
                    println!("From: {}", from);
                    println!("To: {}", to);
                    println!("Amount: {} units", amount);
                }
                Err(e) => {
                    error!("Failed to send transaction: {}", e);
                    println!("❌ Transaction failed: {}", e);
                }
            }
        }
        Some(("status", _)) => {
            println!("🐳 Docker: Executing status command...");
            let blockchain = PolyTorusBlockchain::new()?;
            blockchain.get_status().await?;
            println!("🐳 Docker: Status command completed.");
        }
        _ => {
            println!("PolyTorus - 4-Layer Modular Blockchain Platform");
            println!("Usage: polytorus <COMMAND>");
            println!();
            println!("Commands:");
            println!("  start     Initialize and start the blockchain node");
            println!("  send      Send a transaction");
            println!("  status    Show blockchain status");
            println!();
            println!("Use 'polytorus <COMMAND> --help' for more information on a command");
        }
    }

    Ok(())
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_blockchain_initialization() -> Result<()> {
        let mut blockchain = PolyTorusBlockchain::new()?;
        let genesis_id = blockchain.initialize_genesis().await?;
        assert_eq!(genesis_id.tx_hash, "genesis_tx");
        assert_eq!(genesis_id.output_index, 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_transaction_processing() -> Result<()> {
        let mut blockchain = PolyTorusBlockchain::new()?;
        let _genesis_id = blockchain.initialize_genesis().await?;
        
        let tx_hash = blockchain.send_transaction("alice", "bob", 100_000).await?;
        assert!(!tx_hash.is_empty());
        assert!(tx_hash.starts_with("tx_alice_bob_100000_"));
        Ok(())
    }

    #[tokio::test] 
    async fn test_blockchain_status() -> Result<()> {
        let blockchain = PolyTorusBlockchain::new()?;
        // This should not panic
        blockchain.get_status().await?;
        Ok(())
    }
}
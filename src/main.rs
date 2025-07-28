use anyhow::Result;
use clap::{Arg, Command};
use log::{error, info};
use std::collections::HashMap;
use std::env;

use consensus::consensus_engine::{PolyTorusUtxoConsensusLayer, UtxoConsensusConfig};
use execution::execution_engine::{PolyTorusUtxoExecutionLayer, UtxoExecutionConfig};
use execution::script_engine::ScriptType;
use p2p_network::{P2PConfig, WebRTCP2PNetwork};
use traits::{
    ExecutionLayer, Hash, ScriptTransactionType, Transaction, TxInput, TxOutput,
    UtxoConsensusLayer, UtxoExecutionLayer, UtxoId, UtxoTransaction,
};
use wallet::{HdWallet, KeyPair, KeyType, Wallet};

pub struct PolyTorusBlockchain {
    execution_layer: PolyTorusUtxoExecutionLayer,
    consensus_layer: PolyTorusUtxoConsensusLayer,
    p2p_network: WebRTCP2PNetwork,
    wallet: HdWallet,
    user_wallets: HashMap<String, (KeyPair, Wallet)>,
}

impl PolyTorusBlockchain {
    pub fn new() -> Result<Self> {
        Self::new_with_p2p_config(None)
    }

    pub fn new_with_p2p_config(p2p_config: Option<P2PConfig>) -> Result<Self> {
        let execution_config = UtxoExecutionConfig::default();

        // テスト用設定: PoW難易度を0に設定
        let consensus_config = UtxoConsensusConfig {
            difficulty: 0,  // 即座にマイニング完了
            slot_time: 100, // 100ms slot time for faster testing
            ..UtxoConsensusConfig::default()
        };

        info!(
            "Using test configuration: difficulty={}, slot_time={}ms",
            consensus_config.difficulty, consensus_config.slot_time
        );

        let execution_layer = PolyTorusUtxoExecutionLayer::new(execution_config)?;
        let consensus_layer = PolyTorusUtxoConsensusLayer::new_as_validator(
            consensus_config,
            "main_validator".to_string(),
        )?;

        // Initialize P2P network with provided or default config
        let p2p_config = p2p_config.unwrap_or_else(|| Self::p2p_config_from_env());
        let p2p_network = WebRTCP2PNetwork::new(p2p_config)?;

        // Initialize HD wallet
        let wallet = HdWallet::new(KeyType::Ed25519)
            .map_err(|e| anyhow::anyhow!("Failed to create HD wallet: {:?}", e))?;
        let mnemonic = wallet.get_mnemonic().phrase();

        info!("Initialized HD wallet with mnemonic: {}", mnemonic);

        Ok(Self {
            execution_layer,
            consensus_layer,
            p2p_network,
            wallet,
            user_wallets: HashMap::new(),
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
            script: vec![],    // Empty script = "always true"
            datum: Some(b"Genesis UTXO for PolyTorus".to_vec()),
            datum_hash: Some("genesis_datum_hash".to_string()),
        };

        info!("Calling initialize_genesis_utxo_set");
        self.execution_layer
            .initialize_genesis_utxo_set(vec![(genesis_utxo_id.clone(), genesis_utxo)])?;
        info!("Genesis UTXO created: {:?}", genesis_utxo_id);
        info!("Genesis initialization completed successfully");
        Ok(genesis_utxo_id)
    }

    fn get_or_create_wallet(&mut self, user: &str) -> Result<&(KeyPair, Wallet)> {
        if !self.user_wallets.contains_key(user) {
            let index = self.user_wallets.len() as u32;
            let keypair = self
                .wallet
                .derive_key(index)
                .map_err(|e| anyhow::anyhow!("Failed to derive keypair for {}: {:?}", user, e))?;

            // Use a fixed coin type for now
            let user_wallet = self
                .wallet
                .derive_receiving_wallet(0, 0, index, KeyType::Ed25519)
                .map_err(|e| anyhow::anyhow!("Failed to derive wallet for {}: {:?}", user, e))?;

            self.user_wallets
                .insert(user.to_string(), (keypair, user_wallet));
            info!("Created new wallet for user: {} (index: {})", user, index);
        }
        Ok(self.user_wallets.get(user).unwrap())
    }

    fn get_address(&mut self, user: &str) -> Result<String> {
        self.get_or_create_wallet(user)?;
        let (_keypair, wallet) = self.user_wallets.get_mut(user).unwrap();
        let address = wallet
            .default_address()
            .map_err(|e| anyhow::anyhow!("Failed to get address for {}: {:?}", user, e))?;
        Ok(address.to_string())
    }

    pub async fn send_transaction(&mut self, from: &str, to: &str, amount: u64) -> Result<String> {
        // Use the genesis UTXO as the source for all transactions (simplified demo)
        let from_utxo_id = UtxoId {
            tx_hash: "genesis_tx".to_string(),
            output_index: 0,
        };

        let tx_hash = format!("tx_{}_{}_{}_{}", from, to, amount, uuid::Uuid::new_v4());
        let fee = 1000; // Fixed fee
        let genesis_value = 10_000_000; // Match the genesis UTXO value

        if amount + fee > genesis_value {
            return Err(anyhow::anyhow!(
                "Insufficient funds: need {} but genesis UTXO has {}",
                amount + fee,
                genesis_value
            ));
        }

        let change = genesis_value - amount - fee;

        // Get real addresses for from and to
        let from_address = self.get_address(from)?;
        let to_address = self.get_address(to)?;

        // Get wallet for signing
        self.get_or_create_wallet(from)?;
        let (_keypair, from_wallet) = self.user_wallets.get_mut(from).unwrap();

        // Create message to sign (transaction hash)
        let message = tx_hash.as_bytes();
        let signature = from_wallet
            .sign(message)
            .map_err(|e| anyhow::anyhow!("Failed to sign transaction: {:?}", e))?;

        let transaction = UtxoTransaction {
            hash: tx_hash.clone(),
            inputs: vec![TxInput {
                utxo_id: from_utxo_id,
                redeemer: format!("address:{}", from_address).into_bytes(),
                signature: signature.as_bytes().to_vec(),
            }],
            outputs: vec![
                TxOutput {
                    value: amount,
                    script: vec![],
                    datum: Some(format!("Payment to {} ({})", to, to_address).into_bytes()),
                    datum_hash: Some(format!("datum_hash_{}", to_address)),
                },
                TxOutput {
                    value: change,
                    script: vec![],
                    datum: Some(format!("Change for {} ({})", from, from_address).into_bytes()),
                    datum_hash: Some(format!("change_datum_hash_{}", from_address)),
                },
            ],
            fee,
            validity_range: Some((0, 1000)),
            script_witness: vec![format!("wallet_signature_{}", from_address).into_bytes()],
            auxiliary_data: Some(
                format!(
                    "Transfer from {} ({}) to {} ({})",
                    from, from_address, to, to_address
                )
                .into_bytes(),
            ),
        };

        info!("Transaction created with real wallet signatures:");
        info!("  From: {} ({})", from, from_address);
        info!("  To: {} ({})", to, to_address);
        info!("  Signature length: {}", signature.as_bytes().len());

        info!("Executing transaction: {}", tx_hash);

        match self
            .execution_layer
            .execute_utxo_transaction(&transaction)
            .await
        {
            Ok(receipt) => {
                info!("Transaction executed successfully: {}", receipt.success);

                // Mine a block with this transaction
                info!("Starting block mining for transaction: {}", tx_hash);
                let block = self
                    .consensus_layer
                    .mine_utxo_block(vec![transaction])
                    .await?;
                info!(
                    "Block mined successfully: {} (slot {})",
                    block.hash, block.slot
                );

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

    pub async fn deploy_contract(
        &mut self,
        owner: &str,
        wasm_bytes: Vec<u8>,
        name: Option<&str>,
    ) -> Result<Hash> {
        info!("Deploying WASM contract for owner: {}", owner);

        let tx_hash = format!("tx_deploy_contract_{}_{}", owner, uuid::Uuid::new_v4());

        // Create deployment transaction
        let transaction = Transaction {
            hash: tx_hash.clone(),
            from: owner.to_string(),
            to: None, // No target for deployment
            value: 0,
            gas_limit: 200000,
            gas_price: 1,
            data: vec![],
            nonce: 0,
            signature: vec![],
            script_type: Some(ScriptTransactionType::Deploy {
                script_data: wasm_bytes,
                init_params: vec![],
            }),
        };

        // Sign transaction
        self.get_or_create_wallet(owner)?;
        let (_keypair, from_wallet) = self.user_wallets.get_mut(owner).unwrap();
        let signature = from_wallet
            .sign(tx_hash.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to sign deployment: {:?}", e))?;

        let mut signed_transaction = transaction;
        signed_transaction.signature = signature.as_bytes().to_vec();

        // Convert to UTXO transaction for execution
        let utxo_tx = self.convert_to_utxo_transaction(&signed_transaction)?;

        // Execute deployment
        match self
            .execution_layer
            .execute_utxo_transaction(&utxo_tx)
            .await
        {
            Ok(receipt) => {
                info!("Contract deployed successfully: {}", receipt.success);

                // Mine a block with the deployment transaction
                let block = self.consensus_layer.mine_utxo_block(vec![utxo_tx]).await?;
                info!("Block mined: {} (slot {})", block.hash, block.slot);

                // Validate and add block
                let is_valid = self.consensus_layer.validate_utxo_block(&block).await?;
                if is_valid {
                    self.consensus_layer.add_utxo_block(block).await?;
                    info!("Deployment block added to chain");
                }

                Ok(tx_hash)
            }
            Err(e) => {
                error!("Contract deployment failed: {}", e);
                Err(e)
            }
        }
    }

    pub async fn call_contract(
        &mut self,
        from: &str,
        contract_hash: &str,
        method: &str,
        params: Vec<u8>,
    ) -> Result<String> {
        let tx_hash = format!("tx_contract_call_{}_{}", from, uuid::Uuid::new_v4());

        info!("Creating contract call transaction: {}", tx_hash);

        // Create a transaction with script call
        let transaction = Transaction {
            hash: tx_hash.clone(),
            from: from.to_string(),
            to: Some(contract_hash.to_string()),
            value: 0, // No value transfer for now
            gas_limit: 100000,
            gas_price: 1,
            nonce: 0,
            data: params.clone(),
            signature: vec![], // Will be signed below
            script_type: Some(ScriptTransactionType::Call {
                script_hash: contract_hash.to_string(),
                method: method.to_string(),
                params,
            }),
        };

        // Sign transaction with wallet
        self.get_or_create_wallet(from)?;
        let (_keypair, from_wallet) = self.user_wallets.get_mut(from).unwrap();
        let signature = from_wallet
            .sign(tx_hash.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to sign contract call: {:?}", e))?;

        let mut signed_transaction = transaction;
        signed_transaction.signature = signature.as_bytes().to_vec();

        info!("Executing contract call transaction");

        // Convert to UTXO transaction and execute
        let utxo_tx = self.convert_to_utxo_transaction(&signed_transaction)?;

        match self
            .execution_layer
            .execute_utxo_transaction(&utxo_tx)
            .await
        {
            Ok(receipt) => {
                info!("Contract call executed successfully: {}", receipt.success);

                // Mine a block with this transaction
                info!("Mining block for contract call");
                let block = self.consensus_layer.mine_utxo_block(vec![utxo_tx]).await?;
                info!("Block mined: {} (slot {})", block.hash, block.slot);

                // Validate and add block
                let is_valid = self.consensus_layer.validate_utxo_block(&block).await?;
                if is_valid {
                    self.consensus_layer.add_utxo_block(block).await?;
                    info!("Block added to chain");
                }

                Ok(tx_hash)
            }
            Err(e) => {
                error!("Contract call failed: {}", e);
                Err(e)
            }
        }
    }

    fn convert_to_utxo_transaction(&self, tx: &Transaction) -> Result<UtxoTransaction> {
        // Simple conversion for contract calls
        Ok(UtxoTransaction {
            hash: tx.hash.clone(),
            inputs: vec![TxInput {
                utxo_id: UtxoId {
                    tx_hash: "genesis_tx".to_string(),
                    output_index: 0,
                },
                redeemer: tx.data.clone(),
                signature: tx.signature.clone(),
            }],
            outputs: vec![],
            fee: 1000, // Fixed fee for conversion
            validity_range: Some((0, 1000)),
            script_witness: vec![],
            auxiliary_data: tx
                .script_type
                .as_ref()
                .map(|st| format!("Contract call: {:?}", st).into_bytes()),
        })
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
        .subcommand(Command::new("start").about("Initialize and start the blockchain node"))
        .subcommand(
            Command::new("start-p2p")
                .about("Start the blockchain node with P2P networking")
                .arg(
                    Arg::new("node-id")
                        .long("node-id")
                        .value_name("NODE_ID")
                        .help("Node identifier"),
                )
                .arg(
                    Arg::new("listen-port")
                        .long("listen-port")
                        .value_name("PORT")
                        .help("Port to listen on for P2P connections")
                        .default_value("8080"),
                )
                .arg(
                    Arg::new("bootstrap-peers")
                        .long("bootstrap-peers")
                        .value_name("PEERS")
                        .help("Comma-separated list of bootstrap peer addresses"),
                ),
        )
        .subcommand(
            Command::new("send")
                .about("Send a transaction")
                .arg(
                    Arg::new("from")
                        .long("from")
                        .value_name("FROM")
                        .help("Sender address")
                        .required(true),
                )
                .arg(
                    Arg::new("to")
                        .long("to")
                        .value_name("TO")
                        .help("Recipient address")
                        .required(true),
                )
                .arg(
                    Arg::new("amount")
                        .long("amount")
                        .value_name("AMOUNT")
                        .help("Amount to send")
                        .required(true),
                ),
        )
        .subcommand(Command::new("status").about("Show blockchain status"))
        .subcommand(
            Command::new("deploy-contract")
                .about("Deploy a smart contract")
                .arg(
                    Arg::new("wasm-file")
                        .long("wasm-file")
                        .value_name("FILE")
                        .help("Path to the compiled WASM contract file")
                        .required(true),
                )
                .arg(
                    Arg::new("owner")
                        .long("owner")
                        .value_name("OWNER")
                        .help("Contract owner address")
                        .required(true),
                )
                .arg(
                    Arg::new("name")
                        .long("name")
                        .value_name("NAME")
                        .help("Contract name/description"),
                ),
        )
        .subcommand(
            Command::new("call-contract")
                .about("Call a smart contract method")
                .arg(
                    Arg::new("contract")
                        .long("contract")
                        .value_name("HASH")
                        .help("Contract hash/address")
                        .required(true),
                )
                .arg(
                    Arg::new("method")
                        .long("method")
                        .value_name("METHOD")
                        .help("Method to call")
                        .required(true),
                )
                .arg(
                    Arg::new("params")
                        .long("params")
                        .value_name("PARAMS")
                        .help("Method parameters (JSON format)"),
                )
                .arg(
                    Arg::new("from")
                        .long("from")
                        .value_name("FROM")
                        .help("Caller address")
                        .required(true),
                ),
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
            let node_id = sub_matches
                .get_one::<String>("node-id")
                .map(|s| s.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            let listen_port = sub_matches
                .get_one::<String>("listen-port")
                .unwrap()
                .parse::<u16>()
                .unwrap_or(8080);

            let bootstrap_peers = sub_matches
                .get_one::<String>("bootstrap-peers")
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
        Some(("deploy-contract", sub_matches)) => {
            let wasm_file = sub_matches.get_one::<String>("wasm-file").unwrap();
            let owner = sub_matches.get_one::<String>("owner").unwrap();
            let name = sub_matches.get_one::<String>("name").map(|s| s.as_str());

            info!("Deploying contract from: {}", wasm_file);

            // Read WASM file
            let wasm_bytes = std::fs::read(wasm_file)
                .map_err(|e| anyhow::anyhow!("Failed to read WASM file: {}", e))?;

            info!("WASM file size: {} bytes", wasm_bytes.len());

            let mut blockchain = PolyTorusBlockchain::new()?;
            let _genesis_id = blockchain.initialize_genesis().await?;

            match blockchain.deploy_contract(owner, wasm_bytes, name).await {
                Ok(script_hash) => {
                    println!("✅ Contract deployed successfully");
                    println!("Contract Hash: {}", script_hash);
                    println!("Owner: {}", owner);
                    if let Some(n) = name {
                        println!("Name: {}", n);
                    }
                }
                Err(e) => {
                    error!("Failed to deploy contract: {}", e);
                    println!("❌ Contract deployment failed: {}", e);
                }
            }
        }
        Some(("call-contract", sub_matches)) => {
            let contract = sub_matches.get_one::<String>("contract").unwrap();
            let method = sub_matches.get_one::<String>("method").unwrap();
            let params = sub_matches.get_one::<String>("params").map(|s| s.as_str());
            let from = sub_matches.get_one::<String>("from").unwrap();

            info!("Calling contract method: {}::{}", contract, method);

            let mut blockchain = PolyTorusBlockchain::new()?;
            let _genesis_id = blockchain.initialize_genesis().await?;

            let params_bytes = if let Some(p) = params {
                p.as_bytes().to_vec()
            } else {
                vec![]
            };

            match blockchain
                .call_contract(from, contract, method, params_bytes)
                .await
            {
                Ok(tx_hash) => {
                    println!("✅ Contract call successful");
                    println!("Transaction Hash: {}", tx_hash);
                    println!("Contract: {}", contract);
                    println!("Method: {}", method);
                    println!("Caller: {}", from);
                }
                Err(e) => {
                    error!("Failed to call contract: {}", e);
                    println!("❌ Contract call failed: {}", e);
                }
            }
        }
        _ => {
            println!("PolyTorus - 4-Layer Modular Blockchain Platform");
            println!("Usage: polytorus <COMMAND>");
            println!();
            println!("Commands:");
            println!("  start            Initialize and start the blockchain node");
            println!("  start-p2p        Start node with P2P networking");
            println!("  send             Send a transaction");
            println!("  status           Show blockchain status");
            println!("  deploy-contract  Deploy a smart contract");
            println!("  call-contract    Call a smart contract method");
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

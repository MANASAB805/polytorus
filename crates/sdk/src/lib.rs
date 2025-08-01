//! PolyTorus SDK - Developer-friendly interface for the 4-layer modular blockchain
//!
//! This SDK provides a high-level, easy-to-use interface for developers to interact with
//! the PolyTorus blockchain platform. It abstracts the complexity of the 4-layer architecture
//! and provides simple methods for common blockchain operations.
//!
//! # Features
//!
//! - **Transaction Management**: Create, sign, and submit transactions
//! - **Wallet Integration**: Full HD wallet support with BIP32/BIP44
//! - **Smart Contracts**: Deploy and interact with WASM-based contracts
//! - **Block Operations**: Query blocks, mining, and validation
//! - **Data Availability**: Store and retrieve data with proofs
//! - **Layer Abstraction**: Direct access to individual layers when needed
//!
//! # Quick Start
//!
//! ```rust
//! use sdk::{PolyTorusClient, ClientConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create a new client
//!     let client = PolyTorusClient::new(ClientConfig::default()).await?;
//!     
//!     // Create a wallet
//!     let wallet = client.create_wallet().await?;
//!     
//!     // Send a transaction
//!     let tx_hash = client.send_transaction(
//!         &wallet,
//!         "recipient_address",
//!         1000, // amount
//!         None  // data
//!     ).await?;
//!     
//!     println!("Transaction sent: {}", tx_hash);
//!     Ok(())
//! }
//! ```

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Re-export core types for convenience
pub use traits::*;
pub use wallet::{HdWallet, Wallet, Address as WalletAddress, KeyPair, Signature, Mnemonic};

// Internal layer imports
use consensus::{PolyTorusConsensusLayer, ConsensusConfig};
use data_availability::{PolyTorusDataAvailabilityLayer, DataAvailabilityConfig};
use execution::{PolyTorusExecutionLayer, ExecutionConfig};
use settlement::{PolyTorusSettlementLayer, SettlementConfig};

// ============================================================================
// SDK Configuration
// ============================================================================

/// Configuration for the PolyTorus client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub network: NetworkConfig,
    pub layers: LayerConfigs,
    pub wallet: WalletConfig,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            network: NetworkConfig::default(),
            layers: LayerConfigs::default(),
            wallet: WalletConfig::default(),
        }
    }
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub chain_id: u64,
    pub network_name: String,
    pub is_testnet: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            chain_id: 1,
            network_name: "polytorus-mainnet".to_string(),
            is_testnet: false,
        }
    }
}

/// Layer configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerConfigs {
    pub execution: ExecutionConfig,
    pub settlement: SettlementConfig,
    pub consensus: ConsensusConfig,
    pub data_availability: DataAvailabilityConfig,
}

impl Default for LayerConfigs {
    fn default() -> Self {
        Self {
            execution: ExecutionConfig::default(),
            settlement: SettlementConfig::default(),
            consensus: ConsensusConfig::default(),
            data_availability: DataAvailabilityConfig::default(),
        }
    }
}

/// Wallet configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    pub derivation_path: String,
    pub address_format: String,
}

impl Default for WalletConfig {
    fn default() -> Self {
        Self {
            derivation_path: "m/44'/0'/0'".to_string(),
            address_format: "native_segwit".to_string(),
        }
    }
}

// ============================================================================
// SDK Client
// ============================================================================

/// High-level client for interacting with PolyTorus blockchain
pub struct PolyTorusClient {
    config: ClientConfig,
    execution_layer: Arc<RwLock<PolyTorusExecutionLayer>>,
    settlement_layer: Arc<RwLock<PolyTorusSettlementLayer>>,
    consensus_layer: Arc<RwLock<PolyTorusConsensusLayer>>,
    data_availability_layer: Arc<RwLock<PolyTorusDataAvailabilityLayer>>,
    wallets: Arc<RwLock<HashMap<String, HdWallet>>>,
}

impl PolyTorusClient {
    /// Create a new PolyTorus client
    pub async fn new(config: ClientConfig) -> Result<Self> {
        let execution_layer = Arc::new(RwLock::new(
            PolyTorusExecutionLayer::new(config.layers.execution.clone())?,
        ));
        let settlement_layer = Arc::new(RwLock::new(
            PolyTorusSettlementLayer::new(config.layers.settlement.clone())?,
        ));
        let consensus_layer = Arc::new(RwLock::new(
            PolyTorusConsensusLayer::new(config.layers.consensus.clone())?,
        ));
        let data_availability_layer = Arc::new(RwLock::new(
            PolyTorusDataAvailabilityLayer::new(config.layers.data_availability.clone())?,
        ));

        Ok(Self {
            config,
            execution_layer,
            settlement_layer,
            consensus_layer,
            data_availability_layer,
            wallets: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create a new client with default configuration
    pub async fn new_default() -> Result<Self> {
        Self::new(ClientConfig::default()).await
    }

    // ========================================================================
    // Wallet Management
    // ========================================================================

    /// Create a new HD wallet
    pub async fn create_wallet(&self) -> Result<WalletInfo> {
        let wallet = HdWallet::new(wallet::KeyType::Ed25519)
            .map_err(|e| anyhow!("Failed to create wallet: {}", e))?;
        
        let mnemonic_phrase = wallet.get_mnemonic().phrase().to_string();
        
        let mut base_wallet = wallet.derive_wallet("m/44'/9999'/0'/0/0", wallet::KeyType::Ed25519)
            .map_err(|e| anyhow!("Failed to derive wallet: {}", e))?;
        let address = base_wallet.default_address()
            .map_err(|e| anyhow!("Failed to get address: {}", e))?.value;
        
        let wallet_id = uuid::Uuid::new_v4().to_string();
        
        let mut wallets = self.wallets.write().await;
        wallets.insert(wallet_id.clone(), wallet);
        
        Ok(WalletInfo {
            id: wallet_id,
            address,
            mnemonic: mnemonic_phrase,
            derivation_path: self.config.wallet.derivation_path.clone(),
        })
    }

    /// Import wallet from mnemonic
    pub async fn import_wallet(&self, mnemonic: &str, _passphrase: Option<&str>) -> Result<WalletInfo> {
        let wallet = HdWallet::from_phrase(mnemonic, wallet::KeyType::Ed25519)
            .map_err(|e| anyhow!("Failed to import wallet: {}", e))?;
        
        let mut base_wallet = wallet.derive_wallet("m/44'/9999'/0'/0/0", wallet::KeyType::Ed25519)
            .map_err(|e| anyhow!("Failed to derive wallet: {}", e))?;
        let address = base_wallet.default_address()
            .map_err(|e| anyhow!("Failed to get address: {}", e))?.value;
        
        let wallet_id = uuid::Uuid::new_v4().to_string();
        
        let mut wallets = self.wallets.write().await;
        wallets.insert(wallet_id.clone(), wallet);
        
        Ok(WalletInfo {
            id: wallet_id,
            address,
            mnemonic: mnemonic.to_string(),
            derivation_path: self.config.wallet.derivation_path.clone(),
        })
    }

    /// Get wallet information
    pub async fn get_wallet(&self, wallet_id: &str) -> Result<Option<WalletInfo>> {
        let wallets = self.wallets.read().await;
        
        if let Some(wallet) = wallets.get(wallet_id) {
            let mut base_wallet = wallet.derive_wallet("m/44'/9999'/0'/0/0", wallet::KeyType::Ed25519)
                .map_err(|e| anyhow!("Failed to derive wallet: {}", e))?;
            let address = base_wallet.default_address()
                .map_err(|e| anyhow!("Failed to get address: {}", e))?.value;
            
            Ok(Some(WalletInfo {
                id: wallet_id.to_string(),
                address,
                mnemonic: String::new(), // Don't expose mnemonic in get operations
                derivation_path: self.config.wallet.derivation_path.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    // ========================================================================
    // Transaction Operations
    // ========================================================================

    /// Send a simple transaction
    pub async fn send_transaction(
        &self,
        wallet_info: &WalletInfo,
        to: &str,
        amount: u64,
        data: Option<Vec<u8>>,
    ) -> Result<Hash> {
        let wallets = self.wallets.read().await;
        let wallet = wallets.get(&wallet_info.id)
            .ok_or_else(|| anyhow!("Wallet not found"))?;
        
        let keypair = wallet.derive_key(0)
            .map_err(|e| anyhow!("Failed to derive keypair: {}", e))?;
        
        let nonce = self.get_account_nonce(&wallet_info.address).await?;
        
        let tx = Transaction {
            hash: self.generate_transaction_hash(),
            from: wallet_info.address.clone(),
            to: Some(to.to_string()),
            value: amount,
            gas_limit: 21000, // Standard gas limit
            gas_price: 1,     // Default gas price
            data: data.unwrap_or_default(),
            nonce,
            signature: vec![], // Will be filled after signing
            script_type: None,
        };
        
        let signed_tx = self.sign_transaction(tx, &keypair).await?;
        self.submit_transaction(signed_tx).await
    }

    /// Deploy a smart contract
    pub async fn deploy_contract(
        &self,
        wallet_info: &WalletInfo,
        contract_code: &[u8],
        init_params: &[u8],
        gas_limit: u64,
    ) -> Result<Hash> {
        let wallets = self.wallets.read().await;
        let wallet = wallets.get(&wallet_info.id)
            .ok_or_else(|| anyhow!("Wallet not found"))?;
        
        let keypair = wallet.derive_key(0)
            .map_err(|e| anyhow!("Failed to derive keypair: {}", e))?;
        
        let nonce = self.get_account_nonce(&wallet_info.address).await?;
        
        let tx = Transaction {
            hash: self.generate_transaction_hash(),
            from: wallet_info.address.clone(),
            to: None, // Contract deployment
            value: 0,
            gas_limit,
            gas_price: 1,
            data: vec![],
            nonce,
            signature: vec![],
            script_type: Some(ScriptTransactionType::Deploy {
                script_data: contract_code.to_vec(),
                init_params: init_params.to_vec(),
            }),
        };
        
        let signed_tx = self.sign_transaction(tx, &keypair).await?;
        self.submit_transaction(signed_tx).await
    }

    /// Call a smart contract method
    pub async fn call_contract(
        &self,
        wallet_info: &WalletInfo,
        contract_hash: &Hash,
        method: &str,
        params: &[u8],
        gas_limit: u64,
    ) -> Result<Hash> {
        let wallets = self.wallets.read().await;
        let wallet = wallets.get(&wallet_info.id)
            .ok_or_else(|| anyhow!("Wallet not found"))?;
        
        let keypair = wallet.derive_key(0)
            .map_err(|e| anyhow!("Failed to derive keypair: {}", e))?;
        
        let nonce = self.get_account_nonce(&wallet_info.address).await?;
        
        let tx = Transaction {
            hash: self.generate_transaction_hash(),
            from: wallet_info.address.clone(),
            to: Some(contract_hash.clone()),
            value: 0,
            gas_limit,
            gas_price: 1,
            data: vec![],
            nonce,
            signature: vec![],
            script_type: Some(ScriptTransactionType::Call {
                script_hash: contract_hash.clone(),
                method: method.to_string(),
                params: params.to_vec(),
            }),
        };
        
        let signed_tx = self.sign_transaction(tx, &keypair).await?;
        self.submit_transaction(signed_tx).await
    }

    // ========================================================================
    // Query Operations
    // ========================================================================

    /// Get account balance
    pub async fn get_balance(&self, address: &str) -> Result<u64> {
        let execution = self.execution_layer.read().await;
        let account_state = execution.get_account_state(&address.to_string()).await?;
        Ok(account_state.balance)
    }

    /// Get account nonce
    pub async fn get_account_nonce(&self, address: &str) -> Result<u64> {
        let execution = self.execution_layer.read().await;
        let account_state = execution.get_account_state(&address.to_string()).await?;
        Ok(account_state.nonce)
    }

    /// Get block by hash
    pub async fn get_block(&self, block_hash: &Hash) -> Result<Option<Block>> {
        let consensus = self.consensus_layer.read().await;
        consensus.get_block_by_hash(block_hash).await
    }

    /// Get current block height
    pub async fn get_block_height(&self) -> Result<u64> {
        let consensus = self.consensus_layer.read().await;
        consensus.get_block_height().await
    }

    /// Get transaction receipt (simplified)
    pub async fn get_transaction_receipt(&self, _tx_hash: &Hash) -> Result<Option<TransactionReceipt>> {
        // This would typically query a transaction pool or blockchain storage
        // For now, returning None as this requires more complex state management
        Ok(None)
    }

    // ========================================================================
    // Data Availability Operations
    // ========================================================================

    /// Store data on the blockchain
    pub async fn store_data(&self, data: &[u8]) -> Result<Hash> {
        let mut da_layer = self.data_availability_layer.write().await;
        da_layer.store_data(data).await
    }

    /// Retrieve data from the blockchain
    pub async fn retrieve_data(&self, data_hash: &Hash) -> Result<Option<Vec<u8>>> {
        let da_layer = self.data_availability_layer.read().await;
        da_layer.retrieve_data(data_hash).await
    }

    /// Verify data availability
    pub async fn verify_data_availability(&self, data_hash: &Hash) -> Result<bool> {
        let da_layer = self.data_availability_layer.read().await;
        da_layer.verify_availability(data_hash).await
    }

    // ========================================================================
    // Mining Operations
    // ========================================================================

    /// Mine a new block with pending transactions
    pub async fn mine_block(&self) -> Result<Block> {
        let mut consensus = self.consensus_layer.write().await;
        // For simplicity, mining an empty block
        // In a real implementation, this would gather pending transactions
        consensus.mine_block(vec![]).await
    }

    /// Set mining difficulty
    pub async fn set_mining_difficulty(&self, difficulty: usize) -> Result<()> {
        let mut consensus = self.consensus_layer.write().await;
        consensus.set_difficulty(difficulty).await
    }

    // ========================================================================
    // Layer Access
    // ========================================================================

    /// Get direct access to execution layer
    pub fn execution_layer(&self) -> Arc<RwLock<PolyTorusExecutionLayer>> {
        self.execution_layer.clone()
    }

    /// Get direct access to settlement layer
    pub fn settlement_layer(&self) -> Arc<RwLock<PolyTorusSettlementLayer>> {
        self.settlement_layer.clone()
    }

    /// Get direct access to consensus layer
    pub fn consensus_layer(&self) -> Arc<RwLock<PolyTorusConsensusLayer>> {
        self.consensus_layer.clone()
    }

    /// Get direct access to data availability layer
    pub fn data_availability_layer(&self) -> Arc<RwLock<PolyTorusDataAvailabilityLayer>> {
        self.data_availability_layer.clone()
    }

    // ========================================================================
    // Internal Helper Methods
    // ========================================================================

    async fn sign_transaction(&self, mut tx: Transaction, keypair: &KeyPair) -> Result<Transaction> {
        let tx_data = serde_json::to_vec(&tx)?;
        let signature_vec = keypair.sign(&tx_data)
            .map_err(|e| anyhow!("Failed to sign transaction: {}", e))?;
        tx.signature = signature_vec;
        Ok(tx)
    }

    async fn submit_transaction(&self, tx: Transaction) -> Result<Hash> {
        let mut execution = self.execution_layer.write().await;
        let receipt = execution.execute_transaction(&tx).await?;
        
        if receipt.success {
            Ok(tx.hash)
        } else {
            Err(anyhow!("Transaction execution failed"))
        }
    }

    fn generate_transaction_hash(&self) -> Hash {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(chrono::Utc::now().timestamp().to_string());
        hasher.update(uuid::Uuid::new_v4().to_string());
        hex::encode(hasher.finalize())
    }
}

// ============================================================================
// SDK Types
// ============================================================================

/// Wallet information returned by SDK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    pub id: String,
    pub address: String,
    pub mnemonic: String,
    pub derivation_path: String,
}

/// Contract deployment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractDeployment {
    pub contract_hash: Hash,
    pub transaction_hash: Hash,
    pub gas_used: u64,
}

/// Contract call result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractCallResult {
    pub transaction_hash: Hash,
    pub return_data: Vec<u8>,
    pub gas_used: u64,
    pub events: Vec<Event>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = PolyTorusClient::new_default().await;
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_wallet_creation() {
        let client = PolyTorusClient::new_default().await.unwrap();
        let wallet = client.create_wallet().await;
        assert!(wallet.is_ok());
        
        let wallet_info = wallet.unwrap();
        assert!(!wallet_info.id.is_empty());
        assert!(!wallet_info.address.is_empty());
        assert!(!wallet_info.mnemonic.is_empty());
    }

    #[tokio::test]
    async fn test_data_storage() {
        let client = PolyTorusClient::new_default().await.unwrap();
        let data = b"Hello, PolyTorus!";
        
        let hash = client.store_data(data).await;
        assert!(hash.is_ok());
        
        let stored_data = client.retrieve_data(&hash.unwrap()).await;
        assert!(stored_data.is_ok());
        assert_eq!(stored_data.unwrap(), Some(data.to_vec()));
    }

    #[tokio::test]
    async fn test_mining() {
        let client = PolyTorusClient::new_default().await.unwrap();
        
        // Set low difficulty for fast mining in tests
        client.set_mining_difficulty(0).await.unwrap();
        
        let block = client.mine_block().await;
        assert!(block.is_ok());
        
        let mined_block = block.unwrap();
        assert!(!mined_block.hash.is_empty());
        assert_eq!(mined_block.transactions.len(), 0); // Empty block
    }
}
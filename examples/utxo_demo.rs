//! eUTXO Demo - Extended UTXO Model Demonstration
//!
//! This example demonstrates the complete eUTXO implementation including:
//! - Creating and managing UTXOs
//! - Transaction creation and validation
//! - Script execution
//! - Block mining and consensus
//! - Rollup batch processing

use consensus::consensus_engine::{PolyTorusUtxoConsensusLayer, UtxoConsensusConfig};
use execution::execution_engine::{PolyTorusUtxoExecutionLayer, UtxoExecutionConfig};
use traits::{
    ScriptContext, TxInput, TxOutput, UtxoConsensusLayer, UtxoExecutionLayer, UtxoId,
    UtxoTransaction,
};

/// Demonstration of eUTXO functionality
pub struct UtxoDemo {
    execution_layer: PolyTorusUtxoExecutionLayer,
    consensus_layer: PolyTorusUtxoConsensusLayer,
}

impl UtxoDemo {
    /// Create new eUTXO demo instance
    pub fn new() -> anyhow::Result<Self> {
        let execution_config = UtxoExecutionConfig::default();
        let consensus_config = UtxoConsensusConfig::default();

        let execution_layer = PolyTorusUtxoExecutionLayer::new(execution_config)?;
        let consensus_layer = PolyTorusUtxoConsensusLayer::new_as_validator(
            consensus_config,
            "demo_validator".to_string(),
        )?;

        Ok(Self {
            execution_layer,
            consensus_layer,
        })
    }

    /// Create a genesis UTXO for testing
    pub async fn create_genesis_utxo(&mut self) -> anyhow::Result<UtxoId> {
        // Create genesis UTXO properly using the new API
        let genesis_utxo_id = UtxoId {
            tx_hash: "genesis_tx".to_string(),
            output_index: 0,
        };

        let genesis_utxo = traits::Utxo {
            id: genesis_utxo_id.clone(),
            value: 1_000_000, // 1M units
            script: vec![],   // Empty script = "always true"
            datum: Some(b"Genesis UTXO".to_vec()),
            datum_hash: Some("genesis_datum_hash".to_string()),
        };

        // Initialize genesis UTXO set properly
        self.execution_layer
            .initialize_genesis_utxo_set(vec![(genesis_utxo_id.clone(), genesis_utxo)])?;

        println!("Created genesis UTXO: {genesis_utxo_id:?}");
        Ok(genesis_utxo_id)
    }

    /// Create a simple transfer transaction
    pub fn create_transfer_transaction(
        &self,
        from_utxo: UtxoId,
        to_value: u64,
        change_value: u64,
        fee: u64,
    ) -> UtxoTransaction {
        let tx_hash = format!("tx_{}", uuid::Uuid::new_v4());

        UtxoTransaction {
            hash: tx_hash.clone(),
            inputs: vec![TxInput {
                utxo_id: from_utxo,
                redeemer: b"simple_signature".to_vec(), // Simplified
                signature: b"demo_signature".to_vec(),
            }],
            outputs: vec![
                TxOutput {
                    value: to_value,
                    script: vec![], // Empty script = "always true"
                    datum: Some(b"Transferred value".to_vec()),
                    datum_hash: Some("transfer_datum_hash".to_string()),
                },
                TxOutput {
                    value: change_value,
                    script: vec![], // Empty script = "always true"
                    datum: Some(b"Change output".to_vec()),
                    datum_hash: Some("change_datum_hash".to_string()),
                },
            ],
            fee,
            validity_range: Some((0, 1000)), // Valid for slots 0-1000
            script_witness: vec![b"witness_data".to_vec()],
            auxiliary_data: Some(b"transaction_metadata".to_vec()),
        }
    }

    /// Create a smart contract transaction
    pub fn create_contract_transaction(
        &self,
        input_utxo: UtxoId,
        contract_script: Vec<u8>,
        contract_datum: Vec<u8>,
    ) -> UtxoTransaction {
        let tx_hash = format!("contract_tx_{}", uuid::Uuid::new_v4());

        UtxoTransaction {
            hash: tx_hash,
            inputs: vec![TxInput {
                utxo_id: input_utxo,
                redeemer: b"contract_redeemer".to_vec(),
                signature: b"contract_signature".to_vec(),
            }],
            outputs: vec![TxOutput {
                value: 500_000,
                script: contract_script,
                datum: Some(contract_datum),
                datum_hash: Some("contract_datum_hash".to_string()),
            }],
            fee: 10_000,
            validity_range: None, // No time restrictions
            script_witness: vec![b"contract_witness".to_vec()],
            auxiliary_data: Some(b"contract_metadata".to_vec()),
        }
    }

    /// Run a complete eUTXO demonstration
    pub async fn run_demo(&mut self) -> anyhow::Result<()> {
        println!("🚀 Starting eUTXO Demonstration");
        println!("================================");

        // Step 1: Create genesis UTXO
        println!("\n📦 Creating Genesis UTXO...");
        let genesis_utxo_id = self.create_genesis_utxo().await?;

        // Step 2: Create a simple transfer
        println!("\n💸 Creating Transfer Transaction...");
        let transfer_tx = self.create_transfer_transaction(
            genesis_utxo_id.clone(),
            300_000, // Send 300k to recipient
            680_000, // 680k change (1M - 300k - 20k fee)
            20_000,  // 20k fee
        );
        println!("Transfer TX: {}", transfer_tx.hash);

        // Step 3: Execute the transaction
        println!("\n⚡ Executing Transaction...");
        match self
            .execution_layer
            .execute_utxo_transaction(&transfer_tx)
            .await
        {
            Ok(receipt) => {
                println!("✅ Transaction executed successfully!");
                println!("   - Success: {}", receipt.success);
                println!(
                    "   - Script execution units: {}",
                    receipt.script_execution_units
                );
                println!("   - Consumed UTXOs: {}", receipt.consumed_utxos.len());
                println!("   - Created UTXOs: {}", receipt.created_utxos.len());
                println!("   - Events: {}", receipt.events.len());
            }
            Err(e) => {
                println!("❌ Transaction execution failed: {}", e);
            }
        }

        // Step 4: Check UTXO set state
        println!("\n📊 Current UTXO Set State:");
        let utxo_set_hash = self.execution_layer.get_utxo_set_hash().await?;
        let total_supply = self.execution_layer.get_total_supply().await?;
        println!("   - UTXO Set Hash: {}", utxo_set_hash);
        println!("   - Total Supply: {} units", total_supply);

        // Step 5: Create and mine a block
        println!("\n⛏️ Mining Block with Transaction...");
        let block = self
            .consensus_layer
            .mine_utxo_block(vec![transfer_tx])
            .await?;
        println!("✅ Block mined successfully!");
        println!("   - Block #{} (Slot {})", block.number, block.slot);
        println!("   - Hash: {}", block.hash);
        println!("   - Transactions: {}", block.transactions.len());

        // Step 6: Validate and add block to chain
        println!("\n🔍 Validating and Adding Block...");
        println!("   - Block hash: {}", block.hash);
        println!("   - Block slot: {}", block.slot);
        println!("   - Parent hash: {}", block.parent_hash);
        println!("   - Transactions: {}", block.transactions.len());

        let is_valid = self.consensus_layer.validate_utxo_block(&block).await?;
        if is_valid {
            self.consensus_layer.add_utxo_block(block).await?;
            println!("✅ Block added to chain!");
        } else {
            println!("❌ Block validation failed!");
            println!("   ℹ️  This may be due to strict consensus rules or slot timing");
        }

        // Step 7: Check consensus state
        println!("\n⛓️ Consensus Layer State:");
        let chain_height = self.consensus_layer.get_block_height().await?;
        let current_slot = self.consensus_layer.get_current_slot().await?;
        let canonical_chain = self.consensus_layer.get_canonical_chain().await?;
        println!("   - Chain Height: {chain_height}");
        println!("   - Current Slot: {current_slot}");
        println!("   - Chain Length: {} blocks", canonical_chain.len());

        // Step 8: Demonstrate batch processing
        println!("\n📦 Creating Transaction Batch...");
        let batch_txs = vec![
            self.create_transfer_transaction(
                UtxoId {
                    tx_hash: "batch_tx_1".to_string(),
                    output_index: 0,
                },
                100_000,
                580_000,
                20_000,
            ),
            self.create_transfer_transaction(
                UtxoId {
                    tx_hash: "batch_tx_2".to_string(),
                    output_index: 0,
                },
                150_000,
                430_000,
                20_000,
            ),
        ];

        match self.execution_layer.execute_utxo_batch(batch_txs).await {
            Ok(batch_result) => {
                println!("✅ Batch executed successfully!");
                println!("   - Batch ID: {}", batch_result.batch_id);
                println!("   - Transactions: {}", batch_result.transactions.len());
                println!("   - Results: {}", batch_result.results.len());
                println!("   - Slot: {}", batch_result.slot);
            }
            Err(e) => {
                println!("❌ Batch execution failed: {e}");
            }
        }

        println!("\n🎉 eUTXO Demonstration Complete!");
        println!("================================");

        Ok(())
    }

    /// Demonstrate script validation
    pub async fn demonstrate_script_validation(&self) -> anyhow::Result<()> {
        println!("\n🔐 Script Validation Demonstration");
        println!("===================================");

        // Create a simple script context
        let dummy_tx = UtxoTransaction {
            hash: "script_test_tx".to_string(),
            inputs: vec![],
            outputs: vec![],
            fee: 0,
            validity_range: None,
            script_witness: vec![],
            auxiliary_data: None,
        };

        let script_context = ScriptContext {
            tx: dummy_tx,
            input_index: 0,
            consumed_utxos: vec![],
            current_slot: 42,
        };

        // Test 1: Empty script (should always succeed)
        let empty_script = vec![];
        let empty_redeemer = vec![];
        let result1 = self
            .execution_layer
            .validate_script(&empty_script, &empty_redeemer, &script_context)
            .await?;
        println!("✅ Empty script validation: {}", result1);

        // Test 2: Simple "always true" script simulation
        // Instead of invalid WASM, we'll use empty script which always returns true
        let simple_script = vec![]; // Empty script simulates "always true" validation
        let simple_redeemer = vec![0x04, 0x05, 0x06]; // Dummy redeemer
        let result2 = self
            .execution_layer
            .validate_script(&simple_script, &simple_redeemer, &script_context)
            .await;

        match result2 {
            Ok(valid) => println!("✅ Simple script validation: {}", valid),
            Err(e) => println!("❌ Simple script validation failed: {}", e),
        }

        // Test 3: Demonstrate WASM module requirement
        println!(
            "ℹ️  Note: Real eUTXO scripts require valid WASM modules with 'validate' function"
        );
        println!("   Example WASM script structure:");
        println!("   - Module must export 'validate(redeemer_ptr: u32, redeemer_len: u32) -> i32'");
        println!("   - Return 1 for valid, 0 for invalid");
        println!(
            "   - Can use host functions: get_utxo_value, get_current_slot, validate_signature"
        );

        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    // Initialize logging
    env_logger::init();

    // Create async runtime
    let rt = tokio::runtime::Runtime::new()?;

    rt.block_on(async {
        // Create and run the demo
        let mut demo = UtxoDemo::new()?;

        // Run the main demonstration
        demo.run_demo().await?;

        // Demonstrate script validation
        demo.demonstrate_script_validation().await?;

        Ok(())
    })
}

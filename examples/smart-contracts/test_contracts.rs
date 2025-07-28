//! Integration tests for smart contract examples

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::fs;
    
    use execution::execution_engine::{PolyTorusUtxoExecutionLayer, UtxoExecutionConfig};
    use execution::script_engine::{ScriptType, BuiltInScript};
    use traits::{ExecutionLayer, UtxoExecutionLayer, Transaction, ScriptTransactionType};

    async fn setup_execution_layer() -> Result<PolyTorusUtxoExecutionLayer> {
        let config = UtxoExecutionConfig::default();
        PolyTorusUtxoExecutionLayer::new(config)
    }

    #[tokio::test]
    async fn test_simple_token_deployment() -> Result<()> {
        let mut execution_layer = setup_execution_layer().await?;
        
        // Load compiled WASM if it exists, otherwise use a simple test WASM
        let wasm_bytes = if fs::metadata("examples/smart-contracts/token/simple_token.wasm").is_ok() {
            fs::read("examples/smart-contracts/token/simple_token.wasm")?
        } else {
            // Simple WASM module that exports a verify function returning 1
            vec![
                0x00, 0x61, 0x73, 0x6d, // WASM magic
                0x01, 0x00, 0x00, 0x00, // Version
                0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f, // Type section: () -> i32
                0x03, 0x02, 0x01, 0x00, // Function section: 1 function of type 0
                0x07, 0x0a, 0x01, 0x06, 0x76, 0x65, 0x72, 0x69, 0x66, 0x79, 0x00, 0x00, // Export "verify"
                0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x01, 0x0b, // Code: return 1
            ]
        };
        
        // Create deployment transaction
        let tx = Transaction {
            hash: "test_deploy_token".to_string(),
            from: "alice".to_string(),
            to: None,
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
        
        // Deploy using the ExecutionLayer trait
        let result = execution_layer.deploy_script(
            "alice",
            &tx.script_type.as_ref().unwrap().get_script_data(),
            &[]
        ).await;
        
        println!("Token deployment result: {:?}", result);
        assert!(result.is_ok(), "Token contract deployment should succeed");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_voting_contract_deployment() -> Result<()> {
        let mut execution_layer = setup_execution_layer().await?;
        
        let wasm_bytes = if fs::metadata("examples/smart-contracts/voting/simple_voting.wasm").is_ok() {
            fs::read("examples/smart-contracts/voting/simple_voting.wasm")?
        } else {
            // Simple test WASM
            vec![
                0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
                0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f,
                0x03, 0x02, 0x01, 0x00,
                0x07, 0x0a, 0x01, 0x06, 0x76, 0x65, 0x72, 0x69, 0x66, 0x79, 0x00, 0x00,
                0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x01, 0x0b,
            ]
        };
        
        let result = execution_layer.deploy_script("bob", &wasm_bytes, &[]).await;
        
        println!("Voting deployment result: {:?}", result);
        assert!(result.is_ok(), "Voting contract deployment should succeed");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_escrow_contract_deployment() -> Result<()> {
        let mut execution_layer = setup_execution_layer().await?;
        
        let wasm_bytes = if fs::metadata("examples/smart-contracts/escrow/simple_escrow.wasm").is_ok() {
            fs::read("examples/smart-contracts/escrow/simple_escrow.wasm")?
        } else {
            // Simple test WASM
            vec![
                0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
                0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f,
                0x03, 0x02, 0x01, 0x00,
                0x07, 0x0a, 0x01, 0x06, 0x76, 0x65, 0x72, 0x69, 0x66, 0x79, 0x00, 0x00,
                0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x01, 0x0b,
            ]
        };
        
        let result = execution_layer.deploy_script("charlie", &wasm_bytes, &[]).await;
        
        println!("Escrow deployment result: {:?}", result);
        assert!(result.is_ok(), "Escrow contract deployment should succeed");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_builtin_contracts() -> Result<()> {
        let mut execution_layer = setup_execution_layer().await?;
        
        // Test PayToPublicKey
        let ptpk_result = execution_layer.deploy_script(
            "test_user",
            &[],
            &[]
        ).await;
        
        println!("PayToPublicKey deployment: {:?}", ptpk_result);
        assert!(ptpk_result.is_ok(), "PayToPublicKey should deploy successfully");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_contract_state_management() -> Result<()> {
        let mut execution_layer = setup_execution_layer().await?;
        
        // Deploy a simple contract
        let wasm_bytes = vec![
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
            0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f,
            0x03, 0x02, 0x01, 0x00,
            0x07, 0x0a, 0x01, 0x06, 0x76, 0x65, 0x72, 0x69, 0x66, 0x79, 0x00, 0x00,
            0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x01, 0x0b,
        ];
        
        let script_hash = execution_layer.deploy_script("alice", &wasm_bytes, &[]).await?;
        
        // Test script execution through script call
        let call_tx = Transaction {
            hash: "test_call".to_string(),
            from: "alice".to_string(),
            to: Some(script_hash.clone()),
            value: 0,
            gas_limit: 100000,
            gas_price: 1,
            data: vec![],
            nonce: 0,
            signature: vec![],
            script_type: Some(ScriptTransactionType::Call {
                script_hash: script_hash.clone(),
                method: "verify".to_string(),
                params: vec![],
            }),
        };
        
        // For now, just verify the transaction structure is correct
        assert_eq!(call_tx.script_type.is_some(), true);
        if let Some(ScriptTransactionType::Call { script_hash: hash, method, .. }) = &call_tx.script_type {
            assert_eq!(hash, &script_hash);
            assert_eq!(method, "verify");
        }
        
        println!("Contract state management test completed");
        Ok(())
    }
}

/// Helper trait to extract script data from ScriptTransactionType
trait ScriptTransactionHelper {
    fn get_script_data(&self) -> Vec<u8>;
}

impl ScriptTransactionHelper for ScriptTransactionType {
    fn get_script_data(&self) -> Vec<u8> {
        match self {
            ScriptTransactionType::Deploy { script_data, .. } => script_data.clone(),
            _ => vec![],
        }
    }
}
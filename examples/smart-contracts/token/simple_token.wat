;; Simple Token Contract for PolyTorus
;; This contract implements a basic token with transfer functionality

(module
  ;; Import host functions
  (import "env" "get_balance" (func $get_balance (param i32 i32) (result i64)))
  (import "env" "log" (func $log (param i32 i32)))
  (import "env" "get_state" (func $get_state (param i32 i32 i32 i32) (result i32)))
  (import "env" "set_state" (func $set_state (param i32 i32 i32 i32) (result i32)))
  (import "env" "verify_signature" (func $verify_signature (param i32 i32 i32 i32 i32 i32) (result i32)))
  
  ;; Memory for data storage
  (memory (export "memory") 1)
  
  ;; Constants
  (global $TOTAL_SUPPLY i64 (i64.const 1000000))
  
  ;; Data section for strings
  (data (i32.const 0) "Token initialized with supply: ")
  (data (i32.const 32) "Transfer: ")
  (data (i32.const 64) "Insufficient balance")
  (data (i32.const 96) "Invalid signature")
  (data (i32.const 128) "balance:")
  
  ;; Helper function to store i64 at memory location
  (func $store_i64 (param $ptr i32) (param $value i64)
    (i64.store (local.get $ptr) (local.get $value))
  )
  
  ;; Helper function to load i64 from memory location
  (func $load_i64 (param $ptr i32) (result i64)
    (i64.load (local.get $ptr))
  )
  
  ;; Initialize token with total supply to owner
  (func $initialize (param $owner_ptr i32) (param $owner_len i32) (result i32)
    (local $state_key_ptr i32)
    (local $state_value_ptr i32)
    
    ;; Create state key: "balance:<owner_address>"
    (local.set $state_key_ptr (i32.const 200))
    (memory.copy 
      (local.get $state_key_ptr)
      (i32.const 128)  ;; "balance:"
      (i32.const 8))
    (memory.copy
      (i32.add (local.get $state_key_ptr) (i32.const 8))
      (local.get $owner_ptr)
      (local.get $owner_len))
    
    ;; Store total supply as owner's balance
    (local.set $state_value_ptr (i32.const 300))
    (call $store_i64 (local.get $state_value_ptr) (global.get $TOTAL_SUPPLY))
    
    ;; Save to state
    (call $set_state
      (local.get $state_key_ptr)
      (i32.add (i32.const 8) (local.get $owner_len))
      (local.get $state_value_ptr)
      (i32.const 8))
    
    ;; Log initialization
    (call $log (i32.const 0) (i32.const 31))
    
    (i32.const 1)  ;; Success
  )
  
  ;; Get balance of an address
  (func $get_token_balance (param $addr_ptr i32) (param $addr_len i32) (result i64)
    (local $state_key_ptr i32)
    (local $state_value_ptr i32)
    (local $result i32)
    
    ;; Create state key
    (local.set $state_key_ptr (i32.const 200))
    (memory.copy 
      (local.get $state_key_ptr)
      (i32.const 128)  ;; "balance:"
      (i32.const 8))
    (memory.copy
      (i32.add (local.get $state_key_ptr) (i32.const 8))
      (local.get $addr_ptr)
      (local.get $addr_len))
    
    ;; Get balance from state
    (local.set $state_value_ptr (i32.const 300))
    (local.set $result
      (call $get_state
        (local.get $state_key_ptr)
        (i32.add (i32.const 8) (local.get $addr_len))
        (local.get $state_value_ptr)
        (i32.const 8)))
    
    ;; Return balance or 0 if not found
    (if (result i64) (local.get $result)
      (then (call $load_i64 (local.get $state_value_ptr)))
      (else (i64.const 0))
    )
  )
  
  ;; Transfer tokens from one address to another
  (func $transfer (param $from_ptr i32) (param $from_len i32)
                  (param $to_ptr i32) (param $to_len i32)
                  (param $amount i64) (result i32)
    (local $from_balance i64)
    (local $to_balance i64)
    (local $state_key_ptr i32)
    (local $state_value_ptr i32)
    
    ;; Get sender's balance
    (local.set $from_balance (call $get_token_balance (local.get $from_ptr) (local.get $from_len)))
    
    ;; Check sufficient balance
    (if (i64.lt_u (local.get $from_balance) (local.get $amount))
      (then
        (call $log (i32.const 64) (i32.const 20))  ;; "Insufficient balance"
        (return (i32.const 0))
      )
    )
    
    ;; Get receiver's balance
    (local.set $to_balance (call $get_token_balance (local.get $to_ptr) (local.get $to_len)))
    
    ;; Update sender's balance
    (local.set $from_balance (i64.sub (local.get $from_balance) (local.get $amount)))
    (local.set $state_key_ptr (i32.const 200))
    (memory.copy 
      (local.get $state_key_ptr)
      (i32.const 128)  ;; "balance:"
      (i32.const 8))
    (memory.copy
      (i32.add (local.get $state_key_ptr) (i32.const 8))
      (local.get $from_ptr)
      (local.get $from_len))
    
    (local.set $state_value_ptr (i32.const 300))
    (call $store_i64 (local.get $state_value_ptr) (local.get $from_balance))
    (call $set_state
      (local.get $state_key_ptr)
      (i32.add (i32.const 8) (local.get $from_len))
      (local.get $state_value_ptr)
      (i32.const 8))
    
    ;; Update receiver's balance
    (local.set $to_balance (i64.add (local.get $to_balance) (local.get $amount)))
    (memory.copy 
      (local.get $state_key_ptr)
      (i32.const 128)  ;; "balance:"
      (i32.const 8))
    (memory.copy
      (i32.add (local.get $state_key_ptr) (i32.const 8))
      (local.get $to_ptr)
      (local.get $to_len))
    
    (call $store_i64 (local.get $state_value_ptr) (local.get $to_balance))
    (call $set_state
      (local.get $state_key_ptr)
      (i32.add (i32.const 8) (local.get $to_len))
      (local.get $state_value_ptr)
      (i32.const 8))
    
    ;; Log transfer
    (call $log (i32.const 32) (i32.const 10))  ;; "Transfer: "
    
    (i32.const 1)  ;; Success
  )
  
  ;; Main entry point - verify function required by PolyTorus
  (func (export "verify") (param $witness_ptr i32) (param $witness_len i32)
                         (param $params_ptr i32) (param $params_len i32) (result i32)
    ;; For this simple example, we always return success
    ;; In a real implementation, you would:
    ;; 1. Parse the params to determine the operation (init, transfer, balance)
    ;; 2. Verify signatures if needed
    ;; 3. Execute the appropriate function
    ;; 4. Return 1 for success, 0 for failure
    
    (i32.const 1)
  )
  
  ;; Export additional functions for direct calls
  (export "initialize" (func $initialize))
  (export "get_token_balance" (func $get_token_balance))
  (export "transfer" (func $transfer))
)
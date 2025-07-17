# Database Storage Implementation for Smart Contracts

This document describes the advanced database storage implementation for Polytorus smart contracts, which provides persistent storage using PostgreSQL and Redis with intelligent fallback mechanisms.

## Overview

The `DatabaseContractStorage` implementation provides:

- **PostgreSQL**: Primary persistent storage for contract metadata, state, and execution history
- **Redis**: High-performance caching layer for frequently accessed data
- **Memory Fallback**: Automatic fallback to in-memory storage when databases are unavailable
- **Connection Pooling**: Efficient connection management for both databases
- **Health Monitoring**: Real-time monitoring of database connectivity and performance

## Features

### Multi-Backend Storage
- PostgreSQL for durable, ACID-compliant storage
- Redis for high-speed caching and temporary data
- Automatic failover to in-memory storage

### Performance Optimization
- Connection pooling for both PostgreSQL and Redis
- Intelligent caching strategies
- Asynchronous operations with proper error handling

### Monitoring and Statistics
- Real-time connection statistics
- Database health checks
- Performance metrics and query tracking

## Configuration

### Basic Configuration

```toml
[database_storage]
fallback_to_memory = true
connection_timeout_secs = 30
max_connections = 20
use_ssl = false

[database_storage.postgres]
host = "localhost"
port = 5432
database = "polytorus"
username = "polytorus"
password = "polytorus"
schema = "smart_contracts"
max_connections = 20

[database_storage.redis]
url = "redis://localhost:6379"
database = 0
max_connections = 20
key_prefix = "polytorus:contracts:"
ttl_seconds = 3600
```

### Environment-Specific Configurations

See `config/database-storage.toml` for complete examples of development, production, and testing configurations.

## Usage Examples

### Basic Setup

```rust
use polytorus::smart_contract::database_storage::{
    DatabaseContractStorage, DatabaseStorageConfig, PostgresConfig, RedisConfig
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create configuration
    let config = DatabaseStorageConfig {
        postgres: Some(PostgresConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "polytorus".to_string(),
            username: "polytorus".to_string(),
            password: "polytorus".to_string(),
            schema: "smart_contracts".to_string(),
            max_connections: 20,
        }),
        redis: Some(RedisConfig {
            url: "redis://localhost:6379".to_string(),
            password: None,
            database: 0,
            max_connections: 20,
            key_prefix: "polytorus:contracts:".to_string(),
            ttl_seconds: Some(3600),
        }),
        fallback_to_memory: true,
        connection_timeout_secs: 30,
        max_connections: 20,
        use_ssl: false,
    };

    // Initialize storage
    let storage = DatabaseContractStorage::new(config).await?;

    // Use storage for contract operations
    // ... (see ContractStateStorage trait methods)

    Ok(())
}
```

### Health Monitoring

```rust
// Check database connectivity
let status = storage.check_connectivity().await?;
println!("PostgreSQL: {}", if status.postgres_connected { "Connected" } else { "Disconnected" });
println!("Redis: {}", if status.redis_connected { "Connected" } else { "Disconnected" });
println!("Fallback available: {}", status.fallback_available);

// Get performance statistics
let stats = storage.get_stats().await;
println!("Total queries: {}", stats.total_queries);
println!("Cache hits: {}", stats.cache_hits);
println!("Cache misses: {}", stats.cache_misses);
println!("Failed queries: {}", stats.failed_queries);

// Get database information
let info = storage.get_database_info().await?;
println!("PostgreSQL size: {} bytes", info.postgres_size_bytes);
println!("Total contracts: {}", info.total_contracts);
println!("Total state entries: {}", info.total_state_entries);
```

### Contract Operations

```rust
use polytorus::smart_contract::unified_engine::{
    UnifiedContractMetadata, ContractType, ContractExecutionRecord
};

// Store contract metadata
let metadata = UnifiedContractMetadata {
    address: "0x1234567890abcdef".to_string(),
    name: "MyContract".to_string(),
    description: "A sample smart contract".to_string(),
    contract_type: ContractType::Wasm {
        bytecode: vec![0x00, 0x61, 0x73, 0x6d], // WASM magic number
        abi: Some("contract_abi".to_string()),
    },
    deployment_tx: "0xdeployment_hash".to_string(),
    deployment_time: 1640995200, // Unix timestamp
    owner: "0xowner_address".to_string(),
    is_active: true,
};

storage.store_contract_metadata(&metadata)?;

// Set contract state
storage.set_contract_state("0x1234567890abcdef", "balance", &1000u64.to_le_bytes())?;

// Get contract state
if let Some(balance_bytes) = storage.get_contract_state("0x1234567890abcdef", "balance")? {
    let balance = u64::from_le_bytes(balance_bytes.try_into().unwrap());
    println!("Contract balance: {}", balance);
}

// Store execution record
let execution = ContractExecutionRecord {
    execution_id: "exec_001".to_string(),
    contract_address: "0x1234567890abcdef".to_string(),
    function_name: "transfer".to_string(),
    caller: "0xcaller_address".to_string(),
    timestamp: 1640995260,
    gas_used: 21000,
    success: true,
    error_message: None,
};

storage.store_execution(&execution)?;

// Get execution history
let history = storage.get_execution_history("0x1234567890abcdef")?;
println!("Execution history: {} entries", history.len());
```

## Database Schema

### PostgreSQL Tables

The implementation automatically creates the following tables:

#### contracts
```sql
CREATE TABLE smart_contracts.contracts (
    address VARCHAR(42) PRIMARY KEY,
    data BYTEA NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

#### contract_state
```sql
CREATE TABLE smart_contracts.contract_state (
    state_key VARCHAR(255) PRIMARY KEY,
    contract_address VARCHAR(42) NOT NULL,
    key_name VARCHAR(255) NOT NULL,
    value BYTEA NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_contract_state_address ON smart_contracts.contract_state(contract_address);
```

#### execution_history
```sql
CREATE TABLE smart_contracts.execution_history (
    execution_key VARCHAR(255) PRIMARY KEY,
    contract_address VARCHAR(42) NOT NULL,
    execution_id VARCHAR(255) NOT NULL,
    data BYTEA NOT NULL,
    timestamp BIGINT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_execution_history_address ON smart_contracts.execution_history(contract_address);
CREATE INDEX idx_execution_history_timestamp ON smart_contracts.execution_history(timestamp DESC);
```

### Redis Key Structure

Redis keys follow this pattern:
- Contract metadata: `polytorus:contracts:contract:{address}`
- Contract state: `polytorus:contracts:state:{contract}:{key}`

## Error Handling and Fallback

The storage implementation provides robust error handling:

1. **Connection Failures**: Automatic fallback to in-memory storage when databases are unavailable
2. **Query Failures**: Graceful degradation with error logging
3. **Timeout Handling**: Configurable connection timeouts
4. **Health Monitoring**: Continuous health checks for proactive issue detection

## Performance Considerations

### Optimization Strategies

1. **Connection Pooling**: Reuse database connections to reduce overhead
2. **Caching**: Redis caching for frequently accessed data
3. **Indexing**: Proper database indexes for fast queries
4. **Batch Operations**: Efficient bulk operations where possible

### Monitoring

Monitor these metrics for optimal performance:
- Connection pool utilization
- Cache hit/miss ratios
- Query execution times
- Database size growth

## Security Considerations

1. **SSL/TLS**: Enable encryption for production environments
2. **Authentication**: Use strong passwords and authentication mechanisms
3. **Network Security**: Restrict database access to authorized hosts
4. **Data Encryption**: Consider encrypting sensitive contract data

## Deployment

### Prerequisites

1. PostgreSQL 12+ with the specified database and schema
2. Redis 6+ for caching
3. Network connectivity between application and databases

### Production Checklist

- [ ] SSL/TLS enabled for both PostgreSQL and Redis
- [ ] Strong authentication credentials configured
- [ ] Database backups configured
- [ ] Monitoring and alerting set up
- [ ] Connection limits properly configured
- [ ] Fallback behavior tested

## Troubleshooting

### Common Issues

1. **Connection Timeouts**: Increase `connection_timeout_secs` or check network connectivity
2. **Pool Exhaustion**: Increase `max_connections` or optimize query patterns
3. **Cache Misses**: Adjust TTL settings or cache warming strategies
4. **Schema Errors**: Ensure proper database permissions and schema creation

### Debugging

Enable debug logging to troubleshoot issues:
```rust
env_logger::init();
```

Check connection statistics and health status regularly:
```rust
let stats = storage.get_stats().await;
let status = storage.check_connectivity().await?;
```

## Migration from Other Storage Backends

To migrate from existing storage implementations:

1. Export data from current storage
2. Configure DatabaseContractStorage
3. Import data using the ContractStateStorage interface
4. Verify data integrity
5. Update application configuration

## Contributing

When contributing to the database storage implementation:

1. Add comprehensive tests for new features
2. Update documentation for configuration changes
3. Consider backward compatibility
4. Test with both PostgreSQL and Redis
5. Verify fallback behavior works correctly

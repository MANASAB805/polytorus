//! Advanced Database Storage Implementation
//!
//! This module provides advanced database storage implementations for enterprise deployment,
//! including PostgreSQL for relational data and Redis for high-performance caching.

use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Result;
use redis::{aio::ConnectionManager, AsyncCommands, Client as RedisClient};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use tokio::{sync::RwLock, time::timeout};

use super::unified_engine::{
    ContractExecutionRecord, ContractStateStorage, UnifiedContractMetadata,
};

/// Configuration for database storage backends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStorageConfig {
    /// PostgreSQL connection configuration
    pub postgres: Option<PostgresConfig>,
    /// Redis connection configuration
    pub redis: Option<RedisConfig>,
    /// Fallback to in-memory storage if databases unavailable
    pub fallback_to_memory: bool,
    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,
    /// Maximum connection pool size
    pub max_connections: u32,
    /// Enable connection encryption
    pub use_ssl: bool,
}

/// PostgreSQL configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub schema: String,
    pub max_connections: u32,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub password: Option<String>,
    pub database: u8,
    pub max_connections: u32,
    pub key_prefix: String,
    pub ttl_seconds: Option<u64>,
}

impl Default for DatabaseStorageConfig {
    fn default() -> Self {
        Self {
            postgres: None,
            redis: None,
            fallback_to_memory: true,
            connection_timeout_secs: 30,
            max_connections: 20,
            use_ssl: false,
        }
    }
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            database: "polytorus".to_string(),
            username: "polytorus".to_string(),
            password: "polytorus".to_string(),
            schema: "smart_contracts".to_string(),
            max_connections: 20,
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            password: None,
            database: 0,
            max_connections: 20,
            key_prefix: "polytorus:contracts:".to_string(),
            ttl_seconds: Some(3600), // 1 hour default TTL
        }
    }
}

/// Advanced database storage implementation with multiple backends
pub struct DatabaseContractStorage {
    config: DatabaseStorageConfig,
    postgres_pool: Option<Arc<PostgresConnectionPool>>,
    redis_pool: Option<Arc<RedisConnectionPool>>,
    memory_fallback: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    connection_stats: Arc<RwLock<ConnectionStats>>,
}

/// PostgreSQL connection pool
pub struct PostgresConnectionPool {
    pool: PgPool,
    config: PostgresConfig,
    active_connections: Arc<RwLock<u32>>,
}

/// Redis connection pool
pub struct RedisConnectionPool {
    manager: ConnectionManager,
    config: RedisConfig,
    active_connections: Arc<RwLock<u32>>,
}

/// Connection statistics
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    pub postgres_connections: u32,
    pub redis_connections: u32,
    pub total_queries: u64,
    pub failed_queries: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

/// Database connectivity status
#[derive(Debug, Clone)]
pub struct DatabaseConnectivityStatus {
    pub postgres_connected: bool,
    pub redis_connected: bool,
    pub fallback_available: bool,
}

/// Database information
#[derive(Debug, Clone)]
pub struct DatabaseInfo {
    pub postgres_size_bytes: u64,
    pub redis_memory_usage_bytes: u64,
    pub memory_fallback_entries: usize,
    pub total_contracts: usize,
    pub total_state_entries: usize,
    pub total_executions: usize,
}

/// PostgreSQL database information
#[derive(Debug, Clone)]
pub struct PostgresDatabaseInfo {
    pub size_bytes: u64,
    pub contracts_count: usize,
    pub state_entries_count: usize,
    pub executions_count: usize,
}

impl DatabaseContractStorage {
    /// Create a new database storage instance
    pub async fn new(config: DatabaseStorageConfig) -> Result<Self> {
        let mut postgres_pool = None;
        let mut redis_pool = None;

        // Initialize PostgreSQL connection pool
        if let Some(pg_config) = &config.postgres {
            match timeout(
                Duration::from_secs(config.connection_timeout_secs),
                PostgresConnectionPool::new(pg_config.clone()),
            )
            .await
            {
                Ok(Ok(pool)) => {
                    postgres_pool = Some(Arc::new(pool));
                }
                Ok(Err(e)) => {
                    if !config.fallback_to_memory {
                        return Err(anyhow::anyhow!("PostgreSQL connection failed: {}", e));
                    }
                }
                Err(_) => {
                    if !config.fallback_to_memory {
                        return Err(anyhow::anyhow!("PostgreSQL connection timeout"));
                    }
                }
            }
        }

        // Initialize Redis connection pool
        if let Some(redis_config) = &config.redis {
            match timeout(
                Duration::from_secs(config.connection_timeout_secs),
                RedisConnectionPool::new(redis_config.clone()),
            )
            .await
            {
                Ok(Ok(pool)) => {
                    redis_pool = Some(Arc::new(pool));
                }
                Ok(Err(e)) => {
                    if !config.fallback_to_memory {
                        return Err(anyhow::anyhow!("Redis connection failed: {}", e));
                    }
                }
                Err(_) => {
                    if !config.fallback_to_memory {
                        return Err(anyhow::anyhow!("Redis connection timeout"));
                    }
                }
            }
        }

        Ok(Self {
            config,
            postgres_pool,
            redis_pool,
            memory_fallback: Arc::new(RwLock::new(HashMap::new())),
            connection_stats: Arc::new(RwLock::new(ConnectionStats::default())),
        })
    }

    /// Create a testing instance with memory fallback
    pub fn testing() -> Self {
        Self {
            config: DatabaseStorageConfig {
                fallback_to_memory: true,
                ..Default::default()
            },
            postgres_pool: None,
            redis_pool: None,
            memory_fallback: Arc::new(RwLock::new(HashMap::new())),
            connection_stats: Arc::new(RwLock::new(ConnectionStats::default())),
        }
    }

    /// Get connection statistics
    pub async fn get_stats(&self) -> ConnectionStats {
        let mut stats = self.connection_stats.read().await.clone();

        // Update connection counts from actual pools
        if let Some(postgres) = &self.postgres_pool {
            stats.postgres_connections = postgres.get_connection_count().await;
        }

        if let Some(redis) = &self.redis_pool {
            stats.redis_connections = redis.get_connection_count().await;
        }

        stats
    }

    /// Store data in Redis cache
    async fn cache_store(&self, key: &str, value: &[u8]) -> Result<()> {
        if let Some(redis) = &self.redis_pool {
            match redis.set(key, value).await {
                Ok(_) => {
                    let mut stats = self.connection_stats.write().await;
                    stats.total_queries += 1;
                    return Ok(());
                }
                Err(e) => {
                    let mut stats = self.connection_stats.write().await;
                    stats.failed_queries += 1;
                    if !self.config.fallback_to_memory {
                        return Err(anyhow::anyhow!("Redis cache store failed: {}", e));
                    }
                    eprintln!("Redis cache store failed, using fallback: {}", e);
                }
            }
        }

        // Fallback to memory
        if self.config.fallback_to_memory {
            let mut memory = self.memory_fallback.write().await;
            memory.insert(key.to_string(), value.to_vec());
        }

        Ok(())
    }

    /// Retrieve data from Redis cache
    async fn cache_get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        if let Some(redis) = &self.redis_pool {
            match redis.get(key).await {
                Ok(Some(value)) => {
                    let mut stats = self.connection_stats.write().await;
                    stats.total_queries += 1;
                    stats.cache_hits += 1;
                    return Ok(Some(value));
                }
                Ok(None) => {
                    let mut stats = self.connection_stats.write().await;
                    stats.total_queries += 1;
                    stats.cache_misses += 1;
                }
                Err(e) => {
                    let mut stats = self.connection_stats.write().await;
                    stats.failed_queries += 1;
                    eprintln!("Redis cache get failed: {}", e);
                }
            }
        }

        // Fallback to memory
        if self.config.fallback_to_memory {
            let memory = self.memory_fallback.read().await;
            if let Some(value) = memory.get(key) {
                let mut stats = self.connection_stats.write().await;
                stats.cache_hits += 1;
                return Ok(Some(value.clone()));
            } else {
                let mut stats = self.connection_stats.write().await;
                stats.cache_misses += 1;
            }
        }

        Ok(None)
    }

    /// Store data in PostgreSQL
    async fn postgres_store(&self, table: &str, key: &str, value: &[u8]) -> Result<()> {
        if let Some(postgres) = &self.postgres_pool {
            match postgres.insert(table, key, value).await {
                Ok(_) => {
                    let mut stats = self.connection_stats.write().await;
                    stats.total_queries += 1;
                    return Ok(());
                }
                Err(e) => {
                    let mut stats = self.connection_stats.write().await;
                    stats.failed_queries += 1;
                    if !self.config.fallback_to_memory {
                        return Err(anyhow::anyhow!("PostgreSQL store failed: {}", e));
                    }
                    eprintln!("PostgreSQL store failed, using fallback: {}", e);
                }
            }
        }

        // Fallback to memory
        if self.config.fallback_to_memory {
            let composite_key = format!("{}:{}", table, key);
            let mut memory = self.memory_fallback.write().await;
            memory.insert(composite_key, value.to_vec());
        }

        Ok(())
    }

    /// Retrieve data from PostgreSQL
    async fn postgres_get(&self, table: &str, key: &str) -> Result<Option<Vec<u8>>> {
        if let Some(postgres) = &self.postgres_pool {
            match postgres.select(table, key).await {
                Ok(value) => {
                    let mut stats = self.connection_stats.write().await;
                    stats.total_queries += 1;
                    return Ok(value);
                }
                Err(e) => {
                    let mut stats = self.connection_stats.write().await;
                    stats.failed_queries += 1;
                    if !self.config.fallback_to_memory {
                        return Err(e);
                    }
                }
            }
        }

        // Fallback to memory
        if self.config.fallback_to_memory {
            let composite_key = format!("{}:{}", table, key);
            let memory = self.memory_fallback.read().await;
            return Ok(memory.get(&composite_key).cloned());
        }

        Ok(None)
    }

    /// Create a cache key for contract state
    fn make_cache_key(&self, contract: &str, key: &str) -> String {
        let prefix = self
            .config
            .redis
            .as_ref()
            .map(|r| r.key_prefix.as_str())
            .unwrap_or("");
        format!("{}state:{}:{}", prefix, contract, key)
    }

    /// Check database connectivity
    pub async fn check_connectivity(&self) -> Result<DatabaseConnectivityStatus> {
        let mut status = DatabaseConnectivityStatus {
            postgres_connected: false,
            redis_connected: false,
            fallback_available: self.config.fallback_to_memory,
        };

        // Check PostgreSQL connectivity
        if let Some(postgres) = &self.postgres_pool {
            status.postgres_connected = postgres.check_health().await.is_ok();
        }

        // Check Redis connectivity
        if let Some(redis) = &self.redis_pool {
            status.redis_connected = redis.check_health().await.is_ok();
        }

        Ok(status)
    }

    /// Clear all cached data
    pub async fn clear_cache(&self) -> Result<()> {
        // Clear Redis cache
        if let Some(redis) = &self.redis_pool {
            if let Err(e) = redis.flush_db().await {
                eprintln!("Failed to clear Redis cache: {}", e);
            }
        }

        // Clear memory fallback
        if self.config.fallback_to_memory {
            let mut memory = self.memory_fallback.write().await;
            memory.clear();
        }

        Ok(())
    }

    /// Get database size information
    pub async fn get_database_info(&self) -> Result<DatabaseInfo> {
        let mut info = DatabaseInfo {
            postgres_size_bytes: 0,
            redis_memory_usage_bytes: 0,
            memory_fallback_entries: 0,
            total_contracts: 0,
            total_state_entries: 0,
            total_executions: 0,
        };

        // Get memory fallback info
        if self.config.fallback_to_memory {
            let memory = self.memory_fallback.read().await;
            info.memory_fallback_entries = memory.len();

            for key in memory.keys() {
                if key.starts_with("contracts:") {
                    info.total_contracts += 1;
                } else if key.starts_with("contract_state:") {
                    info.total_state_entries += 1;
                } else if key.starts_with("execution_history:") {
                    info.total_executions += 1;
                }
            }
        }

        // Get PostgreSQL info
        if let Some(postgres) = &self.postgres_pool {
            if let Ok(pg_info) = postgres.get_database_info().await {
                info.postgres_size_bytes = pg_info.size_bytes;
                info.total_contracts = pg_info.contracts_count;
                info.total_state_entries = pg_info.state_entries_count;
                info.total_executions = pg_info.executions_count;
            }
        }

        Ok(info)
    }
}

impl ContractStateStorage for DatabaseContractStorage {
    fn store_contract_metadata(&self, metadata: &UnifiedContractMetadata) -> Result<()> {
        let serialized = bincode::serialize(metadata)?;

        // Use async runtime for database operations
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                handle.block_on(async {
                    // Store in PostgreSQL
                    if let Err(e) = self
                        .postgres_store("contracts", &metadata.address, &serialized)
                        .await
                    {
                        eprintln!("Failed to store contract metadata in PostgreSQL: {}", e);
                    }

                    // Cache in Redis
                    let cache_key = format!("contract:{}", metadata.address);
                    if let Err(e) = self.cache_store(&cache_key, &serialized).await {
                        eprintln!("Failed to cache contract metadata: {}", e);
                    }
                })
            });
        } else {
            // No async runtime, use blocking fallback
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                // Store in PostgreSQL
                if let Err(e) = self
                    .postgres_store("contracts", &metadata.address, &serialized)
                    .await
                {
                    eprintln!("Failed to store contract metadata in PostgreSQL: {}", e);
                }

                // Cache in Redis
                let cache_key = format!("contract:{}", metadata.address);
                if let Err(e) = self.cache_store(&cache_key, &serialized).await {
                    eprintln!("Failed to cache contract metadata: {}", e);
                }
            });
        }

        Ok(())
    }

    fn get_contract_metadata(&self, address: &str) -> Result<Option<UnifiedContractMetadata>> {
        let cache_key = format!("contract:{}", address);

        let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                handle.block_on(async {
                    // Try cache first
                    if let Ok(Some(cached_data)) = self.cache_get(&cache_key).await {
                        if let Ok(metadata) = bincode::deserialize(&cached_data) {
                            return Ok(Some(metadata));
                        }
                    }

                    // Fallback to PostgreSQL
                    if let Ok(Some(pg_data)) = self.postgres_get("contracts", address).await {
                        if let Ok(metadata) = bincode::deserialize(&pg_data) {
                            // Populate cache for future requests
                            let _ = self.cache_store(&cache_key, &pg_data).await;
                            return Ok(Some(metadata));
                        }
                    }

                    Ok(None)
                })
            })
        } else {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                // Try cache first
                if let Ok(Some(cached_data)) = self.cache_get(&cache_key).await {
                    if let Ok(metadata) = bincode::deserialize(&cached_data) {
                        return Ok(Some(metadata));
                    }
                }

                // Fallback to PostgreSQL
                if let Ok(Some(pg_data)) = self.postgres_get("contracts", address).await {
                    if let Ok(metadata) = bincode::deserialize(&pg_data) {
                        // Populate cache for future requests
                        let _ = self.cache_store(&cache_key, &pg_data).await;
                        return Ok(Some(metadata));
                    }
                }

                Ok(None)
            })
        };

        result
    }

    fn set_contract_state(&self, contract: &str, key: &str, value: &[u8]) -> Result<()> {
        let state_key = format!("{}:{}", contract, key);
        let cache_key = self.make_cache_key(contract, key);

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                handle.block_on(async {
                    // Store in PostgreSQL
                    if let Err(e) = self
                        .postgres_store("contract_state", &state_key, value)
                        .await
                    {
                        eprintln!("Failed to store contract state in PostgreSQL: {}", e);
                    }

                    // Cache in Redis
                    if let Err(e) = self.cache_store(&cache_key, value).await {
                        eprintln!("Failed to cache contract state: {}", e);
                    }
                })
            });
        } else {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                // Store in PostgreSQL
                if let Err(e) = self
                    .postgres_store("contract_state", &state_key, value)
                    .await
                {
                    eprintln!("Failed to store contract state in PostgreSQL: {}", e);
                }

                // Cache in Redis
                if let Err(e) = self.cache_store(&cache_key, value).await {
                    eprintln!("Failed to cache contract state: {}", e);
                }
            });
        }

        Ok(())
    }

    fn get_contract_state(&self, contract: &str, key: &str) -> Result<Option<Vec<u8>>> {
        let state_key = format!("{}:{}", contract, key);
        let cache_key = self.make_cache_key(contract, key);

        let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                handle.block_on(async {
                    // Try cache first
                    if let Ok(Some(cached_data)) = self.cache_get(&cache_key).await {
                        return Ok(Some(cached_data));
                    }

                    // Fallback to PostgreSQL
                    if let Ok(Some(pg_data)) = self.postgres_get("contract_state", &state_key).await
                    {
                        // Populate cache for future requests
                        let _ = self.cache_store(&cache_key, &pg_data).await;
                        return Ok(Some(pg_data));
                    }

                    Ok(None)
                })
            })
        } else {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                // Try cache first
                if let Ok(Some(cached_data)) = self.cache_get(&cache_key).await {
                    return Ok(Some(cached_data));
                }

                // Fallback to PostgreSQL
                if let Ok(Some(pg_data)) = self.postgres_get("contract_state", &state_key).await {
                    // Populate cache for future requests
                    let _ = self.cache_store(&cache_key, &pg_data).await;
                    return Ok(Some(pg_data));
                }

                Ok(None)
            })
        };

        result
    }

    fn delete_contract_state(&self, contract: &str, key: &str) -> Result<()> {
        let state_key = format!("{}:{}", contract, key);
        let cache_key = self.make_cache_key(contract, key);

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                handle.block_on(async {
                    // Remove from PostgreSQL
                    if let Some(postgres) = &self.postgres_pool {
                        if let Err(e) = postgres.delete("contract_state", &state_key).await {
                            eprintln!("Failed to delete from PostgreSQL: {}", e);
                        }
                    }

                    // Remove from Redis cache
                    if let Some(redis) = &self.redis_pool {
                        if let Err(e) = redis.delete(&cache_key).await {
                            eprintln!("Failed to delete from Redis: {}", e);
                        }
                    }

                    // Remove from memory fallback
                    if self.config.fallback_to_memory {
                        let mut memory = self.memory_fallback.write().await;
                        memory.remove(&format!("contract_state:{}", state_key));
                        memory.remove(&cache_key);
                    }
                })
            });
        }

        Ok(())
    }

    fn list_contracts(&self) -> Result<Vec<String>> {
        let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                handle.block_on(async {
                    // Try PostgreSQL first
                    if let Some(postgres) = &self.postgres_pool {
                        if let Ok(contracts) = postgres.list_keys("contracts").await {
                            return contracts;
                        }
                    }

                    // Fallback to memory
                    if self.config.fallback_to_memory {
                        let memory = self.memory_fallback.read().await;
                        return memory
                            .keys()
                            .filter_map(|k| {
                                if k.starts_with("contracts:") {
                                    Some(k.strip_prefix("contracts:").unwrap().to_string())
                                } else {
                                    None
                                }
                            })
                            .collect();
                    }

                    Vec::new()
                })
            })
        } else {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                // Try PostgreSQL first
                if let Some(postgres) = &self.postgres_pool {
                    if let Ok(contracts) = postgres.list_keys("contracts").await {
                        return contracts;
                    }
                }

                // Fallback to memory
                if self.config.fallback_to_memory {
                    let memory = self.memory_fallback.read().await;
                    return memory
                        .keys()
                        .filter_map(|k| {
                            if k.starts_with("contracts:") {
                                Some(k.strip_prefix("contracts:").unwrap().to_string())
                            } else {
                                None
                            }
                        })
                        .collect();
                }

                Vec::new()
            })
        };

        Ok(result)
    }

    fn store_execution(&self, execution: &ContractExecutionRecord) -> Result<()> {
        let execution_key = format!("{}:{}", execution.contract_address, execution.execution_id);
        let serialized = bincode::serialize(execution)?;

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                handle.block_on(async {
                    // Store in PostgreSQL
                    if let Err(e) = self
                        .postgres_store("execution_history", &execution_key, &serialized)
                        .await
                    {
                        eprintln!("Failed to store execution history in PostgreSQL: {}", e);
                    }
                })
            });
        }

        Ok(())
    }

    fn get_execution_history(&self, contract: &str) -> Result<Vec<ContractExecutionRecord>> {
        let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                handle.block_on(async {
                    // Try PostgreSQL
                    if let Some(postgres) = &self.postgres_pool {
                        if let Ok(executions) = postgres.get_executions_for_contract(contract).await
                        {
                            return executions;
                        }
                    }

                    // Fallback to memory
                    if self.config.fallback_to_memory {
                        let memory = self.memory_fallback.read().await;
                        let prefix = format!("execution_history:{}:", contract);
                        let mut executions = Vec::new();

                        for (key, value) in memory.iter() {
                            if key.starts_with(&prefix) {
                                if let Ok(execution) =
                                    bincode::deserialize::<ContractExecutionRecord>(value)
                                {
                                    executions.push(execution);
                                }
                            }
                        }

                        // Sort by timestamp (newest first)
                        executions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                        return executions;
                    }

                    Vec::new()
                })
            })
        } else {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                // Try PostgreSQL
                if let Some(postgres) = &self.postgres_pool {
                    if let Ok(executions) = postgres.get_executions_for_contract(contract).await {
                        return executions;
                    }
                }

                // Fallback to memory
                if self.config.fallback_to_memory {
                    let memory = self.memory_fallback.read().await;
                    let prefix = format!("execution_history:{}:", contract);
                    let mut executions = Vec::new();

                    for (key, value) in memory.iter() {
                        if key.starts_with(&prefix) {
                            if let Ok(execution) =
                                bincode::deserialize::<ContractExecutionRecord>(value)
                            {
                                executions.push(execution);
                            }
                        }
                    }

                    // Sort by timestamp (newest first)
                    executions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                    return executions;
                }

                Vec::new()
            })
        };

        Ok(result)
    }
}

impl PostgresConnectionPool {
    pub async fn new(config: PostgresConfig) -> Result<Self> {
        let database_url = format!(
            "postgresql://{}:{}@{}:{}/{}",
            config.username, config.password, config.host, config.port, config.database
        );

        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .connect(&database_url)
            .await?;

        // Initialize database schema
        let instance = Self {
            pool,
            config,
            active_connections: Arc::new(RwLock::new(0)),
        };

        instance.initialize_schema().await?;
        Ok(instance)
    }

    async fn initialize_schema(&self) -> Result<()> {
        // Create contracts table
        sqlx::query(&format!(
            r#"
            CREATE TABLE IF NOT EXISTS {}.contracts (
                address VARCHAR(42) PRIMARY KEY,
                data BYTEA NOT NULL,
                created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
            )
            "#,
            self.config.schema
        ))
        .execute(&self.pool)
        .await?;

        // Create contract_state table
        sqlx::query(&format!(
            r#"
            CREATE TABLE IF NOT EXISTS {}.contract_state (
                state_key VARCHAR(255) PRIMARY KEY,
                contract_address VARCHAR(42) NOT NULL,
                key_name VARCHAR(255) NOT NULL,
                value BYTEA NOT NULL,
                created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
            )
            "#,
            self.config.schema
        ))
        .execute(&self.pool)
        .await?;

        // Create execution_history table
        sqlx::query(&format!(
            r#"
            CREATE TABLE IF NOT EXISTS {}.execution_history (
                execution_key VARCHAR(255) PRIMARY KEY,
                contract_address VARCHAR(42) NOT NULL,
                execution_id VARCHAR(255) NOT NULL,
                data BYTEA NOT NULL,
                timestamp BIGINT NOT NULL,
                created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
            )
            "#,
            self.config.schema
        ))
        .execute(&self.pool)
        .await?;

        // Create indexes for better performance
        sqlx::query(&format!(
            "CREATE INDEX IF NOT EXISTS idx_contract_state_address ON {}.contract_state(contract_address)",
            self.config.schema
        ))
        .execute(&self.pool)
        .await?;

        sqlx::query(&format!(
            "CREATE INDEX IF NOT EXISTS idx_execution_history_address ON {}.execution_history(contract_address)",
            self.config.schema
        ))
        .execute(&self.pool)
        .await?;

        sqlx::query(&format!(
            "CREATE INDEX IF NOT EXISTS idx_execution_history_timestamp ON {}.execution_history(timestamp DESC)",
            self.config.schema
        ))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn insert(&self, table: &str, key: &str, value: &[u8]) -> Result<()> {
        let mut conn_count = self.active_connections.write().await;
        *conn_count += 1;
        drop(conn_count);

        let result = match table {
            "contracts" => {
                sqlx::query(&format!(
                    "INSERT INTO {}.contracts (address, data) VALUES ($1, $2)
                     ON CONFLICT (address) DO UPDATE SET data = $2, updated_at = NOW()",
                    self.config.schema
                ))
                .bind(key)
                .bind(value)
                .execute(&self.pool)
                .await
            }
            "contract_state" => {
                let parts: Vec<&str> = key.split(':').collect();
                if parts.len() != 2 {
                    return Err(anyhow::anyhow!("Invalid state key format: {}", key));
                }
                let (contract_address, key_name) = (parts[0], parts[1]);

                sqlx::query(&format!(
                    "INSERT INTO {}.contract_state (state_key, contract_address, key_name, value)
                     VALUES ($1, $2, $3, $4)
                     ON CONFLICT (state_key) DO UPDATE SET value = $4, updated_at = NOW()",
                    self.config.schema
                ))
                .bind(key)
                .bind(contract_address)
                .bind(key_name)
                .bind(value)
                .execute(&self.pool)
                .await
            }
            "execution_history" => {
                let parts: Vec<&str> = key.split(':').collect();
                if parts.len() != 2 {
                    return Err(anyhow::anyhow!("Invalid execution key format: {}", key));
                }
                let (contract_address, execution_id) = (parts[0], parts[1]);

                // Extract timestamp from the execution data
                let execution: ContractExecutionRecord = bincode::deserialize(value)?;

                sqlx::query(&format!(
                    "INSERT INTO {}.execution_history (execution_key, contract_address, execution_id, data, timestamp)
                     VALUES ($1, $2, $3, $4, $5)",
                    self.config.schema
                ))
                .bind(key)
                .bind(contract_address)
                .bind(execution_id)
                .bind(value)
                .bind(execution.timestamp as i64)
                .execute(&self.pool)
                .await
            }
            _ => return Err(anyhow::anyhow!("Unknown table: {}", table)),
        };

        let mut conn_count = self.active_connections.write().await;
        *conn_count -= 1;

        result?;
        Ok(())
    }

    pub async fn select(&self, table: &str, key: &str) -> Result<Option<Vec<u8>>> {
        let mut conn_count = self.active_connections.write().await;
        *conn_count += 1;
        drop(conn_count);

        let result = match table {
            "contracts" => sqlx::query(&format!(
                "SELECT data FROM {}.contracts WHERE address = $1",
                self.config.schema
            ))
            .bind(key)
            .fetch_optional(&self.pool)
            .await?
            .map(|row| row.get::<Vec<u8>, _>("data")),
            "contract_state" => sqlx::query(&format!(
                "SELECT value FROM {}.contract_state WHERE state_key = $1",
                self.config.schema
            ))
            .bind(key)
            .fetch_optional(&self.pool)
            .await?
            .map(|row| row.get::<Vec<u8>, _>("value")),
            _ => return Err(anyhow::anyhow!("Unknown table: {}", table)),
        };

        let mut conn_count = self.active_connections.write().await;
        *conn_count -= 1;

        Ok(result)
    }

    pub async fn delete(&self, table: &str, key: &str) -> Result<()> {
        let mut conn_count = self.active_connections.write().await;
        *conn_count += 1;
        drop(conn_count);

        let result = match table {
            "contracts" => {
                sqlx::query(&format!(
                    "DELETE FROM {}.contracts WHERE address = $1",
                    self.config.schema
                ))
                .bind(key)
                .execute(&self.pool)
                .await
            }
            "contract_state" => {
                sqlx::query(&format!(
                    "DELETE FROM {}.contract_state WHERE state_key = $1",
                    self.config.schema
                ))
                .bind(key)
                .execute(&self.pool)
                .await
            }
            _ => return Err(anyhow::anyhow!("Unknown table: {}", table)),
        };

        let mut conn_count = self.active_connections.write().await;
        *conn_count -= 1;

        result?;
        Ok(())
    }

    pub async fn list_keys(&self, table: &str) -> Result<Vec<String>> {
        let mut conn_count = self.active_connections.write().await;
        *conn_count += 1;
        drop(conn_count);

        let result = match table {
            "contracts" => {
                let rows = sqlx::query(&format!(
                    "SELECT address FROM {}.contracts ORDER BY address",
                    self.config.schema
                ))
                .fetch_all(&self.pool)
                .await?;

                rows.into_iter()
                    .map(|row| row.get::<String, _>("address"))
                    .collect()
            }
            "contract_state" => {
                let rows = sqlx::query(&format!(
                    "SELECT DISTINCT contract_address FROM {}.contract_state ORDER BY contract_address",
                    self.config.schema
                ))
                .fetch_all(&self.pool)
                .await?;

                rows.into_iter()
                    .map(|row| row.get::<String, _>("contract_address"))
                    .collect()
            }
            _ => return Err(anyhow::anyhow!("Unknown table: {}", table)),
        };

        let mut conn_count = self.active_connections.write().await;
        *conn_count -= 1;

        Ok(result)
    }

    pub async fn get_executions_for_contract(
        &self,
        contract: &str,
    ) -> Result<Vec<ContractExecutionRecord>> {
        let mut conn_count = self.active_connections.write().await;
        *conn_count += 1;
        drop(conn_count);

        let rows = sqlx::query(&format!(
            "SELECT data FROM {}.execution_history
             WHERE contract_address = $1
             ORDER BY timestamp DESC",
            self.config.schema
        ))
        .bind(contract)
        .fetch_all(&self.pool)
        .await?;

        let mut executions = Vec::new();
        for row in rows {
            let data: Vec<u8> = row.get("data");
            if let Ok(execution) = bincode::deserialize::<ContractExecutionRecord>(&data) {
                executions.push(execution);
            }
        }

        let mut conn_count = self.active_connections.write().await;
        *conn_count -= 1;

        Ok(executions)
    }

    pub async fn get_connection_count(&self) -> u32 {
        *self.active_connections.read().await
    }

    pub async fn check_health(&self) -> Result<()> {
        sqlx::query("SELECT 1").fetch_one(&self.pool).await?;
        Ok(())
    }

    pub async fn get_database_info(&self) -> Result<PostgresDatabaseInfo> {
        // Get database size
        let size_result = sqlx::query(&format!(
            "SELECT pg_database_size('{}') as size",
            self.config.database
        ))
        .fetch_one(&self.pool)
        .await?;
        let size_bytes: i64 = size_result.get("size");

        // Get contracts count
        let contracts_result = sqlx::query(&format!(
            "SELECT COUNT(*) as count FROM {}.contracts",
            self.config.schema
        ))
        .fetch_one(&self.pool)
        .await?;
        let contracts_count: i64 = contracts_result.get("count");

        // Get state entries count
        let state_result = sqlx::query(&format!(
            "SELECT COUNT(*) as count FROM {}.contract_state",
            self.config.schema
        ))
        .fetch_one(&self.pool)
        .await?;
        let state_entries_count: i64 = state_result.get("count");

        // Get executions count
        let executions_result = sqlx::query(&format!(
            "SELECT COUNT(*) as count FROM {}.execution_history",
            self.config.schema
        ))
        .fetch_one(&self.pool)
        .await?;
        let executions_count: i64 = executions_result.get("count");

        Ok(PostgresDatabaseInfo {
            size_bytes: size_bytes as u64,
            contracts_count: contracts_count as usize,
            state_entries_count: state_entries_count as usize,
            executions_count: executions_count as usize,
        })
    }
}

impl RedisConnectionPool {
    pub async fn new(config: RedisConfig) -> Result<Self> {
        let client = RedisClient::open(config.url.clone())?;
        let manager = ConnectionManager::new(client).await?;

        // Test connection
        let mut conn = manager.clone();
        if let Some(ref password) = config.password {
            redis::cmd("AUTH")
                .arg(password)
                .query_async::<_, ()>(&mut conn)
                .await?;
        }

        // Select database
        redis::cmd("SELECT")
            .arg(config.database)
            .query_async::<_, ()>(&mut conn)
            .await?;

        Ok(Self {
            manager,
            config,
            active_connections: Arc::new(RwLock::new(0)),
        })
    }

    pub async fn set(&self, key: &str, value: &[u8]) -> Result<()> {
        let mut conn_count = self.active_connections.write().await;
        *conn_count += 1;
        drop(conn_count);

        let mut conn = self.manager.clone();
        let prefixed_key = format!("{}{}", self.config.key_prefix, key);

        let result = if let Some(ttl) = self.config.ttl_seconds {
            conn.set_ex(&prefixed_key, value, ttl).await
        } else {
            conn.set(&prefixed_key, value).await
        };

        let mut conn_count = self.active_connections.write().await;
        *conn_count -= 1;

        result.map_err(|e| anyhow::anyhow!("Redis SET error: {}", e))
    }

    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let mut conn_count = self.active_connections.write().await;
        *conn_count += 1;
        drop(conn_count);

        let mut conn = self.manager.clone();
        let prefixed_key = format!("{}{}", self.config.key_prefix, key);

        let result: Option<Vec<u8>> = conn
            .get(&prefixed_key)
            .await
            .map_err(|e| anyhow::anyhow!("Redis GET error: {}", e))?;

        let mut conn_count = self.active_connections.write().await;
        *conn_count -= 1;

        Ok(result)
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        let mut conn_count = self.active_connections.write().await;
        *conn_count += 1;
        drop(conn_count);

        let mut conn = self.manager.clone();
        let prefixed_key = format!("{}{}", self.config.key_prefix, key);

        let _: () = conn
            .del(&prefixed_key)
            .await
            .map_err(|e| anyhow::anyhow!("Redis DEL error: {}", e))?;

        let mut conn_count = self.active_connections.write().await;
        *conn_count -= 1;

        Ok(())
    }

    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn_count = self.active_connections.write().await;
        *conn_count += 1;
        drop(conn_count);

        let mut conn = self.manager.clone();
        let prefixed_key = format!("{}{}", self.config.key_prefix, key);

        let result: bool = conn
            .exists(&prefixed_key)
            .await
            .map_err(|e| anyhow::anyhow!("Redis EXISTS error: {}", e))?;

        let mut conn_count = self.active_connections.write().await;
        *conn_count -= 1;

        Ok(result)
    }

    pub async fn keys(&self, pattern: &str) -> Result<Vec<String>> {
        let mut conn_count = self.active_connections.write().await;
        *conn_count += 1;
        drop(conn_count);

        let mut conn = self.manager.clone();
        let prefixed_pattern = format!("{}{}", self.config.key_prefix, pattern);

        let keys: Vec<String> = conn
            .keys(&prefixed_pattern)
            .await
            .map_err(|e| anyhow::anyhow!("Redis KEYS error: {}", e))?;

        let mut conn_count = self.active_connections.write().await;
        *conn_count -= 1;

        // Remove prefix from returned keys
        let prefix_len = self.config.key_prefix.len();
        Ok(keys
            .into_iter()
            .map(|k| k[prefix_len..].to_string())
            .collect())
    }

    pub async fn get_connection_count(&self) -> u32 {
        *self.active_connections.read().await
    }

    pub async fn flush_db(&self) -> Result<()> {
        let mut conn = self.manager.clone();
        redis::cmd("FLUSHDB")
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!("Redis FLUSHDB error: {}", e))?;
        Ok(())
    }

    pub async fn check_health(&self) -> Result<()> {
        let mut conn = self.manager.clone();
        redis::cmd("PING")
            .query_async::<_, String>(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!("Redis PING error: {}", e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smart_contract::unified_engine::ContractType;

    fn create_test_metadata() -> UnifiedContractMetadata {
        UnifiedContractMetadata {
            address: "0xtest123".to_string(),
            name: "Test Contract".to_string(),
            description: "A test contract".to_string(),
            contract_type: ContractType::Wasm {
                bytecode: vec![1, 2, 3],
                abi: Some("test_abi".to_string()),
            },
            deployment_tx: "0xdeployment".to_string(),
            deployment_time: 1234567890,
            owner: "0xowner".to_string(),
            is_active: true,
        }
    }

    #[tokio::test]
    async fn test_database_storage_creation() {
        let storage = DatabaseContractStorage::testing();
        let stats = storage.get_stats().await;

        assert_eq!(stats.postgres_connections, 0);
        assert_eq!(stats.redis_connections, 0);
        assert_eq!(stats.total_queries, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_contract_metadata_fallback() {
        let storage = DatabaseContractStorage::testing();
        let metadata = create_test_metadata();

        // Store metadata (should use memory fallback)
        storage.store_contract_metadata(&metadata).unwrap();

        // Retrieve metadata (should hit memory fallback)
        let retrieved = storage.get_contract_metadata(&metadata.address).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, metadata.name);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_contract_state_operations() {
        let storage = DatabaseContractStorage::testing();

        // Set contract state
        storage
            .set_contract_state("0xcontract", "test_key", b"test_value")
            .unwrap();

        // Get contract state
        let value = storage
            .get_contract_state("0xcontract", "test_key")
            .unwrap();
        assert_eq!(value, Some(b"test_value".to_vec()));

        // Delete contract state
        storage
            .delete_contract_state("0xcontract", "test_key")
            .unwrap();

        // Verify deletion
        let value = storage
            .get_contract_state("0xcontract", "test_key")
            .unwrap();
        assert!(value.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_execution_history() {
        let storage = DatabaseContractStorage::testing();

        let execution = ContractExecutionRecord {
            execution_id: "exec_1".to_string(),
            contract_address: "0xcontract".to_string(),
            function_name: "test_function".to_string(),
            caller: "0xcaller".to_string(),
            timestamp: 1234567890,
            gas_used: 50000,
            success: true,
            error_message: None,
        };

        // Store execution
        storage.store_execution(&execution).unwrap();

        // Retrieve execution history
        let history = storage.get_execution_history("0xcontract").unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].execution_id, execution.execution_id);
    }

    #[tokio::test]
    async fn test_config_defaults() {
        let config = DatabaseStorageConfig::default();
        assert!(config.fallback_to_memory);
        assert_eq!(config.connection_timeout_secs, 30);
        assert_eq!(config.max_connections, 20);

        let pg_config = PostgresConfig::default();
        assert_eq!(pg_config.host, "localhost");
        assert_eq!(pg_config.port, 5432);
        assert_eq!(pg_config.database, "polytorus");

        let redis_config = RedisConfig::default();
        assert_eq!(redis_config.url, "redis://localhost:6379");
        assert_eq!(redis_config.database, 0);
        assert!(redis_config.ttl_seconds.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_connection_stats() {
        let storage = DatabaseContractStorage::testing();

        // Initial stats should be zero
        let stats = storage.get_stats().await;
        assert_eq!(stats.total_queries, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.cache_misses, 0);

        // Perform some operations that would update stats
        storage
            .set_contract_state("0xcontract", "key1", b"value1")
            .unwrap();
        let _ = storage.get_contract_state("0xcontract", "key1").unwrap();
        let _ = storage
            .get_contract_state("0xcontract", "nonexistent")
            .unwrap();

        // Note: In this test implementation, stats are only updated for actual Redis/PostgreSQL operations
        // Since we're using memory fallback, stats remain at 0
        let final_stats = storage.get_stats().await;
        assert_eq!(final_stats.total_queries, 0); // Would be > 0 with real databases
    }
}

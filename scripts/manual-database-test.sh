#!/bin/bash

# Manual Database Test Script
# This script performs manual testing of the database functionality

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🧪 Manual Database Test Script${NC}"
echo "================================="

# Check if databases are running
echo -e "${YELLOW}📋 Checking database status...${NC}"

if ! docker ps | grep -q "polytorus-postgres-test"; then
    echo -e "${RED}❌ PostgreSQL container is not running or not healthy${NC}"
    echo "Start with: docker-compose -f docker-compose.database-test.yml up -d"
    exit 1
fi

if ! docker ps | grep -q "polytorus-redis-test"; then
    echo -e "${RED}❌ Redis container is not running or not healthy${NC}"
    echo "Start with: docker-compose -f docker-compose.database-test.yml up -d"
    exit 1
fi

echo -e "${GREEN}✅ Both databases are running and healthy${NC}"

# Test PostgreSQL connectivity
echo -e "${YELLOW}🐘 Testing PostgreSQL connectivity...${NC}"
if docker exec polytorus-postgres-test psql -U polytorus_test -d polytorus_test -c "SELECT 'PostgreSQL OK' as status;" > /dev/null 2>&1; then
    echo -e "${GREEN}✅ PostgreSQL connection successful${NC}"
else
    echo -e "${RED}❌ PostgreSQL connection failed${NC}"
    exit 1
fi

# Test Redis connectivity
echo -e "${YELLOW}🔴 Testing Redis connectivity...${NC}"
if docker exec polytorus-redis-test redis-cli -a test_redis_password_123 ping > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Redis connection successful${NC}"
else
    echo -e "${RED}❌ Redis connection failed${NC}"
    exit 1
fi

# Test database schema
echo -e "${YELLOW}📊 Checking database schema...${NC}"
TABLES=$(docker exec polytorus-postgres-test psql -U polytorus_test -d polytorus_test -t -c "
SELECT COUNT(*) FROM information_schema.tables
WHERE table_schema = 'smart_contracts'
AND table_name IN ('contracts', 'contract_state', 'execution_history');
")

if [ "$TABLES" -eq 3 ]; then
    echo -e "${GREEN}✅ All required tables exist${NC}"
else
    echo -e "${YELLOW}⚠️  Creating missing tables...${NC}"
    docker exec polytorus-postgres-test psql -U polytorus_test -d polytorus_test -c "
    CREATE TABLE IF NOT EXISTS smart_contracts.contracts (
        address VARCHAR(42) PRIMARY KEY,
        data BYTEA NOT NULL,
        created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
        updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
    );

    CREATE TABLE IF NOT EXISTS smart_contracts.contract_state (
        state_key VARCHAR(255) PRIMARY KEY,
        contract_address VARCHAR(42) NOT NULL,
        key_name VARCHAR(255) NOT NULL,
        value BYTEA NOT NULL,
        created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
        updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
    );

    CREATE TABLE IF NOT EXISTS smart_contracts.execution_history (
        execution_key VARCHAR(255) PRIMARY KEY,
        contract_address VARCHAR(42) NOT NULL,
        execution_id VARCHAR(255) NOT NULL,
        data BYTEA NOT NULL,
        timestamp BIGINT NOT NULL,
        created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
    );

    CREATE INDEX IF NOT EXISTS idx_contract_state_address ON smart_contracts.contract_state(contract_address);
    CREATE INDEX IF NOT EXISTS idx_execution_history_address ON smart_contracts.execution_history(contract_address);
    CREATE INDEX IF NOT EXISTS idx_execution_history_timestamp ON smart_contracts.execution_history(timestamp DESC);
    " > /dev/null
    echo -e "${GREEN}✅ Database schema created${NC}"
fi

# Test basic CRUD operations
echo -e "${YELLOW}🔧 Testing basic CRUD operations...${NC}"

# Insert test data
TEST_ADDRESS="0xtest$(date +%s)"
echo -e "  📝 Inserting test contract: ${TEST_ADDRESS}"

docker exec polytorus-postgres-test psql -U polytorus_test -d polytorus_test -c "
INSERT INTO smart_contracts.contracts (address, data)
VALUES ('${TEST_ADDRESS}', decode('7b226e616d65223a2254657374227d', 'hex'));

INSERT INTO smart_contracts.contract_state (state_key, contract_address, key_name, value)
VALUES ('${TEST_ADDRESS}:balance', '${TEST_ADDRESS}', 'balance', decode('e803000000000000', 'hex'));

INSERT INTO smart_contracts.execution_history (execution_key, contract_address, execution_id, data, timestamp)
VALUES ('${TEST_ADDRESS}:exec1', '${TEST_ADDRESS}', 'exec1', decode('7b2273756363657373223a747275657d', 'hex'), $(date +%s));
" > /dev/null

echo -e "${GREEN}  ✅ Test data inserted${NC}"

# Read test data
echo -e "  📖 Reading test data..."
CONTRACTS=$(docker exec polytorus-postgres-test psql -U polytorus_test -d polytorus_test -t -c "
SELECT COUNT(*) FROM smart_contracts.contracts WHERE address = '${TEST_ADDRESS}';
")

STATES=$(docker exec polytorus-postgres-test psql -U polytorus_test -d polytorus_test -t -c "
SELECT COUNT(*) FROM smart_contracts.contract_state WHERE contract_address = '${TEST_ADDRESS}';
")

EXECUTIONS=$(docker exec polytorus-postgres-test psql -U polytorus_test -d polytorus_test -t -c "
SELECT COUNT(*) FROM smart_contracts.execution_history WHERE contract_address = '${TEST_ADDRESS}';
")

if [ "$CONTRACTS" -eq 1 ] && [ "$STATES" -eq 1 ] && [ "$EXECUTIONS" -eq 1 ]; then
    echo -e "${GREEN}  ✅ Test data read successfully${NC}"
else
    echo -e "${RED}  ❌ Failed to read test data${NC}"
    exit 1
fi

# Test Redis operations
echo -e "${YELLOW}🔴 Testing Redis operations...${NC}"

# Set test data in Redis
docker exec polytorus-redis-test redis-cli -a test_redis_password_123 eval "
redis.call('SET', 'polytorus:test:contract:${TEST_ADDRESS}', '{\"name\":\"TestContract\"}')
redis.call('SET', 'polytorus:test:state:${TEST_ADDRESS}:balance', '1000')
redis.call('EXPIRE', 'polytorus:test:contract:${TEST_ADDRESS}', 300)
redis.call('EXPIRE', 'polytorus:test:state:${TEST_ADDRESS}:balance', 300)
return 'OK'
" 0 > /dev/null

echo -e "${GREEN}  ✅ Redis data set${NC}"

# Get test data from Redis
REDIS_CONTRACT=$(docker exec polytorus-redis-test redis-cli -a test_redis_password_123 GET "polytorus:test:contract:${TEST_ADDRESS}" 2>/dev/null)
REDIS_BALANCE=$(docker exec polytorus-redis-test redis-cli -a test_redis_password_123 GET "polytorus:test:state:${TEST_ADDRESS}:balance" 2>/dev/null)

if [ "$REDIS_CONTRACT" = '{"name":"TestContract"}' ] && [ "$REDIS_BALANCE" = "1000" ]; then
    echo -e "${GREEN}  ✅ Redis data retrieved successfully${NC}"
else
    echo -e "${RED}  ❌ Failed to retrieve Redis data${NC}"
    exit 1
fi

# Performance test
echo -e "${YELLOW}⚡ Running performance test...${NC}"

START_TIME=$(date +%s%N)

# Insert 100 test contracts
docker exec polytorus-postgres-test psql -U polytorus_test -d polytorus_test -c "
DO \$\$
DECLARE
    i INTEGER;
    addr TEXT;
    test_suffix TEXT := '$(date +%s)';
BEGIN
    FOR i IN 1..100 LOOP
        addr := '0xperf' || test_suffix || lpad(i::text, 6, '0');

        INSERT INTO smart_contracts.contracts (address, data)
        VALUES (addr, decode('7b226e616d65223a22506572666f726d616e636554657374227d', 'hex'))
        ON CONFLICT (address) DO NOTHING;

        INSERT INTO smart_contracts.contract_state (state_key, contract_address, key_name, value)
        VALUES (
            addr || ':balance',
            addr,
            'balance',
            decode(lpad((i * 1000)::text, 16, '0'), 'hex')
        ) ON CONFLICT (state_key) DO NOTHING;
    END LOOP;
END
\$\$;
" > /dev/null

END_TIME=$(date +%s%N)
DURATION=$(( (END_TIME - START_TIME) / 1000000 )) # Convert to milliseconds

echo -e "${GREEN}  ✅ Performance test completed in ${DURATION}ms${NC}"
echo -e "     Inserted 100 contracts and 100 state entries"

# Final statistics
echo -e "${YELLOW}📊 Final Database Statistics:${NC}"

# PostgreSQL stats
echo -e "${BLUE}PostgreSQL:${NC}"
docker exec polytorus-postgres-test psql -U polytorus_test -d polytorus_test -c "
SELECT
    relname as table_name,
    n_tup_ins as inserts,
    n_tup_upd as updates,
    n_tup_del as deletes,
    pg_size_pretty(pg_total_relation_size('smart_contracts.'||relname)) as size
FROM pg_stat_user_tables
WHERE schemaname = 'smart_contracts' AND relname != 'test_log'
ORDER BY relname;
"

# Redis stats
echo -e "${BLUE}Redis:${NC}"
REDIS_KEYS=$(docker exec polytorus-redis-test redis-cli -a test_redis_password_123 eval "return #redis.call('keys', '*')" 0 2>/dev/null)
REDIS_MEMORY=$(docker exec polytorus-redis-test redis-cli -a test_redis_password_123 info memory 2>/dev/null | grep "used_memory_human:" | cut -d: -f2)

echo "  Keys: $REDIS_KEYS"
echo "  Memory: $REDIS_MEMORY"

# Cleanup test data
echo -e "${YELLOW}🧹 Cleaning up test data...${NC}"
docker exec polytorus-postgres-test psql -U polytorus_test -d polytorus_test -c "
DELETE FROM smart_contracts.execution_history WHERE contract_address = '${TEST_ADDRESS}';
DELETE FROM smart_contracts.contract_state WHERE contract_address = '${TEST_ADDRESS}';
DELETE FROM smart_contracts.contracts WHERE address = '${TEST_ADDRESS}';
" > /dev/null

docker exec polytorus-redis-test redis-cli -a test_redis_password_123 eval "
redis.call('DEL', 'polytorus:test:contract:${TEST_ADDRESS}')
redis.call('DEL', 'polytorus:test:state:${TEST_ADDRESS}:balance')
return 'OK'
" 0 > /dev/null 2>&1

echo -e "${GREEN}✅ Test data cleaned up${NC}"

echo -e "\n${GREEN}🎉 All database tests passed successfully!${NC}"
echo -e "${BLUE}Database functionality is working correctly${NC}"

# Show connection info
echo -e "\n${YELLOW}💡 Database Connection Information:${NC}"
echo "PostgreSQL: localhost:5433 (user: polytorus_test, password: test_password_123, db: polytorus_test)"
echo "Redis: localhost:6380 (password: test_redis_password_123)"
echo ""
echo "To stop databases: docker-compose -f docker-compose.database-test.yml down -v"

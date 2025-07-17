#!/bin/bash

# Database Integration Test Runner
# This script sets up the test environment and runs database integration tests

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Polytorus Database Integration Test Runner${NC}"
echo "=============================================="

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check prerequisites
echo -e "${YELLOW}📋 Checking prerequisites...${NC}"

if ! command_exists docker; then
    echo -e "${RED}❌ Docker is not installed${NC}"
    exit 1
fi

if ! command_exists docker-compose; then
    echo -e "${RED}❌ Docker Compose is not installed${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Prerequisites check passed${NC}"

# Start databases
echo -e "${YELLOW}🐳 Starting test databases...${NC}"
docker-compose -f docker-compose.database-test.yml down -v 2>/dev/null || true
docker-compose -f docker-compose.database-test.yml up -d

# Wait for databases to be healthy
echo -e "${YELLOW}⏳ Waiting for databases to be healthy...${NC}"
timeout=60
counter=0

while [ $counter -lt $timeout ]; do
    if docker-compose -f docker-compose.database-test.yml ps | grep -q "healthy"; then
        postgres_healthy=$(docker-compose -f docker-compose.database-test.yml ps postgres | grep -c "healthy" || echo "0")
        redis_healthy=$(docker-compose -f docker-compose.database-test.yml ps redis | grep -c "healthy" || echo "0")

        if [ "$postgres_healthy" -eq 1 ] && [ "$redis_healthy" -eq 1 ]; then
            echo -e "${GREEN}✅ Databases are healthy${NC}"
            break
        fi
    fi

    echo -n "."
    sleep 2
    counter=$((counter + 2))
done

if [ $counter -ge $timeout ]; then
    echo -e "${RED}❌ Databases failed to become healthy within ${timeout} seconds${NC}"
    echo "Container status:"
    docker-compose -f docker-compose.database-test.yml ps
    echo "PostgreSQL logs:"
    docker-compose -f docker-compose.database-test.yml logs postgres
    echo "Redis logs:"
    docker-compose -f docker-compose.database-test.yml logs redis
    exit 1
fi

# Show database status
echo -e "${BLUE}📊 Database Status:${NC}"
docker-compose -f docker-compose.database-test.yml ps

# Test database connections
echo -e "${YELLOW}🔍 Testing database connections...${NC}"

# Test PostgreSQL
if docker exec polytorus-postgres-test psql -U polytorus_test -d polytorus_test -c "SELECT 'PostgreSQL connection successful' AS status;" > /dev/null 2>&1; then
    echo -e "${GREEN}✅ PostgreSQL connection successful${NC}"
else
    echo -e "${RED}❌ PostgreSQL connection failed${NC}"
    exit 1
fi

# Test Redis
if docker exec polytorus-redis-test redis-cli -a test_redis_password_123 ping > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Redis connection successful${NC}"
else
    echo -e "${RED}❌ Redis connection failed${NC}"
    exit 1
fi

# Run the integration tests
echo -e "${YELLOW}🧪 Running integration tests...${NC}"

# Set environment variables for tests
export RUST_LOG=info
export RUST_BACKTRACE=1

# Run specific test categories
echo -e "${BLUE}Running connectivity tests...${NC}"
cargo test --test database_integration_tests test_database_connectivity -- --ignored --nocapture

echo -e "${BLUE}Running metadata tests...${NC}"
cargo test --test database_integration_tests test_contract_metadata_operations -- --ignored --nocapture

echo -e "${BLUE}Running state tests...${NC}"
cargo test --test database_integration_tests test_contract_state_operations -- --ignored --nocapture

echo -e "${BLUE}Running execution history tests...${NC}"
cargo test --test database_integration_tests test_execution_history -- --ignored --nocapture

echo -e "${BLUE}Running cache tests...${NC}"
cargo test --test database_integration_tests test_cache_behavior -- --ignored --nocapture

echo -e "${BLUE}Running monitoring tests...${NC}"
cargo test --test database_integration_tests test_database_info_and_monitoring -- --ignored --nocapture

echo -e "${BLUE}Running performance tests...${NC}"
cargo test --test database_integration_tests test_performance_and_concurrency -- --ignored --nocapture

echo -e "${BLUE}Running failover tests...${NC}"
cargo test --test database_integration_tests test_failover_behavior -- --ignored --nocapture

# Run full integration test
echo -e "${BLUE}Running full integration test...${NC}"
cargo test --test database_integration_tests test_full_integration -- --ignored --nocapture

echo -e "${GREEN}🎉 All tests completed successfully!${NC}"

# Show final database statistics
echo -e "${BLUE}📈 Final Database Statistics:${NC}"
echo "PostgreSQL:"
docker exec polytorus-postgres-test psql -U polytorus_test -d polytorus_test -c "
SELECT
    schemaname,
    tablename,
    n_tup_ins as inserts,
    n_tup_upd as updates,
    n_tup_del as deletes
FROM pg_stat_user_tables
WHERE schemaname = 'smart_contracts';"

echo "Redis:"
docker exec polytorus-redis-test redis-cli -a test_redis_password_123 info keyspace

# Option to keep databases running
echo -e "${YELLOW}💡 Databases are still running for manual testing${NC}"
echo "PostgreSQL: localhost:5433 (user: polytorus_test, password: test_password_123, db: polytorus_test)"
echo "Redis: localhost:6380 (password: test_redis_password_123)"
echo ""
echo "To access web interfaces (if debug profile is enabled):"
echo "pgAdmin: http://localhost:8080 (admin@polytorus.test / admin_password_123)"
echo "Redis Commander: http://localhost:8081"
echo ""
echo -e "${YELLOW}To stop databases: docker-compose -f docker-compose.database-test.yml down -v${NC}"

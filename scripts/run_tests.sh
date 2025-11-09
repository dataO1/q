#!/bin/bash
set -e

echo "🚀 Starting test infrastructure..."

# Start test services
docker-compose -f docker-compose.test.yml up -d

# Simple wait for services (without docker-compose wait)
echo "⏳ Waiting for services to be ready..."

# Wait for PostgreSQL
echo "  - Waiting for PostgreSQL..."
until docker exec q-postgres-test-1 pg_isready -U test_user -d ai_agent_test > /dev/null 2>&1; do
    sleep 1
done

# Wait for Redis
echo "  - Waiting for Redis..."
until docker exec q-redis-test-1 redis-cli ping > /dev/null 2>&1; do
    sleep 1
done

# Wait for Qdrant
echo "  - Waiting for Qdrant..."
until curl -s http://localhost:16333/healthz > /dev/null 2>&1; do
    sleep 1
done

echo "✅ Test infrastructure ready!"
echo ""

# Load test environment variables
export $(cat .env.test | xargs)

echo "Running PostgreSQL tests..."
cargo test -p ai-agent-storage --test postgres_test -- --ignored --test-threads=1

echo ""
echo "Running Redis tests..."
cargo test -p ai-agent-storage --test redis_test -- --ignored --test-threads=1

echo ""
echo "Running Qdrant tests..."
cargo test -p ai-agent-storage --test qdrant_test -- --ignored --test-threads=1

echo ""
echo "Running common crate tests..."
cargo test -p ai-agent-common

# Cleanup
echo ""
echo "🧹 Cleaning up test infrastructure..."
docker-compose -f docker-compose.test.yml down -v

echo "✅ All tests complete!"

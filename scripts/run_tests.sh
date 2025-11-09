#!/bin/bash
set -e

echo "🚀 Starting test infrastructure..."

# Start test services
docker-compose -f docker-compose.test.yml up -d

# Wait for services to be ready
echo "⏳ Waiting for services to be ready..."
sleep 5

# Load test environment variables
export $(cat .env.test | xargs)

echo "✅ Test infrastructure ready!"
echo ""
echo "Running tests..."

# Run tests
cargo test --workspace -- --ignored --test-threads=1

# Cleanup
echo ""
echo "🧹 Cleaning up test infrastructure..."
docker-compose -f docker-compose.test.yml down -v

echo "✅ Tests complete!"

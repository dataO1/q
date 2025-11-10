#!/bin/bash
set -e

export $(cat .env.test | xargs)

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}🧪 Testing AI Agent Indexer${NC}"
echo ""

# Step 1: Check if test services are running
echo -e "${BLUE}📋 Step 1: Checking test services...${NC}"

if ! curl -s http://localhost:16333 > /dev/null; then
    echo -e "${YELLOW}⚠️  Test Qdrant not running on :16333${NC}"
    echo -e "${BLUE}Starting test services...${NC}"
    docker-compose up -d
    sleep 5
fi

if curl -s http://localhost:16333 > /dev/null; then
    echo -e "${GREEN}✅ Test Qdrant ready${NC}"
else
    echo -e "${RED}❌ Failed to start test Qdrant${NC}"
    exit 1
fi

# if redis-cli -p 16379 ping > /dev/null 2>&1; then
#     echo -e "${GREEN}✅ Test Redis ready${NC}"
# else
#     echo -e "${RED}❌ Test Redis not available${NC}"
#     exit 1
# fi

# Step 2: Check Ollama
echo -e "${BLUE}📋 Step 2: Checking Ollama...${NC}"
if curl -s http://localhost:11434/api/tags > /dev/null; then
    echo -e "${GREEN}✅ Ollama ready${NC}"
else
    echo -e "${RED}❌ Ollama not running. Start it with: ollama serve${NC}"
    exit 1
fi

# Step 3: Create test directories and files
echo -e "${BLUE}📋 Step 3: Creating test workspace...${NC}"
mkdir -p test-workspace/src test-notes

cat > test-workspace/src/main.rs << 'EOF'
/// Main entry point for the application
fn main() {
    println!("Hello from AI Agent!");
}

/// Calculate the sum of two numbers
fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Multiply two numbers
fn multiply(a: i32, b: i32) -> i32 {
    a * b
}
EOF

cat > test-workspace/src/utils.py << 'EOF'
"""Utility functions for the project"""

def greet(name: str) -> str:
    """Greet someone by name"""
    return f"Hello, {name}!"

class Calculator:
    """A simple calculator class"""

    def __init__(self):
        self.result = 0

    def add(self, x: int, y: int) -> int:
        """Add two numbers"""
        self.result = x + y
        return self.result
EOF

cat > test-notes/README.md << 'EOF'
# Test Notes

This is a test markdown file for the AI Agent indexer.

## Features
- Automatic indexing
- Hybrid search (dense + sparse vectors)
- Redis caching for speed

## How it works
Files are watched for changes and automatically indexed.
EOF

echo -e "${GREEN}✅ Test files created${NC}"
echo ""

# Step 4: Build the indexer
echo -e "${BLUE}📋 Step 4: Building indexer...${NC}"
cargo build -p ai-agent-indexing --bin indexer
echo -e "${GREEN}✅ Build complete${NC}"
echo ""

# Step 5: Run the indexer
echo -e "${BLUE}📋 Step 5: Starting indexer (Press Ctrl+C to stop)${NC}"
echo ""
echo -e "${YELLOW}Try editing files in test-workspace/ to see live indexing!${NC}"
echo ""

RUST_LOG=info cargo run -p ai-agent-indexing --bin indexer -- --config config.dev.toml

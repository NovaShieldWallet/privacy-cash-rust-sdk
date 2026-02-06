#!/bin/bash
# Privacy Cash - Send Privately (Presentation Mode)
# Usage: ./send.sh <amount> <token> [recipient]
# Example: ./send.sh 0.02 sol DZk343QuEFUNFWiuaMBQ41NLRgNezM72ypZzaMQ9rFTS

# Load .env.local if exists
if [ -f .env.local ]; then
    export $(grep -v '^#' .env.local | xargs)
fi

# Check for private key
if [ -z "$SOLANA_PRIVATE_KEY" ]; then
    echo "Error: SOLANA_PRIVATE_KEY not set"
    echo "Run: export SOLANA_PRIVATE_KEY=<your-key>"
    exit 1
fi

# Run the pre-built binary directly (no compile output)
./target/release/examples/send_privately "$@"

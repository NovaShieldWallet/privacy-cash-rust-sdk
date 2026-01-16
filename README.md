# Privacy Cash Rust SDK

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Pure Rust** SDK for [Privacy Cash](https://privacycash.org) - Privacy-preserving transactions on Solana using Zero-Knowledge Proofs.

**iOS Compatible** - No Node.js required!

**Created by [Nova Shield](https://nshield.org)**

## Features

- Private Transactions - Send SOL and SPL tokens with complete privacy
- Pure Rust ZK Proofs - Native Groth16 proof generation
- iOS Compatible - Use as a Rust crate in mobile apps
- Multi-Token Support - SOL, USDC, USDT
- One Function API - `send_privately()` does everything

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
privacy-cash = { git = "https://github.com/NovaShieldWallet/privacy-cash-rust-sdk" }
tokio = { version = "1", features = ["full"] }
```

### Circuit Files (Required)

The SDK requires circuit files for ZK proof generation. Create a `circuit` directory and add the required files:

```bash
mkdir -p circuit
# Add transaction2.wasm and transaction2.zkey to the circuit directory
```

**Note:** Contact [Nova Shield](https://nshield.org) or check the project releases for circuit file distribution.

## Quick Start - ONE Function!

```rust
use privacy_cash::send_privately;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Send 0.1 SOL privately - ONE function does everything!
    let result = send_privately(
        "your_base58_private_key",  // Private key
        "recipient_pubkey",          // Recipient address
        0.1,                         // Amount to send
        "sol",                       // Token: "sol", "usdc", "usdt"
        None,                        // Optional RPC URL
    ).await?;
    
    println!("Sent privately!");
    println!("Deposit TX: {}", result.deposit_signature);
    println!("Withdraw TX: {}", result.withdraw_signature);
    println!("Recipient received: {} lamports", result.amount_received);
    
    Ok(())
}
```

The `send_privately()` function automatically:
1. Deposits your tokens into Privacy Cash
2. Waits for blockchain confirmation
3. Withdraws the maximum amount to the recipient

## API

### SendPrivatelyResult

```rust
pub struct SendPrivatelyResult {
    pub deposit_signature: String,   // Deposit transaction
    pub withdraw_signature: String,  // Withdraw transaction
    pub amount_deposited: u64,       // Amount deposited
    pub amount_received: u64,        // Amount recipient received
    pub total_fees: u64,             // Total fees paid
    pub recipient: String,           // Recipient address
    pub token: String,               // Token type
}
```

## Supported Tokens

| Token | Minimum | Fee |
|-------|---------|-----|
| SOL   | 0.02 SOL | ~0.006 SOL |
| USDC  | 2 USDC   | ~0.85 USDC |
| USDT  | 2 USDT   | ~0.85 USDT |

## Examples

```bash
# Check balances
SOLANA_PRIVATE_KEY="your-key" cargo run --release --example check_balance

# Send 0.02 SOL privately
SOLANA_PRIVATE_KEY="your-key" cargo run --release --example send_privately -- 0.02 sol

# Send 10 USDC privately to a recipient
SOLANA_PRIVATE_KEY="your-key" cargo run --release --example send_privately -- 10 usdc RecipientPubkey
```

## Security

- Never hardcode private keys in your code
- Use environment variables or secure key management
- Private keys are used locally and never sent to any server
- All ZK proofs are generated client-side

## License

MIT License - Copyright 2024 Nova Shield

## Links

- [Nova Shield](https://nshield.org)
- [Privacy Cash Protocol](https://privacycash.org)

# Privacy Cash Rust SDK

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Pure Rust** SDK for [Privacy Cash](https://www.privacycash.org) - Privacy-preserving transactions on Solana using Zero-Knowledge Proofs.

**Created by [Nova Shield](https://nshield.org)**

## Features

- **Private Transactions** - Send SOL and SPL tokens with complete privacy using ZK proofs
- **Pure Rust ZK Proofs** - Native Groth16 proof generation, no external dependencies
- **Multi-Token Support** - SOL, USDC, USDT, and more
- **One Function API** - `send_privately()` handles deposit + withdraw in one call
- **Partner Fee Integration** - Earn fees by integrating this SDK into your platform
- **Configurable** - Customize RPC, fees, and referrer via environment variables

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
privacy-cash = { git = "https://github.com/NovaShieldWallet/privacy-cash-rust-sdk" }
tokio = { version = "1", features = ["full"] }
```

### Circuit Files (Required)

The SDK requires circuit files for ZK proof generation.

This repo vendors the required artifacts at:

- `circuit/transaction2.wasm`
- `circuit/transaction2.zkey`

See `circuit/README.md` for provenance and licensing details.

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

## Partner/Platform Fee Integration

Platforms integrating this SDK can earn fees on transactions. Configure via environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `PARTNER_FEE_WALLET` | Your wallet address to receive fees | Nova Shield wallet |
| `PARTNER_FEE_RATE` | Fee rate (e.g., "0.01" for 1%, "0" to disable) | 0.01 (1%) |
| `PARTNER_REFERRER` | Referrer wallet for Privacy Cash referral program | Nova Shield wallet |

Example setup:
```bash
export PARTNER_FEE_WALLET="YourWalletAddressHere"
export PARTNER_FEE_RATE="0.005"  # 0.5% fee
export PARTNER_REFERRER="YourWalletAddressHere"
```

## Configuration

All configuration can be set via environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `SOLANA_PRIVATE_KEY` | Base58-encoded Solana keypair | Required |
| `SOLANA_RPC_URL` | Solana RPC endpoint | Mainnet |
| `PARTNER_FEE_WALLET` | Partner fee recipient wallet | Default wallet |
| `PARTNER_FEE_RATE` | Partner fee rate (0-1) | 0.01 |
| `PARTNER_REFERRER` | Referrer for Privacy Cash | Default wallet |

## Examples

Tip: Copy `.env.local.example` to `.env.local` (gitignored) and set your variables.

```bash
# Check balances
SOLANA_PRIVATE_KEY="your-key" cargo run --release --example check_balance

# Send 0.02 SOL privately
SOLANA_PRIVATE_KEY="your-key" cargo run --release --example send_privately -- 0.02 sol

# Send 10 USDC privately to a recipient
SOLANA_PRIVATE_KEY="your-key" cargo run --release --example send_privately -- 10 usdc RecipientPubkey

# Same send test via helper script (prompts for SOLANA_PRIVATE_KEY if not set)
bash scripts/send-test.sh RecipientPubkey 0.02 sol
```

## Security

- Never hardcode private keys in your code
- Use environment variables or secure key management
- Private keys are used locally and never sent to any server
- All ZK proofs are generated client-side

## License

SDK code is MIT-licensed (see `LICENSE`). Circuit artifacts in `circuit/` have their own upstream license—see `circuit/README.md`.

## Links

- [Nova Shield](https://nshield.org)
- [Privacy Cash Protocol](https://www.privacycash.org)
- [Privacy Cash GitHub](https://github.com/Privacy-Cash/privacy-cash)
- [SDK Repository](https://github.com/NovaShieldWallet/privacy-cash-rust-sdk)

# Privacy Cash Rust SDK

[![Crates.io](https://img.shields.io/crates/v/privacy-cash.svg)](https://crates.io/crates/privacy-cash)
[![Documentation](https://docs.rs/privacy-cash/badge.svg)](https://docs.rs/privacy-cash)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Rust SDK for [Privacy Cash](https://privacycash.org) - Privacy-preserving transactions on Solana using Zero-Knowledge Proofs.

**Created by [Nova Shield](https://nshield.org)**

[![Download on App Store](https://img.shields.io/badge/Download_on_the-App_Store-black?logo=apple&logoColor=white)](https://apps.apple.com/us/app/nova-for-solana/id6753857720)

## Features

- 🔒 **Private Transactions**: Send SOL and SPL tokens with complete privacy
- 🛡️ **Zero-Knowledge Proofs**: Industry-standard ZK-SNARKs for transaction privacy
- 💰 **Multi-Token Support**: SOL, USDC, USDT, and more (dynamically fetched)
- ⚡ **Simple API**: One function `send_privately()` for privacy transfers
- 🔐 **Local Key Management**: Private keys never leave your machine
- 🔧 **Customizable**: Add your own platform fees on transactions

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
privacy-cash = "0.1"
```

### Prerequisites

**Node.js is required** for ZK proof generation:

```bash
# Install Node.js (if not installed)
# macOS: brew install node
# Ubuntu: apt install nodejs npm

# Install TypeScript bridge dependencies
cd path/to/privacy-cash-rust-sdk/ts-bridge
npm install
```

## Quick Start - Send Privately

The main function for privacy transfers:

```rust
use privacy_cash::bridge::send_privately;

fn main() {
    // Send 0.01 SOL privately
    let result = send_privately(
        "https://api.mainnet-beta.solana.com",
        "your_private_key_base58",
        10_000_000, // 0.01 SOL in lamports
        "recipient_pubkey_base58",
    ).unwrap();

    println!("Deposit TX: {}", result.deposit_signature);
    println!("Withdraw TX: {}", result.withdraw_signature);
}
```

This single function:
1. ✅ Deposits into Privacy Cash
2. ✅ Withdraws to recipient privately

## API Reference

### Send Privately (Main Function)

```rust
use privacy_cash::bridge::{send_privately, send_privately_spl};

// Send SOL privately
let result = send_privately(rpc_url, private_key, lamports, recipient)?;

// Send SPL tokens privately (e.g., USDC)
let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
let result = send_privately_spl(rpc_url, private_key, base_units, usdc_mint, recipient)?;
```

### Check Balances

```rust
use privacy_cash::bridge::{ts_get_balance, ts_get_balance_spl};

// Get private SOL balance
let balance = ts_get_balance(rpc_url, private_key)?;
println!("Private SOL: {} lamports", balance.lamports);

// Get private USDC balance
let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
let balance = ts_get_balance_spl(rpc_url, private_key, usdc_mint)?;
println!("Private USDC: {} base units", balance.base_units);
```

### Deposit & Withdraw (Individual Operations)

```rust
use privacy_cash::bridge::{
    ts_deposit, ts_withdraw, ts_withdraw_all,
    ts_deposit_spl, ts_withdraw_spl, ts_withdraw_all_spl,
};

// Deposit SOL
let result = ts_deposit(rpc_url, private_key, lamports)?;

// Withdraw SOL
let result = ts_withdraw(rpc_url, private_key, lamports, Some(recipient))?;

// Withdraw ALL private SOL
let result = ts_withdraw_all(rpc_url, private_key, None)?;

// SPL tokens
let result = ts_deposit_spl(rpc_url, private_key, base_units, mint)?;
let result = ts_withdraw_spl(rpc_url, private_key, base_units, mint, recipient)?;
let result = ts_withdraw_all_spl(rpc_url, private_key, mint, recipient)?;
```

## Examples

### Check Balances

```bash
SOLANA_PRIVATE_KEY="your-key" cargo run --example send_privately
```

### Send Privately

```bash
SOLANA_PRIVATE_KEY="your-key" cargo run --example send_privately -- 0.01 RecipientPubkey
```

### Withdraw All

```bash
SOLANA_PRIVATE_KEY="your-key" cargo run --example withdraw_all_bridge
```

## Supported Tokens (Dynamic)

Tokens are fetched dynamically from the Privacy Cash API:

| Token | Minimum Withdrawal | Rent Fee |
|-------|-------------------|----------|
| SOL   | 0.01 SOL          | ~0.006 SOL |
| USDC  | 2 USDC            | ~0.85 USDC |
| USDT  | 2 USDT            | ~0.85 USDT |
| ZEC   | 0.01 ZEC          | ~0.002 ZEC |
| ORE   | 0.02 ORE          | ~0.007 ORE |
| STORE | 0.02 STORE        | ~0.007 STORE |

New tokens are automatically supported when Privacy Cash adds them.

## Security

⚠️ **IMPORTANT**: 

- **Never hardcode private keys** in your code
- Use environment variables or secure key management
- Private keys are used locally and never sent to any server
- All ZK proofs are generated client-side

## How It Works

1. **Deposit**: Tokens are deposited into Privacy Cash, creating an encrypted UTXO
2. **ZK Proof**: A zero-knowledge proof is generated client-side
3. **Withdraw**: Proof is verified on-chain, tokens sent to recipient
4. **Privacy**: Link between deposit and withdrawal is cryptographically hidden

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Your Rust Application                   │
├─────────────────────────────────────────────────────────────┤
│                  privacy-cash (Rust crate)                  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                    bridge module                      │  │
│  │  send_privately() → ts_deposit() → ts_withdraw()     │  │
│  └──────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│              ts-bridge/ (TypeScript CLI)                    │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  privacy-cash-sdk (npm) + ZK proof generation        │  │
│  └──────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                   Privacy Cash Protocol                     │
│                    (Solana on-chain)                        │
└─────────────────────────────────────────────────────────────┘
```

## License

MIT License - Copyright © 2026 Nova Shield

See [LICENSE](LICENSE) for details.

## Links

- [Nova Shield](https://nshield.org) - Created by Nova Shield
- [Nova for Solana - iOS App](https://apps.apple.com/us/app/nova-for-solana/id6753857720) - Download on the App Store
- [Privacy Cash Protocol](https://privacycash.org) - The underlying privacy protocol
- [Privacy Cash TypeScript SDK](https://github.com/Privacy-Cash/privacy-cash-sdk)

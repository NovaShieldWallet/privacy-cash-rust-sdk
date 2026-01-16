//! # Privacy Cash Rust SDK
//!
//! **Pure Rust** SDK for Privacy Cash on Solana.
//! Privacy-preserving transactions using Zero-Knowledge Proofs.
//! 
//! **iOS Compatible** - No Node.js required!
//!
//! ## Features
//!
//! - 🔒 **Private Transactions**: Send SOL and SPL tokens with complete privacy
//! - 🛡️ **Pure Rust ZK Proofs**: Native Groth16 proof generation
//! - 📱 **iOS Compatible**: Use as a Rust crate in mobile apps
//! - 💰 **Multi-Token Support**: SOL, USDC, USDT
//!
//! ## Quick Start - ONE Function
//!
//! ```rust,no_run
//! use privacy_cash::send_privately;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Send 0.1 SOL privately - ONE function does everything!
//!     let result = send_privately(
//!         "your_base58_private_key",  // Private key
//!         "recipient_pubkey",          // Recipient address  
//!         0.1,                         // Amount to send
//!         "sol",                       // Token: "sol", "usdc", "usdt"
//!         None,                        // Optional RPC URL
//!     ).await?;
//!     
//!     println!("Deposit TX: {}", result.deposit_signature);
//!     println!("Withdraw TX: {}", result.withdraw_signature);
//!     println!("Amount received: {}", result.amount_received);
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod config;
pub mod constants;
pub mod deposit;
pub mod deposit_spl;
pub mod encryption;
pub mod error;
pub mod get_utxos;
pub mod get_utxos_spl;
pub mod keypair;
pub mod merkle_tree;
pub mod poseidon;
pub mod prover;
pub mod prover_rust;
pub mod storage;
pub mod utxo;
pub mod utils;
pub mod withdraw;
pub mod withdraw_spl;

// Re-export main types
pub use client::PrivacyCash;
pub use config::{Config, SupportedToken};
pub use constants::*;
pub use error::{PrivacyCashError, Result};
pub use keypair::ZkKeypair;
pub use utxo::{Utxo, Balance, SplBalance};

// Re-export Solana types for convenience
pub use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};

// ============================================================================
// MAIN FUNCTION: send_privately() - ONE function does everything!
// ============================================================================

use std::str::FromStr;

/// Result of a send_privately operation
#[derive(Debug, Clone)]
pub struct SendPrivatelyResult {
    /// Deposit transaction signature
    pub deposit_signature: String,
    /// Withdraw transaction signature  
    pub withdraw_signature: String,
    /// Amount deposited (in smallest units)
    pub amount_deposited: u64,
    /// Amount received by recipient (after fees)
    pub amount_received: u64,
    /// Total fees paid (Privacy Cash + Nova Shield)
    pub total_fees: u64,
    /// Recipient address
    pub recipient: String,
    /// Token type
    pub token: String,
}

/// 🚀 SEND PRIVATELY - The ONE function you need!
///
/// This function does EVERYTHING:
/// 1. Deposits your tokens into Privacy Cash
/// 2. Waits for confirmation
/// 3. Withdraws the MAXIMUM amount to the recipient
///
/// # Arguments
/// * `private_key` - Your wallet's private key (base58 encoded)
/// * `recipient` - Recipient's public key (base58 encoded)
/// * `amount` - Amount to send (e.g., 0.1 for 0.1 SOL or 10.0 for 10 USDC)
/// * `token` - Token type: "sol", "usdc", or "usdt"
/// * `rpc_url` - Optional RPC URL (defaults to mainnet)
///
/// # Example
/// ```rust,no_run
/// use privacy_cash::send_privately;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let result = send_privately(
///         "your_private_key_base58",
///         "recipient_pubkey",
///         0.1,     // 0.1 SOL
///         "sol",
///         None,    // Use default RPC
///     ).await?;
///     
///     println!("✅ Sent privately!");
///     println!("Deposit TX: {}", result.deposit_signature);
///     println!("Withdraw TX: {}", result.withdraw_signature);
///     println!("Recipient received: {} lamports", result.amount_received);
///     Ok(())
/// }
/// ```
pub async fn send_privately(
    private_key: &str,
    recipient: &str,
    amount: f64,
    token: &str,
    rpc_url: Option<&str>,
) -> Result<SendPrivatelyResult> {
    // Parse private key
    let key_bytes = bs58::decode(private_key)
        .into_vec()
        .map_err(|e| PrivacyCashError::InvalidInput(format!("Invalid private key: {}", e)))?;
    let keypair = Keypair::from_bytes(&key_bytes)
        .map_err(|e| PrivacyCashError::InvalidInput(format!("Invalid keypair: {}", e)))?;

    // Parse recipient
    let recipient_pubkey = Pubkey::from_str(recipient)
        .map_err(|e| PrivacyCashError::InvalidInput(format!("Invalid recipient: {}", e)))?;

    // Create client
    let rpc = rpc_url.unwrap_or("https://api.mainnet-beta.solana.com");
    let client = PrivacyCash::new(rpc, keypair)?;

    let token_lower = token.to_lowercase();
    
    match token_lower.as_str() {
        "sol" => {
            let lamports = (amount * 1_000_000_000.0) as u64;
            
            // Step 1: Deposit
            log::info!("Step 1/3: Depositing {} SOL...", amount);
            let deposit_result = client.deposit(lamports).await?;
            log::info!("Deposit TX: {}", deposit_result.signature);
            
            // Step 2: Wait for indexer
            log::info!("Step 2/3: Waiting for indexer (5 seconds)...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            
            // Step 3: Withdraw ALL to recipient
            log::info!("Step 3/3: Withdrawing to recipient...");
            let withdraw_result = client.withdraw_all(Some(&recipient_pubkey)).await?;
            log::info!("Withdraw TX: {}", withdraw_result.signature);
            
            Ok(SendPrivatelyResult {
                deposit_signature: deposit_result.signature,
                withdraw_signature: withdraw_result.signature,
                amount_deposited: lamports,
                amount_received: withdraw_result.amount_in_lamports,
                total_fees: lamports.saturating_sub(withdraw_result.amount_in_lamports),
                recipient: recipient.to_string(),
                token: "sol".to_string(),
            })
        }
        "usdc" => {
            let base_units = (amount * 1_000_000.0) as u64;
            
            // Step 1: Deposit
            log::info!("Step 1/3: Depositing {} USDC...", amount);
            let deposit_result = client.deposit_usdc(base_units).await?;
            log::info!("Deposit TX: {}", deposit_result.signature);
            
            // Step 2: Wait for indexer
            log::info!("Step 2/3: Waiting for indexer (5 seconds)...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            
            // Step 3: Withdraw ALL to recipient
            log::info!("Step 3/3: Withdrawing to recipient...");
            let withdraw_result = client.withdraw_all_usdc(Some(&recipient_pubkey)).await?;
            log::info!("Withdraw TX: {}", withdraw_result.signature);
            
            Ok(SendPrivatelyResult {
                deposit_signature: deposit_result.signature,
                withdraw_signature: withdraw_result.signature,
                amount_deposited: base_units,
                amount_received: withdraw_result.base_units,
                total_fees: base_units.saturating_sub(withdraw_result.base_units),
                recipient: recipient.to_string(),
                token: "usdc".to_string(),
            })
        }
        "usdt" => {
            let base_units = (amount * 1_000_000.0) as u64;
            
            // Step 1: Deposit
            log::info!("Step 1/3: Depositing {} USDT...", amount);
            let deposit_result = client.deposit_usdt(base_units).await?;
            log::info!("Deposit TX: {}", deposit_result.signature);
            
            // Step 2: Wait for indexer
            log::info!("Step 2/3: Waiting for indexer (5 seconds)...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            
            // Step 3: Withdraw ALL to recipient
            log::info!("Step 3/3: Withdrawing to recipient...");
            let withdraw_result = client.withdraw_all_spl(&USDT_MINT, Some(&recipient_pubkey)).await?;
            log::info!("Withdraw TX: {}", withdraw_result.signature);
            
            Ok(SendPrivatelyResult {
                deposit_signature: deposit_result.signature,
                withdraw_signature: withdraw_result.signature,
                amount_deposited: base_units,
                amount_received: withdraw_result.base_units,
                total_fees: base_units.saturating_sub(withdraw_result.base_units),
                recipient: recipient.to_string(),
                token: "usdt".to_string(),
            })
        }
        _ => Err(PrivacyCashError::InvalidInput(format!(
            "Unsupported token: {}. Use 'sol', 'usdc', or 'usdt'",
            token
        ))),
    }
}

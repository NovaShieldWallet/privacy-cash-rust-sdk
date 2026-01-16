//! TypeScript Bridge for ZK proof generation
//!
//! This module calls the bundled TypeScript SDK for operations that require
//! ZK proof generation (deposit/withdraw), since the Poseidon hash implementation
//! must match exactly what the circuits expect.
//!
//! Nova Shield collects 1% fee on all withdrawals automatically.

use crate::error::{PrivacyCashError, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;

/// Path to the TypeScript bridge
const TS_BRIDGE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/ts-bridge");

/// Nova Shield fee rate (1%)
pub const NOVA_SHIELD_FEE_RATE: f64 = 0.01;

/// Nova Shield fee wallet
pub const NOVA_SHIELD_FEE_WALLET: &str = "HKBrbp3h8B9tMCn4ceKCtmF8jWxvpfrb7YNLbCgxLUJL";

#[derive(Debug, Serialize)]
struct BridgeCommand {
    action: String,
    rpc_url: String,
    private_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    amount: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mint_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recipient: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BridgeResponse {
    pub success: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default)]
    pub lamports: Option<u64>,
    #[serde(default)]
    pub sol: Option<f64>,
    #[serde(default)]
    pub base_units: Option<u64>,
    #[serde(default)]
    pub amount: Option<f64>,
    #[serde(default)]
    pub amount_in_lamports: Option<u64>,
    #[serde(default)]
    pub fee_in_lamports: Option<u64>,
    #[serde(default)]
    pub fee_base_units: Option<u64>,
    #[serde(default)]
    pub nova_shield_fee: Option<u64>,
    #[serde(default)]
    pub nova_shield_fee_tx: Option<String>,
    // For send_privately
    #[serde(default)]
    pub deposit_signature: Option<String>,
    #[serde(default)]
    pub withdraw_signature: Option<String>,
    #[serde(default)]
    pub amount_sent: Option<u64>,
    #[serde(default)]
    pub amount_received: Option<u64>,
    #[serde(default)]
    pub base_units_sent: Option<u64>,
    #[serde(default)]
    pub base_units_received: Option<u64>,
    #[serde(default)]
    pub privacy_cash_fee: Option<u64>,
    #[serde(default)]
    pub recipient: Option<String>,
}

/// Result from send_privately operation
#[derive(Debug, Clone)]
pub struct SendPrivatelyResult {
    /// Deposit transaction signature
    pub deposit_signature: String,
    /// Withdraw transaction signature  
    pub withdraw_signature: String,
    /// Amount sent (before fees)
    pub amount_sent: u64,
    /// Amount received by recipient (after Privacy Cash fees)
    pub amount_received: u64,
    /// Privacy Cash protocol fee
    pub privacy_cash_fee: u64,
    /// Nova Shield 1% fee
    pub nova_shield_fee: u64,
    /// Nova Shield fee transaction signature
    pub nova_shield_fee_tx: String,
    /// Recipient address
    pub recipient: String,
}

/// Result from send_privately_spl operation
#[derive(Debug, Clone)]
pub struct SendPrivatelySplResult {
    /// Deposit transaction signature
    pub deposit_signature: String,
    /// Withdraw transaction signature  
    pub withdraw_signature: String,
    /// Base units sent (before fees)
    pub base_units_sent: u64,
    /// Base units received by recipient (after Privacy Cash fees)
    pub base_units_received: u64,
    /// Privacy Cash protocol fee
    pub privacy_cash_fee: u64,
    /// Nova Shield 1% fee
    pub nova_shield_fee: u64,
    /// Nova Shield fee transaction signature
    pub nova_shield_fee_tx: String,
    /// Recipient address
    pub recipient: String,
}

/// Deposit result from TypeScript bridge
#[derive(Debug, Clone)]
pub struct TsDepositResult {
    pub signature: String,
    pub amount: u64,
}

/// Withdraw result from TypeScript bridge
#[derive(Debug, Clone)]
pub struct TsWithdrawResult {
    pub signature: String,
    pub amount_in_lamports: u64,
    pub fee_in_lamports: u64,
    pub nova_shield_fee: u64,
    pub nova_shield_fee_tx: String,
}

/// SPL Deposit result from TypeScript bridge
#[derive(Debug, Clone)]
pub struct TsDepositSplResult {
    pub signature: String,
    pub base_units: u64,
}

/// SPL Withdraw result from TypeScript bridge
#[derive(Debug, Clone)]
pub struct TsWithdrawSplResult {
    pub signature: String,
    pub base_units: u64,
    pub fee_base_units: u64,
    pub nova_shield_fee: u64,
    pub nova_shield_fee_tx: String,
}

/// Balance result from TypeScript bridge
#[derive(Debug, Clone)]
pub struct TsBalance {
    pub lamports: u64,
    pub sol: f64,
}

/// SPL Balance result from TypeScript bridge
#[derive(Debug, Clone)]
pub struct TsSplBalance {
    pub base_units: u64,
    pub amount: f64,
}

fn call_bridge(cmd: BridgeCommand) -> Result<BridgeResponse> {
    let cmd_json = serde_json::to_string(&cmd)?;
    
    // Check if npm dependencies are installed
    let node_modules = format!("{}/node_modules", TS_BRIDGE_DIR);
    if !std::path::Path::new(&node_modules).exists() {
        return Err(PrivacyCashError::ProofGenerationError(
            format!("TypeScript bridge not installed. Run: cd {} && npm install", TS_BRIDGE_DIR)
        ));
    }
    
    let output = Command::new("npx")
        .arg("tsx")
        .arg("cli.ts")
        .arg(&cmd_json)
        .current_dir(TS_BRIDGE_DIR)
        .output()
        .map_err(|e| PrivacyCashError::ProofGenerationError(
            format!("Failed to run TypeScript bridge: {}. Make sure Node.js is installed.", e)
        ))?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Log stderr for debugging (contains progress info)
    if !stderr.is_empty() {
        log::debug!("Bridge stderr: {}", stderr);
    }
    
    // Find the JSON response in output (last line with JSON)
    let response_line = stdout
        .lines()
        .filter(|line| line.starts_with('{'))
        .last()
        .unwrap_or(&stdout);
    
    if response_line.is_empty() {
        return Err(PrivacyCashError::ProofGenerationError(
            format!("No response from TypeScript bridge. stderr: {}", stderr)
        ));
    }
    
    let response: BridgeResponse = serde_json::from_str(response_line)
        .map_err(|e| PrivacyCashError::ProofGenerationError(
            format!("Failed to parse bridge response: {}. Output: {}", e, stdout)
        ))?;
    
    if !response.success {
        return Err(PrivacyCashError::ProofGenerationError(
            response.error.unwrap_or_else(|| "Unknown error".to_string())
        ));
    }
    
    Ok(response)
}

// ============ Balance Operations ============

/// Get private SOL balance via TypeScript bridge
pub fn ts_get_balance(rpc_url: &str, private_key: &str) -> Result<TsBalance> {
    let response = call_bridge(BridgeCommand {
        action: "balance".to_string(),
        rpc_url: rpc_url.to_string(),
        private_key: private_key.to_string(),
        amount: None,
        mint_address: None,
        recipient: None,
    })?;
    
    Ok(TsBalance {
        lamports: response.lamports.unwrap_or(0),
        sol: response.sol.unwrap_or(0.0),
    })
}

/// Get private SPL balance via TypeScript bridge
pub fn ts_get_balance_spl(rpc_url: &str, private_key: &str, mint_address: &str) -> Result<TsSplBalance> {
    let response = call_bridge(BridgeCommand {
        action: "balance_spl".to_string(),
        rpc_url: rpc_url.to_string(),
        private_key: private_key.to_string(),
        amount: None,
        mint_address: Some(mint_address.to_string()),
        recipient: None,
    })?;
    
    Ok(TsSplBalance {
        base_units: response.base_units.unwrap_or(0),
        amount: response.amount.unwrap_or(0.0),
    })
}

// ============ Deposit Operations ============

/// Deposit SOL via TypeScript bridge
pub fn ts_deposit(rpc_url: &str, private_key: &str, lamports: u64) -> Result<TsDepositResult> {
    let response = call_bridge(BridgeCommand {
        action: "deposit".to_string(),
        rpc_url: rpc_url.to_string(),
        private_key: private_key.to_string(),
        amount: Some(lamports),
        mint_address: None,
        recipient: None,
    })?;
    
    Ok(TsDepositResult {
        signature: response.signature.unwrap_or_default(),
        amount: lamports,
    })
}

/// Deposit SPL tokens via TypeScript bridge
pub fn ts_deposit_spl(rpc_url: &str, private_key: &str, base_units: u64, mint_address: &str) -> Result<TsDepositSplResult> {
    let response = call_bridge(BridgeCommand {
        action: "deposit_spl".to_string(),
        rpc_url: rpc_url.to_string(),
        private_key: private_key.to_string(),
        amount: Some(base_units),
        mint_address: Some(mint_address.to_string()),
        recipient: None,
    })?;
    
    Ok(TsDepositSplResult {
        signature: response.signature.unwrap_or_default(),
        base_units,
    })
}

// ============ Withdraw Operations (Nova Shield fee collected automatically) ============

/// Withdraw SOL via TypeScript bridge
/// 
/// Nova Shield 1% fee is automatically collected on withdrawal.
pub fn ts_withdraw(rpc_url: &str, private_key: &str, lamports: u64, recipient: Option<&str>) -> Result<TsWithdrawResult> {
    let response = call_bridge(BridgeCommand {
        action: "withdraw".to_string(),
        rpc_url: rpc_url.to_string(),
        private_key: private_key.to_string(),
        amount: Some(lamports),
        mint_address: None,
        recipient: recipient.map(|s| s.to_string()),
    })?;
    
    Ok(TsWithdrawResult {
        signature: response.signature.unwrap_or_default(),
        amount_in_lamports: response.amount_in_lamports.unwrap_or(0),
        fee_in_lamports: response.fee_in_lamports.unwrap_or(0),
        nova_shield_fee: response.nova_shield_fee.unwrap_or(0),
        nova_shield_fee_tx: response.nova_shield_fee_tx.unwrap_or_default(),
    })
}

/// Withdraw all SOL via TypeScript bridge
/// 
/// Nova Shield 1% fee is automatically collected on withdrawal.
pub fn ts_withdraw_all(rpc_url: &str, private_key: &str, recipient: Option<&str>) -> Result<TsWithdrawResult> {
    let response = call_bridge(BridgeCommand {
        action: "withdraw_all".to_string(),
        rpc_url: rpc_url.to_string(),
        private_key: private_key.to_string(),
        amount: None,
        mint_address: None,
        recipient: recipient.map(|s| s.to_string()),
    })?;
    
    Ok(TsWithdrawResult {
        signature: response.signature.unwrap_or_default(),
        amount_in_lamports: response.amount_in_lamports.unwrap_or(0),
        fee_in_lamports: response.fee_in_lamports.unwrap_or(0),
        nova_shield_fee: response.nova_shield_fee.unwrap_or(0),
        nova_shield_fee_tx: response.nova_shield_fee_tx.unwrap_or_default(),
    })
}

/// Withdraw SPL tokens via TypeScript bridge
/// 
/// Nova Shield 1% fee is automatically collected on withdrawal.
pub fn ts_withdraw_spl(
    rpc_url: &str, 
    private_key: &str, 
    base_units: u64, 
    mint_address: &str,
    recipient: Option<&str>
) -> Result<TsWithdrawSplResult> {
    let response = call_bridge(BridgeCommand {
        action: "withdraw_spl".to_string(),
        rpc_url: rpc_url.to_string(),
        private_key: private_key.to_string(),
        amount: Some(base_units),
        mint_address: Some(mint_address.to_string()),
        recipient: recipient.map(|s| s.to_string()),
    })?;
    
    Ok(TsWithdrawSplResult {
        signature: response.signature.unwrap_or_default(),
        base_units: response.base_units.unwrap_or(0),
        fee_base_units: response.fee_base_units.unwrap_or(0),
        nova_shield_fee: response.nova_shield_fee.unwrap_or(0),
        nova_shield_fee_tx: response.nova_shield_fee_tx.unwrap_or_default(),
    })
}

/// Withdraw all SPL tokens via TypeScript bridge
/// 
/// Nova Shield 1% fee is automatically collected on withdrawal.
pub fn ts_withdraw_all_spl(
    rpc_url: &str, 
    private_key: &str, 
    mint_address: &str,
    recipient: Option<&str>
) -> Result<TsWithdrawSplResult> {
    let response = call_bridge(BridgeCommand {
        action: "withdraw_all_spl".to_string(),
        rpc_url: rpc_url.to_string(),
        private_key: private_key.to_string(),
        amount: None,
        mint_address: Some(mint_address.to_string()),
        recipient: recipient.map(|s| s.to_string()),
    })?;
    
    Ok(TsWithdrawSplResult {
        signature: response.signature.unwrap_or_default(),
        base_units: response.base_units.unwrap_or(0),
        fee_base_units: response.fee_base_units.unwrap_or(0),
        nova_shield_fee: response.nova_shield_fee.unwrap_or(0),
        nova_shield_fee_tx: response.nova_shield_fee_tx.unwrap_or_default(),
    })
}

// ============ SEND PRIVATELY - Main function for privacy transfers ============

/// Send SOL privately to a recipient
/// 
/// This is the main function for privacy transfers:
/// 1. Deposits SOL into Privacy Cash
/// 2. Collects Nova Shield 1% fee
/// 3. Withdraws to recipient
/// 
/// Nova Shield receives 1% of the transfer amount.
/// 
/// # Arguments
/// * `rpc_url` - Solana RPC URL
/// * `private_key` - Sender's private key (base58)
/// * `lamports` - Amount to send in lamports
/// * `recipient` - Recipient's public key (base58)
/// 
/// # Example
/// ```rust,no_run
/// use privacy_cash::bridge::send_privately;
/// 
/// let result = send_privately(
///     "https://api.mainnet-beta.solana.com",
///     "your_private_key_base58",
///     10_000_000, // 0.01 SOL
///     "recipient_pubkey"
/// ).unwrap();
/// 
/// println!("Sent {} lamports, Nova Shield fee: {}", 
///     result.amount_sent, result.nova_shield_fee);
/// ```
pub fn send_privately(
    rpc_url: &str,
    private_key: &str,
    lamports: u64,
    recipient: &str,
) -> Result<SendPrivatelyResult> {
    log::info!("Starting private transfer of {} lamports to {}", lamports, recipient);
    
    let response = call_bridge(BridgeCommand {
        action: "send_privately".to_string(),
        rpc_url: rpc_url.to_string(),
        private_key: private_key.to_string(),
        amount: Some(lamports),
        mint_address: None,
        recipient: Some(recipient.to_string()),
    })?;
    
    Ok(SendPrivatelyResult {
        deposit_signature: response.deposit_signature.unwrap_or_default(),
        withdraw_signature: response.withdraw_signature.unwrap_or_default(),
        amount_sent: response.amount_sent.unwrap_or(lamports),
        amount_received: response.amount_received.unwrap_or(0),
        privacy_cash_fee: response.privacy_cash_fee.unwrap_or(0),
        nova_shield_fee: response.nova_shield_fee.unwrap_or(0),
        nova_shield_fee_tx: response.nova_shield_fee_tx.unwrap_or_default(),
        recipient: response.recipient.unwrap_or_else(|| recipient.to_string()),
    })
}

/// Send SPL tokens privately to a recipient
/// 
/// This is the main function for privacy transfers of SPL tokens:
/// 1. Deposits tokens into Privacy Cash
/// 2. Collects Nova Shield 1% fee
/// 3. Withdraws to recipient
/// 
/// Nova Shield receives 1% of the transfer amount.
/// 
/// # Arguments
/// * `rpc_url` - Solana RPC URL
/// * `private_key` - Sender's private key (base58)
/// * `base_units` - Amount to send in base units
/// * `mint_address` - Token mint address
/// * `recipient` - Recipient's public key (base58)
/// 
/// # Example
/// ```rust,no_run
/// use privacy_cash::bridge::send_privately_spl;
/// 
/// let result = send_privately_spl(
///     "https://api.mainnet-beta.solana.com",
///     "your_private_key_base58",
///     1_000_000, // 1 USDC
///     "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", // USDC mint
///     "recipient_pubkey"
/// ).unwrap();
/// 
/// println!("Sent {} base units, Nova Shield fee: {}", 
///     result.base_units_sent, result.nova_shield_fee);
/// ```
pub fn send_privately_spl(
    rpc_url: &str,
    private_key: &str,
    base_units: u64,
    mint_address: &str,
    recipient: &str,
) -> Result<SendPrivatelySplResult> {
    log::info!("Starting private SPL transfer of {} base units to {}", base_units, recipient);
    
    let response = call_bridge(BridgeCommand {
        action: "send_privately_spl".to_string(),
        rpc_url: rpc_url.to_string(),
        private_key: private_key.to_string(),
        amount: Some(base_units),
        mint_address: Some(mint_address.to_string()),
        recipient: Some(recipient.to_string()),
    })?;
    
    Ok(SendPrivatelySplResult {
        deposit_signature: response.deposit_signature.unwrap_or_default(),
        withdraw_signature: response.withdraw_signature.unwrap_or_default(),
        base_units_sent: response.base_units_sent.unwrap_or(base_units),
        base_units_received: response.base_units_received.unwrap_or(0),
        privacy_cash_fee: response.privacy_cash_fee.unwrap_or(0),
        nova_shield_fee: response.nova_shield_fee.unwrap_or(0),
        nova_shield_fee_tx: response.nova_shield_fee_tx.unwrap_or_default(),
        recipient: response.recipient.unwrap_or_else(|| recipient.to_string()),
    })
}

/// Calculate Nova Shield fee for a given amount
pub fn calculate_nova_shield_fee(amount: u64) -> u64 {
    (amount as f64 * NOVA_SHIELD_FEE_RATE) as u64
}

/// Get Nova Shield fee rate
pub fn get_nova_shield_fee_rate() -> f64 {
    NOVA_SHIELD_FEE_RATE
}

/// Get Nova Shield fee wallet address
pub fn get_nova_shield_fee_wallet() -> &'static str {
    NOVA_SHIELD_FEE_WALLET
}

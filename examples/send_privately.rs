//! Send Privately - THE main example for Privacy Cash Rust SDK
//!
//! ONE function does everything: deposit + withdraw to recipient
//!
//! Usage:
//!   SOLANA_PRIVATE_KEY=<key> cargo run --release --example send_privately -- <amount> <token> <recipient>
//!
//! Examples:
//!   # Send 0.02 SOL to yourself
//!   SOLANA_PRIVATE_KEY=<key> cargo run --release --example send_privately -- 0.02 sol
//!
//!   # Send 10 USDC to a recipient
//!   SOLANA_PRIVATE_KEY=<key> cargo run --release --example send_privately -- 10 usdc RecipientPubkey

use privacy_cash::{send_privately, Signer};
use solana_sdk::signature::Keypair;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("═══════════════════════════════════════════════════════════════");
    println!("       PRIVACY CASH - Send Privately");
    println!("       Pure Rust SDK (iOS Compatible)");
    println!("═══════════════════════════════════════════════════════════════\n");

    // Get private key from environment
    let private_key = env::var("SOLANA_PRIVATE_KEY")
        .expect("Please set SOLANA_PRIVATE_KEY environment variable");

    // Parse keypair to get pubkey for display
    let key_bytes = bs58::decode(&private_key).into_vec()?;
    let keypair = Keypair::from_bytes(&key_bytes)?;
    let self_pubkey = keypair.pubkey();

    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 3 {
        println!("Usage: {} <amount> <token> [recipient]", args[0]);
        println!("\nExamples:");
        println!("  Send 0.02 SOL to yourself:");
        println!("    SOLANA_PRIVATE_KEY=<key> cargo run --release --example send_privately -- 0.02 sol");
        println!("\n  Send 10 USDC to a recipient:");
        println!("    SOLANA_PRIVATE_KEY=<key> cargo run --release --example send_privately -- 10 usdc RecipientPubkey");
        println!("\nSupported tokens: sol, usdc, usdt");
        return Ok(());
    }

    let amount: f64 = args[1].parse().expect("Invalid amount");
    let token = &args[2];
    let recipient = if args.len() > 3 {
        args[3].clone()
    } else {
        self_pubkey.to_string() // Default to self
    };

    // Get RPC URL from environment or use default
    let rpc_url = env::var("SOLANA_RPC_URL").ok();

    println!("Wallet: {}", self_pubkey);
    println!("Recipient: {}", recipient);
    println!("Amount: {} {}", amount, token.to_uppercase());
    if let Some(ref rpc) = rpc_url {
        println!("RPC: {}", rpc);
    }
    println!();

    println!("🚀 Sending {} {} privately...\n", amount, token.to_uppercase());

    // ONE FUNCTION DOES EVERYTHING!
    let result = send_privately(
        &private_key,
        &recipient,
        amount,
        token,
        rpc_url.as_deref(),
    ).await?;

    println!("\n═══════════════════════════════════════════════════════════════");
    println!("                    ✅ SUCCESS!");
    println!("═══════════════════════════════════════════════════════════════");
    println!("\n📥 Deposit TX:  {}", result.deposit_signature);
    println!("📤 Withdraw TX: {}", result.withdraw_signature);
    println!("\n💰 Amount deposited: {} {}", 
        format_amount(result.amount_deposited, &result.token),
        result.token.to_uppercase()
    );
    println!("💸 Amount received:  {} {}", 
        format_amount(result.amount_received, &result.token),
        result.token.to_uppercase()
    );
    println!("🏷️  Total fees:       {} {}", 
        format_amount(result.total_fees, &result.token),
        result.token.to_uppercase()
    );
    println!("👤 Recipient:        {}", result.recipient);
    println!("\n═══════════════════════════════════════════════════════════════\n");

    Ok(())
}

fn format_amount(amount: u64, token: &str) -> String {
    match token {
        "sol" => format!("{:.6}", amount as f64 / 1_000_000_000.0),
        "usdc" | "usdt" => format!("{:.2}", amount as f64 / 1_000_000.0),
        _ => amount.to_string(),
    }
}

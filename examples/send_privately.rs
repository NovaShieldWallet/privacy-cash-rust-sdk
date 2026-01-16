//! Send Privately Example
//!
//! This is the main function for privacy transfers using Nova Shield.
//! 
//! Usage:
//!   cargo run --example send_privately -- <amount_sol> <recipient>
//!
//! Example:
//!   SOLANA_PRIVATE_KEY=your_key cargo run --example send_privately -- 0.01 RecipientPubkey

use privacy_cash::bridge::{
    send_privately, send_privately_spl, 
    ts_get_balance, ts_get_balance_spl,
    get_nova_shield_fee_rate, get_nova_shield_fee_wallet
};
use std::env;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("═══════════════════════════════════════════════════════════════");
    println!("       NOVA SHIELD - SEND PRIVATELY");
    println!("═══════════════════════════════════════════════════════════════");
    println!("       Nova Shield Fee: {}%", get_nova_shield_fee_rate() * 100.0);
    println!("       Fee Wallet: {}", get_nova_shield_fee_wallet());
    println!("═══════════════════════════════════════════════════════════════\n");

    // Get private key from environment
    let private_key = match env::var("SOLANA_PRIVATE_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("❌ Error: SOLANA_PRIVATE_KEY environment variable not set");
            eprintln!("\nUsage:");
            eprintln!("  SOLANA_PRIVATE_KEY=your_key cargo run --example send_privately -- <amount_sol> <recipient>");
            std::process::exit(1);
        }
    };

    let rpc_url = env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());

    let args: Vec<String> = env::args().collect();
    
    if args.len() < 3 {
        // Just show balances
        println!("📊 Current Private Balances:\n");
        
        match ts_get_balance(&rpc_url, &private_key) {
            Ok(balance) => println!("   SOL: {} lamports ({} SOL)", balance.lamports, balance.sol),
            Err(e) => println!("   SOL: Error - {}", e),
        }
        
        let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        match ts_get_balance_spl(&rpc_url, &private_key, usdc_mint) {
            Ok(balance) => println!("   USDC: {} base units ({} USDC)", balance.base_units, balance.amount),
            Err(e) => println!("   USDC: Error - {}", e),
        }
        
        let usdt_mint = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";
        match ts_get_balance_spl(&rpc_url, &private_key, usdt_mint) {
            Ok(balance) => println!("   USDT: {} base units ({} USDT)", balance.base_units, balance.amount),
            Err(e) => println!("   USDT: Error - {}", e),
        }
        
        println!("\n\nTo send privately:");
        println!("  SOLANA_PRIVATE_KEY=key cargo run --example send_privately -- <amount_sol> <recipient>");
        println!("  SOLANA_PRIVATE_KEY=key cargo run --example send_privately -- 0.01 RecipientPubkey");
        return;
    }

    let amount_sol: f64 = args[1].parse().expect("Invalid amount");
    let recipient = &args[2];
    let lamports = (amount_sol * 1_000_000_000.0) as u64;

    println!("📤 Sending {} SOL ({} lamports) privately to {}\n", amount_sol, lamports, recipient);
    println!("   This will:");
    println!("   1. Deposit {} SOL into Privacy Cash", amount_sol);
    println!("   2. Collect {}% Nova Shield fee ({} lamports)", 
        get_nova_shield_fee_rate() * 100.0,
        (lamports as f64 * get_nova_shield_fee_rate()) as u64
    );
    println!("   3. Withdraw to recipient\n");

    match send_privately(&rpc_url, &private_key, lamports, recipient) {
        Ok(result) => {
            println!("✅ Private transfer successful!\n");
            println!("   Deposit TX:      {}", result.deposit_signature);
            println!("   Withdraw TX:     {}", result.withdraw_signature);
            println!("   Nova Shield TX:  {}\n", result.nova_shield_fee_tx);
            println!("   Amount Sent:     {} lamports", result.amount_sent);
            println!("   Amount Received: {} lamports", result.amount_received);
            println!("   Privacy Cash Fee: {} lamports", result.privacy_cash_fee);
            println!("   Nova Shield Fee: {} lamports (1%)", result.nova_shield_fee);
            println!("   Recipient:       {}", result.recipient);
        }
        Err(e) => {
            println!("❌ Private transfer failed: {}", e);
        }
    }
}

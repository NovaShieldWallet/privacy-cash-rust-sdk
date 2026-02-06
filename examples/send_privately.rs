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

use privacy_cash::{PrivacyCash, Signer};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use std::env;
use std::io::{self, Write};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// ANSI color codes
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const MAGENTA: &str = "\x1b[35m";
const CYAN: &str = "\x1b[36m";
const WHITE: &str = "\x1b[37m";

// Spinner frames
const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

struct Spinner {
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Spinner {
    fn new(message: &str) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let msg = message.to_string();
        
        let handle = thread::spawn(move || {
            let mut i = 0;
            while running_clone.load(Ordering::Relaxed) {
                print!("\r{CYAN}{}{RESET} {msg}", SPINNER[i % SPINNER.len()]);
                io::stdout().flush().unwrap();
                thread::sleep(Duration::from_millis(80));
                i += 1;
            }
        });
        
        Self {
            running,
            handle: Some(handle),
        }
    }
    
    fn success(mut self, message: &str) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        print!("\r\x1b[K"); // Clear line
        println!("{GREEN}✓{RESET} {message}");
    }
    
    fn fail(mut self, message: &str) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        print!("\r\x1b[K");
        println!("{RED}✗{RESET} {message}");
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn print_header() {
    println!();
    println!("{CYAN}╔═══════════════════════════════════════════════════════════════╗{RESET}");
    println!("{CYAN}║{RESET}       {BOLD}{WHITE}PRIVACY CASH{RESET} - {MAGENTA}Send Privately{RESET}                       {CYAN}║{RESET}");
    println!("{CYAN}║{RESET}       {DIM}Pure Rust SDK with ZK Proofs{RESET}                          {CYAN}║{RESET}");
    println!("{CYAN}╚═══════════════════════════════════════════════════════════════╝{RESET}");
    println!();
}

fn print_success_box() {
    println!();
    println!("{GREEN}╔═══════════════════════════════════════════════════════════════╗{RESET}");
    println!("{GREEN}║{RESET}                    {BOLD}{GREEN}✓ SUCCESS!{RESET}                              {GREEN}║{RESET}");
    println!("{GREEN}╚═══════════════════════════════════════════════════════════════╝{RESET}");
}

fn print_error_box(msg: &str) {
    println!();
    println!("{RED}╔═══════════════════════════════════════════════════════════════╗{RESET}");
    println!("{RED}║{RESET}                    {BOLD}{RED}✗ ERROR{RESET}                                 {RED}║{RESET}");
    println!("{RED}╚═══════════════════════════════════════════════════════════════╝{RESET}");
    println!();
    println!("{RED}{msg}{RESET}");
}

fn load_env_files() {
    for path in [".env.local", ".env"] {
        let Ok(contents) = std::fs::read_to_string(path) else {
            continue;
        };

        for raw_line in contents.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let line = line.strip_prefix("export ").unwrap_or(line).trim();
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            let key = key.trim();
            if key.is_empty() {
                continue;
            }

            // Only set if not already present in the process environment.
            if env::var(key).is_ok() {
                continue;
            }

            let mut value = value.trim().to_string();
            if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                value = value[1..value.len().saturating_sub(1)].to_string();
            }

            env::set_var(key, value);
        }
    }
}

fn step_box(step: u32, total: u32, title: &str) {
    println!();
    println!("{BLUE}┌─────────────────────────────────────────────────────────────────┐{RESET}");
    println!("{BLUE}│{RESET} {BOLD}Step {step}/{total}:{RESET} {WHITE}{title}{RESET}");
    println!("{BLUE}└─────────────────────────────────────────────────────────────────┘{RESET}");
}

fn format_amount(amount: u64, token: &str) -> String {
    match token {
        "sol" => format!("{:.6}", amount as f64 / 1_000_000_000.0),
        "usdc" | "usdt" => format!("{:.2}", amount as f64 / 1_000_000.0),
        _ => amount.to_string(),
    }
}

fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs_f64();
    if secs < 1.0 {
        format!("{:.0}ms", secs * 1000.0)
    } else {
        format!("{:.1}s", secs)
    }
}

fn shorten_sig(sig: &str) -> String {
    if sig.len() > 16 {
        format!("{}...{}", &sig[..8], &sig[sig.len()-8..])
    } else {
        sig.to_string()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Suppress default logging - we'll use our own output
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();

    print_header();

    // Load gitignored env files (optional convenience):
    load_env_files();

    // Get private key from environment
    let private_key = match env::var("SOLANA_PRIVATE_KEY") {
        Ok(key) => key,
        Err(_) => {
            print_error_box("SOLANA_PRIVATE_KEY not set.\n\nSet it in your shell:\n  export SOLANA_PRIVATE_KEY=<your-key>\n\nOr add it to .env.local (gitignored)");
            std::process::exit(1);
        }
    };

    // Parse keypair to get pubkey for display
    let key_bytes = bs58::decode(&private_key).into_vec()?;
    #[allow(deprecated)]
    let keypair = Keypair::from_bytes(&key_bytes)?;
    let self_pubkey = keypair.pubkey();

    // Parse command line arguments
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        println!("{BOLD}{WHITE}Usage:{RESET} {} <amount> <token> [recipient]", args[0]);
        println!();
        println!("{BOLD}Examples:{RESET}");
        println!("  {DIM}# Send 0.02 SOL to yourself:{RESET}");
        println!("  {CYAN}SOLANA_PRIVATE_KEY=<key> cargo run --release --example send_privately -- 0.02 sol{RESET}");
        println!();
        println!("  {DIM}# Send 10 USDC to a recipient:{RESET}");
        println!("  {CYAN}SOLANA_PRIVATE_KEY=<key> cargo run --release --example send_privately -- 10 usdc <recipient>{RESET}");
        println!();
        println!("{BOLD}Supported tokens:{RESET} {GREEN}sol{RESET}, {GREEN}usdc{RESET}, {GREEN}usdt{RESET}");
        return Ok(());
    }

    let amount: f64 = args[1].parse().expect("Invalid amount");
    let token = args[2].to_lowercase();
    let recipient_str = if args.len() > 3 {
        args[3].clone()
    } else {
        self_pubkey.to_string() // Default to self
    };
    let recipient = Pubkey::from_str(&recipient_str)?;

    // Get RPC URL from environment or use default
    let rpc_url = env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());

    // Display configuration
    println!("{BOLD}{WHITE}Configuration:{RESET}");
    println!("  {DIM}Wallet:{RESET}    {WHITE}{}{RESET}", self_pubkey);
    println!("  {DIM}Recipient:{RESET} {WHITE}{}{RESET}", recipient_str);
    println!("  {DIM}Amount:{RESET}    {GREEN}{}{RESET} {YELLOW}{}{RESET}", amount, token.to_uppercase());
    println!();

    println!("{YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{RESET}");
    println!("{YELLOW}                    STARTING PRIVATE TRANSFER                     {RESET}");
    println!("{YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{RESET}");

    let total_start = Instant::now();

    // Create Privacy Cash client
    let client = PrivacyCash::new(&rpc_url, keypair)?;

    // ============ STEP 1: DEPOSIT INTO SHIELDED POOL ============
    step_box(1, 3, "Deposit into Shielded Pool");
    
    let spinner = Spinner::new(&format!("Generating ZK proof for {} {}...", amount, token.to_uppercase()));
    let deposit_start = Instant::now();
    
    let (deposit_sig, deposited_amount) = match token.as_str() {
        "sol" => {
            let lamports = (amount * 1_000_000_000.0) as u64;
            let result = client.deposit(lamports).await;
            match result {
                Ok(r) => {
                    spinner.success(&format!(
                        "ZK proof generated & deposit submitted ({CYAN}{}{RESET})",
                        format_duration(deposit_start.elapsed())
                    ));
                    (r.signature, lamports)
                }
                Err(e) => {
                    spinner.fail("Deposit failed");
                    print_error_box(&format!("{}", e));
                    std::process::exit(1);
                }
            }
        }
        "usdc" => {
            let base_units = (amount * 1_000_000.0) as u64;
            let result = client.deposit_usdc(base_units).await;
            match result {
                Ok(r) => {
                    spinner.success(&format!(
                        "ZK proof generated & deposit submitted ({CYAN}{}{RESET})",
                        format_duration(deposit_start.elapsed())
                    ));
                    (r.signature, base_units)
                }
                Err(e) => {
                    spinner.fail("Deposit failed");
                    print_error_box(&format!("{}", e));
                    std::process::exit(1);
                }
            }
        }
        "usdt" => {
            let base_units = (amount * 1_000_000.0) as u64;
            let result = client.deposit_usdt(base_units).await;
            match result {
                Ok(r) => {
                    spinner.success(&format!(
                        "ZK proof generated & deposit submitted ({CYAN}{}{RESET})",
                        format_duration(deposit_start.elapsed())
                    ));
                    (r.signature, base_units)
                }
                Err(e) => {
                    spinner.fail("Deposit failed");
                    print_error_box(&format!("{}", e));
                    std::process::exit(1);
                }
            }
        }
        _ => {
            print_error_box(&format!("Unsupported token: {}", token));
            std::process::exit(1);
        }
    };
    
    println!("  {DIM}├─{RESET} {GREEN}Deposited:{RESET} {} {} into shielded pool", format_amount(deposited_amount, &token), token.to_uppercase());
    println!("  {DIM}├─{RESET} {BLUE}TX Signature:{RESET} {DIM}{}{RESET}", shorten_sig(&deposit_sig));
    println!("  {DIM}└─{RESET} {MAGENTA}Solscan:{RESET} {CYAN}https://solscan.io/tx/{}{RESET}", deposit_sig);

    // ============ STEP 2: WAIT FOR UTXO INDEXING ============
    step_box(2, 3, "Confirming UTXO in Merkle Tree");
    
    let spinner = Spinner::new("Waiting for UTXO to be indexed...");
    let index_start = Instant::now();
    
    // Wait for indexer to pick up the deposit
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    
    spinner.success(&format!(
        "UTXO indexed and ready ({CYAN}{}{RESET})",
        format_duration(index_start.elapsed())
    ));
    
    // Show private balance
    let balance = match token.as_str() {
        "sol" => {
            let b = client.get_private_balance().await?;
            format!("{} SOL", format_amount(b.lamports, "sol"))
        }
        "usdc" => {
            let b = client.get_private_balance_usdc().await?;
            format!("{} USDC", format_amount(b.base_units, "usdc"))
        }
        "usdt" => {
            let b = client.get_private_balance_usdt().await?;
            format!("{} USDT", format_amount(b.base_units, "usdt"))
        }
        _ => "N/A".to_string()
    };
    
    println!("  {DIM}├─{RESET} {GREEN}Private Balance:{RESET} {BOLD}{}{RESET}", balance);
    println!("  {DIM}└─{RESET} {MAGENTA}Status:{RESET} Funds are now in the shielded pool");

    // ============ STEP 3: WITHDRAW TO RECIPIENT ============
    step_box(3, 3, "Withdraw to Recipient");
    
    let spinner = Spinner::new(&format!("Generating ZK proof for withdrawal to {}...", shorten_sig(&recipient_str)));
    let withdraw_start = Instant::now();
    
    let (withdraw_sig, received_amount) = match token.as_str() {
        "sol" => {
            let result = client.withdraw_all(Some(&recipient)).await;
            match result {
                Ok(r) => {
                    spinner.success(&format!(
                        "ZK proof generated & withdrawal submitted ({CYAN}{}{RESET})",
                        format_duration(withdraw_start.elapsed())
                    ));
                    (r.signature, r.amount_in_lamports)
                }
                Err(e) => {
                    spinner.fail("Withdrawal failed");
                    print_error_box(&format!("{}", e));
                    std::process::exit(1);
                }
            }
        }
        "usdc" => {
            let result = client.withdraw_all_usdc(Some(&recipient)).await;
            match result {
                Ok(r) => {
                    spinner.success(&format!(
                        "ZK proof generated & withdrawal submitted ({CYAN}{}{RESET})",
                        format_duration(withdraw_start.elapsed())
                    ));
                    (r.signature, r.base_units)
                }
                Err(e) => {
                    spinner.fail("Withdrawal failed");
                    print_error_box(&format!("{}", e));
                    std::process::exit(1);
                }
            }
        }
        "usdt" => {
            let result = client.withdraw_all_spl(&privacy_cash::USDT_MINT, Some(&recipient)).await;
            match result {
                Ok(r) => {
                    spinner.success(&format!(
                        "ZK proof generated & withdrawal submitted ({CYAN}{}{RESET})",
                        format_duration(withdraw_start.elapsed())
                    ));
                    (r.signature, r.base_units)
                }
                Err(e) => {
                    spinner.fail("Withdrawal failed");
                    print_error_box(&format!("{}", e));
                    std::process::exit(1);
                }
            }
        }
        _ => {
            print_error_box(&format!("Unsupported token: {}", token));
            std::process::exit(1);
        }
    };
    
    println!("  {DIM}├─{RESET} {GREEN}Withdrawn:{RESET} {} {} to recipient", format_amount(received_amount, &token), token.to_uppercase());
    println!("  {DIM}├─{RESET} {BLUE}TX Signature:{RESET} {DIM}{}{RESET}", shorten_sig(&withdraw_sig));
    println!("  {DIM}└─{RESET} {MAGENTA}Solscan:{RESET} {CYAN}https://solscan.io/tx/{}{RESET}", withdraw_sig);

    // ============ FINAL SUMMARY ============
    print_success_box();
    
    let total_fees = deposited_amount.saturating_sub(received_amount);
    let total_time = total_start.elapsed();
    
    println!();
    println!("{BOLD}{WHITE}═══════════════════════════════════════════════════════════════════{RESET}");
    println!("{BOLD}{WHITE}                        TRANSACTION SUMMARY                         {RESET}");
    println!("{BOLD}{WHITE}═══════════════════════════════════════════════════════════════════{RESET}");
    println!();
    
    println!("  {BOLD}Amount Sent:{RESET}      {GREEN}{} {}{RESET}", format_amount(deposited_amount, &token), token.to_uppercase());
    println!("  {BOLD}Amount Received:{RESET}  {GREEN}{} {}{RESET}", format_amount(received_amount, &token), token.to_uppercase());
    println!("  {BOLD}Total Fees:{RESET}       {YELLOW}{} {}{RESET}", format_amount(total_fees, &token), token.to_uppercase());
    println!();
    
    println!("  {BOLD}Recipient:{RESET}        {CYAN}{}{RESET}", recipient_str);
    println!("  {BOLD}Total Time:{RESET}       {CYAN}{}{RESET}", format_duration(total_time));
    println!();
    
    println!("{DIM}─────────────────────────────────────────────────────────────────{RESET}");
    println!("  {BOLD}Transaction Links:{RESET}");
    println!("  {DIM}Deposit:{RESET}  {CYAN}https://solscan.io/tx/{}{RESET}", deposit_sig);
    println!("  {DIM}Withdraw:{RESET} {CYAN}https://solscan.io/tx/{}{RESET}", withdraw_sig);
    println!("{DIM}─────────────────────────────────────────────────────────────────{RESET}");
    println!();

    Ok(())
}

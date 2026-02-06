//! Deposit functionality for SPL tokens

use crate::constants::{
    find_token_by_mint, ALT_ADDRESS, FEE_RECIPIENT, PROGRAM_ID, RELAYER_API_URL,
    TRANSACT_SPL_IX_DISCRIMINATOR,
};
use crate::encryption::EncryptionService;
use crate::error::{PrivacyCashError, Result};
use crate::get_utxos_spl::get_utxos_spl;
use crate::keypair::ZkKeypair;
use crate::merkle_tree::MerkleTree;
use crate::prover::{parse_proof_to_bytes, parse_public_signals_to_bytes, CircuitInput};
use crate::prover_rust::RustProver;
use crate::storage::Storage;
use crate::utxo::{Utxo, UtxoVersion};
use crate::utils::{
    calculate_public_amount, fetch_merkle_proof, find_cross_check_nullifier_pdas,
    find_nullifier_pdas, get_mint_address_field, get_program_accounts, get_spl_tree_account,
    query_remote_tree_state, ExtData,
};
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    address_lookup_table::AddressLookupTableAccount,
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    message::{v0::Message as MessageV0, VersionedMessage},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
    transaction::VersionedTransaction,
};
use spl_associated_token_account::get_associated_token_address;
use spl_token;

/// SPL Deposit result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositSplResult {
    pub signature: String,
}

/// Parameters for SPL deposit
pub struct DepositSplParams<'a> {
    pub connection: &'a RpcClient,
    pub keypair: &'a Keypair,
    pub encryption_service: &'a EncryptionService,
    pub storage: &'a Storage,
    pub base_units: u64,
    pub mint_address: &'a Pubkey,
    pub key_base_path: &'a str,
    pub referrer: Option<&'a str>,
}

/// Execute an SPL token deposit
pub async fn deposit_spl(params: DepositSplParams<'_>) -> Result<DepositSplResult> {
    let DepositSplParams {
        connection,
        keypair,
        encryption_service,
        storage,
        base_units,
        mint_address,
        key_base_path,
        referrer,
    } = params;

    let token = find_token_by_mint(mint_address)
        .ok_or_else(|| PrivacyCashError::TokenNotSupported(mint_address.to_string()))?;

    log::info!(
        "Starting {} deposit of {} base units",
        token.name,
        base_units
    );

    let public_key = keypair.pubkey();
    let fee_base_units = 0u64;

    // Get token accounts
    let signer_token_account = get_associated_token_address(&public_key, mint_address);
    let fee_recipient_token_account = get_associated_token_address(&FEE_RECIPIENT, mint_address);
    // For deposits, recipient is a placeholder (FEE_RECIPIENT) - same as TypeScript SDK
    let recipient = *FEE_RECIPIENT;
    let recipient_ata = get_associated_token_address(&recipient, mint_address);

    // Get SPL tree account
    let tree_account = get_spl_tree_account(mint_address);

    // Check SPL balance
    let account_info = connection.get_token_account_balance(&signer_token_account)?;
    let balance: u64 = account_info.amount.parse().unwrap_or(0);

    if balance < base_units + fee_base_units {
        return Err(PrivacyCashError::InsufficientTokenBalance {
            token: token.name.to_string(),
            have: balance,
            need: base_units + fee_base_units,
        });
    }

    // Check SOL for fees
    let sol_balance = connection.get_balance(&public_key)?;
    if sol_balance < 2_000_000 {
        // 0.002 SOL
        return Err(PrivacyCashError::InsufficientBalance {
            have: sol_balance,
            need: 2_000_000,
        });
    }

    let (_, _, global_config_account) = get_program_accounts();

    // Get tree state
    let tree_state = query_remote_tree_state(Some(token.name)).await?;

    // Get UTXO keypair
    let utxo_private_key = encryption_service.get_utxo_private_key_v2()?;
    let utxo_keypair = ZkKeypair::from_hex(&utxo_private_key)?;

    // Fetch existing UTXOs
    let existing_utxos = get_utxos_spl(
        connection,
        &public_key,
        encryption_service,
        storage,
        mint_address,
        None,
    )
    .await?;

    // Build inputs
    let (inputs, input_merkle_paths, ext_amount, output_amount) = if existing_utxos.is_empty() {
        let inputs = vec![
            Utxo::dummy(utxo_keypair.clone(), Some(&mint_address.to_string())),
            Utxo::dummy(utxo_keypair.clone(), Some(&mint_address.to_string())),
        ];
        let paths = vec![MerkleTree::zero_path(), MerkleTree::zero_path()];

        let ext_amount = base_units as i64;
        let output_amount = BigUint::from(base_units) - BigUint::from(fee_base_units);

        (inputs, paths, ext_amount, output_amount)
    } else {
        let first_utxo = &existing_utxos[0];
        let second_utxo = if existing_utxos.len() > 1 {
            existing_utxos[1].clone()
        } else {
            Utxo::dummy(utxo_keypair.clone(), Some(&mint_address.to_string()))
        };

        let first_commitment = first_utxo.get_commitment()?;
        let first_proof = fetch_merkle_proof(&first_commitment, Some(token.name)).await?;

        let second_proof = if !second_utxo.is_dummy() {
            let second_commitment = second_utxo.get_commitment()?;
            fetch_merkle_proof(&second_commitment, Some(token.name)).await?
        } else {
            MerkleTree::zero_path()
        };

        let ext_amount = base_units as i64;
        let output_amount = first_utxo.amount.clone()
            + second_utxo.amount.clone()
            + BigUint::from(base_units)
            - BigUint::from(fee_base_units);

        (
            vec![first_utxo.clone(), second_utxo],
            vec![first_proof, second_proof],
            ext_amount,
            output_amount,
        )
    };

    let public_amount = calculate_public_amount(ext_amount, fee_base_units);

    // Create outputs
    let outputs = vec![
        Utxo::new(
            output_amount,
            utxo_keypair.clone(),
            tree_state.next_index,
            Some(&mint_address.to_string()),
            Some(UtxoVersion::V2),
        ),
        Utxo::new(
            0u64,
            utxo_keypair.clone(),
            tree_state.next_index + 1,
            Some(&mint_address.to_string()),
            Some(UtxoVersion::V2),
        ),
    ];

    // Generate nullifiers and commitments
    let input_nullifiers = vec![inputs[0].get_nullifier()?, inputs[1].get_nullifier()?];
    let output_commitments = vec![outputs[0].get_commitment()?, outputs[1].get_commitment()?];

    // Encrypt outputs
    let encrypted_output1 = encryption_service.encrypt_utxo(&outputs[0])?;
    let encrypted_output2 = encryption_service.encrypt_utxo(&outputs[1])?;

    // For SPL deposits, ExtData uses token accounts (ATAs), not public keys - same as TypeScript SDK
    // recipient_ata = FEE_RECIPIENT's ATA for the token
    // feeRecipientTokenAccount = FEE_RECIPIENT's ATA for the token
    let ext_data = ExtData {
        recipient: recipient_ata,  // FEE_RECIPIENT's ATA (token account)
        ext_amount,
        encrypted_output1: encrypted_output1.clone(),
        encrypted_output2: encrypted_output2.clone(),
        fee: fee_base_units,
        fee_recipient: fee_recipient_token_account,  // FEE_RECIPIENT's ATA (token account)
        mint_address: *mint_address,
    };

    log::debug!("SPL ExtData recipient (ATA): {}", ext_data.recipient);
    log::debug!("SPL ExtData ext_amount: {}", ext_data.ext_amount);
    log::debug!("SPL ExtData fee: {}", ext_data.fee);
    log::debug!("SPL ExtData fee_recipient (ATA): {}", ext_data.fee_recipient);
    log::debug!("SPL ExtData mint_address: {}", ext_data.mint_address);
    log::debug!("SPL ExtData encrypted_output1 len: {}", ext_data.encrypted_output1.len());
    log::debug!("SPL ExtData encrypted_output2 len: {}", ext_data.encrypted_output2.len());

    let ext_data_hash = ext_data.hash();
    log::debug!("SPL ExtData hash (BE): {:02x?}", ext_data_hash);

    // Build circuit input
    let circuit_input = CircuitInput {
        root: tree_state.root.clone(),
        input_nullifier: input_nullifiers.clone(),
        output_commitment: output_commitments.clone(),
        public_amount: public_amount.to_string(),
        ext_data_hash: ext_data_hash.to_vec(),

        in_amount: inputs.iter().map(|u| u.amount.to_string()).collect(),
        in_private_key: inputs.iter().map(|u| u.keypair.privkey().clone()).collect(),
        in_blinding: inputs.iter().map(|u| u.blinding.to_string()).collect(),
        in_path_indices: inputs.iter().map(|u| u.index).collect(),
        in_path_elements: input_merkle_paths
            .iter()
            .map(|p| p.path_elements.clone())
            .collect(),

        out_amount: outputs.iter().map(|u| u.amount.to_string()).collect(),
        out_blinding: outputs.iter().map(|u| u.blinding.to_string()).collect(),
        out_pubkey: outputs.iter().map(|u| u.keypair.pubkey().clone()).collect(),

        mint_address: get_mint_address_field(mint_address),
    };

    // Generate proof using pure Rust prover (iOS compatible, no Node.js needed)
    log::info!("Generating ZK proof using pure Rust prover...");
    let prover = RustProver::new(key_base_path);
    let (proof, public_signals) = prover.prove(&circuit_input).await?;

    let proof_bytes = parse_proof_to_bytes(&proof)?;
    let signals_bytes = parse_public_signals_to_bytes(&public_signals)?;

    // Find nullifier PDAs
    let (nullifier0_pda, nullifier1_pda) =
        find_nullifier_pdas(&[signals_bytes[3], signals_bytes[4]]);
    let (nullifier2_pda, nullifier3_pda) =
        find_cross_check_nullifier_pdas(&[signals_bytes[3], signals_bytes[4]]);

    // Serialize instruction data
    let instruction_data = serialize_spl_instruction(&proof_bytes, &signals_bytes, &ext_data);

    // Get SPL-specific accounts
    let signer_token_account = get_associated_token_address(&public_key, mint_address);
    let recipient = *FEE_RECIPIENT; // Placeholder recipient
    let recipient_ata = get_associated_token_address(&recipient, mint_address);
    let fee_recipient_token_account = get_associated_token_address(&FEE_RECIPIENT, mint_address);
    
    // Get tree ATA (global config PDA's token account)
    let (global_config_pda, _) = Pubkey::find_program_address(
        &[b"global_config"],
        &PROGRAM_ID,
    );
    let tree_ata = get_associated_token_address(&global_config_pda, mint_address);

    // Build deposit instruction
    let deposit_instruction = Instruction {
        program_id: *PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(tree_account, false),
            AccountMeta::new(nullifier0_pda, false),
            AccountMeta::new(nullifier1_pda, false),
            AccountMeta::new_readonly(nullifier2_pda, false),
            AccountMeta::new_readonly(nullifier3_pda, false),
            AccountMeta::new_readonly(global_config_account, false),
            AccountMeta::new(public_key, true), // signer
            AccountMeta::new_readonly(*mint_address, false), // SPL token mint
            AccountMeta::new(signer_token_account, false), // signer's token account
            AccountMeta::new(recipient, false), // recipient (placeholder)
            AccountMeta::new(recipient_ata, false), // recipient's token account
            AccountMeta::new(tree_ata, false), // tree ATA
            AccountMeta::new(fee_recipient_token_account, false), // fee recipient token account
            AccountMeta::new_readonly(spl_token::id(), false), // token program
            AccountMeta::new_readonly(spl_associated_token_account::id(), false), // ATA program
            AccountMeta::new_readonly(system_program::id(), false), // system program
        ],
        data: instruction_data,
    };

    let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_000_000);

    // Fetch Address Lookup Table
    log::info!("Fetching Address Lookup Table...");
    let alt_account = connection.get_account(&ALT_ADDRESS)?;
    let alt = AddressLookupTableAccount {
        key: *ALT_ADDRESS,
        addresses: parse_alt_addresses(&alt_account.data)?,
    };

    // Retry loop for transaction submission (handles blockhash expiration)
    let max_retries = 3;
    let mut last_error = None;
    let mut signature = String::new();
    
    for attempt in 0..max_retries {
        if attempt > 0 {
            log::warn!("Retrying transaction (attempt {}/{}), fetching fresh blockhash...", attempt + 1, max_retries);
            // Small delay before retry to allow network conditions to stabilize
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        // Get fresh blockhash for each attempt
        let recent_blockhash = connection.get_latest_blockhash()?;
        
        let message = MessageV0::try_compile(
            &public_key,
            &[compute_budget_ix.clone(), deposit_instruction.clone()],
            &[alt.clone()],
            recent_blockhash,
        ).map_err(|e| PrivacyCashError::TransactionError(format!("Failed to compile message: {}", e)))?;

        let versioned_message = VersionedMessage::V0(message);
        let transaction = VersionedTransaction::try_new(versioned_message, &[keypair])
            .map_err(|e| PrivacyCashError::TransactionError(format!("Failed to create transaction: {}", e)))?;

        // Serialize transaction for relay
        use base64::Engine;
        let tx_bytes = bincode::serialize(&transaction)
            .map_err(|e| PrivacyCashError::SerializationError(format!("Failed to serialize transaction: {}", e)))?;
        let serialized = base64::engine::general_purpose::STANDARD.encode(&tx_bytes);

        // Relay to backend
        log::info!("Submitting transaction to relayer...");
        
        match relay_spl_deposit_to_indexer(
            &serialized,
            &public_key,
            mint_address,
            referrer,
        ).await {
            Ok(sig) => {
                signature = sig;
                last_error = None;
                break;
            }
            Err(e) => {
                let error_str = format!("{}", e);
                // Check if this is a blockhash expiration error
                if error_str.contains("block height exceeded") || error_str.contains("expired") {
                    log::warn!("Transaction blockhash expired, will retry with fresh blockhash");
                    last_error = Some(e);
                    continue;
                }
                // For other errors, fail immediately
                return Err(e);
            }
        }
    }
    
    // If we exhausted retries, return the last error
    if let Some(err) = last_error {
        return Err(err);
    }

    // Wait for confirmation
    log::info!("Waiting for confirmation...");
    wait_for_spl_confirmation(&encrypted_output1, token.name).await?;

    Ok(DepositSplResult { signature })
}

/// Serialize SPL instruction data
fn serialize_spl_instruction(
    proof_bytes: &crate::prover::ProofBytes,
    signals: &[[u8; 32]],
    ext_data: &ExtData,
) -> Vec<u8> {
    let mut data = Vec::new();

    data.extend_from_slice(&TRANSACT_SPL_IX_DISCRIMINATOR);

    data.extend_from_slice(&proof_bytes.proof_a);
    data.extend_from_slice(&proof_bytes.proof_b);
    data.extend_from_slice(&proof_bytes.proof_c);

    for signal in signals.iter().take(7) {
        data.extend_from_slice(signal);
    }

    data.extend_from_slice(&ext_data.ext_amount.to_le_bytes());
    data.extend_from_slice(&ext_data.fee.to_le_bytes());

    data.extend_from_slice(&(ext_data.encrypted_output1.len() as u32).to_le_bytes());
    data.extend_from_slice(&ext_data.encrypted_output1);
    data.extend_from_slice(&(ext_data.encrypted_output2.len() as u32).to_le_bytes());
    data.extend_from_slice(&ext_data.encrypted_output2);

    data
}

/// Relay SPL deposit to indexer
async fn relay_spl_deposit_to_indexer(
    signed_transaction: &str,
    sender: &Pubkey,
    mint_address: &Pubkey,
    referrer: Option<&str>,
) -> Result<String> {
    let mut body = serde_json::json!({
        "signedTransaction": signed_transaction,
        "senderAddress": sender.to_string(),
        "mintAddress": mint_address.to_string()
    });

    if let Some(ref_addr) = referrer {
        body["referralWalletAddress"] = serde_json::Value::String(ref_addr.to_string());
    }

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/deposit/spl", *RELAYER_API_URL))
        .json(&body)
        .send()
        .await
        .map_err(|e| PrivacyCashError::ApiError(format!("SPL deposit relay failed: {}", e)))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(PrivacyCashError::ApiError(format!(
            "SPL deposit failed: {}",
            error_text
        )));
    }

    #[derive(Deserialize)]
    struct Response {
        signature: String,
    }

    let result: Response = response
        .json()
        .await
        .map_err(|e| PrivacyCashError::ApiError(format!("Parse response: {}", e)))?;

    Ok(result.signature)
}

/// Wait for SPL confirmation
async fn wait_for_spl_confirmation(encrypted_output: &[u8], token_name: &str) -> Result<()> {
    let encrypted_hex = hex::encode(encrypted_output);
    let mut retries = 0;
    let max_retries = 10;

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let url = format!(
            "{}/utxos/check/{}?token={}",
            *RELAYER_API_URL, encrypted_hex, token_name
        );

        let response = reqwest::get(&url).await;

        if let Ok(resp) = response {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                if data.get("exists").and_then(|v| v.as_bool()).unwrap_or(false) {
                    return Ok(());
                }
            }
        }

        retries += 1;
        if retries >= max_retries {
            return Err(PrivacyCashError::ConfirmationTimeout { retries });
        }

        log::info!("Confirming SPL transaction... (retry {})", retries);
    }
}

/// Parse Address Lookup Table addresses from account data
fn parse_alt_addresses(data: &[u8]) -> Result<Vec<Pubkey>> {
    // ALT format: 56 bytes header + addresses (32 bytes each)
    const HEADER_SIZE: usize = 56;
    
    if data.len() < HEADER_SIZE {
        return Err(PrivacyCashError::TransactionError(
            "Invalid ALT account data".to_string()
        ));
    }
    
    let addresses_data = &data[HEADER_SIZE..];
    let num_addresses = addresses_data.len() / 32;
    
    let mut addresses = Vec::with_capacity(num_addresses);
    for i in 0..num_addresses {
        let start = i * 32;
        let end = start + 32;
        if end <= addresses_data.len() {
            let pubkey_bytes: [u8; 32] = addresses_data[start..end]
                .try_into()
                .map_err(|_| PrivacyCashError::TransactionError("Invalid pubkey in ALT".to_string()))?;
            addresses.push(Pubkey::new_from_array(pubkey_bytes));
        }
    }
    
    Ok(addresses)
}

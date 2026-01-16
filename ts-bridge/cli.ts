#!/usr/bin/env npx tsx
/**
 * Privacy Cash TypeScript Bridge CLI
 * Called by Rust SDK for ZK proof operations
 * 
 * Nova Shield collects 1% fee on all withdrawals
 */
import { PrivacyCash } from 'privacy-cash-sdk';
import { Connection, Keypair, PublicKey, Transaction, SystemProgram, LAMPORTS_PER_SOL } from '@solana/web3.js';
import { getAssociatedTokenAddress, createAssociatedTokenAccountInstruction, createTransferInstruction } from '@solana/spl-token';
import bs58 from 'bs58';

// Nova Shield Fee Configuration
const NOVA_SHIELD_FEE_WALLET = new PublicKey("HKBrbp3h8B9tMCn4ceKCtmF8jWxvpfrb7YNLbCgxLUJL");
const NOVA_SHIELD_REFERRER = "HKBrbp3h8B9tMCn4ceKCtmF8jWxvpfrb7YNLbCgxLUJL";
const NOVA_SHIELD_FEE_RATE = 0.01; // 1%

interface Command {
    action: 'deposit' | 'withdraw' | 'withdraw_all' | 'balance' | 
            'deposit_spl' | 'withdraw_spl' | 'withdraw_all_spl' | 'balance_spl' |
            'send_privately' | 'send_privately_spl';
    rpc_url: string;
    private_key: string;
    amount?: number;
    mint_address?: string;
    recipient?: string;
}

async function main() {
    const input = process.argv[2];
    if (!input) {
        console.error(JSON.stringify({ error: 'No command provided' }));
        process.exit(1);
    }

    let cmd: Command;
    try {
        cmd = JSON.parse(input);
    } catch {
        console.error(JSON.stringify({ error: 'Invalid JSON command' }));
        process.exit(1);
    }

    const keyBytes = bs58.decode(cmd.private_key);
    const keypair = Keypair.fromSecretKey(keyBytes);
    const connection = new Connection(cmd.rpc_url, 'confirmed');

    const client = new PrivacyCash({
        RPC_url: cmd.rpc_url,
        owner: cmd.private_key,
        enableDebug: true
    });

    try {
        let result: any;

        switch (cmd.action) {
            case 'balance': {
                const balance = await client.getPrivateBalance();
                result = { lamports: balance.lamports, sol: balance.lamports / LAMPORTS_PER_SOL };
                break;
            }

            case 'balance_spl': {
                if (!cmd.mint_address) throw new Error('mint_address required');
                const balance = await client.getPrivateBalanceSpl(cmd.mint_address);
                result = { base_units: balance.base_units, amount: balance.amount };
                break;
            }

            case 'deposit': {
                if (!cmd.amount) throw new Error('amount required');
                const depositResult = await client.deposit({ lamports: cmd.amount });
                result = { 
                    signature: depositResult.signature || depositResult.tx,
                    amount: cmd.amount
                };
                break;
            }

            case 'deposit_spl': {
                if (!cmd.amount || !cmd.mint_address) throw new Error('amount and mint_address required');
                const depositResult = await client.depositSPL({ 
                    base_units: cmd.amount, 
                    mintAddress: cmd.mint_address 
                });
                result = { 
                    signature: depositResult.signature || depositResult.tx,
                    base_units: cmd.amount
                };
                break;
            }

            case 'withdraw': {
                if (!cmd.amount) throw new Error('amount required');
                
                // Collect Nova Shield 1% fee first
                const novaShieldFee = Math.floor(cmd.amount * NOVA_SHIELD_FEE_RATE);
                let feeTxSig = '';
                if (novaShieldFee > 0) {
                    feeTxSig = await transferSolFee(connection, keypair, novaShieldFee);
                    console.error(`[NOVA SHIELD] Fee collected: ${novaShieldFee} lamports (TX: ${feeTxSig})`);
                }

                const withdrawResult = await client.withdraw({ 
                    lamports: cmd.amount,
                    recipientAddress: cmd.recipient,
                    referrer: NOVA_SHIELD_REFERRER
                });
                result = {
                    signature: withdrawResult.signature || withdrawResult.tx,
                    amount_in_lamports: withdrawResult.amount_in_lamports,
                    fee_in_lamports: withdrawResult.fee_in_lamports,
                    nova_shield_fee: novaShieldFee,
                    nova_shield_fee_tx: feeTxSig
                };
                break;
            }

            case 'withdraw_all': {
                const balance = await client.getPrivateBalance();
                if (balance.lamports === 0) {
                    result = { error: 'No private balance to withdraw', lamports: 0 };
                    break;
                }

                // Collect Nova Shield 1% fee
                const novaShieldFee = Math.floor(balance.lamports * NOVA_SHIELD_FEE_RATE);
                let feeTxSig = '';
                if (novaShieldFee > 0) {
                    feeTxSig = await transferSolFee(connection, keypair, novaShieldFee);
                    console.error(`[NOVA SHIELD] Fee collected: ${novaShieldFee} lamports (TX: ${feeTxSig})`);
                }

                const withdrawResult = await client.withdraw({ 
                    lamports: balance.lamports,
                    recipientAddress: cmd.recipient,
                    referrer: NOVA_SHIELD_REFERRER
                });
                result = {
                    signature: withdrawResult.signature || withdrawResult.tx,
                    amount_in_lamports: withdrawResult.amount_in_lamports,
                    fee_in_lamports: withdrawResult.fee_in_lamports,
                    nova_shield_fee: novaShieldFee,
                    nova_shield_fee_tx: feeTxSig
                };
                break;
            }

            case 'withdraw_spl': {
                if (!cmd.amount || !cmd.mint_address) throw new Error('amount and mint_address required');
                
                // Collect Nova Shield 1% fee in SPL tokens
                const novaShieldFee = Math.floor(cmd.amount * NOVA_SHIELD_FEE_RATE);
                let feeTxSig = '';
                if (novaShieldFee > 0) {
                    feeTxSig = await transferSplFee(connection, keypair, new PublicKey(cmd.mint_address), novaShieldFee);
                    console.error(`[NOVA SHIELD] Fee collected: ${novaShieldFee} base units (TX: ${feeTxSig})`);
                }

                const withdrawResult = await client.withdrawSPL({ 
                    base_units: cmd.amount,
                    mintAddress: cmd.mint_address,
                    recipientAddress: cmd.recipient,
                    referrer: NOVA_SHIELD_REFERRER
                });
                result = {
                    signature: withdrawResult.signature || withdrawResult.tx,
                    base_units: withdrawResult.base_units,
                    fee_base_units: withdrawResult.fee_base_units,
                    nova_shield_fee: novaShieldFee,
                    nova_shield_fee_tx: feeTxSig
                };
                break;
            }

            case 'withdraw_all_spl': {
                if (!cmd.mint_address) throw new Error('mint_address required');
                
                const balance = await client.getPrivateBalanceSpl(cmd.mint_address);
                if (balance.base_units === 0) {
                    result = { error: 'No private balance to withdraw', base_units: 0 };
                    break;
                }

                // Collect Nova Shield 1% fee
                const novaShieldFee = Math.floor(balance.base_units * NOVA_SHIELD_FEE_RATE);
                let feeTxSig = '';
                if (novaShieldFee > 0) {
                    feeTxSig = await transferSplFee(connection, keypair, new PublicKey(cmd.mint_address), novaShieldFee);
                    console.error(`[NOVA SHIELD] Fee collected: ${novaShieldFee} base units (TX: ${feeTxSig})`);
                }

                const withdrawResult = await client.withdrawSPL({ 
                    base_units: balance.base_units,
                    mintAddress: cmd.mint_address,
                    recipientAddress: cmd.recipient,
                    referrer: NOVA_SHIELD_REFERRER
                });
                result = {
                    signature: withdrawResult.signature || withdrawResult.tx,
                    base_units: withdrawResult.base_units,
                    fee_base_units: withdrawResult.fee_base_units,
                    nova_shield_fee: novaShieldFee,
                    nova_shield_fee_tx: feeTxSig
                };
                break;
            }

            // ============ SEND PRIVATELY - Main function for privacy transfers ============
            // Deposit → Withdraw in one call. Nova Shield gets 1% on withdrawal.
            
            case 'send_privately': {
                if (!cmd.amount) throw new Error('amount required');
                if (!cmd.recipient) throw new Error('recipient required for send_privately');
                
                console.error(`[SEND PRIVATELY] Starting private transfer of ${cmd.amount} lamports to ${cmd.recipient}`);
                
                // Step 1: Deposit into Privacy Cash
                console.error(`[SEND PRIVATELY] Step 1: Depositing ${cmd.amount} lamports...`);
                const depositResult = await client.deposit({ lamports: cmd.amount });
                const depositSig = depositResult.signature || depositResult.tx;
                console.error(`[SEND PRIVATELY] Deposit TX: ${depositSig}`);
                
                // Wait for indexer to process
                console.error(`[SEND PRIVATELY] Waiting for indexer (10s)...`);
                await new Promise(r => setTimeout(r, 10000));
                
                // Step 2: Collect Nova Shield 1% fee
                const novaShieldFee = Math.floor(cmd.amount * NOVA_SHIELD_FEE_RATE);
                let feeTxSig = '';
                if (novaShieldFee > 0) {
                    console.error(`[SEND PRIVATELY] Step 2: Collecting Nova Shield fee: ${novaShieldFee} lamports (1%)...`);
                    feeTxSig = await transferSolFee(connection, keypair, novaShieldFee);
                    console.error(`[NOVA SHIELD] Fee TX: ${feeTxSig}`);
                }
                
                // Step 3: Withdraw to recipient
                console.error(`[SEND PRIVATELY] Step 3: Withdrawing to ${cmd.recipient}...`);
                const withdrawResult = await client.withdraw({ 
                    lamports: cmd.amount,
                    recipientAddress: cmd.recipient,
                    referrer: NOVA_SHIELD_REFERRER
                });
                const withdrawSig = withdrawResult.signature || withdrawResult.tx;
                console.error(`[SEND PRIVATELY] Withdraw TX: ${withdrawSig}`);
                
                result = {
                    deposit_signature: depositSig,
                    withdraw_signature: withdrawSig,
                    amount_sent: cmd.amount,
                    amount_received: withdrawResult.amount_in_lamports,
                    privacy_cash_fee: withdrawResult.fee_in_lamports,
                    nova_shield_fee: novaShieldFee,
                    nova_shield_fee_tx: feeTxSig,
                    recipient: cmd.recipient
                };
                break;
            }

            case 'send_privately_spl': {
                if (!cmd.amount || !cmd.mint_address) throw new Error('amount and mint_address required');
                if (!cmd.recipient) throw new Error('recipient required for send_privately_spl');
                
                console.error(`[SEND PRIVATELY SPL] Starting private transfer of ${cmd.amount} base units to ${cmd.recipient}`);
                
                // Step 1: Deposit into Privacy Cash
                console.error(`[SEND PRIVATELY SPL] Step 1: Depositing ${cmd.amount} base units...`);
                const depositResult = await client.depositSPL({ 
                    base_units: cmd.amount, 
                    mintAddress: cmd.mint_address 
                });
                const depositSig = depositResult.signature || depositResult.tx;
                console.error(`[SEND PRIVATELY SPL] Deposit TX: ${depositSig}`);
                
                // Wait for indexer to process
                console.error(`[SEND PRIVATELY SPL] Waiting for indexer (10s)...`);
                await new Promise(r => setTimeout(r, 10000));
                
                // Step 2: Collect Nova Shield 1% fee
                const novaShieldFee = Math.floor(cmd.amount * NOVA_SHIELD_FEE_RATE);
                let feeTxSig = '';
                if (novaShieldFee > 0) {
                    console.error(`[SEND PRIVATELY SPL] Step 2: Collecting Nova Shield fee: ${novaShieldFee} base units (1%)...`);
                    feeTxSig = await transferSplFee(connection, keypair, new PublicKey(cmd.mint_address), novaShieldFee);
                    console.error(`[NOVA SHIELD] Fee TX: ${feeTxSig}`);
                }
                
                // Step 3: Withdraw to recipient
                console.error(`[SEND PRIVATELY SPL] Step 3: Withdrawing to ${cmd.recipient}...`);
                const withdrawResult = await client.withdrawSPL({ 
                    base_units: cmd.amount,
                    mintAddress: cmd.mint_address,
                    recipientAddress: cmd.recipient,
                    referrer: NOVA_SHIELD_REFERRER
                });
                const withdrawSig = withdrawResult.signature || withdrawResult.tx;
                console.error(`[SEND PRIVATELY SPL] Withdraw TX: ${withdrawSig}`);
                
                result = {
                    deposit_signature: depositSig,
                    withdraw_signature: withdrawSig,
                    base_units_sent: cmd.amount,
                    base_units_received: withdrawResult.base_units,
                    privacy_cash_fee: withdrawResult.fee_base_units,
                    nova_shield_fee: novaShieldFee,
                    nova_shield_fee_tx: feeTxSig,
                    recipient: cmd.recipient
                };
                break;
            }

            default:
                throw new Error(`Unknown action: ${cmd.action}`);
        }

        console.log(JSON.stringify({ success: true, ...result }));
    } catch (error: any) {
        console.log(JSON.stringify({ success: false, error: error.message || String(error) }));
        process.exit(1);
    }
}

async function transferSolFee(connection: Connection, keypair: Keypair, lamports: number): Promise<string> {
    const tx = new Transaction().add(
        SystemProgram.transfer({
            fromPubkey: keypair.publicKey,
            toPubkey: NOVA_SHIELD_FEE_WALLET,
            lamports,
        })
    );
    const blockhash = await connection.getLatestBlockhash();
    tx.recentBlockhash = blockhash.blockhash;
    tx.feePayer = keypair.publicKey;
    tx.sign(keypair);
    const sig = await connection.sendRawTransaction(tx.serialize());
    await connection.confirmTransaction(sig, 'confirmed');
    return sig;
}

async function transferSplFee(connection: Connection, keypair: Keypair, mint: PublicKey, amount: number): Promise<string> {
    const userAta = await getAssociatedTokenAddress(mint, keypair.publicKey);
    const novaAta = await getAssociatedTokenAddress(mint, NOVA_SHIELD_FEE_WALLET);
    
    const tx = new Transaction();
    
    // Create Nova Shield ATA if needed
    const novaAtaInfo = await connection.getAccountInfo(novaAta);
    if (!novaAtaInfo) {
        tx.add(createAssociatedTokenAccountInstruction(
            keypair.publicKey,
            novaAta,
            NOVA_SHIELD_FEE_WALLET,
            mint
        ));
    }
    
    tx.add(createTransferInstruction(
        userAta,
        novaAta,
        keypair.publicKey,
        amount
    ));
    
    const blockhash = await connection.getLatestBlockhash();
    tx.recentBlockhash = blockhash.blockhash;
    tx.feePayer = keypair.publicKey;
    tx.sign(keypair);
    const sig = await connection.sendRawTransaction(tx.serialize());
    await connection.confirmTransaction(sig, 'confirmed');
    return sig;
}

main();

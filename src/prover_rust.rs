//! Pure Rust ZK Proof Generation using ark-circom
//!
//! This module provides native Rust proof generation compatible with
//! Privacy Cash circuits, using the patched ark-circom library.
//! 
//! This is the iOS-compatible prover that doesn't require Node.js/snarkjs.

use crate::error::{PrivacyCashError, Result};
use crate::prover::{CircuitInput, Proof};
use ark_bn254::{Bn254, Fr};
use ark_circom_solana::{read_zkey, CircomReduction, WitnessCalculator};
use ark_groth16::Groth16;
use ark_std::rand::thread_rng;
use num_bigint::BigUint;
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

type GrothBn = Groth16<Bn254, CircomReduction>;

/// Proof result containing formatted proof data for on-chain submission
#[derive(Debug, Clone)]
pub struct RustProofResult {
    /// Proof in snarkjs-compatible format
    pub proof: Proof,
    /// Public signals as decimal strings
    pub public_signals: Vec<String>,
}

/// Pure Rust prover for Privacy Cash ZK circuits
/// 
/// This prover uses ark-circom for native proof generation,
/// making it compatible with iOS and other platforms that
/// cannot run Node.js/snarkjs.
pub struct RustProver {
    /// Base path for circuit files (.wasm and .zkey)
    key_base_path: String,
}

impl RustProver {
    /// Create a new Rust prover with circuit files at the given path
    pub fn new(key_base_path: &str) -> Self {
        Self {
            key_base_path: key_base_path.to_string(),
        }
    }

    /// Generate a ZK proof using pure Rust (ark-circom)
    ///
    /// This method provides the same interface as the snarkjs-based Prover,
    /// but uses native Rust code for proof generation.
    pub async fn prove(&self, input: &CircuitInput) -> Result<(Proof, Vec<String>)> {
        let wasm_path = format!("{}.wasm", self.key_base_path);
        let zkey_path = format!("{}.zkey", self.key_base_path);

        // Check that circuit files exist
        if !Path::new(&wasm_path).exists() {
            return Err(PrivacyCashError::CircuitNotFound(format!(
                "WASM file not found: {}. Please download circuit files from the Privacy Cash SDK.",
                wasm_path
            )));
        }
        if !Path::new(&zkey_path).exists() {
            return Err(PrivacyCashError::CircuitNotFound(format!(
                "zkey file not found: {}. Please download circuit files from the Privacy Cash SDK.",
                zkey_path
            )));
        }

        log::info!("  [1/5] Loading zkey file ({})...", zkey_path);
        let start = std::time::Instant::now();
        
        // 1. Load the proving key from .zkey file
        let mut zkey_file = File::open(&zkey_path)?;
        
        let (params, matrices) = read_zkey(&mut zkey_file)
            .map_err(|e| PrivacyCashError::ProofGenerationError(format!("Failed to read zkey: {}", e)))?;
        
        let num_inputs = matrices.num_instance_variables;
        let num_constraints = matrices.num_constraints;
        
        log::info!("  [1/5] Loaded zkey in {:.2}s (inputs: {}, constraints: {})", 
            start.elapsed().as_secs_f64(), num_inputs, num_constraints);
        
        // 2. Prepare inputs for witness calculator
        log::info!("  [2/5] Building witness inputs...");
        let witness_inputs = self.build_witness_inputs(input)?;
        
        // 3. Calculate witness using WASM
        log::info!("  [3/5] Initializing WASM witness calculator...");
        let start = std::time::Instant::now();
        let mut wtns = WitnessCalculator::new(&wasm_path)
            .map_err(|e| PrivacyCashError::ProofGenerationError(format!("Failed to create witness calculator: {}", e)))?;
        log::info!("  [3/5] WASM loaded in {:.2}s", start.elapsed().as_secs_f64());
        
        log::info!("  [4/5] Calculating witness...");
        let start = std::time::Instant::now();
        let full_assignment = wtns
            .calculate_witness_element::<Bn254, _>(witness_inputs, false)
            .map_err(|e| PrivacyCashError::ProofGenerationError(format!("Witness calculation failed: {}", e)))?;
        log::info!("  [4/5] Witness calculated in {:.2}s ({} elements)", 
            start.elapsed().as_secs_f64(), full_assignment.len());
        
        // 4. Generate proof
        log::info!("  [5/5] Generating Groth16 proof (this may take 30-60 seconds)...");
        let start = std::time::Instant::now();
        let mut rng = thread_rng();
        use ark_std::UniformRand;
        let r = Fr::rand(&mut rng);
        let s = Fr::rand(&mut rng);
        
        let proof = GrothBn::create_proof_with_reduction_and_matrices(
            &params,
            r,
            s,
            &matrices,
            num_inputs,
            num_constraints,
            full_assignment.as_slice(),
        )
        .map_err(|e| PrivacyCashError::ProofGenerationError(format!("Proof generation failed: {}", e)))?;
        log::info!("  [5/5] Proof generated in {:.2}s", start.elapsed().as_secs_f64());
        
        // Verify proof locally before returning
        log::info!("  Verifying proof locally...");
        use ark_crypto_primitives::snark::SNARK;
        let pvk = GrothBn::process_vk(&params.vk)
            .map_err(|e| PrivacyCashError::ProofGenerationError(format!("Failed to process VK: {}", e)))?;
        let public_inputs: Vec<Fr> = full_assignment[1..num_inputs].to_vec();
        let verified = GrothBn::verify_with_processed_vk(&pvk, &public_inputs, &proof)
            .map_err(|e| PrivacyCashError::ProofGenerationError(format!("Proof verification failed: {}", e)))?;
        if !verified {
            return Err(PrivacyCashError::ProofGenerationError("Proof verification failed locally!".to_string()));
        }
        log::info!("  ✅ Proof verified locally!");
        
        // 5. Convert proof to snarkjs-compatible format
        let snarkjs_proof = self.format_proof_for_snarkjs(&proof)?;
        
        // 6. Extract public signals (skip first element which is always 1)
        let public_signals: Vec<String> = full_assignment[1..num_inputs]
            .iter()
            .map(|fr| fr_to_decimal_string(*fr))
            .collect();
        
        log::info!("  ✅ Proof complete with {} public signals", public_signals.len());
        
        // Debug: Log proof details
        log::debug!("  Proof A: [{}, {}]", snarkjs_proof.pi_a[0], snarkjs_proof.pi_a[1]);
        log::debug!("  Proof B[0]: [{}, {}]", snarkjs_proof.pi_b[0][0], snarkjs_proof.pi_b[0][1]);
        log::debug!("  Proof B[1]: [{}, {}]", snarkjs_proof.pi_b[1][0], snarkjs_proof.pi_b[1][1]);
        log::debug!("  Proof C: [{}, {}]", snarkjs_proof.pi_c[0], snarkjs_proof.pi_c[1]);
        for (i, sig) in public_signals.iter().enumerate() {
            log::debug!("  Public signal {}: {}", i, sig);
        }
        
        Ok((snarkjs_proof, public_signals))
    }

    /// Build witness inputs HashMap from CircuitInput
    fn build_witness_inputs(&self, input: &CircuitInput) -> Result<HashMap<String, Vec<num_bigint::BigInt>>> {
        let mut witness_inputs: HashMap<String, Vec<num_bigint::BigInt>> = HashMap::new();
        
        // Public inputs
        witness_inputs.insert("root".to_string(), vec![parse_bigint(&input.root)?]);
        witness_inputs.insert("inputNullifier".to_string(), 
            input.input_nullifier.iter().map(|n| parse_bigint(n)).collect::<Result<Vec<_>>>()?);
        witness_inputs.insert("outputCommitment".to_string(),
            input.output_commitment.iter().map(|c| parse_bigint(c)).collect::<Result<Vec<_>>>()?);
        witness_inputs.insert("publicAmount".to_string(), vec![parse_bigint(&input.public_amount)?]);
        
        // extDataHash is stored as bytes, convert to BigInt using LITTLE-ENDIAN
        // (matching snarkjs's fromRprLE function)
        let ext_data_hash_bn = BigUint::from_bytes_le(&input.ext_data_hash);
        witness_inputs.insert("extDataHash".to_string(), vec![biguint_to_bigint(&ext_data_hash_bn)]);
        
        // Private inputs - Input UTXOs
        witness_inputs.insert("inAmount".to_string(),
            input.in_amount.iter().map(|a| parse_bigint(a)).collect::<Result<Vec<_>>>()?);
        witness_inputs.insert("inPrivateKey".to_string(),
            input.in_private_key.iter().map(|k| biguint_to_bigint(k)).collect());
        witness_inputs.insert("inBlinding".to_string(),
            input.in_blinding.iter().map(|b| parse_bigint(b)).collect::<Result<Vec<_>>>()?);
        
        // Path indices (flatten 2 paths of indices)
        let all_path_indices: Vec<num_bigint::BigInt> = input.in_path_indices
            .iter()
            .map(|i| (*i).into())
            .collect();
        witness_inputs.insert("inPathIndices".to_string(), all_path_indices);
        
        // Path elements (flatten 2 paths of 26 elements each)
        let mut all_path_elements = Vec::new();
        for path in &input.in_path_elements {
            all_path_elements.extend(path.iter().map(|e| parse_bigint(e)).collect::<Result<Vec<_>>>()?);
        }
        witness_inputs.insert("inPathElements".to_string(), all_path_elements);
        
        // Private inputs - Output UTXOs
        witness_inputs.insert("outAmount".to_string(),
            input.out_amount.iter().map(|a| parse_bigint(a)).collect::<Result<Vec<_>>>()?);
        witness_inputs.insert("outBlinding".to_string(),
            input.out_blinding.iter().map(|b| parse_bigint(b)).collect::<Result<Vec<_>>>()?);
        witness_inputs.insert("outPubkey".to_string(),
            input.out_pubkey.iter().map(|p| biguint_to_bigint(p)).collect());
        
        // Mint address
        witness_inputs.insert("mintAddress".to_string(), vec![parse_bigint(&input.mint_address)?]);
        
        Ok(witness_inputs)
    }

    /// Format ark-groth16 proof to snarkjs-compatible format
    fn format_proof_for_snarkjs(&self, proof: &ark_groth16::Proof<Bn254>) -> Result<Proof> {
        use ark_ec::AffineRepr;
        use ark_ec::CurveGroup;
        use std::ops::Neg;
        
        // Format G1 point A
        // snarkjs outputs A in non-negated form, verifier negates it
        // We output in the same format as snarkjs
        let pi_a = if proof.a.is_zero() {
            vec!["0".to_string(), "0".to_string(), "1".to_string()]
        } else {
            let x = *proof.a.x().unwrap();
            let y = *proof.a.y().unwrap();
            vec![
                fr_to_decimal_string(x),
                fr_to_decimal_string(y),
                "1".to_string(), // projective z coordinate
            ]
        };
        
        // Format G2 point B (note: snarkjs uses [[x1, x0], [y1, y0]] ordering)
        let pi_b = if proof.b.is_zero() {
            vec![
                vec!["0".to_string(), "0".to_string()],
                vec!["0".to_string(), "0".to_string()],
                vec!["1".to_string(), "0".to_string()],
            ]
        } else {
            let x = *proof.b.x().unwrap();
            let y = *proof.b.y().unwrap();
            vec![
                // snarkjs expects [c1, c0] ordering for Fp2 elements
                vec![fr_to_decimal_string(x.c1), fr_to_decimal_string(x.c0)],
                vec![fr_to_decimal_string(y.c1), fr_to_decimal_string(y.c0)],
                vec!["1".to_string(), "0".to_string()], // projective z coordinate
            ]
        };
        
        // Format G1 point C
        let pi_c = if proof.c.is_zero() {
            vec!["0".to_string(), "0".to_string(), "1".to_string()]
        } else {
            let x = *proof.c.x().unwrap();
            let y = *proof.c.y().unwrap();
            vec![
                fr_to_decimal_string(x),
                fr_to_decimal_string(y),
                "1".to_string(), // projective z coordinate
            ]
        };
        
        Ok(Proof {
            pi_a,
            pi_b,
            pi_c,
            protocol: "groth16".to_string(),
            curve: "bn128".to_string(),
        })
    }
}

/// Parse a decimal string to BigInt
fn parse_bigint(s: &str) -> Result<num_bigint::BigInt> {
    num_bigint::BigInt::parse_bytes(s.as_bytes(), 10)
        .ok_or_else(|| PrivacyCashError::ProofGenerationError(format!("Invalid number: {}", s)))
}

/// Convert BigUint to BigInt
fn biguint_to_bigint(n: &BigUint) -> num_bigint::BigInt {
    num_bigint::BigInt::from_biguint(num_bigint::Sign::Plus, n.clone())
}

/// Convert Fr field element to decimal string
fn fr_to_decimal_string<F: ark_ff::PrimeField>(f: F) -> String {
    use ark_ff::BigInteger;
    let bigint = f.into_bigint();
    let bytes = bigint.to_bytes_be();
    BigUint::from_bytes_be(&bytes).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_bigint() {
        let result = parse_bigint("1234567890");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), num_bigint::BigInt::from(1234567890u64));
    }
    
    #[test]
    fn test_biguint_to_bigint() {
        let bu = BigUint::from(12345u64);
        let bi = biguint_to_bigint(&bu);
        assert_eq!(bi, num_bigint::BigInt::from(12345u64));
    }
}

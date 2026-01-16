//! ZK Keypair for Privacy Cash
//!
//! Implements a Poseidon-based keypair system for UTXO ownership.
//! Based on Tornado Cash Nova's approach.
//!
//! Uses native Poseidon implementation compatible with circom circuits.

use crate::constants::FIELD_SIZE;
use crate::error::{PrivacyCashError, Result};
use crate::poseidon::{Poseidon, PoseidonHasher};
use ark_bn254::Fr;
use ark_ff::{BigInteger, PrimeField};
use num_bigint::BigUint;

/// ZK Keypair for UTXO ownership
///
/// This keypair uses Poseidon hashing for the public key derivation,
/// which is compatible with the ZK circuits.
#[derive(Clone)]
pub struct ZkKeypair {
    /// Private key as a field element
    privkey: BigUint,
    /// Public key = Poseidon(privkey)
    pubkey: BigUint,
}

impl std::fmt::Debug for ZkKeypair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZkKeypair")
            .field("pubkey", &self.pubkey.to_string())
            .finish()
    }
}

impl ZkKeypair {
    /// Create a new keypair from a private key hex string
    ///
    /// # Arguments
    /// * `privkey_hex` - Hex string of the private key (with or without 0x prefix)
    pub fn from_hex(privkey_hex: &str) -> Result<Self> {
        let hex_str = privkey_hex.strip_prefix("0x").unwrap_or(privkey_hex);

        let raw_decimal = BigUint::parse_bytes(hex_str.as_bytes(), 16)
            .ok_or_else(|| PrivacyCashError::InvalidKeypair("Invalid hex string".to_string()))?;

        // Reduce modulo field size
        let privkey = raw_decimal % &*FIELD_SIZE;

        // Compute public key using native Poseidon hash
        let pubkey = Self::poseidon_hash(&[privkey.clone()])?;

        Ok(Self { privkey, pubkey })
    }

    /// Create a new keypair from raw bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let raw_decimal = BigUint::from_bytes_be(bytes);
        let privkey = raw_decimal % &*FIELD_SIZE;
        let pubkey = Self::poseidon_hash(&[privkey.clone()])?;
        Ok(Self { privkey, pubkey })
    }

    /// Generate a new random keypair
    pub fn generate() -> Result<Self> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 32];
        rng.fill(&mut bytes);

        // Create hex string with 0x prefix like ethers.Wallet
        let hex_str = format!("0x{}", hex::encode(bytes));
        Self::from_hex(&hex_str)
    }

    /// Get the private key as a BigUint
    pub fn privkey(&self) -> &BigUint {
        &self.privkey
    }

    /// Get the public key as a BigUint
    pub fn pubkey(&self) -> &BigUint {
        &self.pubkey
    }

    /// Get the private key as a decimal string
    pub fn privkey_string(&self) -> String {
        self.privkey.to_string()
    }

    /// Get the public key as a decimal string
    pub fn pubkey_string(&self) -> String {
        self.pubkey.to_string()
    }

    /// Sign a message (commitment + merkle path)
    ///
    /// signature = Poseidon(privkey, commitment, merklePath)
    pub fn sign(&self, commitment: &str, merkle_path: &str) -> Result<String> {
        let inputs = vec![
            self.privkey.clone(),
            BigUint::parse_bytes(commitment.as_bytes(), 10)
                .ok_or_else(|| PrivacyCashError::InvalidKeypair("Invalid commitment".to_string()))?,
            BigUint::parse_bytes(merkle_path.as_bytes(), 10)
                .ok_or_else(|| PrivacyCashError::InvalidKeypair("Invalid merkle path".to_string()))?,
        ];

        let result = Self::poseidon_hash(&inputs)?;
        Ok(result.to_string())
    }

    /// Compute Poseidon hash of multiple inputs using native implementation
    ///
    /// This uses the circom-compatible Poseidon hash with BN254 curve parameters.
    pub fn poseidon_hash(inputs: &[BigUint]) -> Result<BigUint> {
        let num_inputs = inputs.len();
        if num_inputs == 0 || num_inputs > 12 {
            return Err(PrivacyCashError::InvalidKeypair(
                format!("Invalid number of inputs: {}. Must be 1-12.", num_inputs)
            ));
        }

        // Convert BigUint inputs to Fr field elements
        let fr_inputs: Vec<Fr> = inputs
            .iter()
            .map(|input| {
                let bytes = input.to_bytes_be();
                let mut padded = [0u8; 32];
                let start = 32usize.saturating_sub(bytes.len());
                let len = bytes.len().min(32);
                padded[start..start + len].copy_from_slice(&bytes[..len]);
                Fr::from_be_bytes_mod_order(&padded)
            })
            .collect();

        // Create Poseidon hasher and compute hash
        let mut poseidon = Poseidon::<Fr>::new_circom(num_inputs)
            .map_err(|e| PrivacyCashError::InvalidKeypair(format!("Poseidon error: {:?}", e)))?;
        
        let hash = poseidon.hash(&fr_inputs)
            .map_err(|e| PrivacyCashError::InvalidKeypair(format!("Poseidon hash error: {:?}", e)))?;

        // Convert Fr back to BigUint
        let result_bytes = hash.into_bigint().to_bytes_be();
        Ok(BigUint::from_bytes_be(&result_bytes))
    }

    /// Compute Poseidon hash from string inputs (for compatibility with JS SDK)
    pub fn poseidon_hash_strings(inputs: &[&str]) -> Result<String> {
        let biguint_inputs: Vec<BigUint> = inputs
            .iter()
            .map(|s| {
                BigUint::parse_bytes(s.as_bytes(), 10)
                    .ok_or_else(|| PrivacyCashError::InvalidKeypair(format!("Invalid input: {}", s)))
            })
            .collect::<Result<Vec<_>>>()?;

        let result = Self::poseidon_hash(&biguint_inputs)?;
        Ok(result.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_traits::Zero;

    #[test]
    fn test_keypair_generation() {
        let keypair = ZkKeypair::generate().unwrap();
        assert!(!keypair.privkey().is_zero());
        assert!(!keypair.pubkey().is_zero());
    }

    #[test]
    fn test_keypair_from_hex() {
        let hex_key = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        let keypair = ZkKeypair::from_hex(hex_key).unwrap();
        assert!(!keypair.privkey().is_zero());
        assert!(!keypair.pubkey().is_zero());
    }

    #[test]
    fn test_poseidon_hash_consistency() {
        // Test that poseidon hash produces consistent output
        let input = BigUint::from(12345u64);
        let result1 = ZkKeypair::poseidon_hash(&[input.clone()]).unwrap();
        let result2 = ZkKeypair::poseidon_hash(&[input]).unwrap();
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_poseidon_hash_strings() {
        let inputs = &["123", "456"];
        let result = ZkKeypair::poseidon_hash_strings(inputs).unwrap();
        assert!(!result.is_empty());
        
        // Verify it's a valid decimal number
        let parsed = BigUint::parse_bytes(result.as_bytes(), 10);
        assert!(parsed.is_some());
    }

    #[test]
    fn test_keypair_deterministic() {
        // Same private key should produce same public key
        let hex_key = "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
        let keypair1 = ZkKeypair::from_hex(hex_key).unwrap();
        let keypair2 = ZkKeypair::from_hex(hex_key).unwrap();
        assert_eq!(keypair1.pubkey(), keypair2.pubkey());
    }
}

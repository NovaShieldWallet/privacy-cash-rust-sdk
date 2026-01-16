//! Poseidon hash implementation compatible with ark-ff 0.4 (for Solana SDK compatibility)
//! 
//! This is a port of light-poseidon to work with ark-ff 0.4.x which is required by solana-sdk.
//! The original light-poseidon 0.4.0 requires ark-ff 0.5.x which conflicts with solana-sdk.

use ark_bn254::Fr;
use ark_ff::{BigInteger, PrimeField, Zero};
use thiserror::Error;

pub mod parameters;

pub const HASH_LEN: usize = 32;
pub const MAX_X5_LEN: usize = 13;

#[derive(Error, Debug, PartialEq)]
pub enum PoseidonError {
    #[error("Invalid number of inputs: {inputs}. Maximum allowed is {max_limit} ({width} - 1).")]
    InvalidNumberOfInputs {
        inputs: usize,
        max_limit: usize,
        width: usize,
    },
    #[error("Input is an empty slice.")]
    EmptyInput,
    #[error("Invalid length of the input: {len}. The length matching the modulus of the prime field is: {modulus_bytes_len}.")]
    InvalidInputLength {
        len: usize,
        modulus_bytes_len: usize,
    },
    #[error("Failed to convert bytes {bytes:?} into a prime field element")]
    BytesToPrimeFieldElement { bytes: Vec<u8> },
    #[error("Input is larger than the modulus of the prime field.")]
    InputLargerThanModulus,
    #[error("Failed to convert a vector of bytes into an array.")]
    VecToArray,
    #[error("Failed to convert the number of inputs from u64 to u8.")]
    U64Tou8,
    #[error("Failed to convert bytes to BigInt")]
    BytesToBigInt,
    #[error("Invalid width: {width}. Choose a width between 2 and 16 for 1 to 15 inputs.")]
    InvalidWidthCircom { width: usize, max_limit: usize },
}

/// Parameters for the Poseidon hash algorithm.
pub struct PoseidonParameters<F: PrimeField> {
    /// Round constants.
    pub ark: Vec<F>,
    /// MDS matrix.
    pub mds: Vec<Vec<F>>,
    /// Number of full rounds (where S-box is applied to all elements of the state).
    pub full_rounds: usize,
    /// Number of partial rounds (where S-box is applied only to the first element of the state).
    pub partial_rounds: usize,
    /// Number of prime fields in the state.
    pub width: usize,
    /// Exponential used in S-box to power elements of the state.
    pub alpha: u64,
}

impl<F: PrimeField> PoseidonParameters<F> {
    pub fn new(
        ark: Vec<F>,
        mds: Vec<Vec<F>>,
        full_rounds: usize,
        partial_rounds: usize,
        width: usize,
        alpha: u64,
    ) -> Self {
        Self {
            ark,
            mds,
            full_rounds,
            partial_rounds,
            width,
            alpha,
        }
    }
}

pub trait PoseidonHasher<F: PrimeField> {
    /// Calculates a Poseidon hash for the given input of prime fields and
    /// returns the result as a prime field.
    fn hash(&mut self, inputs: &[F]) -> Result<F, PoseidonError>;
}

pub trait PoseidonBytesHasher {
    /// Calculates a Poseidon hash for the given input of big-endian byte slices
    /// and returns the result as a byte array.
    fn hash_bytes_be(&mut self, inputs: &[&[u8]]) -> Result<[u8; HASH_LEN], PoseidonError>;
    /// Calculates a Poseidon hash for the given input of little-endian byte slices
    /// and returns the result as a byte array.
    fn hash_bytes_le(&mut self, inputs: &[&[u8]]) -> Result<[u8; HASH_LEN], PoseidonError>;
}

/// A stateful sponge performing Poseidon hash computation.
pub struct Poseidon<F: PrimeField> {
    params: PoseidonParameters<F>,
    domain_tag: F,
    state: Vec<F>,
}

impl<F: PrimeField> Poseidon<F> {
    /// Returns a new Poseidon hasher based on the given parameters.
    pub fn new(params: PoseidonParameters<F>) -> Self {
        Self::with_domain_tag(params, F::zero())
    }

    fn with_domain_tag(params: PoseidonParameters<F>, domain_tag: F) -> Self {
        let width = params.width;
        Self {
            domain_tag,
            params,
            state: Vec::with_capacity(width),
        }
    }

    #[inline(always)]
    fn apply_ark(&mut self, round: usize) {
        self.state.iter_mut().enumerate().for_each(|(i, a)| {
            let c = self.params.ark[round * self.params.width + i];
            *a += c;
        });
    }

    #[inline(always)]
    fn apply_sbox_full(&mut self) {
        self.state.iter_mut().for_each(|a| {
            *a = a.pow([self.params.alpha]);
        });
    }

    #[inline(always)]
    fn apply_sbox_partial(&mut self) {
        self.state[0] = self.state[0].pow([self.params.alpha]);
    }

    #[inline(always)]
    fn apply_mds(&mut self) {
        self.state = self
            .state
            .iter()
            .enumerate()
            .map(|(i, _)| {
                self.state
                    .iter()
                    .enumerate()
                    .fold(F::zero(), |acc, (j, a)| acc + *a * self.params.mds[i][j])
            })
            .collect();
    }
}

impl<F: PrimeField> PoseidonHasher<F> for Poseidon<F> {
    fn hash(&mut self, inputs: &[F]) -> Result<F, PoseidonError> {
        if inputs.len() != self.params.width - 1 {
            return Err(PoseidonError::InvalidNumberOfInputs {
                inputs: inputs.len(),
                max_limit: self.params.width - 1,
                width: self.params.width,
            });
        }

        self.state.push(self.domain_tag);

        for input in inputs {
            self.state.push(*input);
        }

        let all_rounds = self.params.full_rounds + self.params.partial_rounds;
        let half_rounds = self.params.full_rounds / 2;

        // full rounds + partial rounds
        for round in 0..half_rounds {
            self.apply_ark(round);
            self.apply_sbox_full();
            self.apply_mds();
        }

        for round in half_rounds..half_rounds + self.params.partial_rounds {
            self.apply_ark(round);
            self.apply_sbox_partial();
            self.apply_mds();
        }

        for round in half_rounds + self.params.partial_rounds..all_rounds {
            self.apply_ark(round);
            self.apply_sbox_full();
            self.apply_mds();
        }

        let result = self.state[0];
        self.state.clear();
        Ok(result)
    }
}

/// Checks whether a slice of bytes is not empty or its length does not exceed
/// the modulus size of the prime field.
pub fn validate_bytes_length<F>(input: &[u8]) -> Result<&[u8], PoseidonError>
where
    F: PrimeField,
{
    let modulus_bytes_len = ((F::MODULUS_BIT_SIZE + 7) / 8) as usize;
    if input.is_empty() {
        return Err(PoseidonError::EmptyInput);
    }
    if input.len() != modulus_bytes_len {
        return Err(PoseidonError::InvalidInputLength {
            len: input.len(),
            modulus_bytes_len,
        });
    }
    Ok(input)
}

/// Converts a slice of big-endian bytes into a prime field element.
pub fn bytes_to_prime_field_element_be<F>(input: &[u8]) -> Result<F, PoseidonError>
where
    F: PrimeField,
{
    Ok(F::from_be_bytes_mod_order(input))
}

/// Converts a slice of little-endian bytes into a prime field element.
pub fn bytes_to_prime_field_element_le<F>(input: &[u8]) -> Result<F, PoseidonError>
where
    F: PrimeField,
{
    Ok(F::from_le_bytes_mod_order(input))
}

impl<F: PrimeField> PoseidonBytesHasher for Poseidon<F> {
    fn hash_bytes_be(&mut self, inputs: &[&[u8]]) -> Result<[u8; HASH_LEN], PoseidonError> {
        let inputs: Result<Vec<_>, _> = inputs
            .iter()
            .map(|input| validate_bytes_length::<F>(input))
            .collect();
        let inputs = inputs?;
        let inputs: Vec<F> = inputs
            .iter()
            .map(|input| F::from_be_bytes_mod_order(input))
            .collect();
        let hash = self.hash(&inputs)?;

        hash.into_bigint()
            .to_bytes_be()
            .try_into()
            .map_err(|_| PoseidonError::VecToArray)
    }

    fn hash_bytes_le(&mut self, inputs: &[&[u8]]) -> Result<[u8; HASH_LEN], PoseidonError> {
        let inputs: Result<Vec<_>, _> = inputs
            .iter()
            .map(|input| validate_bytes_length::<F>(input))
            .collect();
        let inputs = inputs?;
        let inputs: Vec<F> = inputs
            .iter()
            .map(|input| F::from_le_bytes_mod_order(input))
            .collect();
        let hash = self.hash(&inputs)?;

        hash.into_bigint()
            .to_bytes_le()
            .try_into()
            .map_err(|_| PoseidonError::VecToArray)
    }
}

impl Poseidon<Fr> {
    pub fn new_circom(nr_inputs: usize) -> Result<Poseidon<Fr>, PoseidonError> {
        Self::with_domain_tag_circom(nr_inputs, Fr::zero())
    }

    pub fn with_domain_tag_circom(
        nr_inputs: usize,
        domain_tag: Fr,
    ) -> Result<Poseidon<Fr>, PoseidonError> {
        let width = nr_inputs + 1;
        if width > MAX_X5_LEN {
            return Err(PoseidonError::InvalidWidthCircom {
                width,
                max_limit: MAX_X5_LEN,
            });
        }

        let params = crate::poseidon::parameters::bn254_x5::get_poseidon_parameters::<Fr>(
            (width).try_into().map_err(|_| PoseidonError::U64Tou8)?,
        )?;
        Ok(Poseidon::<Fr>::with_domain_tag(params, domain_tag))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poseidon_hash_1_input() {
        let mut poseidon = Poseidon::<Fr>::new_circom(1).unwrap();
        let input = Fr::from(12345678901234567890u64);
        let hash = poseidon.hash(&[input]).unwrap();
        // Just verify it doesn't panic and returns a result
        assert!(!hash.is_zero());
    }

    #[test]
    fn test_poseidon_hash_2_inputs() {
        let mut poseidon = Poseidon::<Fr>::new_circom(2).unwrap();
        let input1 = Fr::from(111111111111111111u64);
        let input2 = Fr::from(222222222222222222u64);
        let hash = poseidon.hash(&[input1, input2]).unwrap();
        assert!(!hash.is_zero());
    }
}

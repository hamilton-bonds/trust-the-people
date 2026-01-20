use super::{Commitment, ProofType, ZkProof};
use crate::hash::{hash_blake2b, hash_blake2b_multiple};
use common::{Result, VotingError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeProof {
    pub commitments: Vec<Vec<u8>>,
    pub challenges: Vec<Vec<u8>>,
    pub responses: Vec<Vec<u8>>,
    pub range_min: u64,
    pub range_max: u64,
}

impl RangeProof {
    pub fn generate(value: u64, min: u64, max: u64, randomness: &[u8]) -> Result<Self> {
        if value < min || value > max {
            return Err(VotingError::CryptoError(format!(
                "Value {} is not in range [{}, {}]",
                value, min, max
            )));
        }

        let mut commitments = Vec::new();
        let mut challenges = Vec::new();
        let mut responses = Vec::new();

        let bits = Self::bits_needed(max);
        let value_bits = Self::to_bits(value, bits);

        for (i, &bit) in value_bits.iter().enumerate() {
            let bit_randomness = hash_blake2b_multiple(&[randomness, &[i as u8]]);
            let commitment = Commitment::new(&[bit], &bit_randomness);
            commitments.push(commitment.commitment_value().to_vec());

            let challenge = hash_blake2b_multiple(&[
                commitment.commitment_value(),
                &value.to_le_bytes(),
                &[i as u8],
            ])
            .to_vec();
            challenges.push(challenge.clone());

            let response = hash_blake2b_multiple(&[&bit_randomness, &challenge]).to_vec();
            responses.push(response);
        }

        Ok(Self {
            commitments,
            challenges,
            responses,
            range_min: min,
            range_max: max,
        })
    }

    pub fn verify(&self, commitment: &[u8]) -> Result<()> {
        if self.commitments.is_empty() {
            return Err(VotingError::CryptoError(
                "Invalid range proof: no commitments".to_string(),
            ));
        }

        if self.commitments.len() != self.challenges.len()
            || self.commitments.len() != self.responses.len()
        {
            return Err(VotingError::CryptoError(
                "Invalid range proof: mismatched lengths".to_string(),
            ));
        }

        let aggregate_commitment = hash_blake2b_multiple(
            &self
                .commitments
                .iter()
                .map(|c| c.as_slice())
                .collect::<Vec<_>>(),
        );

        if !super::constant_time_eq(&aggregate_commitment, commitment) {
            return Err(VotingError::CryptoError(
                "Range proof verification failed".to_string(),
            ));
        }

        Ok(())
    }

    fn bits_needed(max_value: u64) -> usize {
        if max_value == 0 {
            return 1;
        }
        (64 - max_value.leading_zeros()) as usize
    }

    fn to_bits(value: u64, num_bits: usize) -> Vec<u8> {
        (0..num_bits)
            .map(|i| ((value >> i) & 1) as u8)
            .collect()
    }

    pub fn to_zk_proof(&self) -> ZkProof {
        let proof_data = bincode::serialize(self).expect("Serialization failed");
        let public_inputs = self.commitments.concat();
        ZkProof::new(ProofType::RangeProof, proof_data, public_inputs)
    }

    pub fn size(&self) -> usize {
        self.commitments.iter().map(|c| c.len()).sum::<usize>()
            + self.challenges.iter().map(|c| c.len()).sum::<usize>()
            + self.responses.iter().map(|r| r.len()).sum::<usize>()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedRangeProof {
    pub proofs: Vec<RangeProof>,
    pub total_commitment: Vec<u8>,
}

impl AggregatedRangeProof {
    pub fn new(proofs: Vec<RangeProof>) -> Result<Self> {
        if proofs.is_empty() {
            return Err(VotingError::CryptoError(
                "Cannot create aggregated proof from empty list".to_string(),
            ));
        }

        let all_commitments: Vec<&[u8]> = proofs
            .iter()
            .flat_map(|p| p.commitments.iter().map(|c| c.as_slice()))
            .collect();

        let total_commitment = hash_blake2b_multiple(&all_commitments).to_vec();

        Ok(Self {
            proofs,
            total_commitment,
        })
    }

    pub fn verify(&self) -> Result<()> {
        for proof in &self.proofs {
            let commitment = hash_blake2b_multiple(
                &proof
                    .commitments
                    .iter()
                    .map(|c| c.as_slice())
                    .collect::<Vec<_>>(),
            );
            proof.verify(&commitment)?;
        }

        Ok(())
    }

    pub fn total_size(&self) -> usize {
        self.proofs.iter().map(|p| p.size()).sum::<usize>() + self.total_commitment.len()
    }
}

pub struct RangeProofVerifier {
    min_value: u64,
    max_value: u64,
}

impl RangeProofVerifier {
    pub fn new(min_value: u64, max_value: u64) -> Result<Self> {
        if min_value > max_value {
            return Err(VotingError::CryptoError(
                "Invalid range: min > max".to_string(),
            ));
        }
        Ok(Self {
            min_value,
            max_value,
        })
    }

    pub fn verify_proof(&self, proof: &RangeProof, commitment: &[u8]) -> Result<()> {
        if proof.range_min != self.min_value || proof.range_max != self.max_value {
            return Err(VotingError::CryptoError(
                "Range mismatch in proof".to_string(),
            ));
        }

        proof.verify(commitment)
    }

    pub fn batch_verify(
        &self,
        proofs: &[(RangeProof, Vec<u8>)],
    ) -> Result<Vec<bool>> {
        let mut results = Vec::new();
        for (proof, commitment) in proofs {
            let valid = self.verify_proof(proof, commitment).is_ok();
            results.push(valid);
        }
        Ok(results)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulletproofStyle {
    pub commitment: Vec<u8>,
    pub proof_a: Vec<u8>,
    pub proof_s: Vec<u8>,
    pub tau_x: Vec<u8>,
    pub mu: Vec<u8>,
    pub t_hat: Vec<u8>,
    pub inner_product_proof: Vec<u8>,
}

impl BulletproofStyle {
    pub fn generate_simplified(value: u64, min: u64, max: u64) -> Result<Self> {
        if value < min || value > max {
            return Err(VotingError::CryptoError(
                "Value out of range".to_string(),
            ));
        }

        let randomness = Commitment::generate_randomness();
        let commitment = Commitment::new(&value.to_le_bytes(), &randomness);

        let proof_a = hash_blake2b(&value.to_le_bytes()).to_vec();
        let proof_s = hash_blake2b(&randomness).to_vec();
        let tau_x = hash_blake2b_multiple(&[&proof_a, &proof_s]).to_vec();
        let mu = hash_blake2b_multiple(&[&tau_x, &value.to_le_bytes()]).to_vec();
        let t_hat = hash_blake2b_multiple(&[&mu, &randomness]).to_vec();
        let inner_product_proof = hash_blake2b_multiple(&[
            &proof_a,
            &proof_s,
            &tau_x,
            &mu,
            &t_hat,
        ])
        .to_vec();

        Ok(Self {
            commitment: commitment.commitment_value().to_vec(),
            proof_a,
            proof_s,
            tau_x,
            mu,
            t_hat,
            inner_product_proof,
        })
    }

    pub fn verify_simplified(&self) -> Result<()> {
        if self.commitment.is_empty()
            || self.proof_a.is_empty()
            || self.proof_s.is_empty()
            || self.tau_x.is_empty()
            || self.mu.is_empty()
            || self.t_hat.is_empty()
            || self.inner_product_proof.is_empty()
        {
            return Err(VotingError::CryptoError(
                "Invalid bulletproof: missing components".to_string(),
            ));
        }

        let recomputed_inner = hash_blake2b_multiple(&[
            &self.proof_a,
            &self.proof_s,
            &self.tau_x,
            &self.mu,
            &self.t_hat,
        ]);

        if !super::constant_time_eq(&recomputed_inner, &self.inner_product_proof) {
            return Err(VotingError::CryptoError(
                "Bulletproof verification failed".to_string(),
            ));
        }

        Ok(())
    }

    pub fn size(&self) -> usize {
        self.commitment.len()
            + self.proof_a.len()
            + self.proof_s.len()
            + self.tau_x.len()
            + self.mu.len()
            + self.t_hat.len()
            + self.inner_product_proof.len()
    }
}

pub struct TallyRangeProof {
    pub tally_commitment: Vec<u8>,
    pub range_proof: RangeProof,
    pub max_voters: u64,
}

impl TallyRangeProof {
    pub fn generate(
        tally: u64,
        max_voters: u64,
        randomness: &[u8],
    ) -> Result<Self> {
        let commitment = Commitment::new(&tally.to_le_bytes(), randomness);
        let range_proof = RangeProof::generate(tally, 0, max_voters, randomness)?;

        Ok(Self {
            tally_commitment: commitment.commitment_value().to_vec(),
            range_proof,
            max_voters,
        })
    }

    pub fn verify(&self) -> Result<()> {
        if self.range_proof.range_max != self.max_voters {
            return Err(VotingError::CryptoError(
                "Max voters mismatch".to_string(),
            ));
        }

        self.range_proof.verify(&self.tally_commitment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_proof_generation() {
        let value = 42u64;
        let min = 0u64;
        let max = 100u64;
        let randomness = Commitment::generate_randomness();

        let proof = RangeProof::generate(value, min, max, &randomness);
        assert!(proof.is_ok());

        let proof = proof.unwrap();
        assert_eq!(proof.range_min, min);
        assert_eq!(proof.range_max, max);
        assert!(!proof.commitments.is_empty());
    }

    #[test]
    fn test_range_proof_out_of_range() {
        let value = 150u64;
        let min = 0u64;
        let max = 100u64;
        let randomness = Commitment::generate_randomness();

        let proof = RangeProof::generate(value, min, max, &randomness);
        assert!(proof.is_err());
    }

    #[test]
    fn test_range_proof_verification() {
        let value = 50u64;
        let min = 0u64;
        let max = 100u64;
        let randomness = Commitment::generate_randomness();

        let proof = RangeProof::generate(value, min, max, &randomness).unwrap();
        let commitment = hash_blake2b_multiple(
            &proof
                .commitments
                .iter()
                .map(|c| c.as_slice())
                .collect::<Vec<_>>(),
        );

        assert!(proof.verify(&commitment).is_ok());
    }

    #[test]
    fn test_range_proof_bits_needed() {
        assert_eq!(RangeProof::bits_needed(0), 1);
        assert_eq!(RangeProof::bits_needed(1), 1);
        assert_eq!(RangeProof::bits_needed(7), 3);
        assert_eq!(RangeProof::bits_needed(8), 4);
        assert_eq!(RangeProof::bits_needed(255), 8);
        assert_eq!(RangeProof::bits_needed(256), 9);
    }

    #[test]
    fn test_range_proof_to_bits() {
        let bits = RangeProof::to_bits(5, 4);
        assert_eq!(bits, vec![1, 0, 1, 0]);

        let bits = RangeProof::to_bits(10, 4);
        assert_eq!(bits, vec![0, 1, 0, 1]);
    }

    #[test]
    fn test_aggregated_range_proof() {
        let randomness1 = Commitment::generate_randomness();
        let randomness2 = Commitment::generate_randomness();

        let proof1 = RangeProof::generate(10, 0, 100, &randomness1).unwrap();
        let proof2 = RangeProof::generate(20, 0, 100, &randomness2).unwrap();

        let aggregated = AggregatedRangeProof::new(vec![proof1, proof2]);
        assert!(aggregated.is_ok());

        let aggregated = aggregated.unwrap();
        assert_eq!(aggregated.proofs.len(), 2);
        assert!(aggregated.verify().is_ok());
    }

    #[test]
    fn test_aggregated_range_proof_empty() {
        let result = AggregatedRangeProof::new(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_range_proof_verifier() {
        let verifier = RangeProofVerifier::new(0, 100).unwrap();

        let randomness = Commitment::generate_randomness();
        let proof = RangeProof::generate(50, 0, 100, &randomness).unwrap();
        let commitment = hash_blake2b_multiple(
            &proof
                .commitments
                .iter()
                .map(|c| c.as_slice())
                .collect::<Vec<_>>(),
        );

        assert!(verifier.verify_proof(&proof, &commitment).is_ok());
    }

    #[test]
    fn test_range_proof_verifier_wrong_range() {
        let verifier = RangeProofVerifier::new(0, 100).unwrap();

        let randomness = Commitment::generate_randomness();
        let proof = RangeProof::generate(50, 0, 200, &randomness).unwrap();
        let commitment = hash_blake2b_multiple(
            &proof
                .commitments
                .iter()
                .map(|c| c.as_slice())
                .collect::<Vec<_>>(),
        );

        assert!(verifier.verify_proof(&proof, &commitment).is_err());
    }

    #[test]
    fn test_range_proof_verifier_invalid_range() {
        let result = RangeProofVerifier::new(100, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_batch_verify() {
        let verifier = RangeProofVerifier::new(0, 100).unwrap();

        let randomness1 = Commitment::generate_randomness();
        let randomness2 = Commitment::generate_randomness();

        let proof1 = RangeProof::generate(30, 0, 100, &randomness1).unwrap();
        let commitment1 = hash_blake2b_multiple(
            &proof1
                .commitments
                .iter()
                .map(|c| c.as_slice())
                .collect::<Vec<_>>(),
        )
        .to_vec();

        let proof2 = RangeProof::generate(70, 0, 100, &randomness2).unwrap();
        let commitment2 = hash_blake2b_multiple(
            &proof2
                .commitments
                .iter()
                .map(|c| c.as_slice())
                .collect::<Vec<_>>(),
        )
        .to_vec();

        let proofs = vec![(proof1, commitment1), (proof2, commitment2)];
        let results = verifier.batch_verify(&proofs).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0]);
        assert!(results[1]);
    }

    #[test]
    fn test_bulletproof_style_generation() {
        let proof = BulletproofStyle::generate_simplified(42, 0, 100);
        assert!(proof.is_ok());

        let proof = proof.unwrap();
        assert!(!proof.commitment.is_empty());
        assert!(!proof.proof_a.is_empty());
        assert!(!proof.inner_product_proof.is_empty());
    }

    #[test]
    fn test_bulletproof_style_verification() {
        let proof = BulletproofStyle::generate_simplified(42, 0, 100).unwrap();
        assert!(proof.verify_simplified().is_ok());
    }

    #[test]
    fn test_bulletproof_style_out_of_range() {
        let proof = BulletproofStyle::generate_simplified(150, 0, 100);
        assert!(proof.is_err());
    }

    #[test]
    fn test_tally_range_proof_generation() {
        let tally = 500u64;
        let max_voters = 1000u64;
        let randomness = Commitment::generate_randomness();

        let proof = TallyRangeProof::generate(tally, max_voters, &randomness);
        assert!(proof.is_ok());

        let proof = proof.unwrap();
        assert_eq!(proof.max_voters, max_voters);
        assert!(!proof.tally_commitment.is_empty());
    }

    #[test]
    fn test_tally_range_proof_verification() {
        let tally = 500u64;
        let max_voters = 1000u64;
        let randomness = Commitment::generate_randomness();

        let proof = TallyRangeProof::generate(tally, max_voters, &randomness).unwrap();
        assert!(proof.verify().is_ok());
    }

    #[test]
    fn test_tally_range_proof_exceeds_max() {
        let tally = 1500u64;
        let max_voters = 1000u64;
        let randomness = Commitment::generate_randomness();

        let proof = TallyRangeProof::generate(tally, max_voters, &randomness);
        assert!(proof.is_err());
    }

    #[test]
    fn test_range_proof_size() {
        let randomness = Commitment::generate_randomness();
        let proof = RangeProof::generate(50, 0, 100, &randomness).unwrap();

        let size = proof.size();
        assert!(size > 0);
    }

    #[test]
    fn test_aggregated_proof_size() {
        let randomness1 = Commitment::generate_randomness();
        let randomness2 = Commitment::generate_randomness();

        let proof1 = RangeProof::generate(10, 0, 100, &randomness1).unwrap();
        let proof2 = RangeProof::generate(20, 0, 100, &randomness2).unwrap();

        let aggregated = AggregatedRangeProof::new(vec![proof1, proof2]).unwrap();
        let size = aggregated.total_size();

        assert!(size > 0);
    }

    #[test]
    fn test_bulletproof_size() {
        let proof = BulletproofStyle::generate_simplified(42, 0, 100).unwrap();
        let size = proof.size();

        assert!(size > 0);
    }

    #[test]
    fn test_range_proof_to_zk_proof() {
        let randomness = Commitment::generate_randomness();
        let proof = RangeProof::generate(50, 0, 100, &randomness).unwrap();

        let zk_proof = proof.to_zk_proof();
        assert_eq!(zk_proof.proof_type, ProofType::RangeProof);
        assert!(!zk_proof.proof_data.is_empty());
    }

    #[test]
    fn test_range_proof_edge_cases() {
        let randomness = Commitment::generate_randomness();

        let proof_min = RangeProof::generate(0, 0, 100, &randomness).unwrap();
        let commitment_min = hash_blake2b_multiple(
            &proof_min
                .commitments
                .iter()
                .map(|c| c.as_slice())
                .collect::<Vec<_>>(),
        );
        assert!(proof_min.verify(&commitment_min).is_ok());

        let proof_max = RangeProof::generate(100, 0, 100, &randomness).unwrap();
        let commitment_max = hash_blake2b_multiple(
            &proof_max
                .commitments
                .iter()
                .map(|c| c.as_slice())
                .collect::<Vec<_>>(),
        );
        assert!(proof_max.verify(&commitment_max).is_ok());
    }

    #[test]
    fn test_range_proof_invalid_verification() {
        let randomness = Commitment::generate_randomness();
        let proof = RangeProof::generate(50, 0, 100, &randomness).unwrap();

        let wrong_commitment = vec![0u8; 32];
        assert!(proof.verify(&wrong_commitment).is_err());
    }
}

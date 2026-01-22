use crate::voter::{Voter, VoterId};
use crate::Election;
use common::{ElectionId, PublicKey, Result, Signature, Timestamp, VotingError};
use crypto::keys::KeyPair;
use crypto::signatures::{sign, verify};
use crypto::zkp::{Commitment, SchnorrProof};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct EligibilityChecker {
    election_id: ElectionId,
    requirements: EligibilityRequirements,
    eligible_voters: HashSet<VoterId>,
    verified_proofs: HashMap<VoterId, EligibilityProof>,
}

impl EligibilityChecker {
    pub fn new(election_id: ElectionId, requirements: EligibilityRequirements) -> Self {
        Self {
            election_id,
            requirements,
            eligible_voters: HashSet::new(),
            verified_proofs: HashMap::new(),
        }
    }

    pub fn add_eligible_voter(&mut self, voter_id: VoterId) {
        self.eligible_voters.insert(voter_id);
    }

    pub fn add_eligible_voters(&mut self, voter_ids: Vec<VoterId>) {
        for voter_id in voter_ids {
            self.eligible_voters.insert(voter_id);
        }
    }

    pub fn check_eligibility(
        &self,
        voter: &Voter,
        current_time: Timestamp,
    ) -> Result<EligibilityStatus> {
        if !voter.is_registered() {
            return Ok(EligibilityStatus::NotRegistered);
        }

        if voter.has_voted(self.election_id) {
            return Ok(EligibilityStatus::AlreadyVoted);
        }

        if !self.eligible_voters.contains(&voter.id) {
            return Ok(EligibilityStatus::NotEligible);
        }

        if let Some(credentials) = &voter.credentials {
            if !credentials.is_valid(current_time) {
                return Ok(EligibilityStatus::CredentialsExpired);
            }

            if self.requirements.require_jurisdiction_match {
                if let Some(required_jurisdiction) = &self.requirements.jurisdiction {
                    if &credentials.jurisdiction != required_jurisdiction {
                        return Ok(EligibilityStatus::WrongJurisdiction);
                    }
                }
            }
        } else {
            return Ok(EligibilityStatus::NoCredentials);
        }

        if self.requirements.require_age_verification {
            return Ok(EligibilityStatus::AgeVerificationRequired);
        }

        Ok(EligibilityStatus::Eligible)
    }

    pub fn verify_eligibility_proof(
        &mut self,
        proof: EligibilityProof,
        voter_public_key: &PublicKey,
    ) -> Result<bool> {
        if proof.election_id != self.election_id {
            return Ok(false);
        }

        proof.verify(voter_public_key)?;

        if !self.eligible_voters.contains(&proof.voter_id) {
            return Ok(false);
        }

        self.verified_proofs.insert(proof.voter_id, proof);
        Ok(true)
    }

    pub fn is_eligible(&self, voter_id: &VoterId) -> bool {
        self.eligible_voters.contains(voter_id)
    }

    pub fn has_verified_proof(&self, voter_id: &VoterId) -> bool {
        self.verified_proofs.contains_key(voter_id)
    }

    pub fn eligible_count(&self) -> usize {
        self.eligible_voters.len()
    }

    pub fn verified_count(&self) -> usize {
        self.verified_proofs.len()
    }

    pub fn requirements(&self) -> &EligibilityRequirements {
        &self.requirements
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EligibilityRequirements {
    pub require_registration: bool,
    pub require_jurisdiction_match: bool,
    pub jurisdiction: Option<String>,
    pub require_age_verification: bool,
    pub minimum_age: Option<u32>,
    pub require_citizenship: bool,
    pub require_id_verification: bool,
    pub allow_provisional: bool,
}

impl Default for EligibilityRequirements {
    fn default() -> Self {
        Self {
            require_registration: true,
            require_jurisdiction_match: true,
            jurisdiction: None,
            require_age_verification: false,
            minimum_age: Some(18),
            require_citizenship: true,
            require_id_verification: true,
            allow_provisional: false,
        }
    }
}

impl EligibilityRequirements {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_jurisdiction(mut self, jurisdiction: String) -> Self {
        self.jurisdiction = Some(jurisdiction);
        self
    }

    pub fn with_minimum_age(mut self, age: u32) -> Self {
        self.minimum_age = Some(age);
        self.require_age_verification = true;
        self
    }

    pub fn allow_provisional_voting(mut self) -> Self {
        self.allow_provisional = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EligibilityStatus {
    Eligible,
    NotRegistered,
    NotEligible,
    AlreadyVoted,
    CredentialsExpired,
    NoCredentials,
    WrongJurisdiction,
    AgeVerificationRequired,
    CitizenshipRequired,
    ProvisionalOnly,
}

impl std::fmt::Display for EligibilityStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EligibilityStatus::Eligible => write!(f, "eligible"),
            EligibilityStatus::NotRegistered => write!(f, "not_registered"),
            EligibilityStatus::NotEligible => write!(f, "not_eligible"),
            EligibilityStatus::AlreadyVoted => write!(f, "already_voted"),
            EligibilityStatus::CredentialsExpired => write!(f, "credentials_expired"),
            EligibilityStatus::NoCredentials => write!(f, "no_credentials"),
            EligibilityStatus::WrongJurisdiction => write!(f, "wrong_jurisdiction"),
            EligibilityStatus::AgeVerificationRequired => write!(f, "age_verification_required"),
            EligibilityStatus::CitizenshipRequired => write!(f, "citizenship_required"),
            EligibilityStatus::ProvisionalOnly => write!(f, "provisional_only"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EligibilityProof {
    pub voter_id: VoterId,
    pub election_id: ElectionId,
    pub proof_commitment: Vec<u8>,
    pub schnorr_proof: SchnorrProof,
    pub proof_signature: Signature,
    pub issued_at: Timestamp,
}

impl EligibilityProof {
    pub fn generate(
        voter_id: VoterId,
        election_id: ElectionId,
        voter_keypair: &KeyPair,
    ) -> Result<Self> {
        let issued_at = common::utils::current_timestamp();

        let randomness = Commitment::generate_randomness();
        let commitment = Commitment::new(voter_id.as_bytes(), &randomness);
        let proof_commitment = commitment.commitment_value().to_vec();

        let challenge = crypto::zkp::generate_challenge();
        let response = crypto::hash::hash_blake2b_multiple(&[&randomness, &challenge]).to_vec();
        let schnorr_proof = SchnorrProof::new(challenge, response);

        let message = Self::compute_message(&voter_id, &election_id, &proof_commitment, issued_at);
        let proof_signature = sign(&message, voter_keypair)?;

        Ok(Self {
            voter_id,
            election_id,
            proof_commitment,
            schnorr_proof,
            proof_signature: proof_signature.to_common(),
            issued_at,
        })
    }

    pub fn verify(&self, voter_public_key: &PublicKey) -> Result<()> {
        let message = Self::compute_message(
            &self.voter_id,
            &self.election_id,
            &self.proof_commitment,
            self.issued_at,
        );

        verify(&message, &crypto::signatures::Signature::from_common(&self.proof_signature), &crypto::keys::PublicKey::from_common(voter_public_key))?;

        self.schnorr_proof.verify(&self.proof_commitment)?;

        Ok(())
    }

    fn compute_message(
        voter_id: &VoterId,
        election_id: &ElectionId,
        proof_commitment: &[u8],
        issued_at: Timestamp,
    ) -> Vec<u8> {
        crypto::hash::hash_blake2b_multiple(&[
            voter_id.as_bytes(),
            &election_id.0,
            proof_commitment,
            &issued_at.to_le_bytes(),
        ])
        .to_vec()
    }

    pub fn is_expired(&self, expiration_seconds: u64) -> bool {
        let current_time = common::utils::current_timestamp();
        current_time > self.issued_at + expiration_seconds
    }
}

pub struct EligibilityRegistry {
    election_id: ElectionId,
    eligible_voters: HashMap<VoterId, EligibilityRecord>,
}

impl EligibilityRegistry {
    pub fn new(election_id: ElectionId) -> Self {
        Self {
            election_id,
            eligible_voters: HashMap::new(),
        }
    }

    pub fn register_eligible_voter(
        &mut self,
        voter_id: VoterId,
        jurisdiction: String,
    ) -> Result<()> {
        if self.eligible_voters.contains_key(&voter_id) {
            return Err(VotingError::InvalidInput(
                "Voter already registered as eligible".to_string(),
            ));
        }

        let record = EligibilityRecord {
            voter_id,
            election_id: self.election_id,
            jurisdiction,
            registered_at: common::utils::current_timestamp(),
            status: EligibilityStatus::Eligible,
        };

        self.eligible_voters.insert(voter_id, record);
        Ok(())
    }

    pub fn revoke_eligibility(&mut self, voter_id: &VoterId) -> Result<()> {
        let record = self
            .eligible_voters
            .get_mut(voter_id)
            .ok_or_else(|| VotingError::VoterNotEligible("Voter not found".to_string()))?;

        record.status = EligibilityStatus::NotEligible;
        Ok(())
    }

    pub fn mark_voted(&mut self, voter_id: &VoterId) -> Result<()> {
        let record = self
            .eligible_voters
            .get_mut(voter_id)
            .ok_or_else(|| VotingError::VoterNotEligible("Voter not found".to_string()))?;

        if record.status == EligibilityStatus::AlreadyVoted {
            return Err(VotingError::AlreadyVoted);
        }

        record.status = EligibilityStatus::AlreadyVoted;
        Ok(())
    }

    pub fn get_record(&self, voter_id: &VoterId) -> Option<&EligibilityRecord> {
        self.eligible_voters.get(voter_id)
    }

    pub fn is_eligible(&self, voter_id: &VoterId) -> bool {
        self.eligible_voters
            .get(voter_id)
            .map(|r| r.status == EligibilityStatus::Eligible)
            .unwrap_or(false)
    }

    pub fn has_voted(&self, voter_id: &VoterId) -> bool {
        self.eligible_voters
            .get(voter_id)
            .map(|r| r.status == EligibilityStatus::AlreadyVoted)
            .unwrap_or(false)
    }

    pub fn eligible_voters_by_jurisdiction(&self, jurisdiction: &str) -> Vec<&EligibilityRecord> {
        self.eligible_voters
            .values()
            .filter(|r| r.jurisdiction == jurisdiction && r.status == EligibilityStatus::Eligible)
            .collect()
    }

    pub fn total_eligible(&self) -> usize {
        self.eligible_voters
            .values()
            .filter(|r| r.status == EligibilityStatus::Eligible)
            .count()
    }

    pub fn total_voted(&self) -> usize {
        self.eligible_voters
            .values()
            .filter(|r| r.status == EligibilityStatus::AlreadyVoted)
            .count()
    }

    pub fn turnout_percentage(&self) -> f64 {
        let eligible = self.total_eligible();
        let voted = self.total_voted();

        if eligible == 0 {
            return 0.0;
        }

        (voted as f64 / eligible as f64) * 100.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EligibilityRecord {
    pub voter_id: VoterId,
    pub election_id: ElectionId,
    pub jurisdiction: String,
    pub registered_at: Timestamp,
    pub status: EligibilityStatus,
}

pub struct ProvisionalBallotTracker {
    provisional_votes: HashMap<VoterId, ProvisionalVote>,
}

impl ProvisionalBallotTracker {
    pub fn new() -> Self {
        Self {
            provisional_votes: HashMap::new(),
        }
    }

    pub fn add_provisional_vote(&mut self, vote: ProvisionalVote) -> Result<()> {
        if self.provisional_votes.contains_key(&vote.voter_id) {
            return Err(VotingError::AlreadyVoted);
        }

        self.provisional_votes.insert(vote.voter_id, vote);
        Ok(())
    }

    pub fn approve_provisional(&mut self, voter_id: &VoterId) -> Result<()> {
        let vote = self
            .provisional_votes
            .get_mut(voter_id)
            .ok_or_else(|| VotingError::VoterNotEligible("Provisional vote not found".to_string()))?;

        vote.status = ProvisionalStatus::Approved;
        Ok(())
    }

    pub fn reject_provisional(&mut self, voter_id: &VoterId, reason: String) -> Result<()> {
        let vote = self
            .provisional_votes
            .get_mut(voter_id)
            .ok_or_else(|| VotingError::VoterNotEligible("Provisional vote not found".to_string()))?;

        vote.status = ProvisionalStatus::Rejected { reason };
        Ok(())
    }

    pub fn get_provisional_vote(&self, voter_id: &VoterId) -> Option<&ProvisionalVote> {
        self.provisional_votes.get(voter_id)
    }

    pub fn pending_count(&self) -> usize {
        self.provisional_votes
            .values()
            .filter(|v| matches!(v.status, ProvisionalStatus::Pending))
            .count()
    }

    pub fn approved_count(&self) -> usize {
        self.provisional_votes
            .values()
            .filter(|v| matches!(v.status, ProvisionalStatus::Approved))
            .count()
    }

    pub fn rejected_count(&self) -> usize {
        self.provisional_votes
            .values()
            .filter(|v| matches!(v.status, ProvisionalStatus::Rejected { .. }))
            .count()
    }
}

impl Default for ProvisionalBallotTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionalVote {
    pub voter_id: VoterId,
    pub ballot_id: crate::ballot::BallotId,
    pub election_id: ElectionId,
    pub cast_at: Timestamp,
    pub status: ProvisionalStatus,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProvisionalStatus {
    Pending,
    Approved,
    Rejected { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voter::VoterCredentials;

    #[test]
    fn test_eligibility_checker_creation() {
        let election_id = ElectionId::new([1u8; 16]);
        let requirements = EligibilityRequirements::default();
        let checker = EligibilityChecker::new(election_id, requirements);

        assert_eq!(checker.eligible_count(), 0);
    }

    #[test]
    fn test_add_eligible_voter() {
        let election_id = ElectionId::new([1u8; 16]);
        let requirements = EligibilityRequirements::default();
        let mut checker = EligibilityChecker::new(election_id, requirements);

        let voter_id = VoterId::new([1u8; 32]);
        checker.add_eligible_voter(voter_id);

        assert_eq!(checker.eligible_count(), 1);
        assert!(checker.is_eligible(&voter_id));
    }

    #[test]
    fn test_check_eligibility_registered() {
        let election_id = ElectionId::new([1u8; 16]);
        let requirements = EligibilityRequirements::default();
        let mut checker = EligibilityChecker::new(election_id, requirements);

        let keypair = KeyPair::generate();
        let mut voter = Voter::new(&keypair);
        let credentials = VoterCredentials::new(voter.id, "Test".to_string());
        voter.register(credentials).unwrap();

        checker.add_eligible_voter(voter.id);

        let current_time = common::utils::current_timestamp();
        let status = checker.check_eligibility(&voter, current_time).unwrap();

        assert_eq!(status, EligibilityStatus::Eligible);
    }

    #[test]
    fn test_check_eligibility_not_registered() {
        let election_id = ElectionId::new([1u8; 16]);
        let requirements = EligibilityRequirements::default();
        let checker = EligibilityChecker::new(election_id, requirements);

        let keypair = KeyPair::generate();
        let voter = Voter::new(&keypair);

        let current_time = common::utils::current_timestamp();
        let status = checker.check_eligibility(&voter, current_time).unwrap();

        assert_eq!(status, EligibilityStatus::NotRegistered);
    }

    #[test]
    fn test_check_eligibility_already_voted() {
        let election_id = ElectionId::new([1u8; 16]);
        let requirements = EligibilityRequirements::default();
        let mut checker = EligibilityChecker::new(election_id, requirements);

        let keypair = KeyPair::generate();
        let mut voter = Voter::new(&keypair);
        let credentials = VoterCredentials::new(voter.id, "Test".to_string());
        voter.register(credentials).unwrap();
        voter.mark_voted(election_id).unwrap();

        checker.add_eligible_voter(voter.id);

        let current_time = common::utils::current_timestamp();
        let status = checker.check_eligibility(&voter, current_time).unwrap();

        assert_eq!(status, EligibilityStatus::AlreadyVoted);
    }

    #[test]
    fn test_eligibility_proof_generation() {
        let voter_id = VoterId::new([1u8; 32]);
        let election_id = ElectionId::new([1u8; 16]);
        let keypair = KeyPair::generate();

        let proof = EligibilityProof::generate(voter_id, election_id, &keypair);
        assert!(proof.is_ok());
    }

    #[test]
    fn test_eligibility_proof_verification() {
        let voter_id = VoterId::new([1u8; 32]);
        let election_id = ElectionId::new([1u8; 16]);
        let keypair = KeyPair::generate();

        let proof = EligibilityProof::generate(voter_id, election_id, &keypair).unwrap();
        assert!(proof.verify(keypair.public_key()).is_ok());
    }

    #[test]
    fn test_eligibility_requirements_builder() {
        let requirements = EligibilityRequirements::new()
            .with_jurisdiction("California".to_string())
            .with_minimum_age(21);

        assert_eq!(requirements.jurisdiction, Some("California".to_string()));
        assert_eq!(requirements.minimum_age, Some(21));
        assert!(requirements.require_age_verification);
    }

    #[test]
    fn test_eligibility_registry() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut registry = EligibilityRegistry::new(election_id);

        let voter_id = VoterId::new([1u8; 32]);
        registry
            .register_eligible_voter(voter_id, "Test Jurisdiction".to_string())
            .unwrap();

        assert_eq!(registry.total_eligible(), 1);
        assert!(registry.is_eligible(&voter_id));
    }

    #[test]
    fn test_eligibility_registry_mark_voted() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut registry = EligibilityRegistry::new(election_id);

        let voter_id = VoterId::new([1u8; 32]);
        registry
            .register_eligible_voter(voter_id, "Test".to_string())
            .unwrap();

        registry.mark_voted(&voter_id).unwrap();

        assert!(!registry.is_eligible(&voter_id));
        assert!(registry.has_voted(&voter_id));
        assert_eq!(registry.total_voted(), 1);
    }

    #[test]
    fn test_eligibility_registry_turnout() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut registry = EligibilityRegistry::new(election_id);

        let voter_id1 = VoterId::new([1u8; 32]);
        let voter_id2 = VoterId::new([2u8; 32]);

        registry
            .register_eligible_voter(voter_id1, "Test".to_string())
            .unwrap();
        registry
            .register_eligible_voter(voter_id2, "Test".to_string())
            .unwrap();

        registry.mark_voted(&voter_id1).unwrap();

        let turnout = registry.turnout_percentage();
        assert_eq!(turnout, 50.0);
    }

    #[test]
    fn test_provisional_ballot_tracker() {
        let mut tracker = ProvisionalBallotTracker::new();

        let provisional = ProvisionalVote {
            voter_id: VoterId::new([1u8; 32]),
            ballot_id: crate::ballot::BallotId::generate(),
            election_id: ElectionId::new([1u8; 16]),
            cast_at: common::utils::current_timestamp(),
            status: ProvisionalStatus::Pending,
            reason: "ID verification pending".to_string(),
        };

        tracker.add_provisional_vote(provisional.clone()).unwrap();
        assert_eq!(tracker.pending_count(), 1);
    }

    #[test]
    fn test_provisional_approval() {
        let mut tracker = ProvisionalBallotTracker::new();

        let voter_id = VoterId::new([1u8; 32]);
        let provisional = ProvisionalVote {
            voter_id,
            ballot_id: crate::ballot::BallotId::generate(),
            election_id: ElectionId::new([1u8; 16]),
            cast_at: common::utils::current_timestamp(),
            status: ProvisionalStatus::Pending,
            reason: "ID verification pending".to_string(),
        };

        tracker.add_provisional_vote(provisional).unwrap();
        tracker.approve_provisional(&voter_id).unwrap();

        assert_eq!(tracker.approved_count(), 1);
        assert_eq!(tracker.pending_count(), 0);
    }

    #[test]
    fn test_provisional_rejection() {
        let mut tracker = ProvisionalBallotTracker::new();

        let voter_id = VoterId::new([1u8; 32]);
        let provisional = ProvisionalVote {
            voter_id,
            ballot_id: crate::ballot::BallotId::generate(),
            election_id: ElectionId::new([1u8; 16]),
            cast_at: common::utils::current_timestamp(),
            status: ProvisionalStatus::Pending,
            reason: "ID verification pending".to_string(),
        };

        tracker.add_provisional_vote(provisional).unwrap();
        tracker
            .reject_provisional(&voter_id, "Invalid ID".to_string())
            .unwrap();

        assert_eq!(tracker.rejected_count(), 1);
    }

    #[test]
    fn test_eligibility_status_display() {
        assert_eq!(EligibilityStatus::Eligible.to_string(), "eligible");
        assert_eq!(
            EligibilityStatus::NotRegistered.to_string(),
            "not_registered"
        );
        assert_eq!(EligibilityStatus::AlreadyVoted.to_string(), "already_voted");
    }

    #[test]
    fn test_eligibility_registry_by_jurisdiction() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut registry = EligibilityRegistry::new(election_id);

        let voter_id1 = VoterId::new([1u8; 32]);
        let voter_id2 = VoterId::new([2u8; 32]);

        registry
            .register_eligible_voter(voter_id1, "California".to_string())
            .unwrap();
        registry
            .register_eligible_voter(voter_id2, "Texas".to_string())
            .unwrap();

        let california_voters = registry.eligible_voters_by_jurisdiction("California");
        assert_eq!(california_voters.len(), 1);
    }

    #[test]
    fn test_eligibility_proof_expiration() {
        let voter_id = VoterId::new([1u8; 32]);
        let election_id = ElectionId::new([1u8; 16]);
        let keypair = KeyPair::generate();

        let proof = EligibilityProof::generate(voter_id, election_id, &keypair).unwrap();

        assert!(!proof.is_expired(3600));
    }

    #[test]
    fn test_verify_eligibility_proof() {
        let election_id = ElectionId::new([1u8; 16]);
        let requirements = EligibilityRequirements::default();
        let mut checker = EligibilityChecker::new(election_id, requirements);

        let voter_id = VoterId::new([1u8; 32]);
        let keypair = KeyPair::generate();

        checker.add_eligible_voter(voter_id);

        let proof = EligibilityProof::generate(voter_id, election_id, &keypair).unwrap();
        let result = checker.verify_eligibility_proof(proof, keypair.public_key());

        assert!(result.is_ok());
        assert!(result.unwrap());
        assert!(checker.has_verified_proof(&voter_id));
    }
}

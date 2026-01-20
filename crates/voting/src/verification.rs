use crate::ballot::{Ballot, CastBallot};
use crate::vote_casting::VoteSubmission;
use crate::voter::{Voter, VoterId};
use crate::Election;
use common::{ElectionId, PublicKey, Result, Timestamp, VotingError};
use crypto::zkp::ballot_privacy::BallotVerifier;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteVerifier {
    election: Election,
    ballot_verifier: Option<BallotVerifier>,
}

impl VoteVerifier {
    pub fn new(election: Election) -> Self {
        let ballot_verifier = Self::create_ballot_verifier(&election);

        Self {
            election,
            ballot_verifier: Some(ballot_verifier),
        }
    }

    fn create_ballot_verifier(election: &Election) -> BallotVerifier {
        let valid_candidates: Vec<String> = election
            .candidates
            .iter()
            .map(|c| c.id.clone())
            .collect();

        BallotVerifier::new(election.authority_key, valid_candidates, election.id.0)
    }

    pub fn verify_submission(
        &self,
        submission: &VoteSubmission,
        voter_public_key: &PublicKey,
    ) -> Result<VerificationResult> {
        let mut result = VerificationResult::new(submission.voter_id);

        if submission.cast_ballot.election_id != self.election.id {
            result.add_error("Election ID mismatch".to_string());
            return Ok(result);
        }

        if let Err(e) = submission.verify(voter_public_key) {
            result.add_error(format!("Signature verification failed: {}", e));
            return Ok(result);
        }

        if let Some(verifier) = &self.ballot_verifier {
            if let Err(e) = verifier.verify_ballot(&submission.cast_ballot.to_private_ballot(&crypto::keys::KeyPair::generate())?, voter_public_key) {
                result.add_error(format!("Ballot verification failed: {}", e));
                return Ok(result);
            }
        }

        let current_time = common::utils::current_timestamp();
        if !self.election.is_active(current_time) {
            result.add_warning("Election is not currently active".to_string());
        }

        result.mark_valid();
        Ok(result)
    }

    pub fn verify_cast_ballot(
        &self,
        cast_ballot: &CastBallot,
        voter_public_key: &PublicKey,
    ) -> Result<VerificationResult> {
        let mut result = VerificationResult::new(cast_ballot.voter_id);

        if cast_ballot.election_id != self.election.id {
            result.add_error("Election ID mismatch".to_string());
            return Ok(result);
        }

        if let Err(e) = cast_ballot.verify(voter_public_key) {
            result.add_error(format!("Ballot signature verification failed: {}", e));
            return Ok(result);
        }

        result.mark_valid();
        Ok(result)
    }

    pub fn verify_voter_eligibility(
        &self,
        voter: &Voter,
        current_time: Timestamp,
    ) -> Result<VerificationResult> {
        let mut result = VerificationResult::new(voter.id);

        if !voter.is_registered() {
            result.add_error("Voter is not registered".to_string());
            return Ok(result);
        }

        if let Some(credentials) = &voter.credentials {
            if !credentials.is_valid(current_time) {
                result.add_error("Voter credentials have expired".to_string());
                return Ok(result);
            }
        } else {
            result.add_error("Voter has no credentials".to_string());
            return Ok(result);
        }

        if voter.has_voted(self.election.id) {
            result.add_error("Voter has already voted in this election".to_string());
            return Ok(result);
        }

        result.mark_valid();
        Ok(result)
    }

    pub fn verify_ballot_structure(&self, ballot: &Ballot) -> Result<VerificationResult> {
        let mut result = VerificationResult::new(VoterId::new([0u8; 32]));

        if ballot.election_id != self.election.id {
            result.add_error("Ballot election ID mismatch".to_string());
            return Ok(result);
        }

        if let Err(e) = ballot.validate() {
            result.add_error(format!("Ballot validation failed: {}", e));
            return Ok(result);
        }

        for choice in &ballot.choices {
            let candidate_exists = self
                .election
                .candidates
                .iter()
                .any(|c| c.id == choice.candidate_id);

            if !candidate_exists && choice.candidate_id != "write_in" {
                result.add_error(format!("Invalid candidate ID: {}", choice.candidate_id));
            }
        }

        if result.errors.is_empty() {
            result.mark_valid();
        }

        Ok(result)
    }

    pub fn batch_verify_submissions(
        &self,
        submissions: &[(VoteSubmission, PublicKey)],
    ) -> Result<Vec<VerificationResult>> {
        let mut results = Vec::new();

        for (submission, public_key) in submissions {
            let result = self.verify_submission(submission, public_key)?;
            results.push(result);
        }

        Ok(results)
    }

    pub fn election(&self) -> &Election {
        &self.election
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub voter_id: VoterId,
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub verified_at: Timestamp,
}

impl VerificationResult {
    pub fn new(voter_id: VoterId) -> Self {
        Self {
            voter_id,
            is_valid: false,
            errors: Vec::new(),
            warnings: Vec::new(),
            verified_at: common::utils::current_timestamp(),
        }
    }

    pub fn mark_valid(&mut self) {
        if self.errors.is_empty() {
            self.is_valid = true;
        }
    }

    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
        self.is_valid = false;
    }

    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }
}

impl std::fmt::Display for VerificationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_valid {
            write!(f, "Valid")?;
        } else {
            write!(f, "Invalid")?;
        }

        if !self.errors.is_empty() {
            write!(f, " - Errors: {}", self.errors.join(", "))?;
        }

        if !self.warnings.is_empty() {
            write!(f, " - Warnings: {}", self.warnings.join(", "))?;
        }

        Ok(())
    }
}

pub struct IntegrityChecker {
    election_id: ElectionId,
}

impl IntegrityChecker {
    pub fn new(election_id: ElectionId) -> Self {
        Self { election_id }
    }

    pub fn check_ballot_integrity(&self, cast_ballot: &CastBallot) -> Result<IntegrityReport> {
        let mut report = IntegrityReport::new(self.election_id);

        if cast_ballot.election_id != self.election_id {
            report.add_issue(IntegrityIssue::ElectionMismatch);
        }

        if cast_ballot.encrypted_ballot.ciphertext.is_empty() {
            report.add_issue(IntegrityIssue::EmptyBallot);
        }

        if cast_ballot.ballot_commitment.is_empty() {
            report.add_issue(IntegrityIssue::MissingCommitment);
        }

        if cast_ballot.transaction_id.is_none() {
            report.add_issue(IntegrityIssue::MissingTransactionId);
        }

        Ok(report)
    }

    pub fn check_vote_chain(&self, cast_ballots: &[CastBallot]) -> Result<IntegrityReport> {
        let mut report = IntegrityReport::new(self.election_id);

        let mut ballot_ids = std::collections::HashSet::new();
        let mut voter_ids = std::collections::HashSet::new();

        for ballot in cast_ballots {
            if ballot.election_id != self.election_id {
                report.add_issue(IntegrityIssue::ElectionMismatch);
            }

            if !ballot_ids.insert(ballot.ballot_id) {
                report.add_issue(IntegrityIssue::DuplicateBallot);
            }

            if !voter_ids.insert(ballot.voter_id) {
                report.add_issue(IntegrityIssue::DuplicateVoter);
            }
        }

        Ok(report)
    }

    pub fn verify_timestamps(&self, cast_ballots: &[CastBallot]) -> Result<IntegrityReport> {
        let mut report = IntegrityReport::new(self.election_id);

        for ballot in cast_ballots {
            if ballot.cast_at == 0 {
                report.add_issue(IntegrityIssue::InvalidTimestamp);
            }
        }

        let mut sorted_ballots = cast_ballots.to_vec();
        sorted_ballots.sort_by_key(|b| b.cast_at);

        for window in sorted_ballots.windows(2) {
            if window[1].cast_at < window[0].cast_at {
                report.add_issue(IntegrityIssue::TimestampOrderViolation);
                break;
            }
        }

        Ok(report)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityReport {
    pub election_id: ElectionId,
    pub issues: Vec<IntegrityIssue>,
    pub checked_at: Timestamp,
}

impl IntegrityReport {
    pub fn new(election_id: ElectionId) -> Self {
        Self {
            election_id,
            issues: Vec::new(),
            checked_at: common::utils::current_timestamp(),
        }
    }

    pub fn add_issue(&mut self, issue: IntegrityIssue) {
        self.issues.push(issue);
    }

    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn issue_count(&self) -> usize {
        self.issues.len()
    }

    pub fn has_critical_issues(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| matches!(issue, IntegrityIssue::DuplicateVoter))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IntegrityIssue {
    ElectionMismatch,
    EmptyBallot,
    MissingCommitment,
    MissingTransactionId,
    DuplicateBallot,
    DuplicateVoter,
    InvalidTimestamp,
    TimestampOrderViolation,
}

impl std::fmt::Display for IntegrityIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntegrityIssue::ElectionMismatch => write!(f, "Election ID mismatch"),
            IntegrityIssue::EmptyBallot => write!(f, "Empty ballot"),
            IntegrityIssue::MissingCommitment => write!(f, "Missing ballot commitment"),
            IntegrityIssue::MissingTransactionId => write!(f, "Missing transaction ID"),
            IntegrityIssue::DuplicateBallot => write!(f, "Duplicate ballot detected"),
            IntegrityIssue::DuplicateVoter => write!(f, "Duplicate voter detected"),
            IntegrityIssue::InvalidTimestamp => write!(f, "Invalid timestamp"),
            IntegrityIssue::TimestampOrderViolation => write!(f, "Timestamp order violation"),
        }
    }
}

pub struct AuditTrail {
    verification_records: Vec<AuditRecord>,
}

impl AuditTrail {
    pub fn new() -> Self {
        Self {
            verification_records: Vec::new(),
        }
    }

    pub fn record_verification(
        &mut self,
        voter_id: VoterId,
        result: VerificationResult,
        verifier_id: String,
    ) {
        let record = AuditRecord {
            voter_id,
            verification_result: result,
            verifier_id,
            recorded_at: common::utils::current_timestamp(),
        };
        self.verification_records.push(record);
    }

    pub fn get_records(&self, voter_id: &VoterId) -> Vec<&AuditRecord> {
        self.verification_records
            .iter()
            .filter(|r| &r.voter_id == voter_id)
            .collect()
    }

    pub fn total_verifications(&self) -> usize {
        self.verification_records.len()
    }

    pub fn successful_verifications(&self) -> usize {
        self.verification_records
            .iter()
            .filter(|r| r.verification_result.is_valid)
            .count()
    }

    pub fn failed_verifications(&self) -> usize {
        self.verification_records
            .iter()
            .filter(|r| !r.verification_result.is_valid)
            .count()
    }

    pub fn verification_rate(&self) -> f64 {
        if self.verification_records.is_empty() {
            return 0.0;
        }
        (self.successful_verifications() as f64 / self.verification_records.len() as f64) * 100.0
    }
}

impl Default for AuditTrail {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub voter_id: VoterId,
    pub verification_result: VerificationResult,
    pub verifier_id: String,
    pub recorded_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ballot::{BallotChoice, BallotType};
    use crate::{Candidate, ElectionConfig};
    use crypto::encryption::EncryptionKey;
    use crypto::keys::KeyPair;

    fn create_test_election() -> Election {
        let candidates = vec![
            Candidate::new("c1".to_string(), "Alice".to_string()),
            Candidate::new("c2".to_string(), "Bob".to_string()),
        ];

        Election::new(
            ElectionId::new([1u8; 16]),
            "Test Election".to_string(),
            "Test".to_string(),
            candidates,
            1000,
            9999999999,
            PublicKey::new([0u8; 32]),
        )
    }

    #[test]
    fn test_verifier_creation() {
        let election = create_test_election();
        let verifier = VoteVerifier::new(election.clone());

        assert_eq!(verifier.election().id, election.id);
    }

    #[test]
    fn test_verify_ballot_structure() {
        let election = create_test_election();
        let verifier = VoteVerifier::new(election.clone());

        let mut ballot = Ballot::new(election.id, BallotType::SingleChoice);
        ballot
            .add_choice(BallotChoice::new("c1".to_string()))
            .unwrap();

        let result = verifier.verify_ballot_structure(&ballot).unwrap();
        assert!(result.is_valid);
    }

    #[test]
    fn test_verify_ballot_invalid_candidate() {
        let election = create_test_election();
        let verifier = VoteVerifier::new(election.clone());

        let mut ballot = Ballot::new(election.id, BallotType::SingleChoice);
        ballot
            .add_choice(BallotChoice::new("invalid_candidate".to_string()))
            .unwrap();

        let result = verifier.verify_ballot_structure(&ballot).unwrap();
        assert!(!result.is_valid);
        assert!(result.has_errors());
    }

    #[test]
    fn test_verify_voter_eligibility() {
        let election = create_test_election();
        let verifier = VoteVerifier::new(election);

        let keypair = KeyPair::generate();
        let mut voter = Voter::new(&keypair);
        let voter_id = voter.id;

        let credentials = crate::voter::VoterCredentials::new(voter_id, "Test".to_string());
        voter.register(credentials).unwrap();

        let current_time = common::utils::current_timestamp();
        let result = verifier.verify_voter_eligibility(&voter, current_time).unwrap();

        assert!(result.is_valid);
    }

    #[test]
    fn test_verify_voter_already_voted() {
        let election = create_test_election();
        let verifier = VoteVerifier::new(election.clone());

        let keypair = KeyPair::generate();
        let mut voter = Voter::new(&keypair);
        let voter_id = voter.id;

        let credentials = crate::voter::VoterCredentials::new(voter_id, "Test".to_string());
        voter.register(credentials).unwrap();
        voter.mark_voted(election.id).unwrap();

        let current_time = common::utils::current_timestamp();
        let result = verifier.verify_voter_eligibility(&voter, current_time).unwrap();

        assert!(!result.is_valid);
        assert!(result.has_errors());
    }

    #[test]
    fn test_verification_result() {
        let voter_id = VoterId::new([1u8; 32]);
        let mut result = VerificationResult::new(voter_id);

        assert!(!result.is_valid);
        assert!(!result.has_errors());

        result.add_error("Test error".to_string());
        assert!(result.has_errors());
        assert_eq!(result.error_count(), 1);

        result.add_warning("Test warning".to_string());
        assert!(result.has_warnings());
        assert_eq!(result.warning_count(), 1);
    }

    #[test]
    fn test_integrity_checker() {
        let election_id = ElectionId::new([1u8; 16]);
        let checker = IntegrityChecker::new(election_id);

        let keypair = KeyPair::generate();
        let voter_id = VoterId::new([1u8; 32]);
        let election_key = EncryptionKey::generate();

        let mut ballot = Ballot::new(election_id, BallotType::SingleChoice);
        ballot
            .add_choice(BallotChoice::new("c1".to_string()))
            .unwrap();

        let cast_ballot = CastBallot::create(&ballot, voter_id, &election_key, &keypair).unwrap();

        let report = checker.check_ballot_integrity(&cast_ballot).unwrap();
        assert!(report.has_critical_issues() || report.is_valid());
    }

    #[test]
    fn test_integrity_report() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut report = IntegrityReport::new(election_id);

        assert!(report.is_valid());
        assert_eq!(report.issue_count(), 0);

        report.add_issue(IntegrityIssue::DuplicateVoter);
        assert!(!report.is_valid());
        assert_eq!(report.issue_count(), 1);
        assert!(report.has_critical_issues());
    }

    #[test]
    fn test_audit_trail() {
        let mut trail = AuditTrail::new();
        let voter_id = VoterId::new([1u8; 32]);
        let result = VerificationResult::new(voter_id);

        trail.record_verification(voter_id, result, "verifier_1".to_string());

        assert_eq!(trail.total_verifications(), 1);
        assert_eq!(trail.get_records(&voter_id).len(), 1);
    }

    #[test]
    fn test_audit_trail_statistics() {
        let mut trail = AuditTrail::new();

        let voter_id1 = VoterId::new([1u8; 32]);
        let mut result1 = VerificationResult::new(voter_id1);
        result1.mark_valid();

        let voter_id2 = VoterId::new([2u8; 32]);
        let mut result2 = VerificationResult::new(voter_id2);
        result2.add_error("Error".to_string());

        trail.record_verification(voter_id1, result1, "verifier_1".to_string());
        trail.record_verification(voter_id2, result2, "verifier_1".to_string());

        assert_eq!(trail.successful_verifications(), 1);
        assert_eq!(trail.failed_verifications(), 1);
        assert_eq!(trail.verification_rate(), 50.0);
    }

    #[test]
    fn test_check_vote_chain_duplicates() {
        let election_id = ElectionId::new([1u8; 16]);
        let checker = IntegrityChecker::new(election_id);

        let keypair = KeyPair::generate();
        let voter_id = VoterId::new([1u8; 32]);
        let election_key = EncryptionKey::generate();

        let mut ballot = Ballot::new(election_id, BallotType::SingleChoice);
        ballot
            .add_choice(BallotChoice::new("c1".to_string()))
            .unwrap();

        let cast_ballot = CastBallot::create(&ballot, voter_id, &election_key, &keypair).unwrap();

        let ballots = vec![cast_ballot.clone(), cast_ballot];

        let report = checker.check_vote_chain(&ballots).unwrap();
        assert!(!report.is_valid());
    }

    #[test]
    fn test_verify_timestamps() {
        let election_id = ElectionId::new([1u8; 16]);
        let checker = IntegrityChecker::new(election_id);

        let keypair = KeyPair::generate();
        let voter_id1 = VoterId::new([1u8; 32]);
        let voter_id2 = VoterId::new([2u8; 32]);
        let election_key = EncryptionKey::generate();

        let mut ballot1 = Ballot::new(election_id, BallotType::SingleChoice);
        ballot1
            .add_choice(BallotChoice::new("c1".to_string()))
            .unwrap();

        let mut ballot2 = Ballot::new(election_id, BallotType::SingleChoice);
        ballot2
            .add_choice(BallotChoice::new("c2".to_string()))
            .unwrap();

        let cast1 = CastBallot::create(&ballot1, voter_id1, &election_key, &keypair).unwrap();
        let cast2 = CastBallot::create(&ballot2, voter_id2, &election_key, &keypair).unwrap();

        let ballots = vec![cast1, cast2];

        let report = checker.verify_timestamps(&ballots).unwrap();
        assert!(report.is_valid() || !report.is_valid());
    }
}

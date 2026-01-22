use crate::ballot::{Ballot, CastBallot};
use crate::voter::{Voter, VoterId};
use crate::{Election, ElectionResults};
use common::{ElectionId, PublicKey, Result, Signature, Timestamp, TxId, VotingError};
use crypto::encryption::EncryptionKey;
use crypto::keys::KeyPair;
use crypto::zkp::ballot_privacy::PrivateBallot;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteCaster {
    election: Election,
    votes_cast: HashMap<VoterId, VoteRecord>,
    voter_registry: HashSet<VoterId>,
}

impl VoteCaster {
    pub fn new(election: Election) -> Self {
        Self {
            election,
            votes_cast: HashMap::new(),
            voter_registry: HashSet::new(),
        }
    }

    pub fn register_voter(&mut self, voter_id: VoterId) {
        self.voter_registry.insert(voter_id);
    }

    pub fn cast_vote(
        &mut self,
        voter: &Voter,
        ballot: &Ballot,
        voter_keypair: &KeyPair,
        election_key: &EncryptionKey,
    ) -> Result<VoteReceipt> {
        if !self.voter_registry.contains(&voter.id) {
            return Err(VotingError::VoterNotEligible(
                "Voter not registered".to_string(),
            ));
        }

        if self.votes_cast.contains_key(&voter.id) {
            return Err(VotingError::AlreadyVoted);
        }

        let current_time = common::utils::current_timestamp();
        if !self.election.is_active(current_time) {
            return Err(VotingError::ElectionNotActive);
        }

        if ballot.election_id != self.election.id {
            return Err(VotingError::InvalidBallot(
                "Ballot election ID mismatch".to_string(),
            ));
        }

        ballot.validate()?;

        let cast_ballot = CastBallot::create(ballot, voter.id, election_key, voter_keypair)?;

        let vote_record = VoteRecord {
            voter_id: voter.id,
            cast_ballot: cast_ballot.clone(),
            cast_at: current_time,
        };

        self.votes_cast.insert(voter.id, vote_record);

        Ok(VoteReceipt::new(
            cast_ballot.ballot_id,
            self.election.id,
            voter.id,
            current_time,
        ))
    }

    pub fn submit_vote(
        &mut self,
        submission: VoteSubmission,
        voter_public_key: &PublicKey,
    ) -> Result<VoteReceipt> {
        if !self.voter_registry.contains(&submission.voter_id) {
            return Err(VotingError::VoterNotEligible(
                "Voter not registered".to_string(),
            ));
        }

        if self.votes_cast.contains_key(&submission.voter_id) {
            return Err(VotingError::AlreadyVoted);
        }

        let current_time = common::utils::current_timestamp();
        if !self.election.is_active(current_time) {
            return Err(VotingError::ElectionNotActive);
        }

        submission.verify(voter_public_key)?;

        let vote_record = VoteRecord {
            voter_id: submission.voter_id,
            cast_ballot: submission.cast_ballot.clone(),
            cast_at: current_time,
        };

        self.votes_cast.insert(submission.voter_id, vote_record);

        Ok(VoteReceipt::new(
            submission.cast_ballot.ballot_id,
            self.election.id,
            submission.voter_id,
            current_time,
        ))
    }

    pub fn has_voted(&self, voter_id: &VoterId) -> bool {
        self.votes_cast.contains_key(voter_id)
    }

    pub fn vote_count(&self) -> usize {
        self.votes_cast.len()
    }

    pub fn get_vote_record(&self, voter_id: &VoterId) -> Option<&VoteRecord> {
        self.votes_cast.get(voter_id)
    }

    pub fn all_cast_ballots(&self) -> Vec<&CastBallot> {
        self.votes_cast
            .values()
            .map(|record| &record.cast_ballot)
            .collect()
    }

    pub fn registered_voters(&self) -> usize {
        self.voter_registry.len()
    }

    pub fn turnout_percentage(&self) -> f64 {
        if self.voter_registry.is_empty() {
            return 0.0;
        }
        (self.votes_cast.len() as f64 / self.voter_registry.len() as f64) * 100.0
    }

    pub fn election(&self) -> &Election {
        &self.election
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteRecord {
    pub voter_id: VoterId,
    pub cast_ballot: CastBallot,
    pub cast_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteSubmission {
    pub voter_id: VoterId,
    pub cast_ballot: CastBallot,
    pub submission_signature: Signature,
    pub submitted_at: Timestamp,
}

impl VoteSubmission {
    pub fn create(
        voter_id: VoterId,
        cast_ballot: CastBallot,
        voter_keypair: &KeyPair,
    ) -> Result<Self> {
        let submitted_at = common::utils::current_timestamp();
        let message = Self::compute_message(&voter_id, &cast_ballot, submitted_at);
        let submission_signature = crypto::signatures::sign(&message, voter_keypair)?;

        Ok(Self {
            voter_id,
            cast_ballot,
            submission_signature: submission_signature.to_common(),
            submitted_at,
        })
    }

    pub fn verify(&self, voter_public_key: &PublicKey) -> Result<()> {
        self.cast_ballot.verify(voter_public_key)?;

        let message = Self::compute_message(&self.voter_id, &self.cast_ballot, self.submitted_at);
        crypto::signatures::verify(&message, &crypto::signatures::Signature::from_common(&self.submission_signature), &crypto::keys::PublicKey::from_common(voter_public_key))?;

        Ok(())
    }

    fn compute_message(
        voter_id: &VoterId,
        cast_ballot: &CastBallot,
        submitted_at: Timestamp,
    ) -> Vec<u8> {
        crypto::hash::hash_blake2b_multiple(&[
            voter_id.as_bytes(),
            cast_ballot.ballot_id.as_bytes(),
            &cast_ballot.election_id.0,
            &submitted_at.to_le_bytes(),
        ])
        .to_vec()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteReceipt {
    pub receipt_id: ReceiptId,
    pub ballot_id: crate::ballot::BallotId,
    pub election_id: ElectionId,
    pub voter_id: VoterId,
    pub cast_at: Timestamp,
    pub confirmation_code: String,
}

impl VoteReceipt {
    pub fn new(
        ballot_id: crate::ballot::BallotId,
        election_id: ElectionId,
        voter_id: VoterId,
        cast_at: Timestamp,
    ) -> Self {
        let receipt_id = ReceiptId::generate();
        let confirmation_code = Self::generate_confirmation_code(&receipt_id, &ballot_id);

        Self {
            receipt_id,
            ballot_id,
            election_id,
            voter_id,
            cast_at,
            confirmation_code,
        }
    }

    fn generate_confirmation_code(
        receipt_id: &ReceiptId,
        ballot_id: &crate::ballot::BallotId,
    ) -> String {
        let hash = crypto::hash::hash_blake2b_multiple(&[
            receipt_id.as_bytes(),
            ballot_id.as_bytes(),
        ]);
        hex::encode(&hash[..8])
    }

    pub fn verify_confirmation_code(&self) -> bool {
        let expected = Self::generate_confirmation_code(&self.receipt_id, &self.ballot_id);
        crypto::zkp::constant_time_eq(self.confirmation_code.as_bytes(), expected.as_bytes())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReceiptId(pub [u8; 32]);

impl ReceiptId {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn generate() -> Self {
        Self::new(common::utils::random_hash())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl std::fmt::Display for ReceiptId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

#[derive(Debug)]
pub struct VotingSession {
    election_id: ElectionId,
    voter: Voter,
    ballot: Option<Ballot>,
    state: SessionState,
    started_at: Timestamp,
}

impl VotingSession {
    pub fn new(election_id: ElectionId, voter: Voter) -> Self {
        Self {
            election_id,
            voter,
            ballot: None,
            state: SessionState::Started,
            started_at: common::utils::current_timestamp(),
        }
    }

    pub fn set_ballot(&mut self, ballot: Ballot) -> Result<()> {
        if self.state != SessionState::Started {
            return Err(VotingError::InvalidInput(
                "Session not in correct state".to_string(),
            ));
        }

        if ballot.election_id != self.election_id {
            return Err(VotingError::InvalidBallot(
                "Ballot election mismatch".to_string(),
            ));
        }

        self.ballot = Some(ballot);
        self.state = SessionState::BallotFilled;
        Ok(())
    }

    pub fn submit(
        self,
        voter_keypair: &KeyPair,
        election_key: &EncryptionKey,
    ) -> Result<(VoteSubmission, VoteReceipt)> {
        if self.state != SessionState::BallotFilled {
            return Err(VotingError::InvalidInput(
                "Session not ready for submission".to_string(),
            ));
        }

        let ballot = self
            .ballot
            .ok_or_else(|| VotingError::InvalidInput("No ballot in session".to_string()))?;

        let cast_ballot = CastBallot::create(&ballot, self.voter.id, election_key, voter_keypair)?;

        let submission = VoteSubmission::create(self.voter.id, cast_ballot, voter_keypair)?;

        let receipt = VoteReceipt::new(
            ballot.ballot_id,
            self.election_id,
            self.voter.id,
            common::utils::current_timestamp(),
        );

        Ok((submission, receipt))
    }

    pub fn cancel(mut self) {
        self.state = SessionState::Cancelled;
    }

    pub fn state(&self) -> &SessionState {
        &self.state
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Started,
    BallotFilled,
    Submitted,
    Cancelled,
}

pub struct VoteCastingStats {
    pub total_votes: usize,
    pub votes_per_hour: HashMap<u64, usize>,
    pub average_casting_time_ms: u64,
    pub peak_hour: Option<u64>,
}

impl VoteCastingStats {
    pub fn from_records(records: &[VoteRecord]) -> Self {
        let mut votes_per_hour = HashMap::new();
        let total_votes = records.len();

        for record in records {
            let hour = record.cast_at / 3600;
            *votes_per_hour.entry(hour).or_insert(0) += 1;
        }

        let peak_hour = votes_per_hour
            .iter()
            .max_by_key(|(_, &count)| count)
            .map(|(&hour, _)| hour);

        Self {
            total_votes,
            votes_per_hour,
            average_casting_time_ms: 0,
            peak_hour,
        }
    }

    pub fn votes_in_hour(&self, hour: u64) -> usize {
        self.votes_per_hour.get(&hour).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ballot::{BallotChoice, BallotType};
    use crate::{Candidate, ElectionConfig};

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

    fn create_test_voter() -> (Voter, KeyPair) {
        let keypair = KeyPair::generate();
        let voter = Voter::new(&keypair);
        (voter, keypair)
    }

    fn create_test_ballot(election_id: ElectionId) -> Ballot {
        let mut ballot = Ballot::new(election_id, BallotType::SingleChoice);
        ballot
            .add_choice(BallotChoice::new("c1".to_string()))
            .unwrap();
        ballot
    }

    #[test]
    fn test_vote_caster_creation() {
        let election = create_test_election();
        let caster = VoteCaster::new(election.clone());

        assert_eq!(caster.election().id, election.id);
        assert_eq!(caster.vote_count(), 0);
    }

    #[test]
    fn test_register_voter() {
        let election = create_test_election();
        let mut caster = VoteCaster::new(election);

        let (voter, _) = create_test_voter();
        caster.register_voter(voter.id);

        assert_eq!(caster.registered_voters(), 1);
    }

    #[test]
    fn test_cast_vote() {
        let election = create_test_election();
        let mut caster = VoteCaster::new(election.clone());

        let (mut voter, keypair) = create_test_voter();
        caster.register_voter(voter.id);

        let credentials = crate::voter::VoterCredentials::new(voter.id, "Test".to_string());
        voter.register(credentials).unwrap();

        let ballot = create_test_ballot(election.id);
        let election_key = EncryptionKey::generate();

        let receipt = caster
            .cast_vote(&voter, &ballot, &keypair, &election_key)
            .unwrap();

        assert_eq!(receipt.election_id, election.id);
        assert_eq!(caster.vote_count(), 1);
    }

    #[test]
    fn test_cast_vote_unregistered() {
        let election = create_test_election();
        let mut caster = VoteCaster::new(election.clone());

        let (mut voter, keypair) = create_test_voter();
        let credentials = crate::voter::VoterCredentials::new(voter.id, "Test".to_string());
        voter.register(credentials).unwrap();

        let ballot = create_test_ballot(election.id);
        let election_key = EncryptionKey::generate();

        let result = caster.cast_vote(&voter, &ballot, &keypair, &election_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_cast_vote_twice() {
        let election = create_test_election();
        let mut caster = VoteCaster::new(election.clone());

        let (mut voter, keypair) = create_test_voter();
        caster.register_voter(voter.id);

        let credentials = crate::voter::VoterCredentials::new(voter.id, "Test".to_string());
        voter.register(credentials).unwrap();

        let ballot = create_test_ballot(election.id);
        let election_key = EncryptionKey::generate();

        caster
            .cast_vote(&voter, &ballot, &keypair, &election_key)
            .unwrap();

        let result = caster.cast_vote(&voter, &ballot, &keypair, &election_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_has_voted() {
        let election = create_test_election();
        let mut caster = VoteCaster::new(election.clone());

        let (mut voter, keypair) = create_test_voter();
        caster.register_voter(voter.id);

        let credentials = crate::voter::VoterCredentials::new(voter.id, "Test".to_string());
        voter.register(credentials).unwrap();

        assert!(!caster.has_voted(&voter.id));

        let ballot = create_test_ballot(election.id);
        let election_key = EncryptionKey::generate();

        caster
            .cast_vote(&voter, &ballot, &keypair, &election_key)
            .unwrap();

        assert!(caster.has_voted(&voter.id));
    }

    #[test]
    fn test_turnout_percentage() {
        let election = create_test_election();
        let mut caster = VoteCaster::new(election.clone());

        let (voter1, keypair1) = create_test_voter();
        let (mut voter2, keypair2) = create_test_voter();

        caster.register_voter(voter1.id);
        caster.register_voter(voter2.id);

        let credentials = crate::voter::VoterCredentials::new(voter2.id, "Test".to_string());
        voter2.register(credentials).unwrap();

        let ballot = create_test_ballot(election.id);
        let election_key = EncryptionKey::generate();

        caster
            .cast_vote(&voter2, &ballot, &keypair2, &election_key)
            .unwrap();

        let turnout = caster.turnout_percentage();
        assert_eq!(turnout, 50.0);
    }

    #[test]
    fn test_vote_submission() {
        let (mut voter, keypair) = create_test_voter();
        let credentials = crate::voter::VoterCredentials::new(voter.id, "Test".to_string());
        voter.register(credentials).unwrap();

        let election_id = ElectionId::new([1u8; 16]);
        let ballot = create_test_ballot(election_id);
        let election_key = EncryptionKey::generate();

        let cast_ballot = CastBallot::create(&ballot, voter.id, &election_key, &keypair).unwrap();

        let submission = VoteSubmission::create(voter.id, cast_ballot, &keypair).unwrap();

        assert!(submission.verify(keypair.public_key()).is_ok());
    }

    #[test]
    fn test_vote_receipt() {
        let ballot_id = crate::ballot::BallotId::generate();
        let election_id = ElectionId::new([1u8; 16]);
        let voter_id = VoterId::new([1u8; 32]);

        let receipt = VoteReceipt::new(ballot_id, election_id, voter_id, 1000);

        assert_eq!(receipt.ballot_id, ballot_id);
        assert_eq!(receipt.election_id, election_id);
        assert!(receipt.verify_confirmation_code());
    }

    #[test]
    fn test_voting_session() {
        let election_id = ElectionId::new([1u8; 16]);
        let (voter, _) = create_test_voter();

        let session = VotingSession::new(election_id, voter);

        assert_eq!(session.state(), &SessionState::Started);
        assert_eq!(session.election_id, election_id);
    }

    #[test]
    fn test_voting_session_set_ballot() {
        let election_id = ElectionId::new([1u8; 16]);
        let (voter, _) = create_test_voter();

        let mut session = VotingSession::new(election_id, voter);
        let ballot = create_test_ballot(election_id);

        session.set_ballot(ballot).unwrap();
        assert_eq!(session.state(), &SessionState::BallotFilled);
    }

    #[test]
    fn test_voting_session_submit() {
        let election_id = ElectionId::new([1u8; 16]);
        let (voter, keypair) = create_test_voter();

        let mut session = VotingSession::new(election_id, voter);
        let ballot = create_test_ballot(election_id);
        session.set_ballot(ballot).unwrap();

        let election_key = EncryptionKey::generate();
        let result = session.submit(&keypair, &election_key);

        assert!(result.is_ok());
    }

    #[test]
    fn test_voting_session_cancel() {
        let election_id = ElectionId::new([1u8; 16]);
        let (voter, _) = create_test_voter();

        let mut session = VotingSession::new(election_id, voter);
        session.cancel();

        assert_eq!(session.state(), &SessionState::Cancelled);
    }

    #[test]
    fn test_vote_casting_stats() {
        let records = vec![
            VoteRecord {
                voter_id: VoterId::new([1u8; 32]),
                cast_ballot: create_mock_cast_ballot(),
                cast_at: 3600,
            },
            VoteRecord {
                voter_id: VoterId::new([2u8; 32]),
                cast_ballot: create_mock_cast_ballot(),
                cast_at: 3700,
            },
            VoteRecord {
                voter_id: VoterId::new([3u8; 32]),
                cast_ballot: create_mock_cast_ballot(),
                cast_at: 7200,
            },
        ];

        let stats = VoteCastingStats::from_records(&records);

        assert_eq!(stats.total_votes, 3);
        assert_eq!(stats.votes_in_hour(1), 2);
        assert_eq!(stats.votes_in_hour(2), 1);
    }

    fn create_mock_cast_ballot() -> CastBallot {
        let election_id = ElectionId::new([1u8; 16]);
        let ballot = create_test_ballot(election_id);
        let voter_id = VoterId::new([1u8; 32]);
        let election_key = EncryptionKey::generate();
        let keypair = KeyPair::generate();

        CastBallot::create(&ballot, voter_id, &election_key, &keypair).unwrap()
    }
}

/// Voting system implementation for secure, private ballot casting
///
/// This crate provides the core voting functionality including:
/// - Voter identity and registration
/// - Ballot creation and casting
/// - Vote verification and validation
/// - Eligibility checking
/// - Anonymity preservation
///
/// The voting system builds on top of the blockchain-core and crypto crates
/// to provide a complete end-to-end voting solution.

pub mod voter;
pub mod ballot;
pub mod vote_casting;
pub mod verification;
pub mod eligibility;
pub mod anonymity;

// Re-export commonly used types
pub use voter::{Voter, VoterCredentials, VoterRegistration, VoterStatus};
pub use ballot::{Ballot, BallotChoice, BallotType, CastBallot};
pub use vote_casting::{VoteCaster, VoteReceipt, VoteSubmission};
pub use verification::{VoteVerifier, VerificationResult};
pub use eligibility::{EligibilityChecker, EligibilityProof, EligibilityStatus};
pub use anonymity::{AnonymousVote, AnonymityLayer, VoterAnonymizer};

use common::{ElectionId, PublicKey, Result, Timestamp};
use serde::{Deserialize, Serialize};

/// Election metadata and configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Election {
    /// Unique election identifier
    pub id: ElectionId,

    /// Election title
    pub title: String,

    /// Election description
    pub description: String,

    /// List of candidates or choices
    pub candidates: Vec<Candidate>,

    /// Election start time
    pub start_time: Timestamp,

    /// Election end time
    pub end_time: Timestamp,

    /// Election authority public key
    pub authority_key: PublicKey,

    /// Election configuration
    pub config: ElectionConfig,
}

impl Election {
    /// Create a new election
    pub fn new(
        id: ElectionId,
        title: String,
        description: String,
        candidates: Vec<Candidate>,
        start_time: Timestamp,
        end_time: Timestamp,
        authority_key: PublicKey,
    ) -> Self {
        Self {
            id,
            title,
            description,
            candidates,
            start_time,
            end_time,
            authority_key,
            config: ElectionConfig::default(),
        }
    }

    /// Check if election is active
    pub fn is_active(&self, current_time: Timestamp) -> bool {
        current_time >= self.start_time && current_time <= self.end_time
    }

    /// Check if election has started
    pub fn has_started(&self, current_time: Timestamp) -> bool {
        current_time >= self.start_time
    }

    /// Check if election has ended
    pub fn has_ended(&self, current_time: Timestamp) -> bool {
        current_time > self.end_time
    }

    /// Get candidate by ID
    pub fn get_candidate(&self, candidate_id: &str) -> Option<&Candidate> {
        self.candidates.iter().find(|c| c.id == candidate_id)
    }

    /// Validate election configuration
    pub fn validate(&self) -> Result<()> {
        if self.title.is_empty() {
            return Err(common::VotingError::InvalidInput(
                "Election title cannot be empty".to_string(),
            ));
        }

        if self.candidates.is_empty() {
            return Err(common::VotingError::InvalidInput(
                "Election must have at least one candidate".to_string(),
            ));
        }

        if self.end_time <= self.start_time {
            return Err(common::VotingError::InvalidInput(
                "Election end time must be after start time".to_string(),
            ));
        }

        Ok(())
    }
}

/// Candidate information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Candidate {
    /// Unique candidate identifier
    pub id: String,

    /// Candidate name
    pub name: String,

    /// Party affiliation
    pub party: Option<String>,

    /// Candidate bio or description
    pub description: Option<String>,

    /// Additional metadata
    pub metadata: Option<String>,
}

impl Candidate {
    /// Create a new candidate
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            party: None,
            description: None,
            metadata: None,
        }
    }

    /// Create candidate with party
    pub fn with_party(id: String, name: String, party: String) -> Self {
        Self {
            id,
            name,
            party: Some(party),
            description: None,
            metadata: None,
        }
    }
}

/// Election configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectionConfig {
    /// Allow write-in candidates
    pub allow_write_in: bool,

    /// Require voter registration
    pub require_registration: bool,

    /// Enable ballot secrecy
    pub ballot_secrecy: bool,

    /// Enable receipt-free voting
    pub receipt_free: bool,

    /// Maximum votes per voter
    pub max_votes_per_voter: u32,

    /// Enable ranked choice voting
    pub ranked_choice: bool,

    /// Minimum voters required for valid election
    pub min_voters: Option<u64>,

    /// Maximum voters allowed
    pub max_voters: Option<u64>,
}

impl Default for ElectionConfig {
    fn default() -> Self {
        Self {
            allow_write_in: false,
            require_registration: true,
            ballot_secrecy: true,
            receipt_free: true,
            max_votes_per_voter: 1,
            ranked_choice: false,
            min_voters: None,
            max_voters: None,
        }
    }
}

/// Vote tally and results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectionResults {
    /// Election ID
    pub election_id: ElectionId,

    /// Total votes cast
    pub total_votes: u64,

    /// Vote counts per candidate
    pub candidate_tallies: Vec<CandidateTally>,

    /// Timestamp when tallied
    pub tallied_at: Timestamp,

    /// Whether results are final
    pub is_final: bool,
}

impl ElectionResults {
    /// Create new election results
    pub fn new(election_id: ElectionId) -> Self {
        Self {
            election_id,
            total_votes: 0,
            candidate_tallies: Vec::new(),
            tallied_at: common::utils::current_timestamp(),
            is_final: false,
        }
    }

    /// Add vote to tally
    pub fn add_vote(&mut self, candidate_id: &str) {
        self.total_votes += 1;

        if let Some(tally) = self
            .candidate_tallies
            .iter_mut()
            .find(|t| t.candidate_id == candidate_id)
        {
            tally.vote_count += 1;
        } else {
            self.candidate_tallies.push(CandidateTally {
                candidate_id: candidate_id.to_string(),
                vote_count: 1,
            });
        }
    }

    /// Get winner (candidate with most votes)
    pub fn get_winner(&self) -> Option<&CandidateTally> {
        self.candidate_tallies
            .iter()
            .max_by_key(|t| t.vote_count)
    }

    /// Sort tallies by vote count (descending)
    pub fn sort_by_votes(&mut self) {
        self.candidate_tallies
            .sort_by(|a, b| b.vote_count.cmp(&a.vote_count));
    }
}

/// Vote tally for a specific candidate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateTally {
    /// Candidate identifier
    pub candidate_id: String,

    /// Number of votes received
    pub vote_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_election() -> Election {
        let candidates = vec![
            Candidate::new("candidate_1".to_string(), "Alice".to_string()),
            Candidate::new("candidate_2".to_string(), "Bob".to_string()),
        ];

        Election::new(
            ElectionId::new([1u8; 16]),
            "Test Election".to_string(),
            "A test election".to_string(),
            candidates,
            1000,
            2000,
            PublicKey::new([0u8; 32]),
        )
    }

    #[test]
    fn test_election_creation() {
        let election = create_test_election();
        assert_eq!(election.title, "Test Election");
        assert_eq!(election.candidates.len(), 2);
    }

    #[test]
    fn test_election_is_active() {
        let election = create_test_election();
        assert!(!election.is_active(500));
        assert!(election.is_active(1500));
        assert!(!election.is_active(2500));
    }

    #[test]
    fn test_election_has_started() {
        let election = create_test_election();
        assert!(!election.has_started(500));
        assert!(election.has_started(1000));
        assert!(election.has_started(1500));
    }

    #[test]
    fn test_election_has_ended() {
        let election = create_test_election();
        assert!(!election.has_ended(1500));
        assert!(!election.has_ended(2000));
        assert!(election.has_ended(2001));
    }

    #[test]
    fn test_get_candidate() {
        let election = create_test_election();
        let candidate = election.get_candidate("candidate_1");
        assert!(candidate.is_some());
        assert_eq!(candidate.unwrap().name, "Alice");

        let missing = election.get_candidate("candidate_999");
        assert!(missing.is_none());
    }

    #[test]
    fn test_election_validation() {
        let election = create_test_election();
        assert!(election.validate().is_ok());
    }

    #[test]
    fn test_election_validation_empty_title() {
        let mut election = create_test_election();
        election.title = "".to_string();
        assert!(election.validate().is_err());
    }

    #[test]
    fn test_election_validation_no_candidates() {
        let mut election = create_test_election();
        election.candidates.clear();
        assert!(election.validate().is_err());
    }

    #[test]
    fn test_election_validation_invalid_times() {
        let mut election = create_test_election();
        election.end_time = election.start_time - 1;
        assert!(election.validate().is_err());
    }

    #[test]
    fn test_candidate_creation() {
        let candidate = Candidate::new("id1".to_string(), "John Doe".to_string());
        assert_eq!(candidate.id, "id1");
        assert_eq!(candidate.name, "John Doe");
        assert!(candidate.party.is_none());
    }

    #[test]
    fn test_candidate_with_party() {
        let candidate = Candidate::with_party(
            "id2".to_string(),
            "Jane Smith".to_string(),
            "Party A".to_string(),
        );
        assert_eq!(candidate.party, Some("Party A".to_string()));
    }

    #[test]
    fn test_election_config_default() {
        let config = ElectionConfig::default();
        assert!(!config.allow_write_in);
        assert!(config.require_registration);
        assert!(config.ballot_secrecy);
        assert!(config.receipt_free);
        assert_eq!(config.max_votes_per_voter, 1);
    }

    #[test]
    fn test_election_results_creation() {
        let results = ElectionResults::new(ElectionId::new([1u8; 16]));
        assert_eq!(results.total_votes, 0);
        assert!(results.candidate_tallies.is_empty());
        assert!(!results.is_final);
    }

    #[test]
    fn test_election_results_add_vote() {
        let mut results = ElectionResults::new(ElectionId::new([1u8; 16]));

        results.add_vote("candidate_1");
        results.add_vote("candidate_1");
        results.add_vote("candidate_2");

        assert_eq!(results.total_votes, 3);
        assert_eq!(results.candidate_tallies.len(), 2);

        let tally1 = results
            .candidate_tallies
            .iter()
            .find(|t| t.candidate_id == "candidate_1")
            .unwrap();
        assert_eq!(tally1.vote_count, 2);
    }

    #[test]
    fn test_election_results_get_winner() {
        let mut results = ElectionResults::new(ElectionId::new([1u8; 16]));

        results.add_vote("candidate_1");
        results.add_vote("candidate_1");
        results.add_vote("candidate_2");

        let winner = results.get_winner().unwrap();
        assert_eq!(winner.candidate_id, "candidate_1");
        assert_eq!(winner.vote_count, 2);
    }

    #[test]
    fn test_election_results_sort() {
        let mut results = ElectionResults::new(ElectionId::new([1u8; 16]));

        results.add_vote("candidate_1");
        results.add_vote("candidate_2");
        results.add_vote("candidate_2");
        results.add_vote("candidate_3");
        results.add_vote("candidate_3");
        results.add_vote("candidate_3");

        results.sort_by_votes();

        assert_eq!(results.candidate_tallies[0].candidate_id, "candidate_3");
        assert_eq!(results.candidate_tallies[0].vote_count, 3);
        assert_eq!(results.candidate_tallies[1].candidate_id, "candidate_2");
        assert_eq!(results.candidate_tallies[1].vote_count, 2);
    }
}

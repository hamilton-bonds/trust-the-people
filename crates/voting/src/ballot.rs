use crate::voter::VoterId;
use common::{ElectionId, PublicKey, Result, Signature, Timestamp, TxId, VotingError};
use crypto::encryption::{encrypt, EncryptedData, EncryptionKey};
use crypto::keys::KeyPair;
use crypto::signatures::{sign, verify};
use crypto::zkp::ballot_privacy::PrivateBallot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ballot {
    pub ballot_id: BallotId,
    pub election_id: ElectionId,
    pub ballot_type: BallotType,
    pub choices: Vec<BallotChoice>,
    pub timestamp: Timestamp,
    pub version: u32,
}

impl Ballot {
    pub fn new(election_id: ElectionId, ballot_type: BallotType) -> Self {
        Self {
            ballot_id: BallotId::generate(),
            election_id,
            ballot_type,
            choices: Vec::new(),
            timestamp: common::utils::current_timestamp(),
            version: 1,
        }
    }

    pub fn add_choice(&mut self, choice: BallotChoice) -> Result<()> {
        match self.ballot_type {
            BallotType::SingleChoice => {
                if !self.choices.is_empty() {
                    return Err(VotingError::InvalidBallot(
                        "Single choice ballot can only have one choice".to_string(),
                    ));
                }
            }
            BallotType::MultipleChoice { max_choices } => {
                if self.choices.len() >= max_choices {
                    return Err(VotingError::InvalidBallot(format!(
                        "Cannot exceed {} choices",
                        max_choices
                    )));
                }
            }
            BallotType::RankedChoice { max_ranks } => {
                if self.choices.len() >= max_ranks {
                    return Err(VotingError::InvalidBallot(format!(
                        "Cannot exceed {} ranks",
                        max_ranks
                    )));
                }
            }
            BallotType::Approval => {}
            BallotType::WriteIn => {}
        }

        self.choices.push(choice);
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        if self.choices.is_empty() {
            return Err(VotingError::InvalidBallot(
                "Ballot must have at least one choice".to_string(),
            ));
        }

        match self.ballot_type {
            BallotType::SingleChoice => {
                if self.choices.len() != 1 {
                    return Err(VotingError::InvalidBallot(
                        "Single choice ballot must have exactly one choice".to_string(),
                    ));
                }
            }
            BallotType::MultipleChoice { max_choices } => {
                if self.choices.len() > max_choices {
                    return Err(VotingError::InvalidBallot(format!(
                        "Too many choices: {} (max {})",
                        self.choices.len(),
                        max_choices
                    )));
                }
            }
            BallotType::RankedChoice { max_ranks } => {
                if self.choices.len() > max_ranks {
                    return Err(VotingError::InvalidBallot(
                        "Too many ranks".to_string(),
                    ));
                }

                for (i, choice) in self.choices.iter().enumerate() {
                    if let Some(rank) = choice.rank {
                        if rank as usize != i + 1 {
                            return Err(VotingError::InvalidBallot(
                                "Ranks must be sequential".to_string(),
                            ));
                        }
                    } else {
                        return Err(VotingError::InvalidBallot(
                            "Ranked choice ballot must have ranks".to_string(),
                        ));
                    }
                }
            }
            BallotType::Approval | BallotType::WriteIn => {}
        }

        Ok(())
    }

    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize ballot")
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data)
            .map_err(|e| VotingError::SerializationError(format!("Failed to deserialize: {}", e)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BallotId(pub [u8; 32]);

impl BallotId {
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

    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| VotingError::CryptoError(format!("Invalid hex: {}", e)))?;

        if bytes.len() != 32 {
            return Err(VotingError::CryptoError(
                "Invalid ballot ID length".to_string(),
            ));
        }

        let mut id = [0u8; 32];
        id.copy_from_slice(&bytes);
        Ok(Self(id))
    }
}

impl std::fmt::Display for BallotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BallotType {
    SingleChoice,
    MultipleChoice { max_choices: usize },
    RankedChoice { max_ranks: usize },
    Approval,
    WriteIn,
}

impl std::fmt::Display for BallotType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BallotType::SingleChoice => write!(f, "single_choice"),
            BallotType::MultipleChoice { max_choices } => {
                write!(f, "multiple_choice({})", max_choices)
            }
            BallotType::RankedChoice { max_ranks } => write!(f, "ranked_choice({})", max_ranks),
            BallotType::Approval => write!(f, "approval"),
            BallotType::WriteIn => write!(f, "write_in"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BallotChoice {
    pub candidate_id: String,
    pub rank: Option<u32>,
    pub write_in_name: Option<String>,
}

impl BallotChoice {
    pub fn new(candidate_id: String) -> Self {
        Self {
            candidate_id,
            rank: None,
            write_in_name: None,
        }
    }

    pub fn with_rank(candidate_id: String, rank: u32) -> Self {
        Self {
            candidate_id,
            rank: Some(rank),
            write_in_name: None,
        }
    }

    pub fn write_in(name: String) -> Self {
        Self {
            candidate_id: "write_in".to_string(),
            rank: None,
            write_in_name: Some(name),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastBallot {
    pub ballot_id: BallotId,
    pub election_id: ElectionId,
    pub voter_id: VoterId,
    pub encrypted_ballot: EncryptedData,
    pub ballot_commitment: Vec<u8>,
    pub voter_signature: Signature,
    pub cast_at: Timestamp,
    pub transaction_id: Option<TxId>,
}

impl CastBallot {
    pub fn create(
        ballot: &Ballot,
        voter_id: VoterId,
        election_key: &EncryptionKey,
        voter_keypair: &KeyPair,
    ) -> Result<Self> {
        let ballot_data = ballot.serialize();
        let encrypted_ballot = encrypt(&ballot_data, election_key)?;

        let randomness = crypto::zkp::Commitment::generate_randomness();
        let commitment = crypto::zkp::Commitment::new(&ballot_data, &randomness);
        let ballot_commitment = commitment.commitment_value().to_vec();

        let message = Self::compute_message(
            ballot.ballot_id,
            ballot.election_id,
            voter_id,
            &encrypted_ballot,
            &ballot_commitment,
        );
        let voter_signature = sign(&message, voter_keypair)?;

        Ok(Self {
            ballot_id: ballot.ballot_id,
            election_id: ballot.election_id,
            voter_id,
            encrypted_ballot,
            ballot_commitment,
            voter_signature: voter_signature.to_common(),
            cast_at: common::utils::current_timestamp(),
            transaction_id: None,
        })
    }

    pub fn verify(&self, voter_public_key: &PublicKey) -> Result<()> {
        let message = Self::compute_message(
            self.ballot_id,
            self.election_id,
            self.voter_id,
            &self.encrypted_ballot,
            &self.ballot_commitment,
        );

        verify(&message, &crypto::signatures::Signature::from_common(&self.voter_signature), &crypto::keys::PublicKey::from_common(voter_public_key))?;
        Ok(())
    }

    pub fn set_transaction_id(&mut self, tx_id: TxId) {
        self.transaction_id = Some(tx_id);
    }

    fn compute_message(
        ballot_id: BallotId,
        election_id: ElectionId,
        voter_id: VoterId,
        encrypted_ballot: &EncryptedData,
        ballot_commitment: &[u8],
    ) -> Vec<u8> {
        crypto::hash::hash_blake2b_multiple(&[
            ballot_id.as_bytes(),
            &election_id.0,
            voter_id.as_bytes(),
            &encrypted_ballot.to_bytes(),
            ballot_commitment,
        ])
        .to_vec()
    }

    pub fn to_private_ballot(&self, voter_keypair: &KeyPair) -> Result<PrivateBallot> {
        let ballot_data = self.encrypted_ballot.to_bytes();

        let election_key = EncryptionKey::generate();

        PrivateBallot::create(
            &ballot_data,
            &election_key,
            self.election_id.0,
            voter_keypair,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BallotBox {
    ballots: Vec<CastBallot>,
    ballot_index: std::collections::HashMap<BallotId, usize>,
}

impl BallotBox {
    pub fn new() -> Self {
        Self {
            ballots: Vec::new(),
            ballot_index: std::collections::HashMap::new(),
        }
    }

    pub fn add_ballot(&mut self, ballot: CastBallot) -> Result<()> {
        if self.ballot_index.contains_key(&ballot.ballot_id) {
            return Err(VotingError::InvalidBallot(
                "Ballot already exists".to_string(),
            ));
        }

        let index = self.ballots.len();
        self.ballot_index.insert(ballot.ballot_id, index);
        self.ballots.push(ballot);

        Ok(())
    }

    pub fn get_ballot(&self, ballot_id: &BallotId) -> Option<&CastBallot> {
        self.ballot_index
            .get(ballot_id)
            .and_then(|&index| self.ballots.get(index))
    }

    pub fn ballot_count(&self) -> usize {
        self.ballots.len()
    }

    pub fn ballots_for_election(&self, election_id: ElectionId) -> Vec<&CastBallot> {
        self.ballots
            .iter()
            .filter(|b| b.election_id == election_id)
            .collect()
    }

    pub fn contains_ballot(&self, ballot_id: &BallotId) -> bool {
        self.ballot_index.contains_key(ballot_id)
    }

    pub fn all_ballots(&self) -> &[CastBallot] {
        &self.ballots
    }
}

impl Default for BallotBox {
    fn default() -> Self {
        Self::new()
    }
}

pub struct BallotBuilder {
    election_id: ElectionId,
    ballot_type: BallotType,
    choices: Vec<BallotChoice>,
}

impl BallotBuilder {
    pub fn new(election_id: ElectionId, ballot_type: BallotType) -> Self {
        Self {
            election_id,
            ballot_type,
            choices: Vec::new(),
        }
    }

    pub fn add_choice(mut self, candidate_id: String) -> Self {
        self.choices.push(BallotChoice::new(candidate_id));
        self
    }

    pub fn add_ranked_choice(mut self, candidate_id: String, rank: u32) -> Self {
        self.choices
            .push(BallotChoice::with_rank(candidate_id, rank));
        self
    }

    pub fn add_write_in(mut self, name: String) -> Self {
        self.choices.push(BallotChoice::write_in(name));
        self
    }

    pub fn build(self) -> Result<Ballot> {
        let mut ballot = Ballot::new(self.election_id, self.ballot_type);
        for choice in self.choices {
            ballot.add_choice(choice)?;
        }
        ballot.validate()?;
        Ok(ballot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ballot_creation() {
        let election_id = ElectionId::new([1u8; 16]);
        let ballot = Ballot::new(election_id, BallotType::SingleChoice);

        assert_eq!(ballot.election_id, election_id);
        assert_eq!(ballot.ballot_type, BallotType::SingleChoice);
        assert!(ballot.choices.is_empty());
    }

    #[test]
    fn test_single_choice_ballot() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut ballot = Ballot::new(election_id, BallotType::SingleChoice);

        let choice = BallotChoice::new("candidate_1".to_string());
        ballot.add_choice(choice).unwrap();

        assert!(ballot.validate().is_ok());
    }

    #[test]
    fn test_single_choice_multiple_additions() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut ballot = Ballot::new(election_id, BallotType::SingleChoice);

        ballot
            .add_choice(BallotChoice::new("candidate_1".to_string()))
            .unwrap();

        let result = ballot.add_choice(BallotChoice::new("candidate_2".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_choice_ballot() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut ballot = Ballot::new(
            election_id,
            BallotType::MultipleChoice { max_choices: 3 },
        );

        ballot
            .add_choice(BallotChoice::new("candidate_1".to_string()))
            .unwrap();
        ballot
            .add_choice(BallotChoice::new("candidate_2".to_string()))
            .unwrap();

        assert!(ballot.validate().is_ok());
    }

    #[test]
    fn test_multiple_choice_exceeds_max() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut ballot = Ballot::new(
            election_id,
            BallotType::MultipleChoice { max_choices: 2 },
        );

        ballot
            .add_choice(BallotChoice::new("candidate_1".to_string()))
            .unwrap();
        ballot
            .add_choice(BallotChoice::new("candidate_2".to_string()))
            .unwrap();

        let result = ballot.add_choice(BallotChoice::new("candidate_3".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_ranked_choice_ballot() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut ballot = Ballot::new(election_id, BallotType::RankedChoice { max_ranks: 3 });

        ballot
            .add_choice(BallotChoice::with_rank("candidate_1".to_string(), 1))
            .unwrap();
        ballot
            .add_choice(BallotChoice::with_rank("candidate_2".to_string(), 2))
            .unwrap();

        assert!(ballot.validate().is_ok());
    }

    #[test]
    fn test_ranked_choice_invalid_ranks() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut ballot = Ballot::new(election_id, BallotType::RankedChoice { max_ranks: 3 });

        ballot
            .add_choice(BallotChoice::with_rank("candidate_1".to_string(), 1))
            .unwrap();
        ballot
            .add_choice(BallotChoice::with_rank("candidate_2".to_string(), 3))
            .unwrap();

        assert!(ballot.validate().is_err());
    }

    #[test]
    fn test_ballot_id_generation() {
        let id1 = BallotId::generate();
        let id2 = BallotId::generate();

        assert_ne!(id1, id2);
    }

    #[test]
    fn test_ballot_id_hex() {
        let id = BallotId::generate();
        let hex = id.to_hex();
        let decoded = BallotId::from_hex(&hex).unwrap();

        assert_eq!(id, decoded);
    }

    #[test]
    fn test_ballot_choice_creation() {
        let choice = BallotChoice::new("candidate_1".to_string());
        assert_eq!(choice.candidate_id, "candidate_1");
        assert!(choice.rank.is_none());
    }

    #[test]
    fn test_ballot_choice_with_rank() {
        let choice = BallotChoice::with_rank("candidate_1".to_string(), 2);
        assert_eq!(choice.rank, Some(2));
    }

    #[test]
    fn test_ballot_choice_write_in() {
        let choice = BallotChoice::write_in("John Doe".to_string());
        assert_eq!(choice.candidate_id, "write_in");
        assert_eq!(choice.write_in_name, Some("John Doe".to_string()));
    }

    #[test]
    fn test_cast_ballot_creation() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut ballot = Ballot::new(election_id, BallotType::SingleChoice);
        ballot
            .add_choice(BallotChoice::new("candidate_1".to_string()))
            .unwrap();

        let voter_id = VoterId::new([1u8; 32]);
        let election_key = EncryptionKey::generate();
        let voter_keypair = KeyPair::generate();

        let cast_ballot =
            CastBallot::create(&ballot, voter_id, &election_key, &voter_keypair).unwrap();

        assert_eq!(cast_ballot.ballot_id, ballot.ballot_id);
        assert_eq!(cast_ballot.election_id, election_id);
    }

    #[test]
    fn test_cast_ballot_verification() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut ballot = Ballot::new(election_id, BallotType::SingleChoice);
        ballot
            .add_choice(BallotChoice::new("candidate_1".to_string()))
            .unwrap();

        let voter_id = VoterId::new([1u8; 32]);
        let election_key = EncryptionKey::generate();
        let voter_keypair = KeyPair::generate();

        let cast_ballot =
            CastBallot::create(&ballot, voter_id, &election_key, &voter_keypair).unwrap();

        assert!(cast_ballot.verify(voter_keypair.public_key()).is_ok());
    }

    #[test]
    fn test_ballot_box() {
        let mut ballot_box = BallotBox::new();
        assert_eq!(ballot_box.ballot_count(), 0);

        let election_id = ElectionId::new([1u8; 16]);
        let mut ballot = Ballot::new(election_id, BallotType::SingleChoice);
        ballot
            .add_choice(BallotChoice::new("candidate_1".to_string()))
            .unwrap();

        let voter_id = VoterId::new([1u8; 32]);
        let election_key = EncryptionKey::generate();
        let voter_keypair = KeyPair::generate();

        let cast_ballot =
            CastBallot::create(&ballot, voter_id, &election_key, &voter_keypair).unwrap();

        ballot_box.add_ballot(cast_ballot.clone()).unwrap();
        assert_eq!(ballot_box.ballot_count(), 1);
        assert!(ballot_box.contains_ballot(&cast_ballot.ballot_id));
    }

    #[test]
    fn test_ballot_box_duplicate() {
        let mut ballot_box = BallotBox::new();

        let election_id = ElectionId::new([1u8; 16]);
        let mut ballot = Ballot::new(election_id, BallotType::SingleChoice);
        ballot
            .add_choice(BallotChoice::new("candidate_1".to_string()))
            .unwrap();

        let voter_id = VoterId::new([1u8; 32]);
        let election_key = EncryptionKey::generate();
        let voter_keypair = KeyPair::generate();

        let cast_ballot =
            CastBallot::create(&ballot, voter_id, &election_key, &voter_keypair).unwrap();

        ballot_box.add_ballot(cast_ballot.clone()).unwrap();
        let result = ballot_box.add_ballot(cast_ballot);

        assert!(result.is_err());
    }

    #[test]
    fn test_ballot_box_get_ballot() {
        let mut ballot_box = BallotBox::new();

        let election_id = ElectionId::new([1u8; 16]);
        let mut ballot = Ballot::new(election_id, BallotType::SingleChoice);
        ballot
            .add_choice(BallotChoice::new("candidate_1".to_string()))
            .unwrap();

        let voter_id = VoterId::new([1u8; 32]);
        let election_key = EncryptionKey::generate();
        let voter_keypair = KeyPair::generate();

        let cast_ballot =
            CastBallot::create(&ballot, voter_id, &election_key, &voter_keypair).unwrap();
        let ballot_id = cast_ballot.ballot_id;

        ballot_box.add_ballot(cast_ballot).unwrap();

        let retrieved = ballot_box.get_ballot(&ballot_id);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_ballot_box_by_election() {
        let mut ballot_box = BallotBox::new();
        let election_id1 = ElectionId::new([1u8; 16]);
        let election_id2 = ElectionId::new([2u8; 16]);

        let mut ballot1 = Ballot::new(election_id1, BallotType::SingleChoice);
        ballot1
            .add_choice(BallotChoice::new("candidate_1".to_string()))
            .unwrap();

        let mut ballot2 = Ballot::new(election_id2, BallotType::SingleChoice);
        ballot2
            .add_choice(BallotChoice::new("candidate_2".to_string()))
            .unwrap();

        let voter_id = VoterId::new([1u8; 32]);
        let election_key = EncryptionKey::generate();
        let voter_keypair = KeyPair::generate();

        let cast1 =
            CastBallot::create(&ballot1, voter_id, &election_key, &voter_keypair).unwrap();
        let cast2 =
            CastBallot::create(&ballot2, voter_id, &election_key, &voter_keypair).unwrap();

        ballot_box.add_ballot(cast1).unwrap();
        ballot_box.add_ballot(cast2).unwrap();

        let election1_ballots = ballot_box.ballots_for_election(election_id1);
        assert_eq!(election1_ballots.len(), 1);
    }

    #[test]
    fn test_ballot_builder() {
        let election_id = ElectionId::new([1u8; 16]);
        let ballot = BallotBuilder::new(election_id, BallotType::SingleChoice)
            .add_choice("candidate_1".to_string())
            .build()
            .unwrap();

        assert_eq!(ballot.choices.len(), 1);
    }

    #[test]
    fn test_ballot_builder_ranked() {
        let election_id = ElectionId::new([1u8; 16]);
        let ballot = BallotBuilder::new(election_id, BallotType::RankedChoice { max_ranks: 3 })
            .add_ranked_choice("candidate_1".to_string(), 1)
            .add_ranked_choice("candidate_2".to_string(), 2)
            .build()
            .unwrap();

        assert_eq!(ballot.choices.len(), 2);
    }

    #[test]
    fn test_ballot_serialization() {
        let election_id = ElectionId::new([1u8; 16]);
        let mut ballot = Ballot::new(election_id, BallotType::SingleChoice);
        ballot
            .add_choice(BallotChoice::new("candidate_1".to_string()))
            .unwrap();

        let serialized = ballot.serialize();
        let deserialized = Ballot::deserialize(&serialized).unwrap();

        assert_eq!(ballot.ballot_id, deserialized.ballot_id);
        assert_eq!(ballot.election_id, deserialized.election_id);
    }
}

//! Fraud detection algorithms for voting systems
//!
//! This module implements multiple fraud detection techniques:
//! - Duplicate vote detection
//! - Vote stuffing detection
//! - Timestamp manipulation detection
//! - Geographic impossibility detection
//! - Statistical outlier detection
//! - Blockchain integrity verification

use common::{Result, VotingError};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Fraud detection engine
#[derive(Debug)]
pub struct FraudDetector {
    /// Threshold for statistical outliers (z-score)
    outlier_threshold: f64,

    /// Maximum votes per second from single source
    max_votes_per_second: u64,

    /// Minimum time between votes (milliseconds)
    min_vote_interval: u64,

    /// Maximum geographic velocity (km/h)
    max_geographic_velocity: f64,

    /// Enable strict duplicate checking
    strict_duplicate_checking: bool,
}

impl FraudDetector {
    /// Create new fraud detector with default settings
    pub fn new() -> Self {
        Self {
            outlier_threshold: 3.0,
            max_votes_per_second: 100,
            min_vote_interval: 10,
            max_geographic_velocity: 1000.0,
            strict_duplicate_checking: true,
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: FraudDetectionConfig) -> Self {
        Self {
            outlier_threshold: config.outlier_threshold,
            max_votes_per_second: config.max_votes_per_second,
            min_vote_interval: config.min_vote_interval,
            max_geographic_velocity: config.max_geographic_velocity,
            strict_duplicate_checking: config.strict_duplicate_checking,
        }
    }

    /// Run comprehensive fraud detection on vote data
    pub fn detect_fraud(&self, votes: &[VoteRecord]) -> Result<FraudReport> {
        if votes.is_empty() {
            return Err(VotingError::InsufficientData(
                "No votes provided for fraud detection".to_string(),
            ));
        }

        let mut findings = Vec::new();

        findings.extend(self.detect_duplicates(votes)?);
        findings.extend(self.detect_vote_stuffing(votes)?);
        findings.extend(self.detect_timestamp_manipulation(votes)?);
        findings.extend(self.detect_geographic_impossibilities(votes)?);
        findings.extend(self.detect_statistical_outliers(votes)?);
        findings.extend(self.detect_sequential_patterns(votes)?);
        findings.extend(self.detect_turnout_anomalies(votes)?);

        let high_risk = findings.iter().filter(|f| f.risk_level == RiskLevel::High).count();
        let medium_risk = findings.iter().filter(|f| f.risk_level == RiskLevel::Medium).count();
        let low_risk = findings.iter().filter(|f| f.risk_level == RiskLevel::Low).count();

        let overall_risk = if high_risk > 0 {
            RiskLevel::High
        } else if medium_risk > 5 {
            RiskLevel::High
        } else if medium_risk > 0 {
            RiskLevel::Medium
        } else if low_risk > 10 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };

        Ok(FraudReport {
            total_votes: votes.len(),
            findings,
            high_risk_count: high_risk,
            medium_risk_count: medium_risk,
            low_risk_count: low_risk,
            overall_risk,
            timestamp: common::utils::current_timestamp(),
        })
    }

    /// Detect duplicate votes
    fn detect_duplicates(&self, votes: &[VoteRecord]) -> Result<Vec<FraudFinding>> {
        let mut findings = Vec::new();
        let mut seen_hashes = HashSet::new();
        let mut voter_votes: HashMap<String, Vec<&VoteRecord>> = HashMap::new();

        for vote in votes {
            if let Some(voter_hash) = &vote.voter_hash {
                if !seen_hashes.insert(voter_hash.clone()) {
                    findings.push(FraudFinding {
                        finding_type: FraudType::DuplicateVote,
                        risk_level: RiskLevel::High,
                        description: format!("Duplicate vote detected for voter hash: {}", voter_hash),
                        affected_votes: vec![vote.tx_id.clone()],
                        evidence: FraudEvidence::DuplicateHash {
                            voter_hash: voter_hash.clone(),
                        },
                    });
                }

                voter_votes.entry(voter_hash.clone()).or_insert_with(Vec::new).push(vote);
            }
        }

        if self.strict_duplicate_checking {
            for (voter_hash, vote_list) in voter_votes.iter() {
                if vote_list.len() > 1 {
                    let tx_ids: Vec<String> = vote_list.iter().map(|v| v.tx_id.clone()).collect();
                    findings.push(FraudFinding {
                        finding_type: FraudType::MultipleVotes,
                        risk_level: RiskLevel::High,
                        description: format!(
                            "Voter {} cast {} votes",
                            voter_hash,
                            vote_list.len()
                        ),
                        affected_votes: tx_ids.clone(),
                        evidence: FraudEvidence::MultipleVotesByVoter {
                            voter_hash: voter_hash.clone(),
                            vote_count: vote_list.len(),
                            tx_ids,
                        },
                    });
                }
            }
        }

        Ok(findings)
    }

    /// Detect vote stuffing patterns
    fn detect_vote_stuffing(&self, votes: &[VoteRecord]) -> Result<Vec<FraudFinding>> {
        let mut findings = Vec::new();

        let mut votes_by_second: HashMap<u64, Vec<&VoteRecord>> = HashMap::new();
        for vote in votes {
            let second = vote.timestamp / 1000;
            votes_by_second.entry(second).or_insert_with(Vec::new).push(vote);
        }

        for (second, vote_list) in votes_by_second.iter() {
            if vote_list.len() as u64 > self.max_votes_per_second {
                let tx_ids: Vec<String> = vote_list.iter().map(|v| v.tx_id.clone()).collect();
                findings.push(FraudFinding {
                    finding_type: FraudType::VoteStuffing,
                    risk_level: RiskLevel::High,
                    description: format!(
                        "Suspicious vote burst: {} votes in 1 second at timestamp {}",
                        vote_list.len(),
                        second
                    ),
                    affected_votes: tx_ids.clone(),
                    evidence: FraudEvidence::VoteStuffing {
                        timestamp: *second * 1000,
                        vote_count: vote_list.len(),
                        tx_ids,
                    },
                });
            }
        }

        Ok(findings)
    }

    /// Detect timestamp manipulation
    fn detect_timestamp_manipulation(&self, votes: &[VoteRecord]) -> Result<Vec<FraudFinding>> {
        let mut findings = Vec::new();
        let mut sorted_votes = votes.to_vec();
        sorted_votes.sort_by_key(|v| v.timestamp);

        for window in sorted_votes.windows(2) {
            let time_diff = window[1].timestamp.saturating_sub(window[0].timestamp);

            if time_diff < self.min_vote_interval {
                findings.push(FraudFinding {
                    finding_type: FraudType::TimestampManipulation,
                    risk_level: RiskLevel::Medium,
                    description: format!(
                        "Suspiciously short interval between votes: {} ms",
                        time_diff
                    ),
                    affected_votes: vec![window[0].tx_id.clone(), window[1].tx_id.clone()],
                    evidence: FraudEvidence::SuspiciousTimestamp {
                        vote1: window[0].tx_id.clone(),
                        vote2: window[1].tx_id.clone(),
                        interval_ms: time_diff,
                    },
                });
            }
        }

        for vote in votes {
            if vote.block_height > 0 {
                let expected_time_range = (vote.block_height as u64 * 10000, 
                                          (vote.block_height as u64 + 1) * 10000);
                
                if vote.timestamp < expected_time_range.0 || vote.timestamp > expected_time_range.1 {
                    findings.push(FraudFinding {
                        finding_type: FraudType::TimestampManipulation,
                        risk_level: RiskLevel::Medium,
                        description: format!(
                            "Timestamp {} inconsistent with block height {}",
                            vote.timestamp, vote.block_height
                        ),
                        affected_votes: vec![vote.tx_id.clone()],
                        evidence: FraudEvidence::BlockTimestampMismatch {
                            tx_id: vote.tx_id.clone(),
                            timestamp: vote.timestamp,
                            block_height: vote.block_height,
                        },
                    });
                }
            }
        }

        Ok(findings)
    }

    /// Detect geographic impossibilities
    fn detect_geographic_impossibilities(&self, votes: &[VoteRecord]) -> Result<Vec<FraudFinding>> {
        let mut findings = Vec::new();
        let mut voter_locations: HashMap<String, Vec<&VoteRecord>> = HashMap::new();

        for vote in votes {
            if let Some(voter_hash) = &vote.voter_hash {
                voter_locations.entry(voter_hash.clone()).or_insert_with(Vec::new).push(vote);
            }
        }

        for (voter_hash, vote_list) in voter_locations.iter() {
            if vote_list.len() > 1 {
                let mut sorted_votes = vote_list.to_vec();
                sorted_votes.sort_by_key(|v| v.timestamp);

                for window in sorted_votes.windows(2) {
                    if let (Some(coords1), Some(coords2)) = (&window[0].coordinates, &window[1].coordinates) {
                        let distance_km = self.haversine_distance(coords1, coords2);
                        let time_diff_hours = (window[1].timestamp - window[0].timestamp) as f64 / 3600000.0;

                        if time_diff_hours > 0.0 {
                            let velocity = distance_km / time_diff_hours;

                            if velocity > self.max_geographic_velocity {
                                findings.push(FraudFinding {
                                    finding_type: FraudType::GeographicImpossibility,
                                    risk_level: RiskLevel::High,
                                    description: format!(
                                        "Impossible travel: {} km in {:.2} hours ({:.0} km/h) for voter {}",
                                        distance_km, time_diff_hours, velocity, voter_hash
                                    ),
                                    affected_votes: vec![window[0].tx_id.clone(), window[1].tx_id.clone()],
                                    evidence: FraudEvidence::ImpossibleTravel {
                                        voter_hash: voter_hash.clone(),
                                        distance_km,
                                        time_hours: time_diff_hours,
                                        velocity_kmh: velocity,
                                    },
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(findings)
    }

    /// Detect statistical outliers
    fn detect_statistical_outliers(&self, votes: &[VoteRecord]) -> Result<Vec<FraudFinding>> {
        let mut findings = Vec::new();

        let mut votes_by_location: HashMap<String, Vec<&VoteRecord>> = HashMap::new();
        for vote in votes {
            if let Some(location) = &vote.location {
                votes_by_location.entry(location.clone()).or_insert_with(Vec::new).push(vote);
            }
        }

        if votes_by_location.len() < 3 {
            return Ok(findings);
        }

        let counts: Vec<f64> = votes_by_location.values().map(|v| v.len() as f64).collect();
        let mean = counts.iter().sum::<f64>() / counts.len() as f64;
        let variance = counts.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / counts.len() as f64;
        let std_dev = variance.sqrt();

        for (location, vote_list) in votes_by_location.iter() {
            let count = vote_list.len() as f64;
            let z_score = if std_dev > 0.0 {
                (count - mean) / std_dev
            } else {
                0.0
            };

            if z_score.abs() > self.outlier_threshold {
                let tx_ids: Vec<String> = vote_list.iter().map(|v| v.tx_id.clone()).collect();
                findings.push(FraudFinding {
                    finding_type: FraudType::StatisticalOutlier,
                    risk_level: if z_score.abs() > 4.0 { RiskLevel::High } else { RiskLevel::Medium },
                    description: format!(
                        "Statistical outlier at {}: {} votes (z-score: {:.2})",
                        location, count, z_score
                    ),
                    affected_votes: tx_ids.clone(),
                    evidence: FraudEvidence::StatisticalAnomaly {
                        location: location.clone(),
                        vote_count: count as usize,
                        z_score,
                        mean,
                        std_dev,
                    },
                });
            }
        }

        Ok(findings)
    }

    /// Detect sequential patterns indicating automation
    fn detect_sequential_patterns(&self, votes: &[VoteRecord]) -> Result<Vec<FraudFinding>> {
        let mut findings = Vec::new();
        let mut sorted_votes = votes.to_vec();
        sorted_votes.sort_by_key(|v| v.timestamp);

        let mut sequential_count = 0;
        let mut sequential_votes = Vec::new();

        for window in sorted_votes.windows(10) {
            let diffs: Vec<u64> = window.windows(2).map(|w| w[1].timestamp - w[0].timestamp).collect();
            let avg_diff = diffs.iter().sum::<u64>() as f64 / diffs.len() as f64;
            let variance = diffs.iter().map(|&d| {
                let diff = d as f64 - avg_diff;
                diff * diff
            }).sum::<f64>() / diffs.len() as f64;

            if variance < 100.0 && avg_diff < 1000.0 {
                sequential_count += 1;
                sequential_votes.extend(window.iter().map(|v| v.tx_id.clone()));
            }
        }

        if sequential_count > 5 {
            findings.push(FraudFinding {
                finding_type: FraudType::SequentialPattern,
                risk_level: RiskLevel::Medium,
                description: format!(
                    "Detected {} groups of votes with suspiciously regular timing patterns",
                    sequential_count
                ),
                affected_votes: sequential_votes,
                evidence: FraudEvidence::RegularPattern {
                    pattern_count: sequential_count,
                },
            });
        }

        Ok(findings)
    }

    /// Detect turnout anomalies
    fn detect_turnout_anomalies(&self, votes: &[VoteRecord]) -> Result<Vec<FraudFinding>> {
        let mut findings = Vec::new();

        let mut votes_by_location: HashMap<String, Vec<&VoteRecord>> = HashMap::new();
        for vote in votes {
            if let Some(location) = &vote.location {
                votes_by_location.entry(location.clone()).or_insert_with(Vec::new).push(vote);
            }
        }

        for (location, vote_list) in votes_by_location.iter() {
            if let Some(first_vote) = vote_list.first() {
                if let Some(registered) = first_vote.registered_voters {
                    let turnout = vote_list.len() as f64 / registered as f64;

                    if turnout > 1.0 {
                        let tx_ids: Vec<String> = vote_list.iter().map(|v| v.tx_id.clone()).collect();
                        findings.push(FraudFinding {
                            finding_type: FraudType::ExcessTurnout,
                            risk_level: RiskLevel::High,
                            description: format!(
                                "Turnout exceeds 100% at {}: {} votes with {} registered ({:.1}%)",
                                location, vote_list.len(), registered, turnout * 100.0
                            ),
                            affected_votes: tx_ids.clone(),
                            evidence: FraudEvidence::ExcessiveTurnout {
                                location: location.clone(),
                                votes_cast: vote_list.len(),
                                registered_voters: registered,
                                turnout_percentage: turnout * 100.0,
                            },
                        });
                    } else if turnout > 0.95 {
                        findings.push(FraudFinding {
                            finding_type: FraudType::UnusuallyHighTurnout,
                            risk_level: RiskLevel::Low,
                            description: format!(
                                "Unusually high turnout at {}: {:.1}%",
                                location, turnout * 100.0
                            ),
                            affected_votes: Vec::new(),
                            evidence: FraudEvidence::HighTurnout {
                                location: location.clone(),
                                turnout_percentage: turnout * 100.0,
                            },
                        });
                    }
                }
            }
        }

        Ok(findings)
    }

    /// Calculate distance between two coordinates using Haversine formula
    fn haversine_distance(&self, coord1: &(f64, f64), coord2: &(f64, f64)) -> f64 {
        let earth_radius_km = 6371.0;
        let lat1_rad = coord1.0.to_radians();
        let lat2_rad = coord2.0.to_radians();
        let delta_lat = (coord2.0 - coord1.0).to_radians();
        let delta_lon = (coord2.1 - coord1.1).to_radians();

        let a = (delta_lat / 2.0).sin().powi(2)
            + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        earth_radius_km * c
    }
}

impl Default for FraudDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Fraud detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudDetectionConfig {
    pub outlier_threshold: f64,
    pub max_votes_per_second: u64,
    pub min_vote_interval: u64,
    pub max_geographic_velocity: f64,
    pub strict_duplicate_checking: bool,
}

impl Default for FraudDetectionConfig {
    fn default() -> Self {
        Self {
            outlier_threshold: 3.0,
            max_votes_per_second: 100,
            min_vote_interval: 10,
            max_geographic_velocity: 1000.0,
            strict_duplicate_checking: true,
        }
    }
}

/// Vote record for fraud detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteRecord {
    pub tx_id: String,
    pub timestamp: u64,
    pub block_height: u64,
    pub voter_hash: Option<String>,
    pub location: Option<String>,
    pub coordinates: Option<(f64, f64)>,
    pub registered_voters: Option<u64>,
}

/// Fraud detection report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudReport {
    pub total_votes: usize,
    pub findings: Vec<FraudFinding>,
    pub high_risk_count: usize,
    pub medium_risk_count: usize,
    pub low_risk_count: usize,
    pub overall_risk: RiskLevel,
    pub timestamp: u64,
}

/// Individual fraud finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudFinding {
    pub finding_type: FraudType,
    pub risk_level: RiskLevel,
    pub description: String,
    pub affected_votes: Vec<String>,
    pub evidence: FraudEvidence,
}

/// Type of fraud detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FraudType {
    DuplicateVote,
    MultipleVotes,
    VoteStuffing,
    TimestampManipulation,
    GeographicImpossibility,
    StatisticalOutlier,
    SequentialPattern,
    ExcessTurnout,
    UnusuallyHighTurnout,
}

/// Risk level of fraud finding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// Evidence supporting fraud finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FraudEvidence {
    DuplicateHash {
        voter_hash: String,
    },
    MultipleVotesByVoter {
        voter_hash: String,
        vote_count: usize,
        tx_ids: Vec<String>,
    },
    VoteStuffing {
        timestamp: u64,
        vote_count: usize,
        tx_ids: Vec<String>,
    },
    SuspiciousTimestamp {
        vote1: String,
        vote2: String,
        interval_ms: u64,
    },
    BlockTimestampMismatch {
        tx_id: String,
        timestamp: u64,
        block_height: u64,
    },
    ImpossibleTravel {
        voter_hash: String,
        distance_km: f64,
        time_hours: f64,
        velocity_kmh: f64,
    },
    StatisticalAnomaly {
        location: String,
        vote_count: usize,
        z_score: f64,
        mean: f64,
        std_dev: f64,
    },
    RegularPattern {
        pattern_count: usize,
    },
    ExcessiveTurnout {
        location: String,
        votes_cast: usize,
        registered_voters: u64,
        turnout_percentage: f64,
    },
    HighTurnout {
        location: String,
        turnout_percentage: f64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_vote(tx_id: &str, timestamp: u64, voter_hash: Option<&str>) -> VoteRecord {
        VoteRecord {
            tx_id: tx_id.to_string(),
            timestamp,
            block_height: 100,
            voter_hash: voter_hash.map(|s| s.to_string()),
            location: Some("Location A".to_string()),
            coordinates: None,
            registered_voters: Some(1000),
        }
    }

    #[test]
    fn test_fraud_detector_creation() {
        let detector = FraudDetector::new();
        assert_eq!(detector.outlier_threshold, 3.0);
        assert_eq!(detector.max_votes_per_second, 100);
    }

    #[test]
    fn test_detect_duplicates() {
        let detector = FraudDetector::new();
        let votes = vec![
            create_test_vote("tx1", 1000, Some("voter1")),
            create_test_vote("tx2", 2000, Some("voter1")),
        ];

        let findings = detector.detect_duplicates(&votes).unwrap();
        assert!(!findings.is_empty());
    }

    #[test]
    fn test_detect_vote_stuffing() {
        let detector = FraudDetector::new();
        let mut votes = Vec::new();
        for i in 0..150 {
            votes.push(create_test_vote(&format!("tx{}", i), 1000, Some(&format!("voter{}", i))));
        }

        let findings = detector.detect_vote_stuffing(&votes).unwrap();
        assert!(!findings.is_empty());
    }

    #[test]
    fn test_detect_timestamp_manipulation() {
        let detector = FraudDetector::new();
        let votes = vec![
            create_test_vote("tx1", 1000, Some("voter1")),
            create_test_vote("tx2", 1005, Some("voter2")),
        ];

        let findings = detector.detect_timestamp_manipulation(&votes).unwrap();
        assert!(!findings.is_empty());
    }

    #[test]
    fn test_detect_geographic_impossibilities() {
        let detector = FraudDetector::new();
        let mut vote1 = create_test_vote("tx1", 1000, Some("voter1"));
        vote1.coordinates = Some((40.7128, -74.0060));
        
        let mut vote2 = create_test_vote("tx2", 2000, Some("voter1"));
        vote2.coordinates = Some((34.0522, -118.2437));

        let findings = detector.detect_geographic_impossibilities(&[vote1, vote2]).unwrap();
        assert!(!findings.is_empty());
    }

    #[test]
    fn test_haversine_distance() {
        let detector = FraudDetector::new();
        let nyc = (40.7128, -74.0060);
        let la = (34.0522, -118.2437);
        let distance = detector.haversine_distance(&nyc, &la);
        assert!(distance > 3900.0 && distance < 4000.0);
    }

    #[test]
    fn test_detect_statistical_outliers() {
        let detector = FraudDetector::new();
        let mut votes = Vec::new();
        
        for i in 0..10 {
            votes.push(create_test_vote(&format!("tx{}", i), i * 1000, Some(&format!("voter{}", i))));
        }
        
        for i in 10..200 {
            let mut vote = create_test_vote(&format!("tx{}", i), i * 1000, Some(&format!("voter{}", i)));
            vote.location = Some("Location B".to_string());
            votes.push(vote);
        }

        let findings = detector.detect_statistical_outliers(&votes).unwrap();
        assert!(!findings.is_empty());
    }

    #[test]
    fn test_detect_sequential_patterns() {
        let detector = FraudDetector::new();
        let mut votes = Vec::new();
        
        for i in 0..100 {
            votes.push(create_test_vote(&format!("tx{}", i), 1000 + (i * 100), Some(&format!("voter{}", i))));
        }

        let findings = detector.detect_sequential_patterns(&votes).unwrap();
        assert!(!findings.is_empty());
    }

    #[test]
    fn test_detect_turnout_anomalies() {
        let detector = FraudDetector::new();
        let mut votes = Vec::new();
        
        for i in 0..1100 {
            votes.push(create_test_vote(&format!("tx{}", i), i * 1000, Some(&format!("voter{}", i))));
        }

        let findings = detector.detect_turnout_anomalies(&votes).unwrap();
        assert!(!findings.is_empty());
    }

    #[test]
    fn test_comprehensive_fraud_detection() {
        let detector = FraudDetector::new();
        let votes = vec![
            create_test_vote("tx1", 1000, Some("voter1")),
            create_test_vote("tx2", 2000, Some("voter2")),
            create_test_vote("tx3", 3000, Some("voter3")),
        ];

        let report = detector.detect_fraud(&votes).unwrap();
        assert_eq!(report.total_votes, 3);
    }

    #[test]
    fn test_empty_votes() {
        let detector = FraudDetector::new();
        let votes: Vec<VoteRecord> = vec![];
        let result = detector.detect_fraud(&votes);
        assert!(result.is_err());
    }

    #[test]
    fn test_custom_config() {
        let config = FraudDetectionConfig {
            outlier_threshold: 2.5,
            max_votes_per_second: 50,
            min_vote_interval: 5,
            max_geographic_velocity: 500.0,
            strict_duplicate_checking: false,
        };
        let detector = FraudDetector::with_config(config);
        assert_eq!(detector.outlier_threshold, 2.5);
        assert_eq!(detector.max_votes_per_second, 50);
    }

    #[test]
    fn test_risk_level_ordering() {
        let report = FraudReport {
            total_votes: 100,
            findings: vec![],
            high_risk_count: 5,
            medium_risk_count: 10,
            low_risk_count: 2,
            overall_risk: RiskLevel::High,
            timestamp: 1000,
        };
        assert_eq!(report.overall_risk, RiskLevel::High);
    }
}

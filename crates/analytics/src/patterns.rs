//! Pattern detection and analysis for voting data
//!
//! This module identifies patterns in voting behavior that may indicate:
//! - Temporal patterns (regular intervals, bursts, cyclic behavior)
//! - Sequential patterns (repetitive sequences, automation)
//! - Clustering patterns (geographic, demographic, temporal)
//! - Behavioral patterns (anomalous voting behavior)

use common::{Result, VotingError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Pattern detection engine
#[derive(Debug, Clone)]
pub struct PatternDetector {
    /// Minimum pattern size to detect
    min_pattern_size: usize,

    /// Similarity threshold for pattern matching (0.0 to 1.0)
    similarity_threshold: f64,

    /// Minimum occurrences to consider a pattern
    min_occurrences: usize,
}

impl PatternDetector {
    /// Create new pattern detector with default settings
    pub fn new() -> Self {
        Self {
            min_pattern_size: 3,
            similarity_threshold: 0.8,
            min_occurrences: 3,
        }
    }

    /// Create detector with custom settings
    pub fn with_settings(
        min_pattern_size: usize,
        similarity_threshold: f64,
        min_occurrences: usize,
    ) -> Self {
        Self {
            min_pattern_size,
            similarity_threshold,
            min_occurrences,
        }
    }

    /// Detect all patterns in voting data
    pub fn detect_patterns(&self, votes: &[VotePattern]) -> Result<PatternAnalysisResult> {
        if votes.is_empty() {
            return Err(VotingError::InsufficientData(
                "No votes provided".to_string(),
            ));
        }

        let temporal_patterns = self.detect_temporal_patterns(votes)?;
        let sequential_patterns = self.detect_sequential_patterns(votes)?;
        let clustering_patterns = self.detect_clustering_patterns(votes)?;
        let behavioral_patterns = self.detect_behavioral_patterns(votes)?;

        let total_patterns = temporal_patterns.len()
            + sequential_patterns.len()
            + clustering_patterns.len()
            + behavioral_patterns.len();

        let risk_score = self.calculate_pattern_risk_score(
            &temporal_patterns,
            &sequential_patterns,
            &clustering_patterns,
            &behavioral_patterns,
        );

        Ok(PatternAnalysisResult {
            temporal_patterns,
            sequential_patterns,
            clustering_patterns,
            behavioral_patterns,
            total_patterns,
            risk_score,
            timestamp: common::utils::current_timestamp(),
        })
    }

    /// Detect temporal patterns in vote timing
    fn detect_temporal_patterns(&self, votes: &[VotePattern]) -> Result<Vec<DetectedPattern>> {
        let mut patterns = Vec::new();
        let mut timestamps: Vec<u64> = votes.iter().map(|v| v.timestamp).collect();
        timestamps.sort();

        let intervals = self.extract_time_intervals(&timestamps);

        let periodic_pattern = self.detect_periodic_intervals(&intervals);
        if let Some(pattern) = periodic_pattern {
            patterns.push(pattern);
        }

        let burst_patterns = self.detect_burst_patterns(&timestamps);
        patterns.extend(burst_patterns);

        let cyclic_patterns = self.detect_cyclic_patterns(&timestamps);
        patterns.extend(cyclic_patterns);

        Ok(patterns)
    }

    /// Detect sequential patterns in vote ordering
    fn detect_sequential_patterns(&self, votes: &[VotePattern]) -> Result<Vec<DetectedPattern>> {
        let mut patterns = Vec::new();

        let repetitive_sequences = self.detect_repetitive_sequences(votes);
        patterns.extend(repetitive_sequences);

        let automaton_patterns = self.detect_automaton_behavior(votes);
        patterns.extend(automaton_patterns);

        let identical_intervals = self.detect_identical_intervals(votes);
        patterns.extend(identical_intervals);

        Ok(patterns)
    }

    /// Detect clustering patterns
    fn detect_clustering_patterns(&self, votes: &[VotePattern]) -> Result<Vec<DetectedPattern>> {
        let mut patterns = Vec::new();

        let geographic_clusters = self.detect_geographic_clusters(votes);
        patterns.extend(geographic_clusters);

        let temporal_clusters = self.detect_temporal_clusters(votes);
        patterns.extend(temporal_clusters);

        let voter_clusters = self.detect_voter_clusters(votes);
        patterns.extend(voter_clusters);

        Ok(patterns)
    }

    /// Detect behavioral patterns
    fn detect_behavioral_patterns(&self, votes: &[VotePattern]) -> Result<Vec<DetectedPattern>> {
        let mut patterns = Vec::new();

        let rapid_voting = self.detect_rapid_voting(votes);
        patterns.extend(rapid_voting);

        let coordinated_patterns = self.detect_coordinated_voting(votes);
        patterns.extend(coordinated_patterns);

        Ok(patterns)
    }

    /// Extract time intervals between consecutive votes
    fn extract_time_intervals(&self, timestamps: &[u64]) -> Vec<u64> {
        timestamps
            .windows(2)
            .map(|w| w[1] - w[0])
            .collect()
    }

    /// Detect periodic intervals (regular timing)
    fn detect_periodic_intervals(&self, intervals: &[u64]) -> Option<DetectedPattern> {
        if intervals.len() < self.min_pattern_size {
            return None;
        }

        let mean_interval = intervals.iter().sum::<u64>() as f64 / intervals.len() as f64;
        let variance = intervals
            .iter()
            .map(|&i| {
                let diff = i as f64 - mean_interval;
                diff * diff
            })
            .sum::<f64>()
            / intervals.len() as f64;

        let std_dev = variance.sqrt();
        let coefficient_of_variation = std_dev / mean_interval;

        if coefficient_of_variation < 0.2 {
            let confidence = 1.0 - coefficient_of_variation;
            return Some(DetectedPattern {
                pattern_type: PatternType::Periodic,
                description: format!(
                    "Regular interval pattern detected: ~{:.0}ms between votes (CV: {:.3})",
                    mean_interval, coefficient_of_variation
                ),
                confidence,
                severity: if coefficient_of_variation < 0.1 {
                    PatternSeverity::High
                } else {
                    PatternSeverity::Medium
                },
                occurrences: intervals.len(),
                evidence: PatternEvidence::PeriodicTiming {
                    mean_interval,
                    std_dev,
                    coefficient_of_variation,
                },
            });
        }

        None
    }

    /// Detect burst patterns (sudden spikes in activity)
    fn detect_burst_patterns(&self, timestamps: &[u64]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();
        let window_size = 60000; // 1 minute windows

        if timestamps.is_empty() {
            return patterns;
        }

        let min_time = timestamps[0];
        let max_time = timestamps[timestamps.len() - 1];
        let time_range = max_time - min_time;

        if time_range == 0 {
            return patterns;
        }

        let num_windows = (time_range / window_size).max(1) as usize;
        let mut window_counts = vec![0usize; num_windows];

        for &timestamp in timestamps {
            let window_idx = ((timestamp - min_time) / window_size).min(num_windows as u64 - 1) as usize;
            window_counts[window_idx] += 1;
        }

        let mean_count = window_counts.iter().sum::<usize>() as f64 / num_windows as f64;
        let variance = window_counts
            .iter()
            .map(|&c| {
                let diff = c as f64 - mean_count;
                diff * diff
            })
            .sum::<f64>()
            / num_windows as f64;
        let std_dev = variance.sqrt();

        for (idx, &count) in window_counts.iter().enumerate() {
            if count as f64 > mean_count + 3.0 * std_dev {
                let z_score = (count as f64 - mean_count) / std_dev;
                patterns.push(DetectedPattern {
                    pattern_type: PatternType::Burst,
                    description: format!(
                        "Vote burst detected: {} votes in 1-minute window (z-score: {:.2})",
                        count, z_score
                    ),
                    confidence: (z_score / 10.0).min(1.0),
                    severity: if z_score > 5.0 {
                        PatternSeverity::High
                    } else {
                        PatternSeverity::Medium
                    },
                    occurrences: count,
                    evidence: PatternEvidence::BurstActivity {
                        window_index: idx,
                        vote_count: count,
                        z_score,
                    },
                });
            }
        }

        patterns
    }

    /// Detect cyclic patterns (repeating over time)
    fn detect_cyclic_patterns(&self, timestamps: &[u64]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();

        if timestamps.len() < 10 {
            return patterns;
        }

        let intervals = self.extract_time_intervals(timestamps);
        let cycle_lengths = [60000, 300000, 3600000]; // 1min, 5min, 1hour

        for &cycle_length in &cycle_lengths {
            let matches = intervals
                .iter()
                .filter(|&&i| (i as i64 - cycle_length as i64).abs() < (cycle_length as i64 / 10))
                .count();

            if matches >= self.min_occurrences {
                patterns.push(DetectedPattern {
                    pattern_type: PatternType::Cyclic,
                    description: format!(
                        "Cyclic pattern detected: {} votes with ~{}ms intervals",
                        matches, cycle_length
                    ),
                    confidence: matches as f64 / intervals.len() as f64,
                    severity: PatternSeverity::Medium,
                    occurrences: matches,
                    evidence: PatternEvidence::CyclicTiming {
                        cycle_length,
                        matches,
                    },
                });
            }
        }

        patterns
    }

    /// Detect repetitive sequences
    fn detect_repetitive_sequences(&self, votes: &[VotePattern]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();

        if votes.len() < self.min_pattern_size * 2 {
            return patterns;
        }

        for pattern_length in self.min_pattern_size..=(votes.len() / 2) {
            let mut sequence_counts: HashMap<Vec<String>, usize> = HashMap::new();

            for i in 0..=(votes.len() - pattern_length) {
                let sequence: Vec<String> = votes[i..i + pattern_length]
                    .iter()
                    .map(|v| format!("{}-{}", v.location.as_deref().unwrap_or(""), v.timestamp % 1000))
                    .collect();

                *sequence_counts.entry(sequence).or_insert(0) += 1;
            }

            for (sequence, count) in sequence_counts {
                if count >= self.min_occurrences {
                    patterns.push(DetectedPattern {
                        pattern_type: PatternType::Repetitive,
                        description: format!(
                            "Repetitive sequence detected: length {} repeated {} times",
                            pattern_length, count
                        ),
                        confidence: (count as f64 / votes.len() as f64).min(1.0),
                        severity: if count > 10 {
                            PatternSeverity::High
                        } else {
                            PatternSeverity::Medium
                        },
                        occurrences: count,
                        evidence: PatternEvidence::RepetitiveSequence {
                            sequence_length: pattern_length,
                            repetitions: count,
                        },
                    });
                    break;
                }
            }
        }

        patterns
    }

    /// Detect automaton-like behavior (machine voting)
    fn detect_automaton_behavior(&self, votes: &[VotePattern]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();

        if votes.len() < 10 {
            return patterns;
        }

        let intervals: Vec<u64> = votes
            .windows(2)
            .map(|w| w[1].timestamp - w[0].timestamp)
            .collect();

        let identical_intervals = intervals
            .windows(2)
            .filter(|w| w[0] == w[1])
            .count();

        let identical_ratio = identical_intervals as f64 / intervals.len() as f64;

        if identical_ratio > 0.3 {
            patterns.push(DetectedPattern {
                pattern_type: PatternType::Automaton,
                description: format!(
                    "Automaton behavior detected: {:.1}% of intervals are identical",
                    identical_ratio * 100.0
                ),
                confidence: identical_ratio,
                severity: PatternSeverity::High,
                occurrences: identical_intervals,
                evidence: PatternEvidence::AutomatonBehavior {
                    identical_intervals,
                    total_intervals: intervals.len(),
                    identical_ratio,
                },
            });
        }

        patterns
    }

    /// Detect identical time intervals
    fn detect_identical_intervals(&self, votes: &[VotePattern]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();

        if votes.len() < 5 {
            return patterns;
        }

        let intervals: Vec<u64> = votes
            .windows(2)
            .map(|w| w[1].timestamp - w[0].timestamp)
            .collect();

        let mut interval_counts: HashMap<u64, usize> = HashMap::new();
        for &interval in &intervals {
            *interval_counts.entry(interval).or_insert(0) += 1;
        }

        for (interval, count) in interval_counts {
            if count >= self.min_occurrences && interval > 0 {
                patterns.push(DetectedPattern {
                    pattern_type: PatternType::IdenticalIntervals,
                    description: format!(
                        "Identical interval pattern: {}ms repeated {} times",
                        interval, count
                    ),
                    confidence: count as f64 / intervals.len() as f64,
                    severity: if count > 5 {
                        PatternSeverity::High
                    } else {
                        PatternSeverity::Medium
                    },
                    occurrences: count,
                    evidence: PatternEvidence::IdenticalIntervals {
                        interval,
                        occurrences: count,
                    },
                });
            }
        }

        patterns
    }

    /// Detect geographic clusters
    fn detect_geographic_clusters(&self, votes: &[VotePattern]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();
        let mut location_counts: HashMap<String, usize> = HashMap::new();

        for vote in votes {
            if let Some(location) = &vote.location {
                *location_counts.entry(location.clone()).or_insert(0) += 1;
            }
        }

        let total_votes = votes.len();
        for (location, count) in location_counts {
            let ratio = count as f64 / total_votes as f64;
            if ratio > 0.5 && count >= 10 {
                patterns.push(DetectedPattern {
                    pattern_type: PatternType::GeographicCluster,
                    description: format!(
                        "Geographic clustering: {:.1}% of votes from {}",
                        ratio * 100.0,
                        location
                    ),
                    confidence: ratio,
                    severity: if ratio > 0.8 {
                        PatternSeverity::High
                    } else {
                        PatternSeverity::Medium
                    },
                    occurrences: count,
                    evidence: PatternEvidence::GeographicCluster {
                        location,
                        vote_count: count,
                        percentage: ratio * 100.0,
                    },
                });
            }
        }

        patterns
    }

    /// Detect temporal clusters
    fn detect_temporal_clusters(&self, votes: &[VotePattern]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();

        if votes.len() < 10 {
            return patterns;
        }

        let mut sorted_votes = votes.to_vec();
        sorted_votes.sort_by_key(|v| v.timestamp);

        let cluster_threshold = 5000; // 5 seconds
        let mut current_cluster_size = 1;
        let mut current_cluster_start = sorted_votes[0].timestamp;

        for i in 1..sorted_votes.len() {
            if sorted_votes[i].timestamp - sorted_votes[i - 1].timestamp <= cluster_threshold {
                current_cluster_size += 1;
            } else {
                if current_cluster_size >= 5 {
                    patterns.push(DetectedPattern {
                        pattern_type: PatternType::TemporalCluster,
                        description: format!(
                            "Temporal cluster: {} votes within 5 seconds",
                            current_cluster_size
                        ),
                        confidence: 0.8,
                        severity: if current_cluster_size > 20 {
                            PatternSeverity::High
                        } else {
                            PatternSeverity::Medium
                        },
                        occurrences: current_cluster_size,
                        evidence: PatternEvidence::TemporalCluster {
                            start_timestamp: current_cluster_start,
                            vote_count: current_cluster_size,
                        },
                    });
                }
                current_cluster_size = 1;
                current_cluster_start = sorted_votes[i].timestamp;
            }
        }

        patterns
    }

    /// Detect voter clusters (coordinated voting)
    fn detect_voter_clusters(&self, votes: &[VotePattern]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();

        if votes.len() < 10 {
            return patterns;
        }

        let mut voter_times: HashMap<String, Vec<u64>> = HashMap::new();
        for vote in votes {
            if let Some(voter) = &vote.voter_hash {
                voter_times
                    .entry(voter.clone())
                    .or_insert_with(Vec::new)
                    .push(vote.timestamp);
            }
        }

        let coordination_threshold = 1000; // 1 second
        let mut coordinated_count = 0;

        for timestamps in voter_times.values() {
            if timestamps.len() > 1 {
                let mut sorted_times = timestamps.clone();
                sorted_times.sort();
                for window in sorted_times.windows(2) {
                    if window[1] - window[0] <= coordination_threshold {
                        coordinated_count += 1;
                    }
                }
            }
        }

        if coordinated_count >= 3 {
            patterns.push(DetectedPattern {
                pattern_type: PatternType::VoterCluster,
                description: format!(
                    "Coordinated voting pattern: {} voter pairs within 1 second",
                    coordinated_count
                ),
                confidence: 0.7,
                severity: PatternSeverity::Medium,
                occurrences: coordinated_count,
                evidence: PatternEvidence::VoterCluster {
                    coordinated_pairs: coordinated_count,
                },
            });
        }

        patterns
    }

    /// Detect rapid voting behavior
    fn detect_rapid_voting(&self, votes: &[VotePattern]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();

        if votes.len() < 5 {
            return patterns;
        }

        let rapid_threshold = 100; // 100ms
        let mut rapid_sequences = 0;

        for window in votes.windows(5) {
            let time_span = window[4].timestamp - window[0].timestamp;
            if time_span < rapid_threshold * 4 {
                rapid_sequences += 1;
            }
        }

        if rapid_sequences >= 3 {
            patterns.push(DetectedPattern {
                pattern_type: PatternType::RapidVoting,
                description: format!(
                    "Rapid voting detected: {} sequences of 5 votes in <400ms",
                    rapid_sequences
                ),
                confidence: 0.9,
                severity: PatternSeverity::High,
                occurrences: rapid_sequences,
                evidence: PatternEvidence::RapidVoting {
                    rapid_sequences,
                    threshold_ms: rapid_threshold,
                },
            });
        }

        patterns
    }

    /// Detect coordinated voting patterns
    fn detect_coordinated_voting(&self, votes: &[VotePattern]) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();

        if votes.len() < 10 {
            return patterns;
        }

        let coordination_window = 5000; // 5 seconds
        let mut coordinated_groups = 0;

        let mut sorted_votes = votes.to_vec();
        sorted_votes.sort_by_key(|v| v.timestamp);

        for i in 0..(sorted_votes.len() - 4) {
            let window_votes = &sorted_votes[i..i + 5];
            let time_span = window_votes[4].timestamp - window_votes[0].timestamp;

            if time_span <= coordination_window {
                let unique_locations: std::collections::HashSet<_> = window_votes
                    .iter()
                    .filter_map(|v| v.location.as_ref())
                    .collect();

                if unique_locations.len() >= 3 {
                    coordinated_groups += 1;
                }
            }
        }

        if coordinated_groups >= 3 {
            patterns.push(DetectedPattern {
                pattern_type: PatternType::Coordinated,
                description: format!(
                    "Coordinated voting pattern: {} groups with multi-location synchronization",
                    coordinated_groups
                ),
                confidence: 0.8,
                severity: PatternSeverity::High,
                occurrences: coordinated_groups,
                evidence: PatternEvidence::CoordinatedVoting {
                    coordinated_groups,
                    window_ms: coordination_window,
                },
            });
        }

        patterns
    }

    /// Calculate overall risk score based on detected patterns
    fn calculate_pattern_risk_score(
        &self,
        temporal: &[DetectedPattern],
        sequential: &[DetectedPattern],
        clustering: &[DetectedPattern],
        behavioral: &[DetectedPattern],
    ) -> f64 {
        let mut score = 0.0;

        for pattern in temporal {
            score += match pattern.severity {
                PatternSeverity::High => 3.0,
                PatternSeverity::Medium => 2.0,
                PatternSeverity::Low => 1.0,
            } * pattern.confidence;
        }

        for pattern in sequential {
            score += match pattern.severity {
                PatternSeverity::High => 4.0,
                PatternSeverity::Medium => 2.5,
                PatternSeverity::Low => 1.0,
            } * pattern.confidence;
        }

        for pattern in clustering {
            score += match pattern.severity {
                PatternSeverity::High => 2.0,
                PatternSeverity::Medium => 1.5,
                PatternSeverity::Low => 0.5,
            } * pattern.confidence;
        }

        for pattern in behavioral {
            score += match pattern.severity {
                PatternSeverity::High => 5.0,
                PatternSeverity::Medium => 3.0,
                PatternSeverity::Low => 1.0,
            } * pattern.confidence;
        }

        (score / 10.0).min(10.0)
    }
}

impl Default for PatternDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Vote pattern data for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotePattern {
    pub tx_id: String,
    pub timestamp: u64,
    pub location: Option<String>,
    pub voter_hash: Option<String>,
}

/// Pattern analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternAnalysisResult {
    pub temporal_patterns: Vec<DetectedPattern>,
    pub sequential_patterns: Vec<DetectedPattern>,
    pub clustering_patterns: Vec<DetectedPattern>,
    pub behavioral_patterns: Vec<DetectedPattern>,
    pub total_patterns: usize,
    pub risk_score: f64,
    pub timestamp: u64,
}

/// Detected pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedPattern {
    pub pattern_type: PatternType,
    pub description: String,
    pub confidence: f64,
    pub severity: PatternSeverity,
    pub occurrences: usize,
    pub evidence: PatternEvidence,
}

/// Type of pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternType {
    Periodic,
    Burst,
    Cyclic,
    Repetitive,
    Automaton,
    IdenticalIntervals,
    GeographicCluster,
    TemporalCluster,
    VoterCluster,
    RapidVoting,
    Coordinated,
}

/// Pattern severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternSeverity {
    Low,
    Medium,
    High,
}

/// Evidence supporting detected pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternEvidence {
    PeriodicTiming {
        mean_interval: f64,
        std_dev: f64,
        coefficient_of_variation: f64,
    },
    BurstActivity {
        window_index: usize,
        vote_count: usize,
        z_score: f64,
    },
    CyclicTiming {
        cycle_length: u64,
        matches: usize,
    },
    RepetitiveSequence {
        sequence_length: usize,
        repetitions: usize,
    },
    AutomatonBehavior {
        identical_intervals: usize,
        total_intervals: usize,
        identical_ratio: f64,
    },
    IdenticalIntervals {
        interval: u64,
        occurrences: usize,
    },
    GeographicCluster {
        location: String,
        vote_count: usize,
        percentage: f64,
    },
    TemporalCluster {
        start_timestamp: u64,
        vote_count: usize,
    },
    VoterCluster {
        coordinated_pairs: usize,
    },
    RapidVoting {
        rapid_sequences: usize,
        threshold_ms: u64,
    },
    CoordinatedVoting {
        coordinated_groups: usize,
        window_ms: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_vote_pattern(tx_id: &str, timestamp: u64, location: Option<&str>) -> VotePattern {
        VotePattern {
            tx_id: tx_id.to_string(),
            timestamp,
            location: location.map(|s| s.to_string()),
            voter_hash: Some(format!("voter_{}", tx_id)),
        }
    }

    #[test]
    fn test_pattern_detector_creation() {
        let detector = PatternDetector::new();
        assert_eq!(detector.min_pattern_size, 3);
        assert_eq!(detector.similarity_threshold, 0.8);
        assert_eq!(detector.min_occurrences, 3);
    }

    #[test]
    fn test_detect_periodic_pattern() {
        let detector = PatternDetector::new();
        let votes: Vec<VotePattern> = (0..10)
            .map(|i| create_vote_pattern(&format!("tx{}", i), i * 1000, Some("Location A")))
            .collect();

        let result = detector.detect_patterns(&votes).unwrap();
        assert!(!result.temporal_patterns.is_empty());
    }

    #[test]
    fn test_detect_burst_pattern() {
        let detector = PatternDetector::new();
        let mut votes = Vec::new();

        for i in 0..100 {
            votes.push(create_vote_pattern(&format!("tx{}", i), (i * 1000) as u64, Some("Location A")));
        }

        for i in 100..200 {
            votes.push(create_vote_pattern(&format!("tx{}", i), 150000, Some("Location A")));
        }

        let result = detector.detect_patterns(&votes).unwrap();
        assert!(result.total_patterns > 0);
    }

    #[test]
    fn test_detect_automaton_behavior() {
        let detector = PatternDetector::new();
        let votes: Vec<VotePattern> = (0..20)
            .map(|i| create_vote_pattern(&format!("tx{}", i), i * 100, Some("Location A")))
            .collect();

        let result = detector.detect_patterns(&votes).unwrap();
        assert!(!result.sequential_patterns.is_empty());
    }

    #[test]
    fn test_detect_geographic_cluster() {
        let detector = PatternDetector::new();
        let mut votes = Vec::new();

        for i in 0..80 {
            votes.push(create_vote_pattern(&format!("tx{}", i), i * 1000, Some("Location A")));
        }
        for i in 80..100 {
            votes.push(create_vote_pattern(&format!("tx{}", i), i * 1000, Some("Location B")));
        }

        let result = detector.detect_patterns(&votes).unwrap();
        assert!(!result.clustering_patterns.is_empty());
    }

    #[test]
    fn test_extract_time_intervals() {
        let detector = PatternDetector::new();
        let timestamps = vec![1000, 2000, 3000, 4000];
        let intervals = detector.extract_time_intervals(&timestamps);
        assert_eq!(intervals, vec![1000, 1000, 1000]);
    }

    #[test]
    fn test_empty_votes() {
        let detector = PatternDetector::new();
        let votes: Vec<VotePattern> = vec![];
        let result = detector.detect_patterns(&votes);
        assert!(result.is_err());
    }

    #[test]
    fn test_custom_settings() {
        let detector = PatternDetector::with_settings(5, 0.9, 5);
        assert_eq!(detector.min_pattern_size, 5);
        assert_eq!(detector.similarity_threshold, 0.9);
        assert_eq!(detector.min_occurrences, 5);
    }

    #[test]
    fn test_risk_score_calculation() {
        let detector = PatternDetector::new();
        
        let high_pattern = DetectedPattern {
            pattern_type: PatternType::Automaton,
            description: "Test".to_string(),
            confidence: 0.9,
            severity: PatternSeverity::High,
            occurrences: 10,
            evidence: PatternEvidence::AutomatonBehavior {
                identical_intervals: 10,
                total_intervals: 15,
                identical_ratio: 0.67,
            },
        };

        let score = detector.calculate_pattern_risk_score(&[], &[high_pattern], &[], &[]);
        assert!(score > 0.0);
    }

    #[test]
    fn test_temporal_cluster_detection() {
        let detector = PatternDetector::new();
        let mut votes = Vec::new();

        for i in 0..10 {
            votes.push(create_vote_pattern(&format!("tx{}", i), 1000 + i * 500, Some("Location A")));
        }

        let result = detector.detect_patterns(&votes).unwrap();
        assert!(result.total_patterns > 0);
    }

    #[test]
    fn test_rapid_voting_detection() {
        let detector = PatternDetector::new();
        let votes: Vec<VotePattern> = (0..10)
            .map(|i| create_vote_pattern(&format!("tx{}", i), i * 50, Some("Location A")))
            .collect();

        let result = detector.detect_patterns(&votes).unwrap();
        assert!(!result.behavioral_patterns.is_empty());
    }
}

//! Entropy analysis for detecting randomness and patterns in voting data
//!
//! This module provides entropy-based statistical analysis to detect:
//! - Lack of randomness in vote distribution
//! - Patterns that suggest manipulation or fraud
//! - Predictability in voting sequences
//! - Clustering of votes in time or space

use common::{Result, VotingError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Shannon entropy calculator for discrete distributions
#[derive(Debug, Clone)]
pub struct EntropyAnalyzer {
    /// Minimum entropy threshold for randomness
    min_entropy_threshold: f64,
}

impl EntropyAnalyzer {
    /// Create a new entropy analyzer with default threshold
    pub fn new() -> Self {
        Self {
            min_entropy_threshold: 0.7,
        }
    }

    /// Create analyzer with custom entropy threshold
    pub fn with_threshold(threshold: f64) -> Self {
        Self {
            min_entropy_threshold: threshold,
        }
    }

    /// Calculate Shannon entropy of a discrete distribution
    ///
    /// H(X) = -Σ p(x) * log2(p(x))
    /// Returns value between 0 (completely predictable) and log2(n) (maximum randomness)
    pub fn calculate_shannon_entropy(&self, values: &[u64]) -> Result<f64> {
        if values.is_empty() {
            return Err(VotingError::InsufficientData(
                "No values provided for entropy calculation".to_string(),
            ));
        }

        let frequency_map = self.build_frequency_map(values);
        let total = values.len() as f64;
        let mut entropy = 0.0;

        for count in frequency_map.values() {
            let probability = *count as f64 / total;
            if probability > 0.0 {
                entropy -= probability * probability.log2();
            }
        }

        Ok(entropy)
    }

    /// Calculate normalized entropy (0 to 1)
    ///
    /// Divides Shannon entropy by maximum possible entropy log2(n)
    pub fn calculate_normalized_entropy(&self, values: &[u64]) -> Result<f64> {
        if values.is_empty() {
            return Err(VotingError::InsufficientData(
                "No values provided".to_string(),
            ));
        }

        let shannon_entropy = self.calculate_shannon_entropy(values)?;
        let frequency_map = self.build_frequency_map(values);
        let unique_values = frequency_map.len() as f64;

        if unique_values <= 1.0 {
            return Ok(0.0);
        }

        let max_entropy = unique_values.log2();
        Ok(shannon_entropy / max_entropy)
    }

    /// Analyze temporal entropy of vote timestamps
    pub fn analyze_temporal_entropy(&self, timestamps: &[u64], window_size: u64) -> Result<TemporalEntropyResult> {
        if timestamps.is_empty() {
            return Err(VotingError::InsufficientData(
                "No timestamps provided".to_string(),
            ));
        }

        let mut sorted_timestamps = timestamps.to_vec();
        sorted_timestamps.sort();

        let min_time = sorted_timestamps[0];
        let max_time = sorted_timestamps[sorted_timestamps.len() - 1];
        let time_range = max_time - min_time;

        if time_range == 0 {
            return Ok(TemporalEntropyResult {
                overall_entropy: 0.0,
                normalized_entropy: 0.0,
                window_entropies: vec![],
                low_entropy_windows: vec![],
                is_suspicious: true,
                interpretation: "All votes at identical timestamp".to_string(),
            });
        }

        let num_windows = (time_range / window_size).max(1);
        let mut window_counts = vec![0u64; num_windows as usize];

        for &timestamp in &sorted_timestamps {
            let window_idx = ((timestamp - min_time) / window_size).min(num_windows - 1) as usize;
            window_counts[window_idx] += 1;
        }

        let overall_entropy = self.calculate_shannon_entropy(&window_counts)?;
        let normalized_entropy = self.calculate_normalized_entropy(&window_counts)?;

        let mut window_entropies = Vec::new();
        let mut low_entropy_windows = Vec::new();

        for (idx, &count) in window_counts.iter().enumerate() {
            if count > 0 {
                let start_time = min_time + (idx as u64 * window_size);
                let end_time = start_time + window_size;

                let votes_in_window: Vec<u64> = sorted_timestamps
                    .iter()
                    .filter(|&&t| t >= start_time && t < end_time)
                    .copied()
                    .collect();

                if votes_in_window.len() >= 2 {
                    let window_entropy = self.calculate_normalized_entropy(&votes_in_window).unwrap_or(0.0);
                    
                    window_entropies.push(WindowEntropy {
                        window_index: idx,
                        start_time,
                        end_time,
                        vote_count: count,
                        entropy: window_entropy,
                    });

                    if window_entropy < self.min_entropy_threshold {
                        low_entropy_windows.push(idx);
                    }
                }
            }
        }

        let is_suspicious = normalized_entropy < self.min_entropy_threshold || !low_entropy_windows.is_empty();

        let interpretation = if is_suspicious {
            format!(
                "Low temporal entropy detected ({:.3}). Votes may be clustered or following predictable patterns.",
                normalized_entropy
            )
        } else {
            format!(
                "Normal temporal entropy ({:.3}). Vote timing appears random.",
                normalized_entropy
            )
        };

        Ok(TemporalEntropyResult {
            overall_entropy,
            normalized_entropy,
            window_entropies,
            low_entropy_windows,
            is_suspicious,
            interpretation,
        })
    }

    /// Analyze geographic entropy of vote locations
    pub fn analyze_geographic_entropy(&self, locations: &[String]) -> Result<GeographicEntropyResult> {
        if locations.is_empty() {
            return Err(VotingError::InsufficientData(
                "No locations provided".to_string(),
            ));
        }

        let mut location_counts: HashMap<String, u64> = HashMap::new();
        for location in locations {
            *location_counts.entry(location.clone()).or_insert(0) += 1;
        }

        let counts: Vec<u64> = location_counts.values().copied().collect();
        let entropy = self.calculate_shannon_entropy(&counts)?;
        let normalized_entropy = self.calculate_normalized_entropy(&counts)?;

        let total_votes = locations.len() as f64;
        let unique_locations = location_counts.len();
        let expected_entropy = (unique_locations as f64).log2();

        let mut location_analysis = Vec::new();
        for (location, count) in location_counts.iter() {
            let percentage = (*count as f64 / total_votes) * 100.0;
            location_analysis.push(LocationEntropy {
                location: location.clone(),
                vote_count: *count,
                percentage,
            });
        }

        location_analysis.sort_by(|a, b| b.vote_count.cmp(&a.vote_count));

        let concentration_ratio = if unique_locations > 0 {
            location_analysis[0].percentage / 100.0
        } else {
            0.0
        };

        let is_concentrated = concentration_ratio > 0.5 || normalized_entropy < 0.5;

        let interpretation = if is_concentrated {
            format!(
                "Geographic concentration detected. Top location has {:.1}% of votes (entropy: {:.3}).",
                location_analysis[0].percentage, normalized_entropy
            )
        } else {
            format!(
                "Votes well distributed across {} locations (entropy: {:.3}).",
                unique_locations, normalized_entropy
            )
        };

        Ok(GeographicEntropyResult {
            total_locations: unique_locations,
            entropy,
            normalized_entropy,
            expected_entropy,
            location_analysis,
            is_concentrated,
            concentration_ratio,
            interpretation,
        })
    }

    /// Analyze sequence entropy to detect patterns
    pub fn analyze_sequence_entropy(&self, sequence: &[u64]) -> Result<SequenceEntropyResult> {
        if sequence.len() < 2 {
            return Err(VotingError::InsufficientData(
                "Sequence too short for analysis".to_string(),
            ));
        }

        let first_order_entropy = self.calculate_first_order_entropy(sequence)?;
        let second_order_entropy = self.calculate_second_order_entropy(sequence)?;
        let runs_entropy = self.calculate_runs_entropy(sequence)?;

        let avg_entropy = (first_order_entropy + second_order_entropy + runs_entropy) / 3.0;
        let is_predictable = avg_entropy < self.min_entropy_threshold;

        let interpretation = if is_predictable {
            format!(
                "Low sequence entropy ({:.3}). Data may follow predictable patterns.",
                avg_entropy
            )
        } else {
            format!(
                "Normal sequence entropy ({:.3}). Data appears random.",
                avg_entropy
            )
        };

        Ok(SequenceEntropyResult {
            first_order_entropy,
            second_order_entropy,
            runs_entropy,
            average_entropy: avg_entropy,
            is_predictable,
            interpretation,
        })
    }

    /// Calculate first-order entropy (individual values)
    fn calculate_first_order_entropy(&self, sequence: &[u64]) -> Result<f64> {
        self.calculate_normalized_entropy(sequence)
    }

    /// Calculate second-order entropy (pairs of consecutive values)
    fn calculate_second_order_entropy(&self, sequence: &[u64]) -> Result<f64> {
        if sequence.len() < 2 {
            return Ok(0.0);
        }

        let pairs: Vec<u64> = sequence
            .windows(2)
            .map(|w| {
                let a = w[0] as u128;
                let b = w[1] as u128;
                ((a << 32) | b) as u64
            })
            .collect();

        self.calculate_normalized_entropy(&pairs)
    }

    /// Calculate runs entropy (sequences of repeated values)
    fn calculate_runs_entropy(&self, sequence: &[u64]) -> Result<f64> {
        if sequence.is_empty() {
            return Ok(0.0);
        }

        let mut runs = Vec::new();
        let mut current_value = sequence[0];
        let mut current_length = 1u64;

        for &value in &sequence[1..] {
            if value == current_value {
                current_length += 1;
            } else {
                runs.push(current_length);
                current_value = value;
                current_length = 1;
            }
        }
        runs.push(current_length);

        self.calculate_normalized_entropy(&runs)
    }

    /// Build frequency map from values
    fn build_frequency_map(&self, values: &[u64]) -> HashMap<u64, u64> {
        let mut map = HashMap::new();
        for &value in values {
            *map.entry(value).or_insert(0) += 1;
        }
        map
    }

    /// Calculate mutual information between two variables
    pub fn calculate_mutual_information(&self, x_values: &[u64], y_values: &[u64]) -> Result<f64> {
        if x_values.len() != y_values.len() {
            return Err(VotingError::InvalidInput(
                "Value arrays must have same length".to_string(),
            ));
        }

        if x_values.is_empty() {
            return Err(VotingError::InsufficientData(
                "No values provided".to_string(),
            ));
        }

        let h_x = self.calculate_shannon_entropy(x_values)?;
        let h_y = self.calculate_shannon_entropy(y_values)?;

        let mut joint_counts: HashMap<(u64, u64), u64> = HashMap::new();
        for i in 0..x_values.len() {
            *joint_counts.entry((x_values[i], y_values[i])).or_insert(0) += 1;
        }

        let total = x_values.len() as f64;
        let mut h_xy = 0.0;

        for count in joint_counts.values() {
            let probability = *count as f64 / total;
            if probability > 0.0 {
                h_xy -= probability * probability.log2();
            }
        }

        Ok(h_x + h_y - h_xy)
    }

    /// Set minimum entropy threshold
    pub fn set_threshold(&mut self, threshold: f64) {
        self.min_entropy_threshold = threshold;
    }

    /// Get current threshold
    pub fn threshold(&self) -> f64 {
        self.min_entropy_threshold
    }
}

impl Default for EntropyAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of temporal entropy analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalEntropyResult {
    /// Overall entropy across all time windows
    pub overall_entropy: f64,

    /// Normalized entropy (0 to 1)
    pub normalized_entropy: f64,

    /// Entropy for each time window
    pub window_entropies: Vec<WindowEntropy>,

    /// Indices of windows with suspiciously low entropy
    pub low_entropy_windows: Vec<usize>,

    /// Whether the temporal pattern is suspicious
    pub is_suspicious: bool,

    /// Human-readable interpretation
    pub interpretation: String,
}

/// Entropy information for a time window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowEntropy {
    /// Window index
    pub window_index: usize,

    /// Start timestamp
    pub start_time: u64,

    /// End timestamp
    pub end_time: u64,

    /// Number of votes in window
    pub vote_count: u64,

    /// Entropy within this window
    pub entropy: f64,
}

/// Result of geographic entropy analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicEntropyResult {
    /// Total unique locations
    pub total_locations: usize,

    /// Shannon entropy
    pub entropy: f64,

    /// Normalized entropy
    pub normalized_entropy: f64,

    /// Expected maximum entropy
    pub expected_entropy: f64,

    /// Per-location analysis
    pub location_analysis: Vec<LocationEntropy>,

    /// Whether votes are concentrated in few locations
    pub is_concentrated: bool,

    /// Ratio of votes in top location
    pub concentration_ratio: f64,

    /// Human-readable interpretation
    pub interpretation: String,
}

/// Entropy information for a location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationEntropy {
    /// Location identifier
    pub location: String,

    /// Number of votes
    pub vote_count: u64,

    /// Percentage of total votes
    pub percentage: f64,
}

/// Result of sequence entropy analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceEntropyResult {
    /// First-order entropy (individual values)
    pub first_order_entropy: f64,

    /// Second-order entropy (consecutive pairs)
    pub second_order_entropy: f64,

    /// Runs entropy (repeated values)
    pub runs_entropy: f64,

    /// Average of all entropy measures
    pub average_entropy: f64,

    /// Whether sequence is predictable
    pub is_predictable: bool,

    /// Human-readable interpretation
    pub interpretation: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shannon_entropy_uniform() {
        let analyzer = EntropyAnalyzer::new();
        let values = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let entropy = analyzer.calculate_shannon_entropy(&values).unwrap();
        assert_eq!(entropy, 3.0); // log2(8) = 3.0
    }

    #[test]
    fn test_shannon_entropy_all_same() {
        let analyzer = EntropyAnalyzer::new();
        let values = vec![1, 1, 1, 1, 1];
        let entropy = analyzer.calculate_shannon_entropy(&values).unwrap();
        assert_eq!(entropy, 0.0);
    }

    #[test]
    fn test_normalized_entropy() {
        let analyzer = EntropyAnalyzer::new();
        let values = vec![1, 2, 3, 4];
        let norm_entropy = analyzer.calculate_normalized_entropy(&values).unwrap();
        assert_eq!(norm_entropy, 1.0); // Perfectly uniform
    }

    #[test]
    fn test_normalized_entropy_skewed() {
        let analyzer = EntropyAnalyzer::new();
        let values = vec![1, 1, 1, 1, 2];
        let norm_entropy = analyzer.calculate_normalized_entropy(&values).unwrap();
        assert!(norm_entropy < 1.0);
        assert!(norm_entropy > 0.0);
    }

    #[test]
    fn test_empty_values() {
        let analyzer = EntropyAnalyzer::new();
        let values: Vec<u64> = vec![];
        let result = analyzer.calculate_shannon_entropy(&values);
        assert!(result.is_err());
    }

    #[test]
    fn test_temporal_entropy_uniform() {
        let analyzer = EntropyAnalyzer::new();
        let timestamps = vec![100, 200, 300, 400, 500, 600, 700, 800];
        let result = analyzer.analyze_temporal_entropy(&timestamps, 100).unwrap();
        assert_eq!(result.normalized_entropy, 1.0);
        assert!(!result.is_suspicious);
    }

    #[test]
    fn test_temporal_entropy_clustered() {
        let analyzer = EntropyAnalyzer::new();
        let timestamps = vec![100, 101, 102, 103, 104, 105, 106, 107];
        let result = analyzer.analyze_temporal_entropy(&timestamps, 100).unwrap();
        assert!(result.is_suspicious);
    }

    #[test]
    fn test_geographic_entropy_distributed() {
        let analyzer = EntropyAnalyzer::new();
        let locations = vec![
            "Location A".to_string(),
            "Location B".to_string(),
            "Location C".to_string(),
            "Location D".to_string(),
        ];
        let result = analyzer.analyze_geographic_entropy(&locations).unwrap();
        assert!(!result.is_concentrated);
        assert_eq!(result.total_locations, 4);
    }

    #[test]
    fn test_geographic_entropy_concentrated() {
        let analyzer = EntropyAnalyzer::new();
        let locations = vec![
            "Location A".to_string(),
            "Location A".to_string(),
            "Location A".to_string(),
            "Location B".to_string(),
        ];
        let result = analyzer.analyze_geographic_entropy(&locations).unwrap();
        assert!(result.is_concentrated);
    }

    #[test]
    fn test_sequence_entropy_random() {
        let analyzer = EntropyAnalyzer::new();
        let sequence = vec![1, 5, 2, 8, 3, 7, 4, 6];
        let result = analyzer.analyze_sequence_entropy(&sequence).unwrap();
        assert!(!result.is_predictable);
    }

    #[test]
    fn test_sequence_entropy_predictable() {
        let analyzer = EntropyAnalyzer::new();
        let sequence = vec![1, 1, 1, 1, 1, 1, 1, 1];
        let result = analyzer.analyze_sequence_entropy(&sequence).unwrap();
        assert!(result.is_predictable);
    }

    #[test]
    fn test_first_order_entropy() {
        let analyzer = EntropyAnalyzer::new();
        let sequence = vec![1, 2, 3, 4];
        let entropy = analyzer.calculate_first_order_entropy(&sequence).unwrap();
        assert_eq!(entropy, 1.0);
    }

    #[test]
    fn test_runs_entropy() {
        let analyzer = EntropyAnalyzer::new();
        let sequence = vec![1, 1, 2, 2, 3, 3, 4, 4];
        let entropy = analyzer.calculate_runs_entropy(&sequence).unwrap();
        assert!(entropy > 0.0);
    }

    #[test]
    fn test_mutual_information() {
        let analyzer = EntropyAnalyzer::new();
        let x = vec![1, 2, 3, 4];
        let y = vec![1, 2, 3, 4]; // Perfectly correlated
        let mi = analyzer.calculate_mutual_information(&x, &y).unwrap();
        assert!(mi > 0.0);
    }

    #[test]
    fn test_mutual_information_independent() {
        let analyzer = EntropyAnalyzer::new();
        let x = vec![1, 1, 2, 2];
        let y = vec![3, 4, 3, 4]; // Independent
        let mi = analyzer.calculate_mutual_information(&x, &y).unwrap();
        assert!(mi >= 0.0);
    }

    #[test]
    fn test_mutual_information_length_mismatch() {
        let analyzer = EntropyAnalyzer::new();
        let x = vec![1, 2, 3];
        let y = vec![4, 5];
        let result = analyzer.calculate_mutual_information(&x, &y);
        assert!(result.is_err());
    }

    #[test]
    fn test_threshold_setting() {
        let mut analyzer = EntropyAnalyzer::new();
        assert_eq!(analyzer.threshold(), 0.7);
        analyzer.set_threshold(0.85);
        assert_eq!(analyzer.threshold(), 0.85);
    }

    #[test]
    fn test_custom_threshold_creation() {
        let analyzer = EntropyAnalyzer::with_threshold(0.9);
        assert_eq!(analyzer.threshold(), 0.9);
    }

    #[test]
    fn test_build_frequency_map() {
        let analyzer = EntropyAnalyzer::new();
        let values = vec![1, 2, 2, 3, 3, 3];
        let freq_map = analyzer.build_frequency_map(&values);
        assert_eq!(freq_map.get(&1), Some(&1));
        assert_eq!(freq_map.get(&2), Some(&2));
        assert_eq!(freq_map.get(&3), Some(&3));
    }
}

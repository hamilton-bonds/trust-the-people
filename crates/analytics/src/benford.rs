//! Benford's Law analysis for detecting statistical anomalies
//!
//! Benford's Law states that in many naturally occurring datasets, the leading
//! digit is more likely to be small. Specifically, the digit 1 appears as the
//! leading digit about 30% of the time, while 9 appears less than 5% of the time.
//!
//! Significant deviations from Benford's Law can indicate data manipulation,
//! fraud, or other anomalies in voting patterns.

use common::{Result, VotingError};
use serde::{Deserialize, Serialize};

/// Expected frequencies for first digits according to Benford's Law
pub const BENFORD_FREQUENCIES: [f64; 10] = [
    0.0,    // 0 (not used as first digit)
    0.30103, // 1
    0.17609, // 2
    0.12494, // 3
    0.09691, // 4
    0.07918, // 5
    0.06695, // 6
    0.05799, // 7
    0.05115, // 8
    0.04576, // 9
];

/// Benford's Law analyzer
#[derive(Debug, Clone)]
pub struct BenfordAnalyzer {
    // Critical values are now computed via method instead of HashMap
}

impl BenfordAnalyzer {
    /// Create a new Benford analyzer
    pub fn new() -> Self {
        Self {}
    }

    /// Get critical chi-square value for given significance level
    /// Using 8 degrees of freedom (9 digits - 1)
    fn get_critical_value(&self, alpha: f64) -> f64 {
        // Use epsilon comparison for floating point
        const EPSILON: f64 = 0.0001;
        
        if (alpha - 0.10).abs() < EPSILON {
            13.362  // 90% confidence
        } else if (alpha - 0.05).abs() < EPSILON {
            15.507  // 95% confidence
        } else if (alpha - 0.01).abs() < EPSILON {
            20.090  // 99% confidence
        } else if (alpha - 0.001).abs() < EPSILON {
            26.125  // 99.9% confidence
        } else {
            15.507  // default to 95%
        }
    }

    /// Analyze a dataset for Benford's Law compliance
    pub fn analyze(&self, values: &[u64]) -> Result<BenfordResult> {
        if values.is_empty() {
            return Err(VotingError::InsufficientData(
                "No values provided for Benford analysis".to_string(),
            ));
        }

        if values.len() < 30 {
            return Err(VotingError::InsufficientData(format!(
                "Benford analysis requires at least 30 values, got {}",
                values.len()
            )));
        }

        let digit_counts = self.count_first_digits(values);
        let total = values.len() as f64;
        let chi_square = self.calculate_chi_square(&digit_counts, total);
        let p_value = self.estimate_p_value(chi_square);
        let complies = chi_square < self.get_critical_value(0.05);

        let mut observed_frequencies = [0.0; 10];
        for digit in 1..=9 {
            observed_frequencies[digit] = digit_counts[digit] as f64 / total;
        }

        Ok(BenfordResult {
            total_values: values.len(),
            digit_counts,
            observed_frequencies,
            expected_frequencies: BENFORD_FREQUENCIES,
            chi_square_statistic: chi_square,
            p_value,
            complies_with_benford: complies,
            degrees_of_freedom: 8,
            max_deviation: self.calculate_max_deviation(&observed_frequencies),
        })
    }

    /// Count occurrences of each first digit
    fn count_first_digits(&self, values: &[u64]) -> [u64; 10] {
        let mut counts = [0u64; 10];

        for &value in values {
            if value == 0 {
                continue;
            }

            let first_digit = self.extract_first_digit(value);
            if first_digit > 0 && first_digit <= 9 {
                counts[first_digit] += 1;
            }
        }

        counts
    }

    /// Extract the first digit from a number
    fn extract_first_digit(&self, mut value: u64) -> usize {
        if value == 0 {
            return 0;
        }

        while value >= 10 {
            value /= 10;
        }

        value as usize
    }

    /// Calculate chi-square statistic
    fn calculate_chi_square(&self, observed: &[u64; 10], total: f64) -> f64 {
        let mut chi_square = 0.0;

        for digit in 1..=9 {
            let observed_count = observed[digit] as f64;
            let expected_count = BENFORD_FREQUENCIES[digit] * total;

            if expected_count > 0.0 {
                let diff = observed_count - expected_count;
                chi_square += (diff * diff) / expected_count;
            }
        }

        chi_square
    }

    /// Estimate p-value from chi-square statistic
    fn estimate_p_value(&self, chi_square: f64) -> f64 {
        if chi_square < self.get_critical_value(0.10) {
            1.0 // p > 0.10
        } else if chi_square < self.get_critical_value(0.05) {
            0.075 // 0.05 < p < 0.10
        } else if chi_square < self.get_critical_value(0.01) {
            0.03 // 0.01 < p < 0.05
        } else if chi_square < self.get_critical_value(0.001) {
            0.005 // 0.001 < p < 0.01
        } else {
            0.0005 // p < 0.001
        }
    }

    /// Calculate maximum deviation from expected frequencies
    fn calculate_max_deviation(&self, observed: &[f64; 10]) -> f64 {
        let mut max_deviation = 0.0;

        for digit in 1..=9 {
            let deviation = (observed[digit] - BENFORD_FREQUENCIES[digit]).abs();
            if deviation > max_deviation {
                max_deviation = deviation;
            }
        }

        max_deviation
    }

    /// Analyze digit distribution by position
    pub fn analyze_by_position(&self, values: &[u64], position: usize) -> Result<DigitDistribution> {
        if values.is_empty() {
            return Err(VotingError::InsufficientData(
                "No values provided".to_string(),
            ));
        }

        let mut counts = [0u64; 10];
        let mut valid_count = 0;

        for &value in values {
            if let Some(digit) = self.extract_digit_at_position(value, position) {
                counts[digit] += 1;
                valid_count += 1;
            }
        }

        let mut frequencies = [0.0; 10];
        if valid_count > 0 {
            for i in 0..10 {
                frequencies[i] = counts[i] as f64 / valid_count as f64;
            }
        }

        Ok(DigitDistribution {
            position,
            counts,
            frequencies,
            total_values: valid_count,
        })
    }

    /// Extract digit at specific position (0 = rightmost)
    fn extract_digit_at_position(&self, value: u64, position: usize) -> Option<usize> {
        let s = value.to_string();
        let len = s.len();

        if position >= len {
            return None;
        }

        let digit_char = s.chars().rev().nth(position)?;
        digit_char.to_digit(10).map(|d| d as usize)
    }

    /// Generate detailed report
    pub fn generate_report(&self, result: &BenfordResult) -> BenfordReport {
        let mut digit_analysis = Vec::new();

        for digit in 1..=9 {
            let observed = result.observed_frequencies[digit];
            let expected = BENFORD_FREQUENCIES[digit];
            let deviation = observed - expected;
            let percent_deviation = if expected > 0.0 {
                (deviation / expected) * 100.0
            } else {
                0.0
            };

            digit_analysis.push(DigitAnalysis {
                digit,
                observed_count: result.digit_counts[digit],
                observed_frequency: observed,
                expected_frequency: expected,
                deviation,
                percent_deviation,
            });
        }

        let significance_level = if result.chi_square_statistic < self.get_critical_value(0.10) {
            "Not significant (p > 0.10)"
        } else if result.chi_square_statistic < self.get_critical_value(0.05) {
            "Marginally significant (0.05 < p < 0.10)"
        } else if result.chi_square_statistic < self.get_critical_value(0.01) {
            "Significant (0.01 < p < 0.05)"
        } else if result.chi_square_statistic < self.get_critical_value(0.001) {
            "Very significant (0.001 < p < 0.01)"
        } else {
            "Highly significant (p < 0.001)"
        };

        BenfordReport {
            result: result.clone(),
            digit_analysis,
            significance_level: significance_level.to_string(),
            interpretation: self.interpret_result(result),
        }
    }

    /// Interpret Benford analysis result
    fn interpret_result(&self, result: &BenfordResult) -> String {
        if result.complies_with_benford {
            "Data complies with Benford's Law. No significant evidence of manipulation detected.".to_string()
        } else {
            format!(
                "Data deviates significantly from Benford's Law (chi-square: {:.2}, p-value: {:.4}). \
                Maximum deviation: {:.4}. This may indicate data manipulation, but could also result from \
                legitimate factors such as vote count ranges, jurisdictional boundaries, or other systematic effects.",
                result.chi_square_statistic, result.p_value, result.max_deviation
            )
        }
    }
}

impl Default for BenfordAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of Benford's Law analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenfordResult {
    /// Total number of values analyzed
    pub total_values: usize,

    /// Count of each first digit (index 0 unused)
    pub digit_counts: [u64; 10],

    /// Observed frequencies for each digit
    pub observed_frequencies: [f64; 10],

    /// Expected frequencies according to Benford's Law
    pub expected_frequencies: [f64; 10],

    /// Chi-square statistic
    pub chi_square_statistic: f64,

    /// Estimated p-value
    pub p_value: f64,

    /// Whether data complies with Benford's Law at 95% confidence
    pub complies_with_benford: bool,

    /// Degrees of freedom (always 8 for first digit)
    pub degrees_of_freedom: u8,

    /// Maximum deviation from expected frequency
    pub max_deviation: f64,
}

/// Detailed analysis for a single digit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigitAnalysis {
    /// The digit being analyzed
    pub digit: usize,

    /// Observed count
    pub observed_count: u64,

    /// Observed frequency
    pub observed_frequency: f64,

    /// Expected frequency per Benford's Law
    pub expected_frequency: f64,

    /// Deviation (observed - expected)
    pub deviation: f64,

    /// Percent deviation from expected
    pub percent_deviation: f64,
}

/// Distribution of digits at a specific position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigitDistribution {
    /// Position (0 = rightmost)
    pub position: usize,

    /// Count of each digit at this position
    pub counts: [u64; 10],

    /// Frequency of each digit at this position
    pub frequencies: [f64; 10],

    /// Total values that had a digit at this position
    pub total_values: usize,
}

/// Comprehensive Benford's Law report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenfordReport {
    /// Analysis result
    pub result: BenfordResult,

    /// Per-digit analysis
    pub digit_analysis: Vec<DigitAnalysis>,

    /// Significance level description
    pub significance_level: String,

    /// Interpretation of results
    pub interpretation: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benford_analyzer_creation() {
        let analyzer = BenfordAnalyzer::new();
        assert_eq!(analyzer.critical_values.len(), 4);
    }

    #[test]
    fn test_extract_first_digit() {
        let analyzer = BenfordAnalyzer::new();
        assert_eq!(analyzer.extract_first_digit(123), 1);
        assert_eq!(analyzer.extract_first_digit(456), 4);
        assert_eq!(analyzer.extract_first_digit(7890), 7);
        assert_eq!(analyzer.extract_first_digit(1), 1);
        assert_eq!(analyzer.extract_first_digit(0), 0);
    }

    #[test]
    fn test_count_first_digits() {
        let analyzer = BenfordAnalyzer::new();
        let values = vec![100, 200, 111, 123, 234, 345];
        let counts = analyzer.count_first_digits(&values);

        assert_eq!(counts[1], 3); // 100, 111, 123
        assert_eq!(counts[2], 2); // 200, 234
        assert_eq!(counts[3], 1); // 345
    }

    #[test]
    fn test_benford_compliant_dataset() {
        let analyzer = BenfordAnalyzer::new();
        
        // Generate dataset that roughly follows Benford's Law
        let mut values = Vec::new();
        values.extend(vec![1; 30]); // 30% ones
        values.extend(vec![2; 18]); // 18% twos
        values.extend(vec![3; 12]); // 12% threes
        values.extend(vec![4; 10]); // 10% fours
        values.extend(vec![5; 8]);  // 8% fives
        values.extend(vec![6; 7]);  // 7% sixes
        values.extend(vec![7; 6]);  // 6% sevens
        values.extend(vec![8; 5]);  // 5% eights
        values.extend(vec![9; 4]);  // 4% nines

        let result = analyzer.analyze(&values).unwrap();
        assert!(result.complies_with_benford);
    }

    #[test]
    fn test_benford_non_compliant_dataset() {
        let analyzer = BenfordAnalyzer::new();
        
        // Uniform distribution (should not comply)
        let values = vec![1, 2, 3, 4, 5, 6, 7, 8, 9,
                         10, 20, 30, 40, 50, 60, 70, 80, 90,
                         100, 200, 300, 400, 500, 600, 700, 800, 900,
                         1000, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000];

        let result = analyzer.analyze(&values).unwrap();
        assert!(!result.complies_with_benford);
        assert!(result.chi_square_statistic > 0.0);
    }

    #[test]
    fn test_insufficient_data() {
        let analyzer = BenfordAnalyzer::new();
        let values = vec![1, 2, 3]; // Too few values
        let result = analyzer.analyze(&values);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_dataset() {
        let analyzer = BenfordAnalyzer::new();
        let values: Vec<u64> = vec![];
        let result = analyzer.analyze(&values);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_digit_at_position() {
        let analyzer = BenfordAnalyzer::new();
        assert_eq!(analyzer.extract_digit_at_position(12345, 0), Some(5));
        assert_eq!(analyzer.extract_digit_at_position(12345, 1), Some(4));
        assert_eq!(analyzer.extract_digit_at_position(12345, 2), Some(3));
        assert_eq!(analyzer.extract_digit_at_position(12345, 4), Some(1));
        assert_eq!(analyzer.extract_digit_at_position(12345, 5), None);
    }

    #[test]
    fn test_analyze_by_position() {
        let analyzer = BenfordAnalyzer::new();
        let values = vec![123, 234, 345, 456, 567];
        let dist = analyzer.analyze_by_position(&values, 0).unwrap();
        
        assert_eq!(dist.position, 0);
        assert_eq!(dist.total_values, 5);
        assert_eq!(dist.counts[3], 1); // 123
        assert_eq!(dist.counts[4], 1); // 234
        assert_eq!(dist.counts[5], 1); // 345
        assert_eq!(dist.counts[6], 1); // 456
        assert_eq!(dist.counts[7], 1); // 567
    }

    #[test]
    fn test_generate_report() {
        let analyzer = BenfordAnalyzer::new();
        let values = vec![1; 100];
        let result = analyzer.analyze(&values).unwrap();
        let report = analyzer.generate_report(&result);

        assert_eq!(report.digit_analysis.len(), 9);
        assert!(!report.significance_level.is_empty());
        assert!(!report.interpretation.is_empty());
    }

    #[test]
    fn test_chi_square_calculation() {
        let analyzer = BenfordAnalyzer::new();
        let mut observed = [0u64; 10];
        observed[1] = 30;
        observed[2] = 18;
        observed[3] = 12;
        observed[4] = 10;
        observed[5] = 8;
        observed[6] = 7;
        observed[7] = 6;
        observed[8] = 5;
        observed[9] = 4;

        let chi_square = analyzer.calculate_chi_square(&observed, 100.0);
        assert!(chi_square >= 0.0);
        assert!(chi_square < 20.0); // Should be reasonable for this distribution
    }

    #[test]
    fn test_max_deviation() {
        let analyzer = BenfordAnalyzer::new();
        let mut observed = [0.0; 10];
        observed[1] = 0.5; // 50% instead of 30%
        observed[2] = 0.1;

        let max_dev = analyzer.calculate_max_deviation(&observed);
        assert!((max_dev - 0.19897).abs() < 0.001); // ~0.5 - 0.30103
    }
}

//! Correlation analysis for voting patterns
//!
//! This module provides statistical correlation analysis to identify relationships
//! between different voting variables:
//! - Pearson correlation for linear relationships
//! - Spearman rank correlation for monotonic relationships
//! - Kendall tau for ordinal data
//! - Cross-correlation for time series
//! - Multivariate correlation analysis

use common::{Result, VotingError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Correlation analyzer for voting data
#[derive(Debug, Clone)]
pub struct CorrelationAnalyzer {
    /// Minimum sample size for reliable correlation
    min_sample_size: usize,

    /// Significance threshold (p-value)
    significance_threshold: f64,
}

impl CorrelationAnalyzer {
    /// Create new correlation analyzer with default settings
    pub fn new() -> Self {
        Self {
            min_sample_size: 30,
            significance_threshold: 0.05,
        }
    }

    /// Create analyzer with custom settings
    pub fn with_settings(min_sample_size: usize, significance_threshold: f64) -> Self {
        Self {
            min_sample_size,
            significance_threshold,
        }
    }

    /// Calculate Pearson correlation coefficient
    ///
    /// Measures linear relationship between two continuous variables
    /// Returns value between -1 (perfect negative) and 1 (perfect positive)
    pub fn pearson_correlation(&self, x: &[f64], y: &[f64]) -> Result<CorrelationResult> {
        if x.len() != y.len() {
            return Err(VotingError::InvalidInput(
                "Arrays must have equal length".to_string(),
            ));
        }

        if x.len() < self.min_sample_size {
            return Err(VotingError::InsufficientData(format!(
                "Need at least {} samples, got {}",
                self.min_sample_size,
                x.len()
            )));
        }

        let n = x.len() as f64;
        let mean_x = x.iter().sum::<f64>() / n;
        let mean_y = y.iter().sum::<f64>() / n;

        let mut covariance = 0.0;
        let mut var_x = 0.0;
        let mut var_y = 0.0;

        for i in 0..x.len() {
            let diff_x = x[i] - mean_x;
            let diff_y = y[i] - mean_y;
            covariance += diff_x * diff_y;
            var_x += diff_x * diff_x;
            var_y += diff_y * diff_y;
        }

        if var_x == 0.0 || var_y == 0.0 {
            return Ok(CorrelationResult {
                coefficient: 0.0,
                p_value: 1.0,
                sample_size: x.len(),
                correlation_type: CorrelationType::Pearson,
                is_significant: false,
                interpretation: "No variance in one or both variables".to_string(),
            });
        }

        let correlation = covariance / (var_x.sqrt() * var_y.sqrt());
        let t_statistic = correlation * ((n - 2.0) / (1.0 - correlation * correlation)).sqrt();
        let p_value = self.estimate_p_value_t(t_statistic, n as usize - 2);
        let is_significant = p_value < self.significance_threshold;

        let interpretation = self.interpret_correlation(correlation, is_significant);

        Ok(CorrelationResult {
            coefficient: correlation,
            p_value,
            sample_size: x.len(),
            correlation_type: CorrelationType::Pearson,
            is_significant,
            interpretation,
        })
    }

    /// Calculate Spearman rank correlation
    ///
    /// Measures monotonic relationship between variables
    /// More robust to outliers than Pearson correlation
    pub fn spearman_correlation(&self, x: &[f64], y: &[f64]) -> Result<CorrelationResult> {
        if x.len() != y.len() {
            return Err(VotingError::InvalidInput(
                "Arrays must have equal length".to_string(),
            ));
        }

        if x.len() < self.min_sample_size {
            return Err(VotingError::InsufficientData(format!(
                "Need at least {} samples",
                self.min_sample_size
            )));
        }

        let rank_x = self.calculate_ranks(x);
        let rank_y = self.calculate_ranks(y);

        let result = self.pearson_correlation(&rank_x, &rank_y)?;

        Ok(CorrelationResult {
            coefficient: result.coefficient,
            p_value: result.p_value,
            sample_size: result.sample_size,
            correlation_type: CorrelationType::Spearman,
            is_significant: result.is_significant,
            interpretation: self.interpret_correlation(result.coefficient, result.is_significant),
        })
    }

    /// Calculate Kendall tau correlation
    ///
    /// Measures ordinal association between variables
    /// Better for small samples or many tied ranks
    pub fn kendall_tau(&self, x: &[f64], y: &[f64]) -> Result<CorrelationResult> {
        if x.len() != y.len() {
            return Err(VotingError::InvalidInput(
                "Arrays must have equal length".to_string(),
            ));
        }

        if x.len() < 10 {
            return Err(VotingError::InsufficientData(
                "Kendall tau requires at least 10 samples".to_string(),
            ));
        }

        let n = x.len();
        let mut concordant = 0;
        let mut discordant = 0;

        for i in 0..n {
            for j in (i + 1)..n {
                let sign_x = (x[j] - x[i]).signum();
                let sign_y = (y[j] - y[i]).signum();
                let product = sign_x * sign_y;

                if product > 0.0 {
                    concordant += 1;
                } else if product < 0.0 {
                    discordant += 1;
                }
            }
        }

        let total_pairs = (n * (n - 1)) / 2;
        let tau = (concordant as f64 - discordant as f64) / total_pairs as f64;

        let var_tau = (2.0 * (2.0 * n as f64 + 5.0)) / (9.0 * n as f64 * (n as f64 - 1.0));
        let z_score = tau / var_tau.sqrt();
        let p_value = 2.0 * (1.0 - self.standard_normal_cdf(z_score.abs()));
        let is_significant = p_value < self.significance_threshold;

        Ok(CorrelationResult {
            coefficient: tau,
            p_value,
            sample_size: n,
            correlation_type: CorrelationType::KendallTau,
            is_significant,
            interpretation: self.interpret_correlation(tau, is_significant),
        })
    }

    /// Calculate cross-correlation for time series
    ///
    /// Identifies time-lagged relationships between two series
    pub fn cross_correlation(
        &self,
        series1: &[f64],
        series2: &[f64],
        max_lag: usize,
    ) -> Result<CrossCorrelationResult> {
        if series1.len() != series2.len() {
            return Err(VotingError::InvalidInput(
                "Series must have equal length".to_string(),
            ));
        }

        if series1.len() < self.min_sample_size {
            return Err(VotingError::InsufficientData(format!(
                "Need at least {} samples",
                self.min_sample_size
            )));
        }

        let mut correlations = Vec::new();
        let max_lag = max_lag.min(series1.len() / 2);

        for lag in 0..=max_lag {
            let truncated_length = series1.len() - lag;
            let x = &series1[lag..];
            let y = &series2[..truncated_length];

            if let Ok(result) = self.pearson_correlation(x, y) {
                correlations.push(LaggedCorrelation {
                    lag: lag as i32,
                    correlation: result.coefficient,
                    p_value: result.p_value,
                });
            }
        }

        for lag in 1..=max_lag {
            let truncated_length = series2.len() - lag;
            let x = &series1[..truncated_length];
            let y = &series2[lag..];

            if let Ok(result) = self.pearson_correlation(x, y) {
                correlations.push(LaggedCorrelation {
                    lag: -(lag as i32),
                    correlation: result.coefficient,
                    p_value: result.p_value,
                });
            }
        }

        let max_correlation = correlations
            .iter()
            .max_by(|a, b| a.correlation.abs().partial_cmp(&b.correlation.abs()).unwrap())
            .cloned()
            .unwrap_or(LaggedCorrelation {
                lag: 0,
                correlation: 0.0,
                p_value: 1.0,
            });

        Ok(CrossCorrelationResult {
            correlations,
            max_correlation,
            optimal_lag: max_correlation.lag,
        })
    }

    /// Analyze correlation matrix for multiple variables
    pub fn correlation_matrix(&self, data: &HashMap<String, Vec<f64>>) -> Result<CorrelationMatrix> {
        if data.is_empty() {
            return Err(VotingError::InsufficientData(
                "No data provided".to_string(),
            ));
        }

        let variables: Vec<String> = data.keys().cloned().collect();
        let n = variables.len();
        let mut matrix = vec![vec![0.0; n]; n];
        let mut p_values = vec![vec![0.0; n]; n];

        for i in 0..n {
            for j in 0..n {
                if i == j {
                    matrix[i][j] = 1.0;
                    p_values[i][j] = 0.0;
                } else {
                    let x = &data[&variables[i]];
                    let y = &data[&variables[j]];

                    if let Ok(result) = self.pearson_correlation(x, y) {
                        matrix[i][j] = result.coefficient;
                        p_values[i][j] = result.p_value;
                    }
                }
            }
        }

        let strong_correlations = self.find_strong_correlations(&variables, &matrix, &p_values);

        Ok(CorrelationMatrix {
            variables,
            matrix,
            p_values,
            strong_correlations,
        })
    }

    /// Detect spurious correlations
    pub fn detect_spurious_correlations(
        &self,
        x: &[f64],
        y: &[f64],
        control: &[f64],
    ) -> Result<PartialCorrelationResult> {
        if x.len() != y.len() || x.len() != control.len() {
            return Err(VotingError::InvalidInput(
                "All arrays must have equal length".to_string(),
            ));
        }

        let r_xy = self.pearson_correlation(x, y)?.coefficient;
        let r_xz = self.pearson_correlation(x, control)?.coefficient;
        let r_yz = self.pearson_correlation(y, control)?.coefficient;

        let partial_correlation = (r_xy - r_xz * r_yz)
            / ((1.0 - r_xz * r_xz).sqrt() * (1.0 - r_yz * r_yz).sqrt());

        let is_spurious = r_xy.abs() > 0.5 && partial_correlation.abs() < 0.3;

        Ok(PartialCorrelationResult {
            raw_correlation: r_xy,
            partial_correlation,
            is_spurious,
            interpretation: if is_spurious {
                "Strong raw correlation disappears when controlling for third variable. Likely spurious.".to_string()
            } else {
                "Correlation remains after controlling for third variable.".to_string()
            },
        })
    }

    /// Calculate ranks for Spearman correlation
    fn calculate_ranks(&self, values: &[f64]) -> Vec<f64> {
        let mut indexed_values: Vec<(usize, f64)> = values.iter().copied().enumerate().collect();
        indexed_values.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        let mut ranks = vec![0.0; values.len()];
        let mut i = 0;

        while i < indexed_values.len() {
            let mut j = i;
            while j < indexed_values.len() && indexed_values[j].1 == indexed_values[i].1 {
                j += 1;
            }

            let rank = ((i + j - 1) as f64 / 2.0) + 1.0;
            for k in i..j {
                ranks[indexed_values[k].0] = rank;
            }

            i = j;
        }

        ranks
    }

    /// Estimate p-value from t-statistic
    fn estimate_p_value_t(&self, t: f64, df: usize) -> f64 {
        let t_abs = t.abs();

        if df >= 30 {
            2.0 * (1.0 - self.standard_normal_cdf(t_abs))
        } else if t_abs > 6.0 {
            0.0001
        } else if t_abs > 4.0 {
            0.001
        } else if t_abs > 3.0 {
            0.01
        } else if t_abs > 2.0 {
            0.05
        } else if t_abs > 1.5 {
            0.15
        } else {
            0.5
        }
    }

    /// Approximate standard normal CDF
    fn standard_normal_cdf(&self, x: f64) -> f64 {
        0.5 * (1.0 + self.erf(x / 2.0_f64.sqrt()))
    }

    /// Error function approximation
    fn erf(&self, x: f64) -> f64 {
        let a1 = 0.254829592;
        let a2 = -0.284496736;
        let a3 = 1.421413741;
        let a4 = -1.453152027;
        let a5 = 1.061405429;
        let p = 0.3275911;

        let sign = if x < 0.0 { -1.0 } else { 1.0 };
        let x = x.abs();

        let t = 1.0 / (1.0 + p * x);
        let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

        sign * y
    }

    /// Interpret correlation coefficient
    fn interpret_correlation(&self, r: f64, significant: bool) -> String {
        let strength = if r.abs() >= 0.9 {
            "very strong"
        } else if r.abs() >= 0.7 {
            "strong"
        } else if r.abs() >= 0.5 {
            "moderate"
        } else if r.abs() >= 0.3 {
            "weak"
        } else {
            "very weak"
        };

        let direction = if r > 0.0 { "positive" } else { "negative" };
        let significance = if significant {
            "statistically significant"
        } else {
            "not statistically significant"
        };

        format!(
            "{} {} correlation (r = {:.3}), {}",
            strength, direction, r, significance
        )
    }

    /// Find strong correlations in matrix
    fn find_strong_correlations(
        &self,
        variables: &[String],
        matrix: &[Vec<f64>],
        p_values: &[Vec<f64>],
    ) -> Vec<StrongCorrelation> {
        let mut strong = Vec::new();

        for i in 0..variables.len() {
            for j in (i + 1)..variables.len() {
                let r = matrix[i][j];
                let p = p_values[i][j];

                if r.abs() >= 0.5 && p < self.significance_threshold {
                    strong.push(StrongCorrelation {
                        variable1: variables[i].clone(),
                        variable2: variables[j].clone(),
                        correlation: r,
                        p_value: p,
                    });
                }
            }
        }

        strong.sort_by(|a, b| b.correlation.abs().partial_cmp(&a.correlation.abs()).unwrap());
        strong
    }
}

impl Default for CorrelationAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of correlation analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationResult {
    /// Correlation coefficient
    pub coefficient: f64,

    /// Statistical significance (p-value)
    pub p_value: f64,

    /// Number of samples
    pub sample_size: usize,

    /// Type of correlation computed
    pub correlation_type: CorrelationType,

    /// Whether correlation is statistically significant
    pub is_significant: bool,

    /// Human-readable interpretation
    pub interpretation: String,
}

/// Type of correlation analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorrelationType {
    Pearson,
    Spearman,
    KendallTau,
}

/// Cross-correlation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossCorrelationResult {
    /// Correlations at different lags
    pub correlations: Vec<LaggedCorrelation>,

    /// Maximum correlation found
    pub max_correlation: LaggedCorrelation,

    /// Optimal lag value
    pub optimal_lag: i32,
}

/// Correlation at specific lag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaggedCorrelation {
    /// Lag value (positive or negative)
    pub lag: i32,

    /// Correlation coefficient at this lag
    pub correlation: f64,

    /// P-value
    pub p_value: f64,
}

/// Correlation matrix for multiple variables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationMatrix {
    /// Variable names
    pub variables: Vec<String>,

    /// Correlation coefficients
    pub matrix: Vec<Vec<f64>>,

    /// P-values for each correlation
    pub p_values: Vec<Vec<f64>>,

    /// Strong correlations (abs > 0.5 and significant)
    pub strong_correlations: Vec<StrongCorrelation>,
}

/// Strong correlation between two variables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrongCorrelation {
    pub variable1: String,
    pub variable2: String,
    pub correlation: f64,
    pub p_value: f64,
}

/// Partial correlation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialCorrelationResult {
    /// Raw correlation between x and y
    pub raw_correlation: f64,

    /// Partial correlation controlling for z
    pub partial_correlation: f64,

    /// Whether correlation is likely spurious
    pub is_spurious: bool,

    /// Interpretation
    pub interpretation: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pearson_perfect_positive() {
        let analyzer = CorrelationAnalyzer::new();
        let x: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let y: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let result = analyzer.pearson_correlation(&x, &y).unwrap();
        assert!((result.coefficient - 1.0).abs() < 0.0001);
        assert!(result.is_significant);
    }

    #[test]
    fn test_pearson_perfect_negative() {
        let analyzer = CorrelationAnalyzer::new();
        let x: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let y: Vec<f64> = (0..50).map(|i| -(i as f64)).collect();
        let result = analyzer.pearson_correlation(&x, &y).unwrap();
        assert!((result.coefficient + 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_pearson_no_correlation() {
        let analyzer = CorrelationAnalyzer::new();
        let x: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let y: Vec<f64> = vec![5.0; 50];
        let result = analyzer.pearson_correlation(&x, &y).unwrap();
        assert_eq!(result.coefficient, 0.0);
    }

    #[test]
    fn test_pearson_length_mismatch() {
        let analyzer = CorrelationAnalyzer::new();
        let x = vec![1.0, 2.0, 3.0];
        let y = vec![4.0, 5.0];
        let result = analyzer.pearson_correlation(&x, &y);
        assert!(result.is_err());
    }

    #[test]
    fn test_pearson_insufficient_data() {
        let analyzer = CorrelationAnalyzer::new();
        let x = vec![1.0, 2.0, 3.0];
        let y = vec![4.0, 5.0, 6.0];
        let result = analyzer.pearson_correlation(&x, &y);
        assert!(result.is_err());
    }

    #[test]
    fn test_spearman_correlation() {
        let analyzer = CorrelationAnalyzer::new();
        let x: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let y: Vec<f64> = (0..50).map(|i| (i * i) as f64).collect();
        let result = analyzer.spearman_correlation(&x, &y).unwrap();
        assert!(result.coefficient > 0.9);
    }

    #[test]
    fn test_calculate_ranks() {
        let analyzer = CorrelationAnalyzer::new();
        let values = vec![3.0, 1.0, 4.0, 1.0, 5.0];
        let ranks = analyzer.calculate_ranks(&values);
        assert_eq!(ranks[0], 3.0);
        assert_eq!(ranks[1], 1.5);
        assert_eq!(ranks[2], 4.0);
        assert_eq!(ranks[3], 1.5);
        assert_eq!(ranks[4], 5.0);
    }

    #[test]
    fn test_kendall_tau() {
        let analyzer = CorrelationAnalyzer::new();
        let x: Vec<f64> = (0..30).map(|i| i as f64).collect();
        let y: Vec<f64> = (0..30).map(|i| i as f64).collect();
        let result = analyzer.kendall_tau(&x, &y).unwrap();
        assert!((result.coefficient - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cross_correlation() {
        let analyzer = CorrelationAnalyzer::new();
        let series1: Vec<f64> = (0..50).map(|i| (i as f64).sin()).collect();
        let series2: Vec<f64> = (0..50).map(|i| (i as f64).sin()).collect();
        let result = analyzer.cross_correlation(&series1, &series2, 10).unwrap();
        assert_eq!(result.optimal_lag, 0);
        assert!(result.max_correlation.correlation > 0.9);
    }

    #[test]
    fn test_correlation_matrix() {
        let analyzer = CorrelationAnalyzer::new();
        let mut data = HashMap::new();
        data.insert("var1".to_string(), (0..50).map(|i| i as f64).collect());
        data.insert("var2".to_string(), (0..50).map(|i| (i * 2) as f64).collect());
        data.insert("var3".to_string(), (0..50).map(|i| -(i as f64)).collect());

        let matrix = analyzer.correlation_matrix(&data).unwrap();
        assert_eq!(matrix.variables.len(), 3);
        assert!(!matrix.strong_correlations.is_empty());
    }

    #[test]
    fn test_partial_correlation() {
        let analyzer = CorrelationAnalyzer::new();
        let x: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let y: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let z: Vec<f64> = (0..50).map(|i| i as f64).collect();

        let result = analyzer.detect_spurious_correlations(&x, &y, &z).unwrap();
        assert!(!result.is_spurious);
    }

    #[test]
    fn test_interpret_correlation() {
        let analyzer = CorrelationAnalyzer::new();
        let interp = analyzer.interpret_correlation(0.85, true);
        assert!(interp.contains("strong"));
        assert!(interp.contains("positive"));
        assert!(interp.contains("significant"));
    }

    #[test]
    fn test_standard_normal_cdf() {
        let analyzer = CorrelationAnalyzer::new();
        assert!((analyzer.standard_normal_cdf(0.0) - 0.5).abs() < 0.01);
        assert!(analyzer.standard_normal_cdf(1.96) > 0.97);
        assert!(analyzer.standard_normal_cdf(-1.96) < 0.03);
    }

    #[test]
    fn test_custom_settings() {
        let analyzer = CorrelationAnalyzer::with_settings(20, 0.01);
        assert_eq!(analyzer.min_sample_size, 20);
        assert_eq!(analyzer.significance_threshold, 0.01);
    }
}

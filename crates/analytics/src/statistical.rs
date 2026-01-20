//! Statistical analysis utilities for voting data
//!
//! This module provides comprehensive statistical analysis including:
//! - Descriptive statistics (mean, median, mode, variance, etc.)
//! - Hypothesis testing (t-tests, chi-square tests)
//! - Distribution analysis
//! - Confidence intervals
//! - Outlier detection
//! - Regression analysis

use common::{Result, VotingError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Statistical analyzer for voting data
#[derive(Debug, Clone)]
pub struct StatisticalAnalyzer {
    /// Confidence level for intervals (default 0.95)
    confidence_level: f64,

    /// Number of bootstrap iterations
    bootstrap_iterations: usize,
}

impl StatisticalAnalyzer {
    /// Create new statistical analyzer with default settings
    pub fn new() -> Self {
        Self {
            confidence_level: 0.95,
            bootstrap_iterations: 1000,
        }
    }

    /// Create analyzer with custom confidence level
    pub fn with_confidence_level(confidence_level: f64) -> Self {
        Self {
            confidence_level,
            bootstrap_iterations: 1000,
        }
    }

    /// Calculate descriptive statistics for a dataset
    pub fn descriptive_statistics(&self, data: &[f64]) -> Result<DescriptiveStats> {
        if data.is_empty() {
            return Err(VotingError::InsufficientData(
                "No data provided".to_string(),
            ));
        }

        let n = data.len();
        let mean = self.mean(data);
        let median = self.median(data);
        let mode = self.mode(data);
        let variance = self.variance(data, mean);
        let std_dev = variance.sqrt();
        let min = data.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let range = max - min;
        let quartiles = self.quartiles(data);
        let iqr = quartiles.q3 - quartiles.q1;
        let skewness = self.skewness(data, mean, std_dev);
        let kurtosis = self.kurtosis(data, mean, std_dev);

        Ok(DescriptiveStats {
            count: n,
            mean,
            median,
            mode,
            variance,
            std_dev,
            min,
            max,
            range,
            q1: quartiles.q1,
            q2: quartiles.q2,
            q3: quartiles.q3,
            iqr,
            skewness,
            kurtosis,
        })
    }

    /// Perform one-sample t-test
    pub fn one_sample_t_test(&self, data: &[f64], population_mean: f64) -> Result<TTestResult> {
        if data.len() < 2 {
            return Err(VotingError::InsufficientData(
                "Need at least 2 samples".to_string(),
            ));
        }

        let n = data.len() as f64;
        let sample_mean = self.mean(data);
        let sample_std = self.variance(data, sample_mean).sqrt();

        if sample_std == 0.0 {
            return Err(VotingError::InvalidInput(
                "Standard deviation is zero".to_string(),
            ));
        }

        let t_statistic = (sample_mean - population_mean) / (sample_std / n.sqrt());
        let df = data.len() - 1;
        let p_value = self.t_test_p_value(t_statistic, df);
        let is_significant = p_value < (1.0 - self.confidence_level);

        let ci = self.confidence_interval_mean(data);

        Ok(TTestResult {
            t_statistic,
            degrees_of_freedom: df,
            p_value,
            is_significant,
            sample_mean,
            population_mean,
            confidence_interval: ci,
            test_type: TestType::OneSample,
        })
    }

    /// Perform two-sample t-test
    pub fn two_sample_t_test(&self, sample1: &[f64], sample2: &[f64]) -> Result<TTestResult> {
        if sample1.len() < 2 || sample2.len() < 2 {
            return Err(VotingError::InsufficientData(
                "Each sample needs at least 2 values".to_string(),
            ));
        }

        let n1 = sample1.len() as f64;
        let n2 = sample2.len() as f64;
        let mean1 = self.mean(sample1);
        let mean2 = self.mean(sample2);
        let var1 = self.variance(sample1, mean1);
        let var2 = self.variance(sample2, mean2);

        let pooled_std = ((var1 * (n1 - 1.0) + var2 * (n2 - 1.0)) / (n1 + n2 - 2.0)).sqrt();
        let t_statistic = (mean1 - mean2) / (pooled_std * ((1.0 / n1) + (1.0 / n2)).sqrt());
        let df = (sample1.len() + sample2.len() - 2) as usize;
        let p_value = self.t_test_p_value(t_statistic, df);
        let is_significant = p_value < (1.0 - self.confidence_level);

        Ok(TTestResult {
            t_statistic,
            degrees_of_freedom: df,
            p_value,
            is_significant,
            sample_mean: mean1,
            population_mean: mean2,
            confidence_interval: (0.0, 0.0),
            test_type: TestType::TwoSample,
        })
    }

    /// Perform chi-square goodness of fit test
    pub fn chi_square_test(&self, observed: &[f64], expected: &[f64]) -> Result<ChiSquareResult> {
        if observed.len() != expected.len() {
            return Err(VotingError::InvalidInput(
                "Observed and expected must have same length".to_string(),
            ));
        }

        if observed.is_empty() {
            return Err(VotingError::InsufficientData(
                "No data provided".to_string(),
            ));
        }

        let mut chi_square = 0.0;
        for i in 0..observed.len() {
            if expected[i] > 0.0 {
                let diff = observed[i] - expected[i];
                chi_square += (diff * diff) / expected[i];
            }
        }

        let df = observed.len() - 1;
        let p_value = self.chi_square_p_value(chi_square, df);
        let is_significant = p_value < (1.0 - self.confidence_level);

        Ok(ChiSquareResult {
            chi_square_statistic: chi_square,
            degrees_of_freedom: df,
            p_value,
            is_significant,
            observed: observed.to_vec(),
            expected: expected.to_vec(),
        })
    }

    /// Detect outliers using IQR method
    pub fn detect_outliers_iqr(&self, data: &[f64]) -> Result<OutlierAnalysis> {
        if data.len() < 4 {
            return Err(VotingError::InsufficientData(
                "Need at least 4 data points".to_string(),
            ));
        }

        let quartiles = self.quartiles(data);
        let iqr = quartiles.q3 - quartiles.q1;
        let lower_bound = quartiles.q1 - 1.5 * iqr;
        let upper_bound = quartiles.q3 + 1.5 * iqr;

        let mut outliers = Vec::new();
        let mut outlier_indices = Vec::new();

        for (idx, &value) in data.iter().enumerate() {
            if value < lower_bound || value > upper_bound {
                outliers.push(value);
                outlier_indices.push(idx);
            }
        }

        Ok(OutlierAnalysis {
            method: OutlierMethod::IQR,
            outliers,
            outlier_indices,
            lower_bound,
            upper_bound,
            outlier_count: outliers.len(),
        })
    }

    /// Detect outliers using Z-score method
    pub fn detect_outliers_zscore(&self, data: &[f64], threshold: f64) -> Result<OutlierAnalysis> {
        if data.len() < 3 {
            return Err(VotingError::InsufficientData(
                "Need at least 3 data points".to_string(),
            ));
        }

        let mean = self.mean(data);
        let std_dev = self.variance(data, mean).sqrt();

        if std_dev == 0.0 {
            return Ok(OutlierAnalysis {
                method: OutlierMethod::ZScore,
                outliers: Vec::new(),
                outlier_indices: Vec::new(),
                lower_bound: mean,
                upper_bound: mean,
                outlier_count: 0,
            });
        }

        let lower_bound = mean - threshold * std_dev;
        let upper_bound = mean + threshold * std_dev;

        let mut outliers = Vec::new();
        let mut outlier_indices = Vec::new();

        for (idx, &value) in data.iter().enumerate() {
            let z_score = (value - mean).abs() / std_dev;
            if z_score > threshold {
                outliers.push(value);
                outlier_indices.push(idx);
            }
        }

        Ok(OutlierAnalysis {
            method: OutlierMethod::ZScore,
            outliers,
            outlier_indices,
            lower_bound,
            upper_bound,
            outlier_count: outliers.len(),
        })
    }

    /// Calculate confidence interval for mean
    pub fn confidence_interval_mean(&self, data: &[f64]) -> (f64, f64) {
        if data.len() < 2 {
            return (0.0, 0.0);
        }

        let n = data.len() as f64;
        let mean = self.mean(data);
        let std_dev = self.variance(data, mean).sqrt();
        let se = std_dev / n.sqrt();

        let t_critical = self.t_critical_value(data.len() - 1, self.confidence_level);
        let margin = t_critical * se;

        (mean - margin, mean + margin)
    }

    /// Perform simple linear regression
    pub fn linear_regression(&self, x: &[f64], y: &[f64]) -> Result<RegressionResult> {
        if x.len() != y.len() {
            return Err(VotingError::InvalidInput(
                "X and Y must have same length".to_string(),
            ));
        }

        if x.len() < 3 {
            return Err(VotingError::InsufficientData(
                "Need at least 3 data points".to_string(),
            ));
        }

        let n = x.len() as f64;
        let mean_x = self.mean(x);
        let mean_y = self.mean(y);

        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for i in 0..x.len() {
            let diff_x = x[i] - mean_x;
            numerator += diff_x * (y[i] - mean_y);
            denominator += diff_x * diff_x;
        }

        if denominator == 0.0 {
            return Err(VotingError::InvalidInput(
                "No variance in X variable".to_string(),
            ));
        }

        let slope = numerator / denominator;
        let intercept = mean_y - slope * mean_x;

        let mut ss_total = 0.0;
        let mut ss_residual = 0.0;

        for i in 0..x.len() {
            let predicted = slope * x[i] + intercept;
            ss_residual += (y[i] - predicted).powi(2);
            ss_total += (y[i] - mean_y).powi(2);
        }

        let r_squared = if ss_total > 0.0 {
            1.0 - (ss_residual / ss_total)
        } else {
            0.0
        };

        let mse = ss_residual / (n - 2.0);
        let std_error = mse.sqrt();

        Ok(RegressionResult {
            slope,
            intercept,
            r_squared,
            std_error,
            sample_size: x.len(),
        })
    }

    /// Calculate mean
    fn mean(&self, data: &[f64]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }
        data.iter().sum::<f64>() / data.len() as f64
    }

    /// Calculate median
    fn median(&self, data: &[f64]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }

        let mut sorted = data.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mid = sorted.len() / 2;
        if sorted.len() % 2 == 0 {
            (sorted[mid - 1] + sorted[mid]) / 2.0
        } else {
            sorted[mid]
        }
    }

    /// Calculate mode
    fn mode(&self, data: &[f64]) -> Option<f64> {
        if data.is_empty() {
            return None;
        }

        let mut frequency = HashMap::new();
        for &value in data {
            *frequency.entry(value.to_bits()).or_insert(0) += 1;
        }

        frequency
            .iter()
            .max_by_key(|&(_, count)| count)
            .map(|(&bits, _)| f64::from_bits(bits))
    }

    /// Calculate variance
    fn variance(&self, data: &[f64], mean: f64) -> f64 {
        if data.len() < 2 {
            return 0.0;
        }

        let sum_sq_diff: f64 = data.iter().map(|&x| (x - mean).powi(2)).sum();
        sum_sq_diff / (data.len() - 1) as f64
    }

    /// Calculate quartiles
    fn quartiles(&self, data: &[f64]) -> Quartiles {
        let mut sorted = data.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let q2 = self.median(&sorted);
        let mid = sorted.len() / 2;

        let q1 = if sorted.len() % 2 == 0 {
            self.median(&sorted[..mid])
        } else {
            self.median(&sorted[..mid])
        };

        let q3 = if sorted.len() % 2 == 0 {
            self.median(&sorted[mid..])
        } else {
            self.median(&sorted[mid + 1..])
        };

        Quartiles { q1, q2, q3 }
    }

    /// Calculate skewness
    fn skewness(&self, data: &[f64], mean: f64, std_dev: f64) -> f64 {
        if std_dev == 0.0 || data.len() < 3 {
            return 0.0;
        }

        let n = data.len() as f64;
        let sum_cubed: f64 = data.iter().map(|&x| ((x - mean) / std_dev).powi(3)).sum();

        (n / ((n - 1.0) * (n - 2.0))) * sum_cubed
    }

    /// Calculate kurtosis
    fn kurtosis(&self, data: &[f64], mean: f64, std_dev: f64) -> f64 {
        if std_dev == 0.0 || data.len() < 4 {
            return 0.0;
        }

        let n = data.len() as f64;
        let sum_fourth: f64 = data.iter().map(|&x| ((x - mean) / std_dev).powi(4)).sum();

        let term1 = (n * (n + 1.0)) / ((n - 1.0) * (n - 2.0) * (n - 3.0));
        let term2 = (3.0 * (n - 1.0).powi(2)) / ((n - 2.0) * (n - 3.0));

        term1 * sum_fourth - term2
    }

    /// Estimate p-value for t-test
    fn t_test_p_value(&self, t: f64, df: usize) -> f64 {
        let t_abs = t.abs();

        if df >= 30 {
            return 2.0 * (1.0 - self.standard_normal_cdf(t_abs));
        }

        if t_abs > 6.0 {
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

    /// Estimate p-value for chi-square test
    fn chi_square_p_value(&self, chi_square: f64, df: usize) -> f64 {
        if df == 0 {
            return 1.0;
        }

        if chi_square < 0.0 {
            return 1.0;
        }

        let critical_values = [
            (1, vec![3.841, 6.635, 10.828]),
            (2, vec![5.991, 9.210, 13.816]),
            (3, vec![7.815, 11.345, 16.266]),
            (5, vec![11.070, 15.086, 20.515]),
            (10, vec![18.307, 23.209, 29.588]),
        ];

        for (test_df, values) in critical_values.iter() {
            if df == *test_df {
                if chi_square < values[0] {
                    return 0.10;
                } else if chi_square < values[1] {
                    return 0.05;
                } else if chi_square < values[2] {
                    return 0.01;
                } else {
                    return 0.001;
                }
            }
        }

        0.05
    }

    /// Get critical t-value
    fn t_critical_value(&self, df: usize, confidence: f64) -> f64 {
        let alpha = 1.0 - confidence;

        if df >= 30 {
            if confidence >= 0.99 {
                return 2.576;
            } else if confidence >= 0.95 {
                return 1.96;
            } else {
                return 1.645;
            }
        }

        if confidence >= 0.99 {
            3.0
        } else if confidence >= 0.95 {
            2.0
        } else {
            1.5
        }
    }

    /// Standard normal CDF
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
}

impl Default for StatisticalAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Descriptive statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescriptiveStats {
    pub count: usize,
    pub mean: f64,
    pub median: f64,
    pub mode: Option<f64>,
    pub variance: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
    pub range: f64,
    pub q1: f64,
    pub q2: f64,
    pub q3: f64,
    pub iqr: f64,
    pub skewness: f64,
    pub kurtosis: f64,
}

/// Quartiles
#[derive(Debug, Clone)]
struct Quartiles {
    q1: f64,
    q2: f64,
    q3: f64,
}

/// T-test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TTestResult {
    pub t_statistic: f64,
    pub degrees_of_freedom: usize,
    pub p_value: f64,
    pub is_significant: bool,
    pub sample_mean: f64,
    pub population_mean: f64,
    pub confidence_interval: (f64, f64),
    pub test_type: TestType,
}

/// Type of t-test
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestType {
    OneSample,
    TwoSample,
    Paired,
}

/// Chi-square test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChiSquareResult {
    pub chi_square_statistic: f64,
    pub degrees_of_freedom: usize,
    pub p_value: f64,
    pub is_significant: bool,
    pub observed: Vec<f64>,
    pub expected: Vec<f64>,
}

/// Outlier analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlierAnalysis {
    pub method: OutlierMethod,
    pub outliers: Vec<f64>,
    pub outlier_indices: Vec<usize>,
    pub lower_bound: f64,
    pub upper_bound: f64,
    pub outlier_count: usize,
}

/// Outlier detection method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutlierMethod {
    IQR,
    ZScore,
    ModifiedZScore,
}

/// Linear regression result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionResult {
    pub slope: f64,
    pub intercept: f64,
    pub r_squared: f64,
    pub std_error: f64,
    pub sample_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_descriptive_statistics() {
        let analyzer = StatisticalAnalyzer::new();
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let stats = analyzer.descriptive_statistics(&data).unwrap();
        
        assert_eq!(stats.count, 10);
        assert_eq!(stats.mean, 5.5);
        assert_eq!(stats.median, 5.5);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 10.0);
    }

    #[test]
    fn test_mean() {
        let analyzer = StatisticalAnalyzer::new();
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(analyzer.mean(&data), 3.0);
    }

    #[test]
    fn test_median_odd() {
        let analyzer = StatisticalAnalyzer::new();
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(analyzer.median(&data), 3.0);
    }

    #[test]
    fn test_median_even() {
        let analyzer = StatisticalAnalyzer::new();
        let data = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(analyzer.median(&data), 2.5);
    }

    #[test]
    fn test_variance() {
        let analyzer = StatisticalAnalyzer::new();
        let data = vec![2.0, 4.0, 6.0, 8.0];
        let mean = analyzer.mean(&data);
        let variance = analyzer.variance(&data, mean);
        assert!((variance - 6.666666).abs() < 0.001);
    }

    #[test]
    fn test_one_sample_t_test() {
        let analyzer = StatisticalAnalyzer::new();
        let data = vec![5.0, 5.5, 6.0, 5.2, 5.8, 5.1, 5.9, 5.3];
        let result = analyzer.one_sample_t_test(&data, 5.0).unwrap();
        assert!(result.t_statistic > 0.0);
    }

    #[test]
    fn test_two_sample_t_test() {
        let analyzer = StatisticalAnalyzer::new();
        let sample1 = vec![5.0, 5.5, 6.0, 5.2, 5.8];
        let sample2 = vec![4.0, 4.5, 4.2, 4.8, 4.3];
        let result = analyzer.two_sample_t_test(&sample1, &sample2).unwrap();
        assert!(result.t_statistic > 0.0);
    }

    #[test]
    fn test_chi_square_test() {
        let analyzer = StatisticalAnalyzer::new();
        let observed = vec![10.0, 20.0, 30.0];
        let expected = vec![15.0, 20.0, 25.0];
        let result = analyzer.chi_square_test(&observed, &expected).unwrap();
        assert!(result.chi_square_statistic >= 0.0);
    }

    #[test]
    fn test_detect_outliers_iqr() {
        let analyzer = StatisticalAnalyzer::new();
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 100.0];
        let result = analyzer.detect_outliers_iqr(&data).unwrap();
        assert_eq!(result.outlier_count, 1);
        assert!(result.outliers.contains(&100.0));
    }

    #[test]
    fn test_detect_outliers_zscore() {
        let analyzer = StatisticalAnalyzer::new();
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 100.0];
        let result = analyzer.detect_outliers_zscore(&data, 2.0).unwrap();
        assert!(result.outlier_count > 0);
    }

    #[test]
    fn test_confidence_interval() {
        let analyzer = StatisticalAnalyzer::new();
        let data = vec![5.0, 5.5, 6.0, 5.2, 5.8, 5.1, 5.9, 5.3];
        let ci = analyzer.confidence_interval_mean(&data);
        assert!(ci.0 < ci.1);
        assert!(ci.0 > 0.0);
    }

    #[test]
    fn test_linear_regression() {
        let analyzer = StatisticalAnalyzer::new();
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let result = analyzer.linear_regression(&x, &y).unwrap();
        assert!((result.slope - 2.0).abs() < 0.001);
        assert!((result.intercept - 0.0).abs() < 0.001);
        assert!((result.r_squared - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_quartiles() {
        let analyzer = StatisticalAnalyzer::new();
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        let q = analyzer.quartiles(&data);
        assert_eq!(q.q2, 5.0);
    }

    #[test]
    fn test_skewness() {
        let analyzer = StatisticalAnalyzer::new();
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let mean = analyzer.mean(&data);
        let std_dev = analyzer.variance(&data, mean).sqrt();
        let skew = analyzer.skewness(&data, mean, std_dev);
        assert!(skew.abs() < 0.1);
    }

    #[test]
    fn test_empty_data() {
        let analyzer = StatisticalAnalyzer::new();
        let data: Vec<f64> = vec![];
        let result = analyzer.descriptive_statistics(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_custom_confidence_level() {
        let analyzer = StatisticalAnalyzer::with_confidence_level(0.99);
        assert_eq!(analyzer.confidence_level, 0.99);
    }
}

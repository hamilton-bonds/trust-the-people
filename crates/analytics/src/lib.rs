//! Analytics module for blockchain voting system
//!
//! This crate provides comprehensive analytics capabilities for detecting anomalies,
//! generating statistics, and auditing elections. It consolidates functionality
//! from the CLI, storage, and other modules into a centralized analytics engine.

pub mod anomaly;
pub mod statistics;
pub mod audit;
pub mod metrics;
pub mod benford;
pub mod geographic;
pub mod temporal;
pub mod reports;

use common::{Result, VotingError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Re-export commonly used types
pub use anomaly::{AnomalyDetector, AnomalyReport, AnomalySeverity, AnomalyType};
pub use statistics::{StatisticsEngine, VoteStatistics, TurnoutStatistics};
pub use audit::{AuditEngine, AuditReport, IntegrityCheck};
pub use metrics::{AnalyticsMetrics, MetricsCollector};
pub use reports::{ReportGenerator, ReportFormat};

/// Analytics engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsConfig {
    /// Threshold for anomaly detection (standard deviations)
    pub anomaly_threshold: f64,

    /// Enable Benford's Law analysis
    pub enable_benford_analysis: bool,

    /// Enable geographic analysis
    pub enable_geographic_analysis: bool,

    /// Enable temporal pattern analysis
    pub enable_temporal_analysis: bool,

    /// Minimum sample size for statistical analysis
    pub min_sample_size: usize,

    /// Confidence level for statistical tests (0.0 to 1.0)
    pub confidence_level: f64,

    /// Enable real-time analytics
    pub enable_realtime: bool,

    /// Update interval for real-time analytics (seconds)
    pub realtime_update_interval: u64,

    /// Maximum report history to maintain
    pub max_report_history: usize,
}

impl Default for AnalyticsConfig {
    fn default() -> Self {
        Self {
            anomaly_threshold: 2.5,
            enable_benford_analysis: true,
            enable_geographic_analysis: true,
            enable_temporal_analysis: true,
            min_sample_size: 30,
            confidence_level: 0.95,
            enable_realtime: false,
            realtime_update_interval: 60,
            max_report_history: 100,
        }
    }
}

/// Main analytics engine that coordinates all analysis modules
#[derive(Debug)]
pub struct AnalyticsEngine {
    config: AnalyticsConfig,
    anomaly_detector: AnomalyDetector,
    statistics_engine: StatisticsEngine,
    audit_engine: AuditEngine,
    metrics_collector: MetricsCollector,
    report_generator: ReportGenerator,
}

impl AnalyticsEngine {
    /// Create a new analytics engine with default configuration
    pub fn new() -> Self {
        Self::with_config(AnalyticsConfig::default())
    }

    /// Create a new analytics engine with custom configuration
    pub fn with_config(config: AnalyticsConfig) -> Self {
        let anomaly_detector = AnomalyDetector::new(config.anomaly_threshold);
        let statistics_engine = StatisticsEngine::new(config.min_sample_size);
        let audit_engine = AuditEngine::new();
        let metrics_collector = MetricsCollector::new();
        let report_generator = ReportGenerator::new();

        Self {
            config,
            anomaly_detector,
            statistics_engine,
            audit_engine,
            metrics_collector,
            report_generator,
        }
    }

    /// Run comprehensive analysis on election data
    pub async fn analyze_election(
        &mut self,
        election_id: &str,
        votes: &[VoteData],
    ) -> Result<AnalysisResult> {
        if votes.len() < self.config.min_sample_size {
            return Err(VotingError::InsufficientData(format!(
                "Need at least {} votes for analysis, got {}",
                self.config.min_sample_size,
                votes.len()
            )));
        }

        let mut anomalies = Vec::new();

        // Run all enabled analysis modules
        if self.config.enable_temporal_analysis {
            anomalies.extend(
                self.anomaly_detector
                    .detect_temporal_anomalies(votes, self.config.anomaly_threshold)?,
            );
        }

        if self.config.enable_geographic_analysis {
            anomalies.extend(
                self.anomaly_detector
                    .detect_geographic_anomalies(votes, self.config.anomaly_threshold)?,
            );
        }

        if self.config.enable_benford_analysis {
            anomalies.extend(
                self.anomaly_detector
                    .detect_benford_violations(votes, self.config.anomaly_threshold)?,
            );
        }

        // Generate comprehensive statistics
        let statistics = self.statistics_engine.generate_statistics(votes)?;

        // Run integrity checks
        let integrity = self.audit_engine.check_integrity(election_id, votes).await?;

        // Update metrics
        self.metrics_collector
            .record_analysis(votes.len(), anomalies.len());

        Ok(AnalysisResult {
            election_id: election_id.to_string(),
            total_votes: votes.len(),
            anomalies,
            statistics,
            integrity,
            timestamp: common::utils::current_timestamp(),
        })
    }

    /// Detect anomalies in real-time
    pub async fn realtime_analysis(
        &mut self,
        election_id: &str,
        votes: &[VoteData],
    ) -> Result<Vec<AnomalyReport>> {
        if !self.config.enable_realtime {
            return Err(VotingError::ConfigurationError(
                "Real-time analysis not enabled".to_string(),
            ));
        }

        let mut anomalies = Vec::new();

        // Quick anomaly checks optimized for real-time
        anomalies.extend(
            self.anomaly_detector
                .detect_timing_spikes(votes, self.config.anomaly_threshold)?,
        );

        anomalies.extend(
            self.anomaly_detector
                .detect_sequential_patterns(votes, self.config.anomaly_threshold)?,
        );

        Ok(anomalies)
    }

    /// Generate comprehensive audit report
    pub async fn generate_audit_report(
        &self,
        election_id: &str,
        analysis: &AnalysisResult,
        format: ReportFormat,
    ) -> Result<String> {
        self.report_generator
            .generate_report(election_id, analysis, format)
    }

    /// Get current analytics metrics
    pub fn get_metrics(&self) -> &AnalyticsMetrics {
        self.metrics_collector.get_metrics()
    }

    /// Get configuration
    pub fn config(&self) -> &AnalyticsConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: AnalyticsConfig) {
        self.config = config;
        self.anomaly_detector
            .set_threshold(self.config.anomaly_threshold);
        self.statistics_engine
            .set_min_sample_size(self.config.min_sample_size);
    }

    /// Compare two elections
    pub fn compare_elections(
        &self,
        current: &AnalysisResult,
        baseline: &AnalysisResult,
    ) -> Result<ComparisonResult> {
        Ok(ComparisonResult {
            current_election: current.election_id.clone(),
            baseline_election: baseline.election_id.clone(),
            vote_count_delta: current.total_votes as i64 - baseline.total_votes as i64,
            anomaly_count_delta: current.anomalies.len() as i64
                - baseline.anomalies.len() as i64,
            turnout_delta: current.statistics.turnout_percentage
                - baseline.statistics.turnout_percentage,
            integrity_delta: current.integrity.integrity_percentage
                - baseline.integrity.integrity_percentage,
            timestamp: common::utils::current_timestamp(),
        })
    }

    /// Get turnout statistics by jurisdiction
    pub fn get_turnout_by_jurisdiction(
        &self,
        votes: &[VoteData],
    ) -> Result<HashMap<String, TurnoutData>> {
        let mut turnout_map = HashMap::new();

        for vote in votes {
            if let Some(jurisdiction) = &vote.jurisdiction {
                let entry = turnout_map
                    .entry(jurisdiction.clone())
                    .or_insert(TurnoutData {
                        jurisdiction: jurisdiction.clone(),
                        vote_count: 0,
                        registered_voters: vote.registered_voters_in_jurisdiction.unwrap_or(0),
                        turnout_percentage: 0.0,
                    });
                entry.vote_count += 1;
            }
        }

        for turnout in turnout_map.values_mut() {
            if turnout.registered_voters > 0 {
                turnout.turnout_percentage =
                    (turnout.vote_count as f64 / turnout.registered_voters as f64) * 100.0;
            }
        }

        Ok(turnout_map)
    }
}

impl Default for AnalyticsEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Vote data structure for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteData {
    /// Transaction ID
    pub tx_id: String,

    /// Timestamp when vote was cast
    pub timestamp: u64,

    /// Block height where vote was included
    pub block_height: u64,

    /// Jurisdiction or location
    pub jurisdiction: Option<String>,

    /// Geographic coordinates (if available)
    pub coordinates: Option<(f64, f64)>,

    /// Voter ID hash (for duplicate detection, not actual ID)
    pub voter_hash: Option<String>,

    /// Election ID
    pub election_id: String,

    /// Registered voters in this jurisdiction
    pub registered_voters_in_jurisdiction: Option<u64>,
}

/// Result of comprehensive analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Election being analyzed
    pub election_id: String,

    /// Total votes analyzed
    pub total_votes: usize,

    /// Detected anomalies
    pub anomalies: Vec<AnomalyReport>,

    /// Comprehensive statistics
    pub statistics: VoteStatistics,

    /// Integrity check results
    pub integrity: IntegrityCheck,

    /// Timestamp of analysis
    pub timestamp: u64,
}

/// Result of comparing two elections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    /// Current election ID
    pub current_election: String,

    /// Baseline election ID
    pub baseline_election: String,

    /// Change in vote count
    pub vote_count_delta: i64,

    /// Change in anomaly count
    pub anomaly_count_delta: i64,

    /// Change in turnout percentage
    pub turnout_delta: f64,

    /// Change in integrity percentage
    pub integrity_delta: f64,

    /// Timestamp of comparison
    pub timestamp: u64,
}

/// Turnout data for a jurisdiction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnoutData {
    /// Jurisdiction name
    pub jurisdiction: String,

    /// Number of votes cast
    pub vote_count: u64,

    /// Number of registered voters
    pub registered_voters: u64,

    /// Turnout percentage
    pub turnout_percentage: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analytics_config_default() {
        let config = AnalyticsConfig::default();
        assert_eq!(config.anomaly_threshold, 2.5);
        assert!(config.enable_benford_analysis);
        assert!(config.enable_geographic_analysis);
        assert!(config.enable_temporal_analysis);
        assert_eq!(config.min_sample_size, 30);
        assert_eq!(config.confidence_level, 0.95);
    }

    #[test]
    fn test_analytics_engine_creation() {
        let engine = AnalyticsEngine::new();
        assert_eq!(engine.config.anomaly_threshold, 2.5);
        assert_eq!(engine.config.min_sample_size, 30);
    }

    #[test]
    fn test_analytics_engine_with_custom_config() {
        let config = AnalyticsConfig {
            anomaly_threshold: 3.0,
            min_sample_size: 50,
            ..Default::default()
        };
        let engine = AnalyticsEngine::with_config(config.clone());
        assert_eq!(engine.config.anomaly_threshold, 3.0);
        assert_eq!(engine.config.min_sample_size, 50);
    }

    #[test]
    fn test_turnout_data() {
        let turnout = TurnoutData {
            jurisdiction: "County A".to_string(),
            vote_count: 750,
            registered_voters: 1000,
            turnout_percentage: 75.0,
        };
        assert_eq!(turnout.jurisdiction, "County A");
        assert_eq!(turnout.vote_count, 750);
        assert_eq!(turnout.turnout_percentage, 75.0);
    }

    #[test]
    fn test_comparison_result() {
        let comparison = ComparisonResult {
            current_election: "election_2024".to_string(),
            baseline_election: "election_2020".to_string(),
            vote_count_delta: 5000,
            anomaly_count_delta: -2,
            turnout_delta: 3.5,
            integrity_delta: 0.2,
            timestamp: 1000000,
        };
        assert_eq!(comparison.vote_count_delta, 5000);
        assert_eq!(comparison.anomaly_count_delta, -2);
    }
}

//! Reporting and output formatting for analytics results
//!
//! This module provides comprehensive reporting capabilities including:
//! - Multiple output formats (JSON, CSV, HTML, Markdown, Plain text)
//! - Summary reports
//! - Detailed audit reports
//! - Executive summaries
//! - Visualization data export

use common::{Result, VotingError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Report generator for analytics results
#[derive(Debug, Clone)]
pub struct ReportGenerator {
    /// Include timestamp in reports
    include_timestamp: bool,

    /// Include metadata
    include_metadata: bool,

    /// Maximum detail level
    detail_level: DetailLevel,
}

impl ReportGenerator {
    /// Create new report generator with default settings
    pub fn new() -> Self {
        Self {
            include_timestamp: true,
            include_metadata: true,
            detail_level: DetailLevel::Full,
        }
    }

    /// Create generator with custom settings
    pub fn with_settings(
        include_timestamp: bool,
        include_metadata: bool,
        detail_level: DetailLevel,
    ) -> Self {
        Self {
            include_timestamp,
            include_metadata,
            detail_level,
        }
    }

    /// Generate comprehensive report
    pub fn generate_report(
        &self,
        data: &ReportData,
        format: ReportFormat,
    ) -> Result<String> {
        match format {
            ReportFormat::Json => self.generate_json_report(data),
            ReportFormat::Csv => self.generate_csv_report(data),
            ReportFormat::Html => self.generate_html_report(data),
            ReportFormat::Markdown => self.generate_markdown_report(data),
            ReportFormat::PlainText => self.generate_plaintext_report(data),
        }
    }

    /// Generate executive summary
    pub fn generate_executive_summary(&self, data: &ReportData) -> Result<String> {
        let mut summary = String::new();

        summary.push_str("EXECUTIVE SUMMARY\n");
        summary.push_str("=================\n\n");

        if self.include_timestamp {
            summary.push_str(&format!("Generated: {}\n", self.format_timestamp(data.timestamp)));
            summary.push_str(&format!("Election: {}\n\n", data.election_id));
        }

        summary.push_str(&format!("Total Votes Analyzed: {}\n", data.total_votes));
        summary.push_str(&format!("Overall Risk Level: {}\n", self.format_risk_level(data.risk_score)));
        summary.push_str(&format!("Anomalies Detected: {}\n", data.anomaly_count));
        summary.push_str(&format!("Patterns Identified: {}\n\n", data.pattern_count));

        if data.high_risk_findings > 0 {
            summary.push_str("HIGH PRIORITY ISSUES:\n");
            summary.push_str(&format!("  - {} high-risk findings require immediate attention\n", data.high_risk_findings));
        }

        if data.medium_risk_findings > 0 {
            summary.push_str(&format!("  - {} medium-risk findings for review\n", data.medium_risk_findings));
        }

        summary.push_str("\nKEY METRICS:\n");
        summary.push_str(&format!("  Turnout Rate: {:.2}%\n", data.turnout_percentage));
        summary.push_str(&format!("  Integrity Score: {:.2}%\n", data.integrity_percentage));
        summary.push_str(&format!("  Data Quality: {:.2}%\n", data.data_quality_score));

        if !data.recommendations.is_empty() {
            summary.push_str("\nRECOMMENDATIONS:\n");
            for (i, rec) in data.recommendations.iter().enumerate().take(5) {
                summary.push_str(&format!("  {}. {}\n", i + 1, rec));
            }
        }

        Ok(summary)
    }

    /// Generate JSON report
    fn generate_json_report(&self, data: &ReportData) -> Result<String> {
        serde_json::to_string_pretty(data)
            .map_err(|e| VotingError::SerializationError(e.to_string()))
    }

    /// Generate CSV report
    fn generate_csv_report(&self, data: &ReportData) -> Result<String> {
        let mut csv = String::new();

        csv.push_str("Category,Metric,Value\n");
        csv.push_str(&format!("Election,ID,{}\n", data.election_id));
        csv.push_str(&format!("Election,Total Votes,{}\n", data.total_votes));
        csv.push_str(&format!("Risk,Risk Score,{:.2}\n", data.risk_score));
        csv.push_str(&format!("Risk,High Risk Findings,{}\n", data.high_risk_findings));
        csv.push_str(&format!("Risk,Medium Risk Findings,{}\n", data.medium_risk_findings));
        csv.push_str(&format!("Risk,Low Risk Findings,{}\n", data.low_risk_findings));
        csv.push_str(&format!("Analysis,Anomalies Detected,{}\n", data.anomaly_count));
        csv.push_str(&format!("Analysis,Patterns Identified,{}\n", data.pattern_count));
        csv.push_str(&format!("Metrics,Turnout Percentage,{:.2}\n", data.turnout_percentage));
        csv.push_str(&format!("Metrics,Integrity Percentage,{:.2}\n", data.integrity_percentage));
        csv.push_str(&format!("Metrics,Data Quality Score,{:.2}\n", data.data_quality_score));

        if !data.findings.is_empty() {
            csv.push_str("\nFinding Type,Severity,Description,Count\n");
            for finding in &data.findings {
                csv.push_str(&format!(
                    "{},{},{},{}\n",
                    finding.finding_type,
                    finding.severity,
                    self.escape_csv(&finding.description),
                    finding.count
                ));
            }
        }

        Ok(csv)
    }

    /// Generate HTML report
    fn generate_html_report(&self, data: &ReportData) -> Result<String> {
        let mut html = String::new();

        html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
        html.push_str("<meta charset=\"UTF-8\">\n");
        html.push_str("<title>Election Analytics Report</title>\n");
        html.push_str("<style>\n");
        html.push_str(self.get_html_styles());
        html.push_str("</style>\n</head>\n<body>\n");

        html.push_str("<div class=\"container\">\n");
        html.push_str("<h1>Election Analytics Report</h1>\n");

        if self.include_timestamp {
            html.push_str(&format!(
                "<p class=\"metadata\">Generated: {} | Election: {}</p>\n",
                self.format_timestamp(data.timestamp),
                data.election_id
            ));
        }

        html.push_str("<div class=\"summary-cards\">\n");
        html.push_str(&self.generate_summary_card("Total Votes", &data.total_votes.to_string()));
        html.push_str(&self.generate_summary_card("Risk Score", &format!("{:.1}/10", data.risk_score)));
        html.push_str(&self.generate_summary_card("Anomalies", &data.anomaly_count.to_string()));
        html.push_str(&self.generate_summary_card("Patterns", &data.pattern_count.to_string()));
        html.push_str("</div>\n");

        if data.high_risk_findings > 0 || data.medium_risk_findings > 0 {
            html.push_str("<div class=\"alert alert-warning\">\n");
            html.push_str("<h2>Attention Required</h2>\n");
            if data.high_risk_findings > 0 {
                html.push_str(&format!("<p><strong>{}</strong> high-risk findings require immediate investigation</p>\n", data.high_risk_findings));
            }
            if data.medium_risk_findings > 0 {
                html.push_str(&format!("<p><strong>{}</strong> medium-risk findings for review</p>\n", data.medium_risk_findings));
            }
            html.push_str("</div>\n");
        }

        html.push_str("<h2>Key Metrics</h2>\n");
        html.push_str("<table>\n");
        html.push_str("<tr><th>Metric</th><th>Value</th></tr>\n");
        html.push_str(&format!("<tr><td>Turnout Rate</td><td>{:.2}%</td></tr>\n", data.turnout_percentage));
        html.push_str(&format!("<tr><td>Integrity Score</td><td>{:.2}%</td></tr>\n", data.integrity_percentage));
        html.push_str(&format!("<tr><td>Data Quality</td><td>{:.2}%</td></tr>\n", data.data_quality_score));
        html.push_str("</table>\n");

        if !data.findings.is_empty() {
            html.push_str("<h2>Detailed Findings</h2>\n");
            html.push_str("<table>\n");
            html.push_str("<tr><th>Type</th><th>Severity</th><th>Description</th><th>Count</th></tr>\n");
            for finding in &data.findings {
                let severity_class = match finding.severity.as_str() {
                    "High" => "severity-high",
                    "Medium" => "severity-medium",
                    _ => "severity-low",
                };
                html.push_str(&format!(
                    "<tr><td>{}</td><td class=\"{}\"><span class=\"badge\">{}</span></td><td>{}</td><td>{}</td></tr>\n",
                    finding.finding_type,
                    severity_class,
                    finding.severity,
                    finding.description,
                    finding.count
                ));
            }
            html.push_str("</table>\n");
        }

        if !data.recommendations.is_empty() {
            html.push_str("<h2>Recommendations</h2>\n<ul>\n");
            for rec in &data.recommendations {
                html.push_str(&format!("<li>{}</li>\n", rec));
            }
            html.push_str("</ul>\n");
        }

        html.push_str("</div>\n</body>\n</html>");

        Ok(html)
    }

    /// Generate Markdown report
    fn generate_markdown_report(&self, data: &ReportData) -> Result<String> {
        let mut md = String::new();

        md.push_str("# Election Analytics Report\n\n");

        if self.include_timestamp {
            md.push_str(&format!("**Generated:** {}\n", self.format_timestamp(data.timestamp)));
            md.push_str(&format!("**Election:** {}\n\n", data.election_id));
        }

        md.push_str("## Summary\n\n");
        md.push_str(&format!("- **Total Votes:** {}\n", data.total_votes));
        md.push_str(&format!("- **Risk Score:** {:.1}/10\n", data.risk_score));
        md.push_str(&format!("- **Anomalies Detected:** {}\n", data.anomaly_count));
        md.push_str(&format!("- **Patterns Identified:** {}\n\n", data.pattern_count));

        if data.high_risk_findings > 0 || data.medium_risk_findings > 0 {
            md.push_str("## ⚠️ Attention Required\n\n");
            if data.high_risk_findings > 0 {
                md.push_str(&format!("- **{}** high-risk findings require immediate investigation\n", data.high_risk_findings));
            }
            if data.medium_risk_findings > 0 {
                md.push_str(&format!("- **{}** medium-risk findings for review\n", data.medium_risk_findings));
            }
            md.push_str("\n");
        }

        md.push_str("## Key Metrics\n\n");
        md.push_str("| Metric | Value |\n");
        md.push_str("|--------|-------|\n");
        md.push_str(&format!("| Turnout Rate | {:.2}% |\n", data.turnout_percentage));
        md.push_str(&format!("| Integrity Score | {:.2}% |\n", data.integrity_percentage));
        md.push_str(&format!("| Data Quality | {:.2}% |\n\n", data.data_quality_score));

        if !data.findings.is_empty() {
            md.push_str("## Detailed Findings\n\n");
            md.push_str("| Type | Severity | Description | Count |\n");
            md.push_str("|------|----------|-------------|-------|\n");
            for finding in &data.findings {
                md.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    finding.finding_type,
                    finding.severity,
                    finding.description,
                    finding.count
                ));
            }
            md.push_str("\n");
        }

        if !data.recommendations.is_empty() {
            md.push_str("## Recommendations\n\n");
            for (i, rec) in data.recommendations.iter().enumerate() {
                md.push_str(&format!("{}. {}\n", i + 1, rec));
            }
            md.push_str("\n");
        }

        if self.include_metadata {
            md.push_str("---\n\n");
            md.push_str("*Report generated by Blockchain Voting Analytics System*\n");
        }

        Ok(md)
    }

    /// Generate plain text report
    fn generate_plaintext_report(&self, data: &ReportData) -> Result<String> {
        let mut text = String::new();

        text.push_str("=====================================\n");
        text.push_str("   ELECTION ANALYTICS REPORT\n");
        text.push_str("=====================================\n\n");

        if self.include_timestamp {
            text.push_str(&format!("Generated: {}\n", self.format_timestamp(data.timestamp)));
            text.push_str(&format!("Election: {}\n\n", data.election_id));
        }

        text.push_str("SUMMARY\n");
        text.push_str("-------\n");
        text.push_str(&format!("Total Votes Analyzed:     {}\n", data.total_votes));
        text.push_str(&format!("Overall Risk Score:       {:.1}/10\n", data.risk_score));
        text.push_str(&format!("Anomalies Detected:       {}\n", data.anomaly_count));
        text.push_str(&format!("Patterns Identified:      {}\n\n", data.pattern_count));

        text.push_str("RISK ASSESSMENT\n");
        text.push_str("---------------\n");
        text.push_str(&format!("High Risk Findings:       {}\n", data.high_risk_findings));
        text.push_str(&format!("Medium Risk Findings:     {}\n", data.medium_risk_findings));
        text.push_str(&format!("Low Risk Findings:        {}\n\n", data.low_risk_findings));

        text.push_str("KEY METRICS\n");
        text.push_str("-----------\n");
        text.push_str(&format!("Turnout Rate:             {:.2}%\n", data.turnout_percentage));
        text.push_str(&format!("Integrity Score:          {:.2}%\n", data.integrity_percentage));
        text.push_str(&format!("Data Quality Score:       {:.2}%\n\n", data.data_quality_score));

        if !data.findings.is_empty() {
            text.push_str("DETAILED FINDINGS\n");
            text.push_str("-----------------\n");
            for (i, finding) in data.findings.iter().enumerate() {
                text.push_str(&format!("{}. {} [{}]\n", i + 1, finding.finding_type, finding.severity));
                text.push_str(&format!("   {}\n", finding.description));
                text.push_str(&format!("   Count: {}\n\n", finding.count));
            }
        }

        if !data.recommendations.is_empty() {
            text.push_str("RECOMMENDATIONS\n");
            text.push_str("---------------\n");
            for (i, rec) in data.recommendations.iter().enumerate() {
                text.push_str(&format!("{}. {}\n", i + 1, rec));
            }
            text.push_str("\n");
        }

        text.push_str("=====================================\n");
        text.push_str("         END OF REPORT\n");
        text.push_str("=====================================\n");

        Ok(text)
    }

    /// Generate comparison report between two elections
    pub fn generate_comparison_report(
        &self,
        current: &ReportData,
        baseline: &ReportData,
        format: ReportFormat,
    ) -> Result<String> {
        let comparison = ComparisonData {
            current_election: current.election_id.clone(),
            baseline_election: baseline.election_id.clone(),
            vote_count_change: current.total_votes as i64 - baseline.total_votes as i64,
            vote_count_change_percent: self.calculate_percent_change(
                baseline.total_votes as f64,
                current.total_votes as f64,
            ),
            risk_score_change: current.risk_score - baseline.risk_score,
            anomaly_count_change: current.anomaly_count as i64 - baseline.anomaly_count as i64,
            turnout_change: current.turnout_percentage - baseline.turnout_percentage,
            integrity_change: current.integrity_percentage - baseline.integrity_percentage,
        };

        match format {
            ReportFormat::Json => serde_json::to_string_pretty(&comparison)
                .map_err(|e| VotingError::SerializationError(e.to_string())),
            ReportFormat::Markdown => self.generate_comparison_markdown(&comparison),
            ReportFormat::PlainText => self.generate_comparison_text(&comparison),
            _ => self.generate_comparison_text(&comparison),
        }
    }

    /// Generate comparison in markdown format
    fn generate_comparison_markdown(&self, data: &ComparisonData) -> Result<String> {
        let mut md = String::new();

        md.push_str("# Election Comparison Report\n\n");
        md.push_str(&format!("**Current:** {}\n", data.current_election));
        md.push_str(&format!("**Baseline:** {}\n\n", data.baseline_election));

        md.push_str("## Changes\n\n");
        md.push_str("| Metric | Change | Percent |\n");
        md.push_str("|--------|--------|----------|\n");
        md.push_str(&format!(
            "| Vote Count | {} | {:.2}% |\n",
            self.format_change(data.vote_count_change),
            data.vote_count_change_percent
        ));
        md.push_str(&format!(
            "| Risk Score | {} | - |\n",
            self.format_change_f64(data.risk_score_change)
        ));
        md.push_str(&format!(
            "| Anomalies | {} | - |\n",
            self.format_change(data.anomaly_count_change)
        ));
        md.push_str(&format!(
            "| Turnout | {:.2}% | - |\n",
            data.turnout_change
        ));
        md.push_str(&format!(
            "| Integrity | {:.2}% | - |\n",
            data.integrity_change
        ));

        Ok(md)
    }

    /// Generate comparison in text format
    fn generate_comparison_text(&self, data: &ComparisonData) -> Result<String> {
        let mut text = String::new();

        text.push_str("ELECTION COMPARISON REPORT\n");
        text.push_str("==========================\n\n");
        text.push_str(&format!("Current:  {}\n", data.current_election));
        text.push_str(&format!("Baseline: {}\n\n", data.baseline_election));

        text.push_str("CHANGES\n");
        text.push_str("-------\n");
        text.push_str(&format!(
            "Vote Count:   {} ({:.2}%)\n",
            self.format_change(data.vote_count_change),
            data.vote_count_change_percent
        ));
        text.push_str(&format!(
            "Risk Score:   {}\n",
            self.format_change_f64(data.risk_score_change)
        ));
        text.push_str(&format!(
            "Anomalies:    {}\n",
            self.format_change(data.anomaly_count_change)
        ));
        text.push_str(&format!("Turnout:      {:.2}%\n", data.turnout_change));
        text.push_str(&format!("Integrity:    {:.2}%\n", data.integrity_change));

        Ok(text)
    }

    /// Generate HTML summary card
    fn generate_summary_card(&self, label: &str, value: &str) -> String {
        format!(
            "<div class=\"card\"><div class=\"card-label\">{}</div><div class=\"card-value\">{}</div></div>\n",
            label, value
        )
    }

    /// Get HTML styles
    fn get_html_styles(&self) -> &str {
        r#"
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 0; padding: 20px; background: #f5f5f5; }
.container { max-width: 1200px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
h1 { color: #2c3e50; margin-bottom: 10px; }
h2 { color: #34495e; margin-top: 30px; border-bottom: 2px solid #3498db; padding-bottom: 10px; }
.metadata { color: #7f8c8d; font-size: 14px; margin-bottom: 30px; }
.summary-cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 20px; margin: 30px 0; }
.card { background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: white; padding: 20px; border-radius: 8px; text-align: center; }
.card-label { font-size: 14px; opacity: 0.9; margin-bottom: 10px; }
.card-value { font-size: 32px; font-weight: bold; }
.alert { padding: 20px; border-radius: 8px; margin: 20px 0; }
.alert-warning { background: #fff3cd; border-left: 4px solid #ffc107; }
table { width: 100%; border-collapse: collapse; margin: 20px 0; }
th, td { padding: 12px; text-align: left; border-bottom: 1px solid #e0e0e0; }
th { background: #f8f9fa; font-weight: 600; color: #495057; }
tr:hover { background: #f8f9fa; }
.badge { padding: 4px 12px; border-radius: 12px; font-size: 12px; font-weight: 600; }
.severity-high .badge { background: #ffebee; color: #c62828; }
.severity-medium .badge { background: #fff3e0; color: #ef6c00; }
.severity-low .badge { background: #e8f5e9; color: #2e7d32; }
ul { line-height: 1.8; }
"#
    }

    /// Format timestamp
    fn format_timestamp(&self, timestamp: u64) -> String {
        format!("{}", timestamp)
    }

    /// Format risk level
    fn format_risk_level(&self, score: f64) -> String {
        if score >= 7.0 {
            format!("HIGH ({:.1})", score)
        } else if score >= 4.0 {
            format!("MEDIUM ({:.1})", score)
        } else {
            format!("LOW ({:.1})", score)
        }
    }

    /// Escape CSV values
    fn escape_csv(&self, value: &str) -> String {
        if value.contains(',') || value.contains('"') || value.contains('\n') {
            format!("\"{}\"", value.replace('"', "\"\""))
        } else {
            value.to_string()
        }
    }

    /// Calculate percent change
    fn calculate_percent_change(&self, baseline: f64, current: f64) -> f64 {
        if baseline == 0.0 {
            return 0.0;
        }
        ((current - baseline) / baseline) * 100.0
    }

    /// Format change value
    fn format_change(&self, value: i64) -> String {
        if value > 0 {
            format!("+{}", value)
        } else {
            value.to_string()
        }
    }

    /// Format change value (f64)
    fn format_change_f64(&self, value: f64) -> String {
        if value > 0.0 {
            format!("+{:.2}", value)
        } else {
            format!("{:.2}", value)
        }
    }
}

impl Default for ReportGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Report data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportData {
    pub election_id: String,
    pub timestamp: u64,
    pub total_votes: usize,
    pub risk_score: f64,
    pub high_risk_findings: usize,
    pub medium_risk_findings: usize,
    pub low_risk_findings: usize,
    pub anomaly_count: usize,
    pub pattern_count: usize,
    pub turnout_percentage: f64,
    pub integrity_percentage: f64,
    pub data_quality_score: f64,
    pub findings: Vec<FindingSummary>,
    pub recommendations: Vec<String>,
}

/// Finding summary for reports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingSummary {
    pub finding_type: String,
    pub severity: String,
    pub description: String,
    pub count: usize,
}

/// Comparison data between elections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonData {
    pub current_election: String,
    pub baseline_election: String,
    pub vote_count_change: i64,
    pub vote_count_change_percent: f64,
    pub risk_score_change: f64,
    pub anomaly_count_change: i64,
    pub turnout_change: f64,
    pub integrity_change: f64,
}

/// Report output format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportFormat {
    Json,
    Csv,
    Html,
    Markdown,
    PlainText,
}

/// Report detail level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetailLevel {
    Summary,
    Standard,
    Full,
    Verbose,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_report_data() -> ReportData {
        ReportData {
            election_id: "election_2024".to_string(),
            timestamp: 1000000,
            total_votes: 10000,
            risk_score: 3.5,
            high_risk_findings: 2,
            medium_risk_findings: 5,
            low_risk_findings: 3,
            anomaly_count: 10,
            pattern_count: 4,
            turnout_percentage: 68.5,
            integrity_percentage: 99.2,
            data_quality_score: 95.8,
            findings: vec![
                FindingSummary {
                    finding_type: "Duplicate Vote".to_string(),
                    severity: "High".to_string(),
                    description: "Duplicate voter detected".to_string(),
                    count: 2,
                },
                FindingSummary {
                    finding_type: "Timing Anomaly".to_string(),
                    severity: "Medium".to_string(),
                    description: "Suspicious timing pattern".to_string(),
                    count: 5,
                },
            ],
            recommendations: vec![
                "Review duplicate voter records".to_string(),
                "Investigate timing anomalies".to_string(),
            ],
        }
    }

    #[test]
    fn test_report_generator_creation() {
        let generator = ReportGenerator::new();
        assert!(generator.include_timestamp);
        assert!(generator.include_metadata);
    }

    #[test]
    fn test_generate_json_report() {
        let generator = ReportGenerator::new();
        let data = create_test_report_data();
        let result = generator.generate_report(&data, ReportFormat::Json);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("election_2024"));
    }

    #[test]
    fn test_generate_csv_report() {
        let generator = ReportGenerator::new();
        let data = create_test_report_data();
        let result = generator.generate_report(&data, ReportFormat::Csv);
        assert!(result.is_ok());
        let csv = result.unwrap();
        assert!(csv.contains("Category,Metric,Value"));
        assert!(csv.contains("10000"));
    }

    #[test]
    fn test_generate_markdown_report() {
        let generator = ReportGenerator::new();
        let data = create_test_report_data();
        let result = generator.generate_report(&data, ReportFormat::Markdown);
        assert!(result.is_ok());
        let md = result.unwrap();
        assert!(md.contains("# Election Analytics Report"));
        assert!(md.contains("election_2024"));
    }

    #[test]
    fn test_generate_plaintext_report() {
        let generator = ReportGenerator::new();
        let data = create_test_report_data();
        let result = generator.generate_report(&data, ReportFormat::PlainText);
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("ELECTION ANALYTICS REPORT"));
    }

    #[test]
    fn test_generate_html_report() {
        let generator = ReportGenerator::new();
        let data = create_test_report_data();
        let result = generator.generate_report(&data, ReportFormat::Html);
        assert!(result.is_ok());
        let html = result.unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Election Analytics Report"));
    }

    #[test]
    fn test_executive_summary() {
        let generator = ReportGenerator::new();
        let data = create_test_report_data();
        let result = generator.generate_executive_summary(&data);
        assert!(result.is_ok());
        let summary = result.unwrap();
        assert!(summary.contains("EXECUTIVE SUMMARY"));
        assert!(summary.contains("10000"));
    }

    #[test]
    fn test_comparison_report() {
        let generator = ReportGenerator::new();
        let current = create_test_report_data();
        let mut baseline = create_test_report_data();
        baseline.total_votes = 9000;
        baseline.risk_score = 4.0;

        let result = generator.generate_comparison_report(&current, &baseline, ReportFormat::PlainText);
        assert!(result.is_ok());
    }

    #[test]
    fn test_format_change() {
        let generator = ReportGenerator::new();
        assert_eq!(generator.format_change(100), "+100");
        assert_eq!(generator.format_change(-50), "-50");
        assert_eq!(generator.format_change(0), "0");
    }

    #[test]
    fn test_calculate_percent_change() {
        let generator = ReportGenerator::new();
        assert_eq!(generator.calculate_percent_change(100.0, 150.0), 50.0);
        assert_eq!(generator.calculate_percent_change(100.0, 50.0), -50.0);
        assert_eq!(generator.calculate_percent_change(0.0, 100.0), 0.0);
    }

    #[test]
    fn test_escape_csv() {
        let generator = ReportGenerator::new();
        assert_eq!(generator.escape_csv("simple"), "simple");
        assert_eq!(generator.escape_csv("has,comma"), "\"has,comma\"");
        assert_eq!(generator.escape_csv("has\"quote"), "\"has\"\"quote\"");
    }

    #[test]
    fn test_format_risk_level() {
        let generator = ReportGenerator::new();
        assert!(generator.format_risk_level(8.0).contains("HIGH"));
        assert!(generator.format_risk_level(5.0).contains("MEDIUM"));
        assert!(generator.format_risk_level(2.0).contains("LOW"));
    }

    #[test]
    fn test_custom_settings() {
        let generator = ReportGenerator::with_settings(false, false, DetailLevel::Summary);
        assert!(!generator.include_timestamp);
        assert!(!generator.include_metadata);
        assert_eq!(generator.detail_level, DetailLevel::Summary);
    }
}

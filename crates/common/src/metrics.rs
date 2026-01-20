use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec, IntCounter,
    IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry,
};
use std::sync::Arc;

/// Metrics collection for the voting system
#[derive(Clone)]
pub struct Metrics {
    registry: Arc<Registry>,

    // Blockchain metrics
    pub blockchain_height: IntGauge,
    pub total_transactions: IntCounter,
    pub total_votes: IntCounter,
    pub blocks_produced: IntCounter,
    pub blocks_validated: IntCounter,

    // Network metrics
    pub connected_peers: IntGauge,
    pub messages_sent: IntCounterVec,
    pub messages_received: IntCounterVec,
    pub network_errors: IntCounter,

    // Consensus metrics
    pub validator_active: Gauge,
    pub finalized_blocks: IntCounter,
    pub consensus_rounds: IntCounter,

    // Storage metrics
    pub database_size_bytes: IntGauge,
    pub cache_hits: IntCounter,
    pub cache_misses: IntCounter,

    // Performance metrics
    pub block_processing_time: Histogram,
    pub transaction_processing_time: Histogram,
    pub network_latency: HistogramVec,

    // RPC metrics
    pub rpc_requests_total: IntCounterVec,
    pub rpc_request_duration: HistogramVec,
    pub rpc_errors: IntCounterVec,

    // Vote metrics
    pub votes_cast: IntCounterVec,
    pub votes_verified: IntCounter,
    pub votes_rejected: IntCounter,

    // Election metrics
    pub active_elections: IntGauge,
    pub completed_elections: IntCounter,
}

impl Metrics {
    /// Create a new metrics collection
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Arc::new(Registry::new());

        // Blockchain metrics
        let blockchain_height = IntGauge::new("blockchain_height", "Current blockchain height")?;
        let total_transactions =
            IntCounter::new("total_transactions", "Total number of transactions")?;
        let total_votes = IntCounter::new("total_votes", "Total number of votes cast")?;
        let blocks_produced = IntCounter::new("blocks_produced", "Total blocks produced")?;
        let blocks_validated = IntCounter::new("blocks_validated", "Total blocks validated")?;

        // Network metrics
        let connected_peers = IntGauge::new("connected_peers", "Number of connected peers")?;
        let messages_sent = IntCounterVec::new(
            Opts::new("messages_sent", "Messages sent by type"),
            &["message_type"],
        )?;
        let messages_received = IntCounterVec::new(
            Opts::new("messages_received", "Messages received by type"),
            &["message_type"],
        )?;
        let network_errors = IntCounter::new("network_errors", "Total network errors")?;

        // Consensus metrics
        let validator_active = Gauge::new("validator_active", "Is this node an active validator")?;
        let finalized_blocks =
            IntCounter::new("finalized_blocks", "Total finalized blocks")?;
        let consensus_rounds =
            IntCounter::new("consensus_rounds", "Total consensus rounds completed")?;

        // Storage metrics
        let database_size_bytes =
            IntGauge::new("database_size_bytes", "Total database size in bytes")?;
        let cache_hits = IntCounter::new("cache_hits", "Cache hits")?;
        let cache_misses = IntCounter::new("cache_misses", "Cache misses")?;

        // Performance metrics
        let block_processing_time = Histogram::with_opts(
            HistogramOpts::new("block_processing_time", "Block processing time in seconds")
                .buckets(vec![0.001, 0.01, 0.1, 0.5, 1.0, 5.0, 10.0]),
        )?;
        let transaction_processing_time = Histogram::with_opts(
            HistogramOpts::new(
                "transaction_processing_time",
                "Transaction processing time in seconds",
            )
            .buckets(vec![0.0001, 0.001, 0.01, 0.1, 0.5, 1.0]),
        )?;
        let network_latency = HistogramVec::new(
            HistogramOpts::new("network_latency", "Network latency by peer")
                .buckets(vec![0.001, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]),
            &["peer_id"],
        )?;

        // RPC metrics
        let rpc_requests_total = IntCounterVec::new(
            Opts::new("rpc_requests_total", "Total RPC requests by method"),
            &["method"],
        )?;
        let rpc_request_duration = HistogramVec::new(
            HistogramOpts::new("rpc_request_duration", "RPC request duration by method")
                .buckets(vec![0.001, 0.01, 0.1, 0.5, 1.0, 5.0]),
            &["method"],
        )?;
        let rpc_errors = IntCounterVec::new(
            Opts::new("rpc_errors", "RPC errors by method"),
            &["method"],
        )?;

        // Vote metrics
        let votes_cast = IntCounterVec::new(
            Opts::new("votes_cast", "Votes cast by election"),
            &["election_id"],
        )?;
        let votes_verified = IntCounter::new("votes_verified", "Total votes verified")?;
        let votes_rejected = IntCounter::new("votes_rejected", "Total votes rejected")?;

        // Election metrics
        let active_elections =
            IntGauge::new("active_elections", "Number of active elections")?;
        let completed_elections =
            IntCounter::new("completed_elections", "Total completed elections")?;

        // Register all metrics
        registry.register(Box::new(blockchain_height.clone()))?;
        registry.register(Box::new(total_transactions.clone()))?;
        registry.register(Box::new(total_votes.clone()))?;
        registry.register(Box::new(blocks_produced.clone()))?;
        registry.register(Box::new(blocks_validated.clone()))?;
        registry.register(Box::new(connected_peers.clone()))?;
        registry.register(Box::new(messages_sent.clone()))?;
        registry.register(Box::new(messages_received.clone()))?;
        registry.register(Box::new(network_errors.clone()))?;
        registry.register(Box::new(validator_active.clone()))?;
        registry.register(Box::new(finalized_blocks.clone()))?;
        registry.register(Box::new(consensus_rounds.clone()))?;
        registry.register(Box::new(database_size_bytes.clone()))?;
        registry.register(Box::new(cache_hits.clone()))?;
        registry.register(Box::new(cache_misses.clone()))?;
        registry.register(Box::new(block_processing_time.clone()))?;
        registry.register(Box::new(transaction_processing_time.clone()))?;
        registry.register(Box::new(network_latency.clone()))?;
        registry.register(Box::new(rpc_requests_total.clone()))?;
        registry.register(Box::new(rpc_request_duration.clone()))?;
        registry.register(Box::new(rpc_errors.clone()))?;
        registry.register(Box::new(votes_cast.clone()))?;
        registry.register(Box::new(votes_verified.clone()))?;
        registry.register(Box::new(votes_rejected.clone()))?;
        registry.register(Box::new(active_elections.clone()))?;
        registry.register(Box::new(completed_elections.clone()))?;

        Ok(Self {
            registry,
            blockchain_height,
            total_transactions,
            total_votes,
            blocks_produced,
            blocks_validated,
            connected_peers,
            messages_sent,
            messages_received,
            network_errors,
            validator_active,
            finalized_blocks,
            consensus_rounds,
            database_size_bytes,
            cache_hits,
            cache_misses,
            block_processing_time,
            transaction_processing_time,
            network_latency,
            rpc_requests_total,
            rpc_request_duration,
            rpc_errors,
            votes_cast,
            votes_verified,
            votes_rejected,
            active_elections,
            completed_elections,
        })
    }

    /// Get the registry for exposing metrics via HTTP
    pub fn registry(&self) -> Arc<Registry> {
        self.registry.clone()
    }

    /// Gather all metrics in Prometheus text format
    pub fn gather(&self) -> String {
        use prometheus::Encoder;
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new().expect("Failed to create metrics")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new().unwrap();
        assert_eq!(metrics.blockchain_height.get(), 0);
        assert_eq!(metrics.total_transactions.get(), 0);
    }

    #[test]
    fn test_metrics_increment() {
        let metrics = Metrics::new().unwrap();
        metrics.total_transactions.inc();
        assert_eq!(metrics.total_transactions.get(), 1);
    }

    #[test]
    fn test_metrics_gauge() {
        let metrics = Metrics::new().unwrap();
        metrics.blockchain_height.set(100);
        assert_eq!(metrics.blockchain_height.get(), 100);
    }

    #[test]
    fn test_metrics_gather() {
        let metrics = Metrics::new().unwrap();
        metrics.total_votes.inc();
        let output = metrics.gather();
        assert!(output.contains("total_votes"));
    }
}

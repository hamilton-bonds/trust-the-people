use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use analytics::{
    BenfordAnalyzer, EntropyAnalyzer, FraudDetector, PatternDetector, StatisticalAnalyzer,
    CorrelationAnalyzer, VoteData, VotePattern, VoteRecord,
};

fn create_vote_data(count: usize) -> Vec<VoteData> {
    (0..count)
        .map(|i| VoteData {
            tx_id: format!("tx_{}", i),
            timestamp: 1000000 + (i as u64 * 1000),
            block_height: (i / 10) as u64,
            jurisdiction: Some(format!("District_{}", i % 10)),
            coordinates: Some((40.7128 + (i as f64 * 0.001), -74.0060 + (i as f64 * 0.001))),
            voter_hash: Some(format!("voter_{}", i)),
            election_id: "election_2024".to_string(),
            registered_voters_in_jurisdiction: Some(10000),
        })
        .collect()
}

fn create_vote_patterns(count: usize) -> Vec<VotePattern> {
    (0..count)
        .map(|i| VotePattern {
            tx_id: format!("tx_{}", i),
            timestamp: 1000000 + (i as u64 * 100),
            location: Some(format!("Location_{}", i % 5)),
            voter_hash: Some(format!("voter_{}", i)),
        })
        .collect()
}

fn create_vote_records(count: usize) -> Vec<VoteRecord> {
    (0..count)
        .map(|i| VoteRecord {
            tx_id: format!("tx_{}", i),
            timestamp: 1000000 + (i as u64 * 500),
            block_height: (i / 10) as u64,
            voter_hash: Some(format!("voter_{}", i)),
            location: Some(format!("Location_{}", i % 8)),
            coordinates: Some((40.7128 + (i as f64 * 0.01), -74.0060 + (i as f64 * 0.01))),
            registered_voters: Some(5000),
        })
        .collect()
}

fn create_benford_data(count: usize) -> Vec<u64> {
    (1..=count)
        .map(|i| {
            let digit = (i % 9) + 1;
            digit as u64 * 10u64.pow((i % 4) as u32)
        })
        .collect()
}

fn bench_benford_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("benford_analysis");
    
    for count in [100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            count,
            |b, &count| {
                let analyzer = BenfordAnalyzer::new();
                let data = create_benford_data(count);
                
                b.iter(|| {
                    let result = analyzer.analyze(&data);
                    black_box(result);
                });
            },
        );
    }
    group.finish();
}

fn bench_entropy_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("entropy_analysis");
    
    for count in [100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            count,
            |b, &count| {
                let analyzer = EntropyAnalyzer::new();
                let data: Vec<u64> = (0..*count).map(|i| i as u64 % 100).collect();
                
                b.iter(|| {
                    let result = analyzer.calculate_shannon_entropy(&data);
                    black_box(result);
                });
            },
        );
    }
    group.finish();
}

fn bench_temporal_entropy(c: &mut Criterion) {
    let analyzer = EntropyAnalyzer::new();
    let timestamps: Vec<u64> = (0..1000).map(|i| 1000000 + (i * 1000)).collect();
    
    c.bench_function("temporal_entropy_1000", |b| {
        b.iter(|| {
            let result = analyzer.analyze_temporal_entropy(&timestamps, 60000);
            black_box(result);
        });
    });
}

fn bench_geographic_entropy(c: &mut Criterion) {
    let analyzer = EntropyAnalyzer::new();
    let locations: Vec<String> = (0..1000)
        .map(|i| format!("Location_{}", i % 20))
        .collect();
    
    c.bench_function("geographic_entropy_1000", |b| {
        b.iter(|| {
            let result = analyzer.analyze_geographic_entropy(&locations);
            black_box(result);
        });
    });
}

fn bench_fraud_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("fraud_detection");
    
    for count in [100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            count,
            |b, &count| {
                let detector = FraudDetector::new();
                let votes = create_vote_records(count);
                
                b.iter(|| {
                    let result = detector.detect_fraud(&votes);
                    black_box(result);
                });
            },
        );
    }
    group.finish();
}

fn bench_duplicate_detection(c: &mut Criterion) {
    let detector = FraudDetector::new();
    let votes = create_vote_records(1000);
    
    c.bench_function("detect_duplicates_1000", |b| {
        b.iter(|| {
            let result = detector.detect_duplicates(&votes);
            black_box(result);
        });
    });
}

fn bench_pattern_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_detection");
    
    for count in [100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            count,
            |b, &count| {
                let detector = PatternDetector::new();
                let patterns = create_vote_patterns(count);
                
                b.iter(|| {
                    let result = detector.detect_patterns(&patterns);
                    black_box(result);
                });
            },
        );
    }
    group.finish();
}

fn bench_statistical_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistical_analysis");
    
    for count in [100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            count,
            |b, &count| {
                let analyzer = StatisticalAnalyzer::new();
                let data: Vec<f64> = (0..*count).map(|i| i as f64).collect();
                
                b.iter(|| {
                    let result = analyzer.descriptive_statistics(&data);
                    black_box(result);
                });
            },
        );
    }
    group.finish();
}

fn bench_correlation_analysis(c: &mut Criterion) {
    let analyzer = CorrelationAnalyzer::new();
    let x: Vec<f64> = (0..1000).map(|i| i as f64).collect();
    let y: Vec<f64> = (0..1000).map(|i| (i * 2) as f64).collect();
    
    c.bench_function("pearson_correlation_1000", |b| {
        b.iter(|| {
            let result = analyzer.pearson_correlation(&x, &y);
            black_box(result);
        });
    });
    
    c.bench_function("spearman_correlation_1000", |b| {
        b.iter(|| {
            let result = analyzer.spearman_correlation(&x, &y);
            black_box(result);
        });
    });
}

fn bench_outlier_detection(c: &mut Criterion) {
    let analyzer = StatisticalAnalyzer::new();
    let mut data: Vec<f64> = (0..1000).map(|i| i as f64).collect();
    data.push(10000.0);
    data.push(10001.0);
    
    c.bench_function("outlier_detection_iqr", |b| {
        b.iter(|| {
            let result = analyzer.detect_outliers_iqr(&data);
            black_box(result);
        });
    });
    
    c.bench_function("outlier_detection_zscore", |b| {
        b.iter(|| {
            let result = analyzer.detect_outliers_zscore(&data, 3.0);
            black_box(result);
        });
    });
}

fn bench_linear_regression(c: &mut Criterion) {
    let analyzer = StatisticalAnalyzer::new();
    let x: Vec<f64> = (0..1000).map(|i| i as f64).collect();
    let y: Vec<f64> = (0..1000).map(|i| (i * 2 + 5) as f64).collect();
    
    c.bench_function("linear_regression_1000", |b| {
        b.iter(|| {
            let result = analyzer.linear_regression(&x, &y);
            black_box(result);
        });
    });
}

fn bench_t_test(c: &mut Criterion) {
    let analyzer = StatisticalAnalyzer::new();
    let sample1: Vec<f64> = (0..100).map(|i| 50.0 + i as f64 * 0.1).collect();
    let sample2: Vec<f64> = (0..100).map(|i| 52.0 + i as f64 * 0.1).collect();
    
    c.bench_function("one_sample_t_test", |b| {
        b.iter(|| {
            let result = analyzer.one_sample_t_test(&sample1, 50.0);
            black_box(result);
        });
    });
    
    c.bench_function("two_sample_t_test", |b| {
        b.iter(|| {
            let result = analyzer.two_sample_t_test(&sample1, &sample2);
            black_box(result);
        });
    });
}

fn bench_chi_square_test(c: &mut Criterion) {
    let analyzer = StatisticalAnalyzer::new();
    let observed = vec![10.0, 20.0, 30.0, 40.0, 50.0];
    let expected = vec![15.0, 25.0, 30.0, 35.0, 45.0];
    
    c.bench_function("chi_square_test", |b| {
        b.iter(|| {
            let result = analyzer.chi_square_test(&observed, &expected);
            black_box(result);
        });
    });
}

fn bench_cross_correlation(c: &mut Criterion) {
    let analyzer = CorrelationAnalyzer::new();
    let series1: Vec<f64> = (0..500).map(|i| (i as f64).sin()).collect();
    let series2: Vec<f64> = (0..500).map(|i| (i as f64 + 10.0).sin()).collect();
    
    c.bench_function("cross_correlation_500", |b| {
        b.iter(|| {
            let result = analyzer.cross_correlation(&series1, &series2, 20);
            black_box(result);
        });
    });
}

fn bench_benford_report_generation(c: &mut Criterion) {
    let analyzer = BenfordAnalyzer::new();
    let data = create_benford_data(500);
    let result = analyzer.analyze(&data).unwrap();
    
    c.bench_function("benford_report_generation", |b| {
        b.iter(|| {
            let report = analyzer.generate_report(&result);
            black_box(report);
        });
    });
}

fn bench_sequence_entropy(c: &mut Criterion) {
    let analyzer = EntropyAnalyzer::new();
    let sequence: Vec<u64> = (0..1000).map(|i| i % 50).collect();
    
    c.bench_function("sequence_entropy_1000", |b| {
        b.iter(|| {
            let result = analyzer.analyze_sequence_entropy(&sequence);
            black_box(result);
        });
    });
}

criterion_group!(
    benches,
    bench_benford_analysis,
    bench_entropy_analysis,
    bench_temporal_entropy,
    bench_geographic_entropy,
    bench_fraud_detection,
    bench_duplicate_detection,
    bench_pattern_detection,
    bench_statistical_analysis,
    bench_correlation_analysis,
    bench_outlier_detection,
    bench_linear_regression,
    bench_t_test,
    bench_chi_square_test,
    bench_cross_correlation,
    bench_benford_report_generation,
    bench_sequence_entropy,
);

criterion_main!(benches);

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use blockchain_core::consensus::{ProofOfAuthority, ValidatorSet, ConsensusEngine};
use blockchain_core::{Block, Transaction, TransactionType};
use common::{BlockHash, PublicKey, Signature, TxId};

fn create_validators(count: usize) -> Vec<PublicKey> {
    (0..count)
        .map(|i| PublicKey::new([i as u8; 32]))
        .collect()
}

fn create_test_transaction(id: u8) -> Transaction {
    Transaction {
        id: TxId::new([id; 32]),
        tx_type: TransactionType::Vote {
            election_id: format!("election_{}", id),
            encrypted_vote: vec![id; 64],
            proof: vec![id; 128],
        },
        timestamp: 1000000 + (id as u64 * 1000),
        signature: Signature::new([id; 64]),
        public_key: PublicKey::new([id; 32]),
    }
}

fn bench_validator_set_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("validator_set_creation");
    
    for count in [3, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            count,
            |b, &count| {
                let validators = create_validators(count);
                b.iter(|| {
                    let validator_set = ValidatorSet::new(validators.clone());
                    black_box(validator_set);
                });
            },
        );
    }
    group.finish();
}

fn bench_validator_selection(c: &mut Criterion) {
    let validators = create_validators(10);
    let validator_set = ValidatorSet::new(validators);
    
    c.bench_function("select_validator_round_robin", |b| {
        let mut height = 0u64;
        b.iter(|| {
            let validator = validator_set.get_validator_for_height(height);
            height += 1;
            black_box(validator);
        });
    });
}

fn bench_is_validator_check(c: &mut Criterion) {
    let validators = create_validators(100);
    let validator_set = ValidatorSet::new(validators.clone());
    
    c.bench_function("is_validator_check", |b| {
        b.iter(|| {
            let result = validator_set.is_validator(&validators[50]);
            black_box(result);
        });
    });
}

fn bench_poa_propose_block(c: &mut Criterion) {
    let validators = create_validators(5);
    let poa = ProofOfAuthority::new(validators.clone(), 0.67).unwrap();
    
    let transactions = vec![
        create_test_transaction(1),
        create_test_transaction(2),
        create_test_transaction(3),
    ];
    
    c.bench_function("poa_propose_block", |b| {
        let mut height = 1u64;
        b.iter(|| {
            let result = poa.propose_block(
                height,
                BlockHash::new([0u8; 32]),
                transactions.clone(),
                &validators[0],
            );
            height += 1;
            black_box(result);
        });
    });
}

fn bench_poa_validate_block(c: &mut Criterion) {
    let validators = create_validators(5);
    let poa = ProofOfAuthority::new(validators.clone(), 0.67).unwrap();
    
    let transactions = vec![create_test_transaction(1)];
    let block = Block::new(
        1,
        BlockHash::new([0u8; 32]),
        transactions,
        validators[0],
    );
    
    c.bench_function("poa_validate_block", |b| {
        b.iter(|| {
            let result = poa.validate_block(&block);
            black_box(result);
        });
    });
}

fn bench_finality_check(c: &mut Criterion) {
    let validators = create_validators(10);
    let poa = ProofOfAuthority::new(validators.clone(), 0.67).unwrap();
    
    let block_hash = BlockHash::new([1u8; 32]);
    
    for validator in validators.iter().take(7) {
        let _ = poa.add_signature(1, block_hash, validator.clone());
    }
    
    c.bench_function("finality_check", |b| {
        b.iter(|| {
            let result = poa.is_finalized(1, &block_hash);
            black_box(result);
        });
    });
}

fn bench_add_validator_signature(c: &mut Criterion) {
    let validators = create_validators(10);
    let poa = ProofOfAuthority::new(validators.clone(), 0.67).unwrap();
    
    let block_hash = BlockHash::new([1u8; 32]);
    
    c.bench_function("add_validator_signature", |b| {
        let mut idx = 0;
        b.iter(|| {
            let validator = validators[idx % validators.len()];
            let result = poa.add_signature(1, block_hash, validator);
            idx += 1;
            black_box(result);
        });
    });
}

fn bench_validator_set_rotation(c: &mut Criterion) {
    let validators = create_validators(20);
    let mut validator_set = ValidatorSet::new(validators.clone());
    
    c.bench_function("add_validator", |b| {
        let mut next_id = 20;
        b.iter(|| {
            let new_validator = PublicKey::new([next_id as u8; 32]);
            validator_set.add_validator(new_validator);
            next_id += 1;
            black_box(());
        });
    });
}

fn bench_get_active_validators(c: &mut Criterion) {
    let validators = create_validators(100);
    let validator_set = ValidatorSet::new(validators);
    
    c.bench_function("get_active_validators", |b| {
        b.iter(|| {
            let active = validator_set.get_active_validators();
            black_box(active);
        });
    });
}

fn bench_consensus_with_varying_validators(c: &mut Criterion) {
    let mut group = c.benchmark_group("consensus_validation");
    
    for validator_count in [3, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(validator_count),
            validator_count,
            |b, &validator_count| {
                let validators = create_validators(validator_count);
                let poa = ProofOfAuthority::new(validators.clone(), 0.67).unwrap();
                
                let transactions = vec![create_test_transaction(1)];
                let block = Block::new(
                    1,
                    BlockHash::new([0u8; 32]),
                    transactions,
                    validators[0],
                );
                
                b.iter(|| {
                    let result = poa.validate_block(&block);
                    black_box(result);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_validator_set_creation,
    bench_validator_selection,
    bench_is_validator_check,
    bench_poa_propose_block,
    bench_poa_validate_block,
    bench_finality_check,
    bench_add_validator_signature,
    bench_validator_set_rotation,
    bench_get_active_validators,
    bench_consensus_with_varying_validators,
);

criterion_main!(benches);

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use blockchain_core::{Block, Blockchain, Transaction, TransactionType, GenesisBlock};
use common::{BlockHash, PublicKey, Signature, TxId};

fn create_genesis() -> GenesisBlock {
    GenesisBlock::new(
        "bench-chain".to_string(),
        vec![PublicKey::new([1u8; 32])],
        10000,
    )
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

fn bench_blockchain_creation(c: &mut Criterion) {
    c.bench_function("blockchain_new", |b| {
        b.iter(|| {
            let genesis = create_genesis();
            let blockchain = Blockchain::new(genesis).unwrap();
            black_box(blockchain);
        });
    });
}

fn bench_block_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_creation");
    
    for tx_count in [1, 10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(tx_count),
            tx_count,
            |b, &tx_count| {
                let transactions: Vec<Transaction> = (0..tx_count)
                    .map(|i| create_test_transaction(i as u8))
                    .collect();
                
                b.iter(|| {
                    let block = Block::new(
                        1,
                        BlockHash::new([0u8; 32]),
                        transactions.clone(),
                        PublicKey::new([1u8; 32]),
                    );
                    black_box(block);
                });
            },
        );
    }
    group.finish();
}

fn bench_block_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_validation");
    
    for tx_count in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(tx_count),
            tx_count,
            |b, &tx_count| {
                let transactions: Vec<Transaction> = (0..tx_count)
                    .map(|i| create_test_transaction(i as u8))
                    .collect();
                
                let block = Block::new(
                    1,
                    BlockHash::new([0u8; 32]),
                    transactions,
                    PublicKey::new([1u8; 32]),
                );
                
                b.iter(|| {
                    let result = block.validate();
                    black_box(result);
                });
            },
        );
    }
    group.finish();
}

fn bench_block_hash_computation(c: &mut Criterion) {
    let transactions: Vec<Transaction> = (0..100)
        .map(|i| create_test_transaction(i as u8))
        .collect();
    
    let block = Block::new(
        1,
        BlockHash::new([0u8; 32]),
        transactions,
        PublicKey::new([1u8; 32]),
    );
    
    c.bench_function("block_hash_100tx", |b| {
        b.iter(|| {
            let hash = block.hash();
            black_box(hash);
        });
    });
}

fn bench_blockchain_add_block(c: &mut Criterion) {
    let genesis = create_genesis();
    let mut blockchain = Blockchain::new(genesis).unwrap();
    
    c.bench_function("blockchain_add_block", |b| {
        let mut block_height = 1;
        b.iter(|| {
            let transactions = vec![create_test_transaction(1)];
            let prev_hash = blockchain.latest_block().hash();
            let block = Block::new(
                block_height,
                prev_hash,
                transactions,
                PublicKey::new([1u8; 32]),
            );
            
            let result = blockchain.add_block(block);
            black_box(result);
            block_height += 1;
        });
    });
}

fn bench_blockchain_get_block(c: &mut Criterion) {
    let genesis = create_genesis();
    let mut blockchain = Blockchain::new(genesis).unwrap();
    
    for i in 1..=100 {
        let transactions = vec![create_test_transaction(i as u8)];
        let prev_hash = blockchain.latest_block().hash();
        let block = Block::new(i, prev_hash, transactions, PublicKey::new([1u8; 32]));
        blockchain.add_block(block).unwrap();
    }
    
    c.bench_function("blockchain_get_block_by_height", |b| {
        b.iter(|| {
            let block = blockchain.get_block_by_height(50);
            black_box(block);
        });
    });
}

fn bench_blockchain_get_transaction(c: &mut Criterion) {
    let genesis = create_genesis();
    let mut blockchain = Blockchain::new(genesis).unwrap();
    
    let target_tx_id = TxId::new([42u8; 32]);
    let mut transactions = vec![create_test_transaction(1)];
    transactions.push(Transaction {
        id: target_tx_id,
        tx_type: TransactionType::Vote {
            election_id: "election_42".to_string(),
            encrypted_vote: vec![42; 64],
            proof: vec![42; 128],
        },
        timestamp: 1000000,
        signature: Signature::new([42u8; 64]),
        public_key: PublicKey::new([42u8; 32]),
    });
    
    for i in 1..=50 {
        let prev_hash = blockchain.latest_block().hash();
        let block = Block::new(i, prev_hash, transactions.clone(), PublicKey::new([1u8; 32]));
        blockchain.add_block(block).unwrap();
    }
    
    c.bench_function("blockchain_get_transaction", |b| {
        b.iter(|| {
            let tx = blockchain.get_transaction(&target_tx_id);
            black_box(tx);
        });
    });
}

fn bench_merkle_tree_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("merkle_tree");
    
    for tx_count in [10, 50, 100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(tx_count),
            tx_count,
            |b, &tx_count| {
                let transactions: Vec<Transaction> = (0..tx_count)
                    .map(|i| create_test_transaction((i % 256) as u8))
                    .collect();
                
                b.iter(|| {
                    let tx_hashes: Vec<[u8; 32]> = transactions
                        .iter()
                        .map(|tx| tx.id.0)
                        .collect();
                    
                    let merkle = blockchain_core::MerkleTree::new(tx_hashes);
                    black_box(merkle);
                });
            },
        );
    }
    group.finish();
}

fn bench_transaction_serialization(c: &mut Criterion) {
    let tx = create_test_transaction(1);
    
    c.bench_function("transaction_serialize", |b| {
        b.iter(|| {
            let serialized = bincode::serialize(&tx).unwrap();
            black_box(serialized);
        });
    });
    
    let serialized = bincode::serialize(&tx).unwrap();
    c.bench_function("transaction_deserialize", |b| {
        b.iter(|| {
            let deserialized: Transaction = bincode::deserialize(&serialized).unwrap();
            black_box(deserialized);
        });
    });
}

fn bench_block_serialization(c: &mut Criterion) {
    let transactions: Vec<Transaction> = (0..100)
        .map(|i| create_test_transaction(i as u8))
        .collect();
    
    let block = Block::new(
        1,
        BlockHash::new([0u8; 32]),
        transactions,
        PublicKey::new([1u8; 32]),
    );
    
    c.bench_function("block_serialize_100tx", |b| {
        b.iter(|| {
            let serialized = bincode::serialize(&block).unwrap();
            black_box(serialized);
        });
    });
    
    let serialized = bincode::serialize(&block).unwrap();
    c.bench_function("block_deserialize_100tx", |b| {
        b.iter(|| {
            let deserialized: Block = bincode::deserialize(&serialized).unwrap();
            black_box(deserialized);
        });
    });
}

criterion_group!(
    benches,
    bench_blockchain_creation,
    bench_block_creation,
    bench_block_validation,
    bench_block_hash_computation,
    bench_blockchain_add_block,
    bench_blockchain_get_block,
    bench_blockchain_get_transaction,
    bench_merkle_tree_construction,
    bench_transaction_serialization,
    bench_block_serialization,
);

criterion_main!(benches);

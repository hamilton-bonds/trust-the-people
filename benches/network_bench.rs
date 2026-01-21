use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use network::protocol::{Message, MessageType, HandshakeInfo};
use common::BlockHash;

fn create_handshake() -> HandshakeInfo {
    HandshakeInfo {
        protocol_version: 1,
        network_id: "mainnet".to_string(),
        user_agent: "benchmark/1.0".to_string(),
        best_height: 1000,
        best_hash: BlockHash::new([1u8; 32]),
        genesis_hash: BlockHash::new([0u8; 32]),
        timestamp: 1000000,
    }
}

fn bench_message_creation(c: &mut Criterion) {
    c.bench_function("message_ping", |b| {
        b.iter(|| {
            let msg = Message::ping();
            black_box(msg);
        });
    });
    
    c.bench_function("message_pong", |b| {
        b.iter(|| {
            let msg = Message::pong();
            black_box(msg);
        });
    });
}

fn bench_handshake_message(c: &mut Criterion) {
    let handshake = create_handshake();
    
    c.bench_function("message_handshake", |b| {
        b.iter(|| {
            let msg = Message::handshake(handshake.clone()).unwrap();
            black_box(msg);
        });
    });
}

fn bench_message_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_serialization");
    
    let ping = Message::ping();
    let handshake = Message::handshake(create_handshake()).unwrap();
    
    group.bench_function("serialize_ping", |b| {
        b.iter(|| {
            let bytes = bincode::serialize(&ping).unwrap();
            black_box(bytes);
        });
    });
    
    group.bench_function("serialize_handshake", |b| {
        b.iter(|| {
            let bytes = bincode::serialize(&handshake).unwrap();
            black_box(bytes);
        });
    });
    
    group.finish();
}

fn bench_message_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_deserialization");
    
    let ping = Message::ping();
    let ping_bytes = bincode::serialize(&ping).unwrap();
    
    let handshake = Message::handshake(create_handshake()).unwrap();
    let handshake_bytes = bincode::serialize(&handshake).unwrap();
    
    group.bench_function("deserialize_ping", |b| {
        b.iter(|| {
            let msg: Message = bincode::deserialize(&ping_bytes).unwrap();
            black_box(msg);
        });
    });
    
    group.bench_function("deserialize_handshake", |b| {
        b.iter(|| {
            let msg: Message = bincode::deserialize(&handshake_bytes).unwrap();
            black_box(msg);
        });
    });
    
    group.finish();
}

fn bench_message_with_varying_payload_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_payload_sizes");
    
    for size in [64, 256, 1024, 4096, 16384].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                let payload = vec![0u8; size];
                b.iter(|| {
                    let msg = Message::new(MessageType::Block, payload.clone());
                    black_box(msg);
                });
            },
        );
    }
    group.finish();
}

fn bench_message_payload_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("payload_encoding");
    
    for size in [256, 1024, 4096].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                let payload = vec![0u8; size];
                let msg = Message::new(MessageType::Block, payload);
                
                b.iter(|| {
                    let encoded = bincode::serialize(&msg).unwrap();
                    black_box(encoded);
                });
            },
        );
    }
    group.finish();
}

fn bench_message_payload_decoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("payload_decoding");
    
    for size in [256, 1024, 4096].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                let payload = vec![0u8; size];
                let msg = Message::new(MessageType::Block, payload);
                let encoded = bincode::serialize(&msg).unwrap();
                
                b.iter(|| {
                    let decoded: Message = bincode::deserialize(&encoded).unwrap();
                    black_box(decoded);
                });
            },
        );
    }
    group.finish();
}

fn bench_message_type_matching(c: &mut Criterion) {
    let messages = vec![
        Message::ping(),
        Message::pong(),
        Message::handshake(create_handshake()).unwrap(),
    ];
    
    c.bench_function("message_type_match", |b| {
        let mut idx = 0;
        b.iter(|| {
            let msg = &messages[idx % messages.len()];
            let result = match msg.msg_type {
                MessageType::Ping => 1,
                MessageType::Pong => 2,
                MessageType::Handshake => 3,
                _ => 0,
            };
            idx += 1;
            black_box(result);
        });
    });
}

fn bench_message_clone(c: &mut Criterion) {
    let handshake = Message::handshake(create_handshake()).unwrap();
    
    c.bench_function("message_clone_small", |b| {
        let ping = Message::ping();
        b.iter(|| {
            let cloned = ping.clone();
            black_box(cloned);
        });
    });
    
    c.bench_function("message_clone_large", |b| {
        b.iter(|| {
            let cloned = handshake.clone();
            black_box(cloned);
        });
    });
}

fn bench_message_size_calculation(c: &mut Criterion) {
    let messages = vec![
        Message::ping(),
        Message::handshake(create_handshake()).unwrap(),
        Message::new(MessageType::Block, vec![0u8; 1024]),
    ];
    
    c.bench_function("message_size", |b| {
        let mut idx = 0;
        b.iter(|| {
            let size = messages[idx % messages.len()].size();
            idx += 1;
            black_box(size);
        });
    });
}

criterion_group!(
    benches,
    bench_message_creation,
    bench_handshake_message,
    bench_message_serialization,
    bench_message_deserialization,
    bench_message_with_varying_payload_sizes,
    bench_message_payload_encoding,
    bench_message_payload_decoding,
    bench_message_type_matching,
    bench_message_clone,
    bench_message_size_calculation,
);

criterion_main!(benches);

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use crypto::{KeyPair, hash_data, encrypt, decrypt};

fn bench_keypair_generation(c: &mut Criterion) {
    c.bench_function("keypair_generate", |b| {
        b.iter(|| {
            let keypair = KeyPair::generate();
            black_box(keypair);
        });
    });
}

fn bench_signing(c: &mut Criterion) {
    let keypair = KeyPair::generate();
    let message = b"This is a test message for signing benchmarks";
    
    c.bench_function("sign_message", |b| {
        b.iter(|| {
            let signature = keypair.sign(message);
            black_box(signature);
        });
    });
}

fn bench_verification(c: &mut Criterion) {
    let keypair = KeyPair::generate();
    let message = b"This is a test message for verification benchmarks";
    let signature = keypair.sign(message);
    
    c.bench_function("verify_signature", |b| {
        b.iter(|| {
            let result = keypair.verify(message, &signature);
            black_box(result);
        });
    });
}

fn bench_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashing");
    
    for size in [64, 256, 1024, 4096, 16384].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                let data = vec![0u8; size];
                b.iter(|| {
                    let hash = hash_data(&data);
                    black_box(hash);
                });
            },
        );
    }
    group.finish();
}

fn bench_encryption(c: &mut Criterion) {
    let mut group = c.benchmark_group("encryption");
    
    let key = [1u8; 32];
    
    for size in [64, 256, 1024, 4096].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                let plaintext = vec![0u8; size];
                b.iter(|| {
                    let encrypted = encrypt(&key, &plaintext).unwrap();
                    black_box(encrypted);
                });
            },
        );
    }
    group.finish();
}

fn bench_decryption(c: &mut Criterion) {
    let mut group = c.benchmark_group("decryption");
    
    let key = [1u8; 32];
    
    for size in [64, 256, 1024, 4096].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                let plaintext = vec![0u8; size];
                let encrypted = encrypt(&key, &plaintext).unwrap();
                
                b.iter(|| {
                    let decrypted = decrypt(&key, &encrypted).unwrap();
                    black_box(decrypted);
                });
            },
        );
    }
    group.finish();
}

fn bench_hash_multiple_data(c: &mut Criterion) {
    let data1 = vec![0u8; 256];
    let data2 = vec![1u8; 256];
    let data3 = vec![2u8; 256];
    
    c.bench_function("hash_multiple_256", |b| {
        b.iter(|| {
            let hash = crypto::hash_multiple(&[&data1, &data2, &data3]);
            black_box(hash);
        });
    });
}

fn bench_key_serialization(c: &mut Criterion) {
    let keypair = KeyPair::generate();
    
    c.bench_function("keypair_to_bytes", |b| {
        b.iter(|| {
            let bytes = keypair.to_bytes();
            black_box(bytes);
        });
    });
    
    let bytes = keypair.to_bytes();
    c.bench_function("keypair_from_bytes", |b| {
        b.iter(|| {
            let keypair = KeyPair::from_bytes(&bytes).unwrap();
            black_box(keypair);
        });
    });
}

criterion_group!(
    benches,
    bench_keypair_generation,
    bench_signing,
    bench_verification,
    bench_hashing,
    bench_encryption,
    bench_decryption,
    bench_hash_multiple_data,
    bench_key_serialization,
);

criterion_main!(benches);

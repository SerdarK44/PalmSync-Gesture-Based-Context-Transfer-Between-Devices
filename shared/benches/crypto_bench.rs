use criterion::{black_box, criterion_group, criterion_main, Criterion};

use palmsync_shared::auth::AuthManager;
use palmsync_shared::crypto::{compute_password_hash, CryptoManager};
use palmsync_shared::{RpcMessage, StateSnapshot};

fn snapshot() -> StateSnapshot {
    StateSnapshot {
        project_path: "C:\\Users\\dev\\big-project".into(),
        file_name: "main.rs".into(),
        cursor_line: 1234,
        clipboard_content: "x".repeat(4096), // typical code-snippet size
        timestamp: 1_700_000_000_000,
        source_node_id: "bench-node".into(),
    }
}

fn bench_crypto(c: &mut Criterion) {
    let cm = CryptoManager::new("bench_password");
    let payload_small = b"small gesture packet payload".to_vec();
    let payload_4k = vec![0u8; 4096];
    let payload_1m = vec![0u8; 1024 * 1024];

    c.bench_function("aes256gcm_encrypt_small", |b| {
        b.iter(|| cm.encrypt(black_box(&payload_small)).unwrap())
    });

    c.bench_function("aes256gcm_encrypt_4kb", |b| {
        b.iter(|| cm.encrypt(black_box(&payload_4k)).unwrap())
    });

    c.bench_function("aes256gcm_encrypt_1mb", |b| {
        b.iter(|| cm.encrypt(black_box(&payload_1m)).unwrap())
    });

    let sealed = cm.encrypt(&payload_4k).unwrap();
    c.bench_function("aes256gcm_decrypt_4kb", |b| {
        b.iter(|| cm.decrypt(black_box(&sealed)).unwrap())
    });

    c.bench_function("sha256_password_hash", |b| {
        b.iter(|| compute_password_hash(black_box("bench_password")))
    });
}

fn bench_argon2(c: &mut Criterion) {
    // Argon2 is intentionally slow — reduce sample count so the suite
    // finishes in reasonable time.
    let mut group = c.benchmark_group("argon2");
    group.sample_size(10);

    group.bench_function("argon2id_hash", |b| {
        b.iter(|| AuthManager::hash_password(black_box("bench_password")).unwrap())
    });

    let hash = AuthManager::hash_password("bench_password").unwrap();
    group.bench_function("argon2id_verify", |b| {
        b.iter(|| AuthManager::verify_password(black_box("bench_password"), &hash).unwrap())
    });

    group.finish();
}

fn bench_serialization(c: &mut Criterion) {
    let msg = RpcMessage::Grab(snapshot());
    let json = serde_json::to_string(&msg).unwrap();

    c.bench_function("rpc_serialize_4kb_snapshot", |b| {
        b.iter(|| serde_json::to_string(black_box(&msg)).unwrap())
    });

    c.bench_function("rpc_deserialize_4kb_snapshot", |b| {
        b.iter(|| serde_json::from_str::<RpcMessage>(black_box(&json)).unwrap())
    });
}

fn bench_full_seal_pipeline(c: &mut Criterion) {
    // The realistic hot path: serialize snapshot -> seal -> open -> parse
    let cm = CryptoManager::new("bench_password");
    let msg = RpcMessage::Grab(snapshot());

    c.bench_function("full_seal_open_roundtrip", |b| {
        b.iter(|| {
            let json = serde_json::to_string(black_box(&msg)).unwrap();
            let sealed = cm.seal(&json).unwrap();
            let opened = cm.open(&sealed).unwrap();
            serde_json::from_str::<RpcMessage>(&opened).unwrap()
        })
    });
}

criterion_group!(
    benches,
    bench_crypto,
    bench_argon2,
    bench_serialization,
    bench_full_seal_pipeline
);
criterion_main!(benches);

use std::sync::{Arc, Barrier};
use zerant_core::encode_base64url;
use zerant_disclosure::{ReplayEntry, ReplayStatus, ReplayStore, SqliteReplay};
#[test]
fn independent_sqlite_connections_accept_at_most_once_and_survive_restart() {
    let path = std::env::temp_dir().join(format!(
        "zerant-atomic-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let entry = ReplayEntry {
        request_id: encode_base64url(&[1; 16]),
        digest: encode_base64url(&[2; 32]),
        origin: "https://verifier.example".into(),
        expires_at: 100,
        status: ReplayStatus::Pending,
    };
    {
        let store = SqliteReplay::open(&path).unwrap();
        store.register(entry.clone()).unwrap();
    }
    let barrier = Arc::new(Barrier::new(8));
    let mut threads = vec![];
    for _ in 0..8 {
        let store = SqliteReplay::open(&path).unwrap();
        let e = entry.clone();
        let barrier = barrier.clone();
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            store
                .consume_if_pending(&e.request_id, &e.digest, &e.origin, 10)
                .is_ok()
        }));
    }
    assert_eq!(
        threads
            .into_iter()
            .map(|t| t.join().unwrap())
            .filter(|v| *v)
            .count(),
        1
    );
    {
        let store = SqliteReplay::open(&path).unwrap();
        assert_eq!(
            store.get(&entry.request_id).unwrap().unwrap().status,
            ReplayStatus::Consumed
        );
        assert!(
            store
                .consume_if_pending(&entry.request_id, &entry.digest, &entry.origin, 10)
                .is_err()
        );
        assert!(store.register(entry).is_err());
    }
    std::fs::remove_file(path).unwrap();
}

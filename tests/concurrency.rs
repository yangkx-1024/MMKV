//! Concurrent use of the public API: many writers on one store, readers running while
//! the writer trims the file, and several handles on the same directory.
//!
//! Everything synchronises through joins and handle drops; no test sleeps.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use common::Store;
use mmkv::Error::KeyNotFound;
use mmkv::MMKV;

/// Three writers on distinct keys plus one thread churning a shared key. Every value
/// must be on disk once the threads are joined and the handles are gone.
#[test]
fn concurrent_writers_all_land_on_disk() {
    const THREADS: i32 = 3;
    const PUTS: i32 = 500;

    let store = Store::new();
    thread::scope(|scope| {
        for thread_id in 0..THREADS {
            let mmkv = store.open();
            scope.spawn(move || {
                for i in 0..PUTS {
                    mmkv.put(&format!("thread{thread_id}_key{i}"), i).unwrap();
                }
            });
        }
        // Overwriting and deleting the same key keeps the writer trimming underneath
        // the other threads.
        let mmkv = store.open();
        scope.spawn(move || {
            for i in 0..PUTS {
                if i % 2 == 0 {
                    mmkv.put("shared", i).unwrap();
                } else {
                    mmkv.delete("shared").unwrap();
                }
            }
        });
    });

    let mmkv = store.open();
    for thread_id in 0..THREADS {
        for i in 0..PUTS {
            let key = format!("thread{thread_id}_key{i}");
            assert_eq!(mmkv.get::<i32>(&key), Ok(i), "{key} after reopen");
        }
    }
    // The churner's last operation was a delete.
    assert_eq!(mmkv.get::<i32>("shared"), Err(KeyNotFound));
}

/// Readers must never observe a torn or missing value while the writer replaces the
/// whole file underneath them with a shadow-file trim.
#[test]
fn readers_see_a_stable_key_while_the_writer_trims() {
    const READERS: usize = 4;
    const READS: usize = 600;
    const TRIMS: usize = 300;

    let store = Store::new();
    let writer = store.open();
    writer.put("stable", 42i32).unwrap();

    let wrong_reads = AtomicUsize::new(0);
    thread::scope(|scope| {
        for _ in 0..READERS {
            let mmkv = store.open();
            let wrong_reads = &wrong_reads;
            scope.spawn(move || {
                for _ in 0..READS {
                    if mmkv.get::<i32>("stable") != Ok(42) {
                        wrong_reads.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
        // ~3/4 of a page, so every duplicate put has to trim.
        scope.spawn(move || {
            let value = common::bytes(3000, 7);
            for _ in 0..TRIMS {
                writer.put("trim_trigger", value.as_slice()).unwrap();
            }
        });
    });

    assert_eq!(
        wrong_reads.load(Ordering::Relaxed),
        0,
        "concurrent reads during trim observed a wrong value"
    );
    // The stable value survived every trim cycle, on disk too.
    assert_eq!(store.open().get::<i32>("stable"), Ok(42));
}

#[test]
fn two_handles_on_the_same_dir_see_each_others_writes() {
    let store = Store::new();
    let first = store.open();
    let second = store.open();

    first.put("key", "from first").unwrap();
    assert_eq!(second.get::<String>("key"), Ok("from first".to_string()));

    second.put("key", "from second").unwrap();
    assert_eq!(first.get::<String>("key"), Ok("from second".to_string()));

    second.delete("key").unwrap();
    assert_eq!(first.get::<String>("key"), Err(KeyNotFound));
}

/// Racing `MMKV::new` calls for one directory must all succeed and share one instance.
#[test]
fn opening_the_same_dir_from_many_threads_shares_one_instance() {
    const HANDLES: usize = 8;

    let store = Store::new();
    let handles: Vec<MMKV> = thread::scope(|scope| {
        let spawned: Vec<_> = (0..HANDLES).map(|_| scope.spawn(|| store.open())).collect();
        spawned
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    });

    handles[0].put("shared", 1i32).unwrap();
    for (i, mmkv) in handles.iter().enumerate() {
        assert_eq!(mmkv.get::<i32>("shared"), Ok(1), "handle {i}");
        mmkv.put(&format!("key{i}"), i as i32).unwrap();
    }
    drop(handles);

    let mmkv = store.open();
    for i in 0..HANDLES {
        assert_eq!(mmkv.get::<i32>(&format!("key{i}")), Ok(i as i32));
    }
}

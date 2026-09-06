//! The write contract at the public API: `put`/`delete` return only once the record is
//! in the file, a failed write leaves the previous value in place (in memory and on
//! disk), and everything survives dropping the handle and reopening.

mod common;

use std::fs;

use common::{HEADER_LEN, Store};
use mmkv::Error::KeyNotFound;

/// The value length whose record fills a page exactly. Measured with the encoder of the
/// current build flavour instead of assuming CRC (17 bytes of framing) or AEAD (24).
fn value_len_filling_a_page(page: u64, key: &str) -> usize {
    let target = page as i64 - HEADER_LEN as i64;
    let mut len = target - 32;
    for _ in 0..4 {
        assert!(len > 0, "page {page} is too small for this test");
        let record = common::record_len(key, &common::bytes(len as usize, 1)) as i64;
        if record == target {
            return len as usize;
        }
        len += target - record;
    }
    panic!("could not size a value whose record fills exactly {page} bytes");
}

#[test]
fn each_reopen_sees_the_value_written_by_the_previous_run() {
    let store = Store::new();

    for round in 0..10 {
        // The handle is dropped at the end of every iteration, so the next `open` has to
        // decode the file again instead of reusing the cached instance.
        let mmkv = store.open();
        let previous = mmkv.get::<String>("counter");
        if round == 0 {
            assert_eq!(previous, Err(KeyNotFound));
        } else {
            assert_eq!(previous, Ok((round - 1).to_string()));
        }
        mmkv.put("counter", round.to_string().as_str()).unwrap();
    }

    let mmkv = store.open();
    assert_eq!(mmkv.get::<String>("counter"), Ok("9".to_string()));
    mmkv.clear_data().unwrap();
    assert_eq!(mmkv.get::<String>("counter"), Err(KeyNotFound));
}

/// `Ok(())` means the bytes are in the memory-mapped file: the stored content length has
/// already grown by exactly one record, tombstones included.
#[test]
fn put_and_delete_reach_the_file_before_they_return() {
    let store = Store::new();
    let mmkv = store.open();
    let value = common::bytes(100, 7);
    let expected_record = common::record_len("key0", &value);

    assert_eq!(store.content_len(), 0);
    let mut previous = 0;
    for i in 0..5 {
        mmkv.put(&format!("key{i}"), value.as_slice()).unwrap();
        let content_len = store.content_len();
        assert_eq!(
            content_len - previous,
            expected_record,
            "put {i} must add exactly one record"
        );
        previous = content_len;
    }

    // A tombstone is a record too.
    mmkv.delete("key0").unwrap();
    assert!(store.content_len() > previous);
}

/// With the shadow file blocked the trim cannot start: the write fails and the caller
/// keeps the value it had, both in memory and after a reopen.
#[test]
fn a_blocked_trim_fails_the_write_and_keeps_the_old_value() {
    let store = Store::new();
    let mmkv = store.open();
    let page = store.file_len();
    let fill = value_len_filling_a_page(page, "k");
    let first = common::bytes(fill, 1);
    let second = common::bytes(fill, 2);

    mmkv.put("k", first.as_slice()).unwrap();
    assert_eq!(
        store.content_len(),
        page - HEADER_LEN as u64,
        "page is full"
    );

    // Occupy the tmp paths the next two trims would `create_new`, so both fail.
    let blockers = store.block_next_trims(2);

    // The page is full and the put is a duplicate, so it has to trim first.
    assert!(mmkv.put("k", second.as_slice()).is_err());
    assert_eq!(mmkv.get::<Vec<u8>>("k"), Ok(first.clone()));
    // Not even a tombstone fits, so the delete has to trim as well.
    assert!(mmkv.delete("k").is_err());
    assert_eq!(mmkv.get::<Vec<u8>>("k"), Ok(first.clone()));
    drop(mmkv);
    for blocker in &blockers {
        let _ = fs::remove_file(blocker);
    }

    // Disk agrees with what the caller was told.
    let mmkv = store.open();
    assert_eq!(mmkv.get::<Vec<u8>>("k"), Ok(first));
    // And once the fault is gone the same operations succeed.
    mmkv.put("k", second.as_slice()).unwrap();
    assert_eq!(mmkv.get::<Vec<u8>>("k"), Ok(second.clone()));
    assert_eq!(store.file_len(), page, "the trim reuses one page");
    mmkv.delete("k").unwrap();
    assert_eq!(mmkv.get::<Vec<u8>>("k"), Err(KeyNotFound));
    drop(mmkv);

    let mmkv = store.open();
    assert_eq!(mmkv.get::<Vec<u8>>("k"), Err(KeyNotFound));
    mmkv.put("k", second.as_slice()).unwrap();
    assert_eq!(mmkv.get::<Vec<u8>>("k"), Ok(second));
}

/// No `clear_data`, no explicit flush: the handle simply goes away, the way a process
/// exit would leave the file behind.
#[test]
fn a_store_dropped_without_clear_data_reopens_with_the_same_values() {
    let store = Store::new();
    {
        let mmkv = store.open();
        for i in 0..20 {
            mmkv.put(&format!("key{i}"), format!("value{i}").as_str())
                .unwrap();
        }
        mmkv.delete("key3").unwrap();
    }

    let mmkv = store.open();
    for i in 0..20 {
        let key = format!("key{i}");
        if i == 3 {
            assert_eq!(mmkv.get::<String>(&key), Err(KeyNotFound));
        } else {
            assert_eq!(mmkv.get::<String>(&key), Ok(format!("value{i}")));
        }
    }
}

/// Overwriting one big key forever must not grow the file forever: the writer trims the
/// dead copies away instead of expanding.
#[test]
fn repeated_puts_of_a_large_value_keep_the_file_bounded() {
    const ROUNDS: i32 = 300;
    let store = Store::new();
    let mmkv = store.open();
    let page = store.file_len();

    // ~3/4 of a 4 KiB page, so a second copy never fits and every duplicate put trims.
    for round in 0..ROUNDS {
        let value = common::bytes(3000, (round % 251) as u8);
        mmkv.put("hot", value.as_slice()).unwrap();
    }

    assert_eq!(
        store.file_len(),
        page,
        "{ROUNDS} overwrites must trim, not expand"
    );
    let last = common::bytes(3000, ((ROUNDS - 1) % 251) as u8);
    assert_eq!(mmkv.get::<Vec<u8>>("hot"), Ok(last.clone()));
    drop(mmkv);
    assert_eq!(store.open().get::<Vec<u8>>("hot"), Ok(last));
}

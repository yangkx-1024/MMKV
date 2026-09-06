//! Damaged files seen through the public API: crafted headers and frames, garbage
//! appended after the last record, a single flipped byte anywhere in the content and
//! (under encryption) a damaged, missing or mismatched key/meta file.
//!
//! The contract is always the same: `MMKV::new` either opens the store or returns an
//! error, it never panics, and whatever opens must still be usable.

mod common;

use std::fs;

use common::{HEADER_LEN, Rng, Store};
use mmkv::Error::{IOError, KeyNotFound};

/// A store whose content cannot be framed must still open, accept writes and reopen.
#[test]
fn crafted_frames_open_and_stay_writable() {
    let cases: [(&str, u64, Vec<u8>); 4] = [
        ("content shorter than a frame prefix", 2, vec![1, 2]),
        ("a zero-length frame", 4, vec![0, 0, 0, 0]),
        (
            "a frame claiming 1000 bytes inside 8",
            8,
            vec![0, 0, 0x03, 0xE8, 1, 2, 3, 4],
        ),
        ("64 bytes of 0xFF", 64, common::bytes(64, 0xFF)),
    ];

    for (name, content_len, body) in cases {
        let store = Store::new();
        store.write_raw_store(content_len, &body, 4096);

        let mmkv = store
            .try_open()
            .unwrap_or_else(|e| panic!("{name} must still open, got {e:?}"));
        assert_eq!(mmkv.get::<i32>("anything"), Err(KeyNotFound), "{name}");
        mmkv.put("recovered", 7i32)
            .unwrap_or_else(|e| panic!("{name} must stay writable, got {e:?}"));
        assert_eq!(mmkv.get::<i32>("recovered"), Ok(7), "{name}");
        drop(mmkv);

        assert_eq!(
            store.open().get::<i32>("recovered"),
            Ok(7),
            "{name} after reopen"
        );
    }
}

/// A header that claims more content than the file holds is refused, not trusted.
#[test]
fn a_header_longer_than_the_file_is_rejected() {
    let store = Store::new();
    store.write_raw_store(5000, &[1, 2, 3, 4], 64);

    assert!(
        matches!(store.try_open(), Err(IOError(_))),
        "an oversized content length must be an IOError"
    );
}

/// Garbage that a torn write left after the last good record is dropped at the next
/// open, and the following record lands exactly where the good content ended.
#[test]
fn garbage_after_the_last_record_is_discarded_at_the_next_open() {
    let store = Store::new();
    let mmkv = store.open();
    mmkv.put("k1", b"v1".as_slice()).unwrap();
    mmkv.put("k2", b"v2".as_slice()).unwrap();
    drop(mmkv);
    let good_len = store.content_len();

    // A frame whose declared length runs past the content, with the header bumped so the
    // store believes those bytes are live records.
    let garbage = [0xFF, 0xFF, 0xFF, 0xFF, 0xAA, 0xBB];
    store.append_raw(&garbage);
    assert_eq!(store.content_len(), good_len + garbage.len() as u64);

    let mmkv = store.open();
    assert_eq!(mmkv.get::<Vec<u8>>("k1"), Ok(b"v1".to_vec()));
    assert_eq!(mmkv.get::<Vec<u8>>("k2"), Ok(b"v2".to_vec()));
    mmkv.put("k3", b"v3".as_slice()).unwrap();
    drop(mmkv);

    // The garbage is gone: k3 sits right after the last good record.
    let expected = good_len + common::record_len("k3", b"v3");
    assert_eq!(store.content_len(), expected);
    let mmkv = store.open();
    assert_eq!(store.content_len(), expected, "nothing left to discard");
    assert_eq!(mmkv.get::<Vec<u8>>("k3"), Ok(b"v3".to_vec()));
    assert_eq!(mmkv.get::<Vec<u8>>("k1"), Ok(b"v1".to_vec()));
}

/// Flip one bit anywhere in the content and reopen, 150 times. Nothing may panic, every
/// `get` must answer (`Ok` or `Err`) and the store must stay writable. Under encryption
/// the AEAD tag also rules out a wrong value ever being returned; CRC-8 accepts roughly
/// one corruption in 256, so the default flavour only requires an answer.
#[test]
fn flipping_one_bit_never_panics_and_keeps_the_store_usable() {
    const KEYS: usize = 20;
    const ITERATIONS: usize = 150;

    let store = Store::new();
    let mmkv = store.open();
    let records: Vec<(String, Vec<u8>)> = (0..KEYS)
        .map(|i| (format!("key{i:02}"), common::bytes(16 + i, i as u8)))
        .collect();
    for (key, value) in &records {
        mmkv.put(key, value.as_slice()).unwrap();
    }
    drop(mmkv);

    let snapshot = store.snapshot();
    let content_len = store.content_len() as usize;
    let mut rng = Rng::new(0x00C0_FFEE);

    for iteration in 0..ITERATIONS {
        store.restore(&snapshot);
        let index = HEADER_LEN + rng.below(content_len as u64) as usize;
        let bit = 1u8 << rng.below(8);
        let mut data = fs::read(store.data_file()).unwrap();
        data[index] ^= bit;
        fs::write(store.data_file(), &data).unwrap();

        let Ok(mmkv) = store.try_open() else {
            // Refusing the file is a valid answer too.
            continue;
        };
        for (key, value) in &records {
            match mmkv.get::<Vec<u8>>(key) {
                // CRC-8 accepts roughly one corruption in 256, so only the AEAD flavour
                // can require an accepted value to be the original one.
                Ok(got) if cfg!(feature = "encryption") => assert_eq!(
                    &got, value,
                    "iteration {iteration}: AEAD must never return a wrong value for {key}"
                ),
                // Any answer is fine: losing a record to corruption is expected.
                Ok(_) | Err(_) => {}
            }
        }

        // Whatever survived, the store still takes writes and reopens.
        mmkv.put("after_flip", b"ok".as_slice())
            .unwrap_or_else(|e| panic!("iteration {iteration}: store must stay writable: {e:?}"));
        drop(mmkv);
        assert_eq!(
            store.open().get::<Vec<u8>>("after_flip"),
            Ok(b"ok".to_vec()),
            "iteration {iteration}: after reopen"
        );
    }
}

/// Damage that only the encrypted flavour can suffer: the meta file holding the nonce,
/// and the key itself.
#[cfg(feature = "encryption")]
mod encrypted {
    use super::common::Store;
    use mmkv::Error::{EncryptFailed, KeyNotFound};
    use std::fs;

    /// A valid key that is not the one the store was written with.
    const OTHER_KEY: &str = "0123456789ABCDEF0123456789ABCDEF";

    /// Without the nonce the old records cannot be decrypted, so they are skipped at
    /// open; the store regenerates a nonce and carries on.
    fn old_records_are_lost_but_the_store_survives(damage: impl Fn(&Store)) {
        let store = Store::new();
        let mmkv = store.open();
        mmkv.put("old", "value").unwrap();
        assert_eq!(mmkv.get::<String>("old"), Ok("value".to_string()));
        drop(mmkv);

        damage(&store);

        let mmkv = store.open();
        assert_eq!(mmkv.get::<String>("old"), Err(KeyNotFound));
        mmkv.put("new", "fresh").unwrap();
        assert_eq!(mmkv.get::<String>("new"), Ok("fresh".to_string()));
        drop(mmkv);

        let mmkv = store.open();
        assert_eq!(mmkv.get::<String>("new"), Ok("fresh".to_string()));
        assert_eq!(mmkv.get::<String>("old"), Err(KeyNotFound));
    }

    #[test]
    fn a_truncated_meta_file_is_regenerated() {
        old_records_are_lost_but_the_store_survives(|store| {
            fs::write(store.meta_file(), [0u8; 5]).unwrap();
        });
    }

    #[test]
    fn a_missing_meta_file_is_regenerated() {
        old_records_are_lost_but_the_store_survives(|store| {
            fs::remove_file(store.meta_file()).unwrap();
        });
    }

    #[test]
    fn opening_with_a_different_key_hides_the_records() {
        let store = Store::new();
        let mmkv = store.open();
        mmkv.put("old", "value").unwrap();
        drop(mmkv);

        let other = store.try_open_with_key(OTHER_KEY).unwrap();
        assert_eq!(other.get::<String>("old"), Err(KeyNotFound));
        other.put("new", 1i32).unwrap();
        assert_eq!(other.get::<i32>("new"), Ok(1));
        drop(other);

        // The right key still reads its own record, and ignores the foreign one.
        let mmkv = store.open();
        assert_eq!(mmkv.get::<String>("old"), Ok("value".to_string()));
        assert_eq!(mmkv.get::<i32>("new"), Err(KeyNotFound));
    }

    /// Handles on one dir share one instance, and that instance encrypts with exactly one
    /// key. A second open with another key must be rejected while the first is alive: it
    /// would otherwise read and write through the first key while believing it uses its
    /// own, and a `clear_data` through it would re-key the shared instance.
    #[test]
    fn opening_a_live_dir_with_a_different_key_is_rejected() {
        let store = Store::new();
        let mmkv = store.open();
        mmkv.put("old", "value").unwrap();

        assert!(matches!(
            store.try_open_with_key(OTHER_KEY),
            Err(EncryptFailed(_))
        ));
        // The rejection leaves the live handle untouched.
        assert_eq!(mmkv.get::<String>("old"), Ok("value".to_string()));
        mmkv.put("still", 1i32).unwrap();
        assert_eq!(mmkv.get::<i32>("still"), Ok(1));
        drop(mmkv);

        // Once every handle is gone the dir can be opened with the other key again.
        let other = store.try_open_with_key(OTHER_KEY).unwrap();
        assert_eq!(other.get::<String>("old"), Err(KeyNotFound));
        other.put("new", 2i32).unwrap();
        assert_eq!(other.get::<i32>("new"), Ok(2));
    }

    /// The key check compares decoded bytes, so the same key in another hex case is the
    /// same key and shares the live instance.
    #[test]
    fn the_same_key_in_another_hex_case_shares_the_live_instance() {
        let store = Store::new();
        let mmkv = store.open();
        mmkv.put("shared", 1i32).unwrap();

        let lower = store
            .try_open_with_key(&super::common::TEST_KEY.to_lowercase())
            .unwrap();
        assert_eq!(lower.get::<i32>("shared"), Ok(1));
        lower.put("back", 2i32).unwrap();
        assert_eq!(mmkv.get::<i32>("back"), Ok(2));
    }

    #[test]
    fn a_key_that_is_not_32_hex_chars_is_rejected() {
        let store = Store::new();
        for key in [
            "",
            "not a hex key at all",
            // one char short
            "88C51C536176AD8A8EE4A06F62EE897",
            // right length, not hex
            "ZZC51C536176AD8A8EE4A06F62EE897E",
        ] {
            assert!(
                matches!(store.try_open_with_key(key), Err(EncryptFailed(_))),
                "key {key:?} must be rejected"
            );
        }
    }
}

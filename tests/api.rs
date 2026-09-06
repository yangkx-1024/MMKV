//! Public API behaviour of [`mmkv::MMKV`]: every supported value type, the edge values,
//! type changes, missing keys, immediate visibility, custom types, persistence across a
//! reopen and `clear_data`.

mod common;

use common::Store;
use mmkv::Error::{KeyNotFound, TypeMissMatch};
use mmkv::{FromBytes, ProvideTypeToken, ToBytes, TypeToken};

#[test]
fn every_supported_value_type_roundtrips() {
    let store = Store::new();
    let mmkv = store.open();

    mmkv.put("i32", 1i32).unwrap();
    mmkv.put("i64", 2i64).unwrap();
    mmkv.put("f32", 2.2f32).unwrap();
    mmkv.put("f64", 2.22f64).unwrap();
    mmkv.put("bool", false).unwrap();
    mmkv.put("str", "four").unwrap();
    mmkv.put("byte_array", vec![1u8, 2, 3].as_slice()).unwrap();
    mmkv.put("i32_array", vec![1i32, 2, 3].as_slice()).unwrap();
    mmkv.put("i64_array", vec![1i64, 2, 3].as_slice()).unwrap();
    mmkv.put("f32_array", vec![1.1f32, 2.2, 3.3].as_slice())
        .unwrap();
    mmkv.put("f64_array", vec![1.1f64, 2.2, 3.3].as_slice())
        .unwrap();

    assert_eq!(mmkv.get::<i32>("i32"), Ok(1));
    assert_eq!(mmkv.get::<i64>("i64"), Ok(2));
    assert_eq!(mmkv.get::<f32>("f32"), Ok(2.2));
    assert_eq!(mmkv.get::<f64>("f64"), Ok(2.22));
    assert_eq!(mmkv.get::<bool>("bool"), Ok(false));
    assert_eq!(mmkv.get::<String>("str"), Ok("four".to_string()));
    assert_eq!(mmkv.get::<Vec<u8>>("byte_array"), Ok(vec![1, 2, 3]));
    assert_eq!(mmkv.get::<Vec<i32>>("i32_array"), Ok(vec![1, 2, 3]));
    assert_eq!(mmkv.get::<Vec<i64>>("i64_array"), Ok(vec![1, 2, 3]));
    assert_eq!(mmkv.get::<Vec<f32>>("f32_array"), Ok(vec![1.1, 2.2, 3.3]));
    assert_eq!(mmkv.get::<Vec<f64>>("f64_array"), Ok(vec![1.1, 2.2, 3.3]));

    // The same values come back after a reopen from disk.
    drop(mmkv);
    let mmkv = store.open();
    assert_eq!(mmkv.get::<i32>("i32"), Ok(1));
    assert_eq!(mmkv.get::<bool>("bool"), Ok(false));
    assert_eq!(mmkv.get::<String>("str"), Ok("four".to_string()));
    assert_eq!(mmkv.get::<Vec<f64>>("f64_array"), Ok(vec![1.1, 2.2, 3.3]));
}

/// Empty values are not tombstones: the key stays present and reads back as empty.
#[test]
fn empty_values_roundtrip() {
    let store = Store::new();
    let mmkv = store.open();

    mmkv.put("empty_str", "").unwrap();
    mmkv.put("empty_bytes", Vec::<u8>::new().as_slice())
        .unwrap();
    mmkv.put("empty_i32s", Vec::<i32>::new().as_slice())
        .unwrap();
    mmkv.put("empty_f64s", Vec::<f64>::new().as_slice())
        .unwrap();

    let check = |mmkv: &mmkv::MMKV| {
        assert_eq!(mmkv.get::<String>("empty_str"), Ok(String::new()));
        assert_eq!(mmkv.get::<Vec<u8>>("empty_bytes"), Ok(vec![]));
        assert_eq!(mmkv.get::<Vec<i32>>("empty_i32s"), Ok(vec![]));
        assert_eq!(mmkv.get::<Vec<f64>>("empty_f64s"), Ok(vec![]));
        // An empty value must still report a type mismatch for the wrong type.
        assert_eq!(mmkv.get::<i32>("empty_str"), Err(TypeMissMatch));
    };

    check(&mmkv);
    drop(mmkv);
    // And again after the records have been decoded back from the file.
    check(&store.open());
}

#[test]
fn large_values_and_unusual_keys_roundtrip() {
    let store = Store::new();
    let mmkv = store.open();

    let big = common::bytes(64 * 1024, 0xA7);
    let unicode_key = "键 🔑 clé";
    let long_key = "k".repeat(1000);

    mmkv.put("big", big.as_slice()).unwrap();
    mmkv.put(unicode_key, "unicode").unwrap();
    mmkv.put(&long_key, 42i32).unwrap();

    assert_eq!(mmkv.get::<Vec<u8>>("big"), Ok(big.clone()));
    assert_eq!(mmkv.get::<String>(unicode_key), Ok("unicode".to_string()));
    assert_eq!(mmkv.get::<i32>(&long_key), Ok(42));

    drop(mmkv);
    let mmkv = store.open();
    assert_eq!(mmkv.get::<Vec<u8>>("big"), Ok(big));
    assert_eq!(mmkv.get::<String>(unicode_key), Ok("unicode".to_string()));
    assert_eq!(mmkv.get::<i32>(&long_key), Ok(42));
}

#[test]
fn overwriting_a_key_with_another_type_replaces_the_type() {
    let store = Store::new();
    let mmkv = store.open();

    mmkv.put("key", 1i32).unwrap();
    assert_eq!(mmkv.get::<i32>("key"), Ok(1));
    assert_eq!(mmkv.get::<String>("key"), Err(TypeMissMatch));
    assert_eq!(mmkv.get::<bool>("key"), Err(TypeMissMatch));

    mmkv.put("key", "one").unwrap();
    assert_eq!(mmkv.get::<i32>("key"), Err(TypeMissMatch));
    assert_eq!(mmkv.get::<String>("key"), Ok("one".to_string()));

    drop(mmkv);
    let mmkv = store.open();
    assert_eq!(mmkv.get::<i32>("key"), Err(TypeMissMatch));
    assert_eq!(mmkv.get::<String>("key"), Ok("one".to_string()));
}

#[test]
fn missing_keys_report_key_not_found() {
    let store = Store::new();
    let mmkv = store.open();

    assert_eq!(mmkv.get::<i32>("never_written"), Err(KeyNotFound));
    // Deleting a key that was never written is a no-op, not an error.
    assert_eq!(mmkv.delete("never_written"), Ok(()));

    mmkv.put("key", 1i32).unwrap();
    assert_eq!(mmkv.delete("key"), Ok(()));
    assert_eq!(mmkv.get::<i32>("key"), Err(KeyNotFound));
    // And the deletion is on disk, not only in memory.
    drop(mmkv);
    assert_eq!(store.open().get::<i32>("key"), Err(KeyNotFound));
}

/// `put` and `delete` return only once the record is visible, so the very next `get`
/// on the same handle observes it without any synchronisation.
#[test]
fn put_and_delete_are_visible_immediately() {
    let store = Store::new();
    let mmkv = store.open();

    for round in 0..50i32 {
        mmkv.put("sync_key", round).unwrap();
        assert_eq!(mmkv.get::<i32>("sync_key"), Ok(round));
        mmkv.delete("sync_key").unwrap();
        assert_eq!(mmkv.get::<i32>("sync_key"), Err(KeyNotFound));
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct MyStruct {
    int_value: i32,
    str_value: String,
}

impl ProvideTypeToken for MyStruct {
    fn type_token() -> TypeToken {
        TypeToken::new(101)
    }
}

impl ToBytes for MyStruct {
    fn to_bytes(&self) -> Vec<u8> {
        let mut vec = vec![];
        vec.extend(self.int_value.to_be_bytes());
        vec.extend(self.str_value.as_bytes().to_vec());
        vec
    }
}

impl FromBytes for MyStruct {
    fn from_bytes(bytes: &[u8]) -> mmkv::Result<Self> {
        let int_len = size_of::<i32>();
        let int_value = i32::from_be_bytes(bytes[0..int_len].try_into().unwrap());
        let str_value = String::from_utf8(bytes[int_len..].to_vec()).unwrap();
        Ok(MyStruct {
            int_value,
            str_value,
        })
    }
}

#[test]
fn a_custom_type_roundtrips_through_its_type_token() {
    let store = Store::new();
    let mmkv = store.open();
    let value = MyStruct {
        int_value: 1,
        str_value: "abc".to_string(),
    };

    mmkv.put("my_struct", &value).unwrap();
    assert_eq!(mmkv.get::<MyStruct>("my_struct"), Ok(value.clone()));
    // A user token is not confused with the built-in ones.
    assert_eq!(mmkv.get::<String>("my_struct"), Err(TypeMissMatch));

    drop(mmkv);
    let mmkv = store.open();
    assert_eq!(mmkv.get::<MyStruct>("my_struct"), Ok(value));
}

#[test]
fn values_survive_dropping_every_handle_and_reopening() {
    let store = Store::new();
    let first = store.open();
    let second = store.open();
    first.put("key", "value").unwrap();

    // While any handle is alive the instance is cached and never re-read from disk.
    drop(first);
    assert_eq!(second.get::<String>("key"), Ok("value".to_string()));
    drop(second);

    // With every handle gone the next open decodes the file again.
    let reopened = store.open();
    assert_eq!(reopened.get::<String>("key"), Ok("value".to_string()));
}

#[test]
fn two_thousand_keys_roundtrip_and_survive_a_reopen() {
    const COUNT: i32 = 2000;
    let store = Store::new();
    let mmkv = store.open();

    for i in 0..COUNT {
        mmkv.put(&format!("key_{i}"), i).unwrap();
    }
    for i in 0..COUNT {
        assert_eq!(mmkv.get::<i32>(&format!("key_{i}")), Ok(i));
    }

    drop(mmkv);
    let mmkv = store.open();
    for i in 0..COUNT {
        assert_eq!(mmkv.get::<i32>(&format!("key_{i}")), Ok(i), "after reopen");
    }
}

/// `clear_data` removes the file and immediately re-creates an empty store, so the same
/// handle keeps working and old keys are gone.
#[test]
fn clear_data_empties_the_store_and_keeps_the_handle_usable() {
    let store = Store::new();
    let mmkv = store.open();
    mmkv.put("key", 1i32).unwrap();
    assert!(store.content_len() > 0);

    mmkv.clear_data().unwrap();

    assert!(store.data_file().exists(), "the store is re-created empty");
    assert_eq!(store.content_len(), 0);
    #[cfg(feature = "encryption")]
    assert!(store.meta_file().exists(), "the meta file is re-created");
    assert_eq!(mmkv.get::<i32>("key"), Err(KeyNotFound));

    // The same handle keeps working afterwards.
    mmkv.put("key", 2i32).unwrap();
    assert_eq!(mmkv.get::<i32>("key"), Ok(2));
    drop(mmkv);
    assert_eq!(store.open().get::<i32>("key"), Ok(2));
}

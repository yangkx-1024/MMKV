//! Randomized model-based test: a `HashMap` mirrors what the store should hold, and a
//! deterministic RNG drives puts, gets, deletes and reopens against both of them.
//!
//! Values range from an `i32` to a 2500-byte array, so on a 4 KiB page the writer keeps
//! appending, expanding and trimming while the model checks every read.

mod common;

use std::collections::HashMap;

use common::{Rng, Store};
use mmkv::Error::{KeyNotFound, TypeMissMatch};
use mmkv::MMKV;

const KEYS: u64 = 24;
const OPERATIONS: usize = 3000;

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Int(i32),
    Text(String),
    Bytes(Vec<u8>),
}

type Model = HashMap<String, Value>;

fn random_bytes(rng: &mut Rng, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len + 8);
    while out.len() < len {
        out.extend_from_slice(&rng.next_u64().to_be_bytes());
    }
    out.truncate(len);
    out
}

fn random_value(rng: &mut Rng) -> Value {
    match rng.below(3) {
        0 => Value::Int(rng.next_u64() as i32),
        1 => {
            let len = rng.below(65) as usize;
            Value::Text(
                random_bytes(rng, len)
                    .into_iter()
                    // Keep it printable so a failure message is readable.
                    .map(|byte| char::from(b'a' + byte % 26))
                    .collect(),
            )
        }
        // Big enough to force expands and trims at the 4 KiB page size.
        _ => {
            let len = rng.below(2501) as usize;
            Value::Bytes(random_bytes(rng, len))
        }
    }
}

fn put(mmkv: &MMKV, key: &str, value: &Value, context: &str) {
    let result = match value {
        Value::Int(int) => mmkv.put(key, *int),
        Value::Text(text) => mmkv.put(key, text.as_str()),
        Value::Bytes(bytes) => mmkv.put(key, bytes.as_slice()),
    };
    result.unwrap_or_else(|e| panic!("{context}: put {key} failed: {e:?}"));
}

/// Every read of `key` must agree with the model, for the stored type and for a wrong one.
fn check(mmkv: &MMKV, model: &Model, key: &str, context: &str) {
    match model.get(key) {
        None => {
            assert_eq!(mmkv.get::<i32>(key), Err(KeyNotFound), "{context}: {key}");
            assert_eq!(
                mmkv.get::<String>(key),
                Err(KeyNotFound),
                "{context}: {key}"
            );
        }
        Some(Value::Int(int)) => {
            assert_eq!(mmkv.get::<i32>(key), Ok(*int), "{context}: {key}");
            assert_eq!(
                mmkv.get::<String>(key),
                Err(TypeMissMatch),
                "{context}: {key}"
            );
        }
        Some(Value::Text(text)) => {
            assert_eq!(
                mmkv.get::<String>(key),
                Ok(text.clone()),
                "{context}: {key}"
            );
            assert_eq!(
                mmkv.get::<Vec<u8>>(key),
                Err(TypeMissMatch),
                "{context}: {key}"
            );
        }
        Some(Value::Bytes(bytes)) => {
            assert_eq!(
                mmkv.get::<Vec<u8>>(key),
                Ok(bytes.clone()),
                "{context}: {key}"
            );
            assert_eq!(mmkv.get::<i32>(key), Err(TypeMissMatch), "{context}: {key}");
        }
    }
}

fn run_model(seed: u64) {
    let keys: Vec<String> = (0..KEYS).map(|i| format!("k{i:02}")).collect();
    let store = Store::new();
    let mut mmkv = store.open();
    let mut model = Model::new();
    let mut rng = Rng::new(seed);

    for step in 0..OPERATIONS {
        let context = format!("seed {seed} step {step}");
        let key = keys[rng.below(KEYS) as usize].clone();
        match rng.below(100) {
            // 55% put
            0..=54 => {
                let value = random_value(&mut rng);
                put(&mmkv, &key, &value, &context);
                model.insert(key.clone(), value);
                check(&mmkv, &model, &key, &context);
            }
            // 25% read back
            55..=79 => check(&mmkv, &model, &key, &context),
            // 15% delete
            80..=94 => {
                mmkv.delete(&key)
                    .unwrap_or_else(|e| panic!("{context}: delete {key} failed: {e:?}"));
                model.remove(&key);
                check(&mmkv, &model, &key, &context);
            }
            // 5% drop every handle and reopen from disk
            _ => {
                drop(mmkv);
                mmkv = store.open();
                check(&mmkv, &model, &key, &context);
            }
        }
    }

    drop(mmkv);
    let mmkv = store.open();
    for key in &keys {
        check(&mmkv, &model, key, &format!("seed {seed} final"));
    }
}

#[test]
fn model_seed_1() {
    run_model(1);
}

#[test]
fn model_seed_2() {
    run_model(2);
}

#[test]
fn model_seed_3() {
    run_model(3);
}

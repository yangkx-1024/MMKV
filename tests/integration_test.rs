use mmkv::MMKV;
use std::thread::{spawn, JoinHandle};

fn test_api() {
    MMKV::put_i32("first", 1);
    MMKV::put_i32("second", 2);
    assert_eq!(MMKV::get_i32("first"), Some(1));
    assert_eq!(MMKV::get_str("first"), None);
    assert_eq!(MMKV::get_bool("first"), None);
    assert_eq!(MMKV::get_i32("second"), Some(2));
    assert_eq!(MMKV::get_i32("third"), None);
    MMKV::put_i32("third", 3);
    assert_eq!(MMKV::get_i32("third"), Some(3));
    MMKV::put_str("fourth", "four");
    assert_eq!(MMKV::get_str("fourth"), Some("four"));
    MMKV::put_str("first", "one");
    assert_eq!(MMKV::get_i32("first"), None);
    assert_eq!(MMKV::get_str("first"), Some("one"));
    MMKV::put_bool("second", false);
    assert_eq!(MMKV::get_str("second"), None);
    assert_eq!(MMKV::get_bool("second"), Some(false));
}

#[test]
fn test_multi_thread() {
    MMKV::initialize(
        ".",
        #[cfg(feature = "encryption")]
        "88C51C536176AD8A8EE4A06F62EE897E",
    );
    let handle = spawn(|| {
        test_api();
    });
    let mut handles = Vec::<JoinHandle<()>>::new();

    for i in 0..4 {
        handles.push(spawn(move || {
            for j in (i * 1000)..(i + 1) * 1000 {
                let key = format!("key_{j}");
                MMKV::put_str(&key, &key);
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
    handle.join().unwrap();
    for i in 0..4000 {
        let key = format!("key_{i}");
        assert_eq!(MMKV::get_str(&key).unwrap(), &key)
    }
    MMKV::clear_data();
}

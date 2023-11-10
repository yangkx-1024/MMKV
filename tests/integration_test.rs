use std::thread;

use mmkv::MMKV;

fn test_api() {
    MMKV::put_i32("first", 1).unwrap();
    MMKV::put_i32("second", 2).unwrap();
    assert_eq!(MMKV::get_i32("first"), Ok(1));
    assert_eq!(MMKV::get_str("first").is_err(), true);
    assert_eq!(MMKV::get_bool("first").is_err(), true);
    assert_eq!(MMKV::get_i32("second"), Ok(2));
    assert_eq!(MMKV::get_i32("third").is_err(), true);
    MMKV::put_i32("third", 3).unwrap();
    assert_eq!(MMKV::get_i32("third"), Ok(3));
    MMKV::put_str("fourth", "four").unwrap();
    assert_eq!(MMKV::get_str("fourth"), Ok("four".to_string()));
    MMKV::put_str("first", "one").unwrap();
    assert_eq!(MMKV::get_i32("first").is_err(), true);
    assert_eq!(MMKV::get_str("first"), Ok("one".to_string()));
    MMKV::put_bool("second", false).unwrap();
    assert_eq!(MMKV::get_str("second").is_err(), true);
    assert_eq!(MMKV::get_bool("second"), Ok(false));
}

#[test]
fn test_multi_thread() {
    MMKV::initialize(
        ".",
        #[cfg(feature = "encryption")]
            "88C51C536176AD8A8EE4A06F62EE897E",
    );
    thread::scope(|s| {
        s.spawn(|| {
            test_api();
        });
        for i in 0..4 {
            s.spawn(move || {
                for j in (i * 1000)..(i + 1) * 1000 {
                    let key = format!("key_{j}");
                    MMKV::put_str(&key, &key).unwrap();
                }
            });
        }
    });
    for i in 0..4000 {
        let key = format!("key_{i}");
        assert_eq!(MMKV::get_str(&key).unwrap(), key)
    }
    MMKV::clear_data();
}

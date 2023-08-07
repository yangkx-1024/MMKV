use std::fs;

use mmkv::MMKV;

#[test]
fn integration() {
    let _ = fs::remove_file("mini_mmkv");
    let _ = fs::remove_file("mini_mmkv.meta");
    MMKV::initialize(".", #[cfg(feature = "encryption")] "88C51C536176AD8A8EE4A06F62EE897E");
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
    println!("{}", MMKV::dump());
    let _ = fs::remove_file("mini_mmkv");
    let _ = fs::remove_file("mini_mmkv.meta");
}
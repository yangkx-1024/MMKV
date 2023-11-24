use mmkv::Error::KeyNotFound;
use mmkv::MMKV;
use std::fs;

#[test]
fn integration_test() {
    let _ = fs::remove_file("mini_mmkv");
    let _ = fs::remove_file("mini_mmkv.meta");
    for i in 0..10 {
        println!("repeat {}", i);
        MMKV::initialize(
            ".",
            #[cfg(feature = "encryption")]
            "88C51C536176AD8A8EE4A06F62EE897E",
        );
        let result = MMKV::get_str("integration_test");
        if i == 0 {
            assert_eq!(result, Err(KeyNotFound));
        } else {
            assert_eq!(result, Ok((i - 1).to_string()))
        }
        MMKV::put_str("integration_test", &i.to_string()).unwrap();
    }
    MMKV::clear_data();
}

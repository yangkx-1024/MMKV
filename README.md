[![Crates.io](https://img.shields.io/crates/l/MMKV)](https://crates.io/crates/mmkv)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](https://github.com/yangkx1024/MMKV/pulls)
[![Crates.io](https://img.shields.io/crates/v/MMKV)](https://crates.io/crates/mmkv)
[![Crates.io](https://img.shields.io/crates/d/MMKV)](https://crates.io/crates/mmkv)

# Library uses file-based mmap to store key-values

This is a Rust version of [MMKV](https://github.com/Tencent/MMKV).
By default, this lib uses CRC8 to check data integrity.

If include feature `encryption`, this lib will encrypt 
the data with [AES-EAX](https://github.com/RustCrypto/AEADs/tree/master/eax). 

### How to use
Add dependency:

`cargo add mmkv`

And use `MMKV` directly:
```rust
use mmkv::MMKV;

fn main() {
    // initialize it with a directory, 
    // the library will crate a file,
    // named "mini_mmkv" under this dir
    MMKV::initialize(".");
    MMKV::put_i32("key1", 1);
    // Some(1)
    println!("{:?}", MMKV::get_i32("key1"));
    MMKV::put_str("key1", "value");
    // None, cause "key1" was override by put_str
    println!("{:?}", MMKV::get_i32("key1"));
    // Some("value")
    println!("{:?}", MMKV::get_str("key1"));
    MMKV::put_bool("key1", true);
    // Some(true)
    println!("{:?}", MMKV::get_bool("key1"));
}
```

### Use with encryption feature
Add dependency:

`cargo add mmkv --features encryption`

Then init `MMKV` with an encryption credential:

`MMKV::initialize(".", "88C51C536176AD8A8EE4A06F62EE897E")`

Encryption will greatly reduce the efficiency of reading and writing, 
and will also increase the file size, use at your own risk!
![Crates.io](https://img.shields.io/crates/l/MMKV)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](https://github.com/yangkx1024/MMKV/pulls)
[![Crates.io](https://img.shields.io/crates/v/MMKV)](https://crates.io/crates/mmkv)
![Crates.io](https://img.shields.io/crates/d/MMKV)
![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/yangkx1024/MMKV/rust.yml)

# Library uses file-based mmap to store key-values

This is a simple rust version of [mmkv](https://github.com/Tencent/MMKV), 
only part of the core features have been implemented so far, 
and it is still far from production availability.

### How to use
Add dependency:
```toml
[dependencies]
mmkv = { version = "0.1.0" }
```
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

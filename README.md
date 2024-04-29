# Library uses file-based mmap to store key-values

[![Crates.io](https://img.shields.io/crates/l/MMKV)](https://crates.io/crates/mmkv)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](https://github.com/yangkx1024/MMKV/pulls)
[![Crates.io](https://img.shields.io/crates/v/MMKV)](https://crates.io/crates/mmkv)
[![Crates.io](https://img.shields.io/crates/d/MMKV)](https://crates.io/crates/mmkv)
[![Cargo Test](https://github.com/yangkx-1024/MMKV/actions/workflows/rust.yml/badge.svg)](https://github.com/yangkx-1024/MMKV/actions/workflows/rust.yml)
[![Android Build Check](https://github.com/yangkx-1024/MMKV/actions/workflows/android.yml/badge.svg)](https://github.com/yangkx-1024/MMKV/actions/workflows/android.yml)
[![Swift Test](https://github.com/yangkx-1024/MMKV/actions/workflows/swift.yml/badge.svg)](https://github.com/yangkx-1024/MMKV/actions/workflows/swift.yml)

This is a Rust version of [MMKV](https://github.com/Tencent/MMKV).

By default, this lib uses [CRC8](https://github.com/mrhooray/crc-rs) to check data integrity.

If include feature `encryption`, this lib will encrypt the data
with [AES-EAX](https://github.com/RustCrypto/AEADs/tree/master/eax).

MMKV is thread-safe but cannot guarantee cross-process data consistency.
If you want to use it in a cross-process scenario, please ensure that there is no competing write.

## How to use

Add dependency:

`cargo add mmkv`

And use `MMKV` directly:

```rust
use mmkv::{LogLevel, MMKV};

fn main() {
    // Set the log level of the library
    MMKV::set_log_level(LogLevel::Verbose);
    let temp_dir = std::env::temp_dir();
    let dir = temp_dir.join("test1");
    let _ = std::fs::create_dir(&dir);
    // Initialize it with a directory, the library will crate a file,
    // named "mini_mmkv" under this dir.
    let mmkv = MMKV::new(dir.to_str().unwrap());
    // Put and get result, for most case should be Ok(()),
    // if something wrong, it contains the useful info.
    let ret = mmkv.put_i32("key1", 1);
    println!("{:?}", ret); // Ok(())
    // Get result with key
    println!("{:?}", mmkv.get_i32("key1")); // Ok(1)
    // Put and unwrap the result
    mmkv.put_str("key1", "value").unwrap();
    println!("{:?}", mmkv.get_i32("key1")); // Err(TypeMissMatch)
    println!("{:?}", mmkv.get_str("key1")); // Ok("value")

    let dir = temp_dir.join("test2");
    let _ = std::fs::create_dir(&dir);
    // Create another instance with different path
    let new_mmkv = MMKV::new(dir.to_str().unwrap());
    new_mmkv.put_bool("key1", true).unwrap();
    println!("{:?}", new_mmkv.get_bool("key1")); // Ok(true)
    // clear all data to free disk space
    new_mmkv.clear_data();
}
```

## Use with encryption feature

Add dependency:

`cargo add mmkv --features encryption`

Then init `MMKV` with an encryption credential:

`let mmkv = MMKV::new(".", "88C51C536176AD8A8EE4A06F62EE897E")`

Encryption will greatly reduce the efficiency of reading and writing, and will also increase the file size, use at your
own risk!

## Use in Android projects

Add lib dependency to gradle:

```kotlin
dependencies {
    implementation("net.yangkx:mmkv:0.4.0")
    // Or another one with encryption feature
    // implementation("net.yangkx:mmkv-encrypt:0.4.0")
}
```

You can find all versions in the [Releases](https://github.com/yangkx-1024/MMKV/releases).

Use the kotlin API:

```kotlin
class MyApplication : Application() {

    companion object {
        lateinit var mmkv: MMKV
    }

    override fun onCreate() {
        super.onCreate()
        MMKV.setLogLevel(LogLevel.VERBOSE)
        val dir = this.getDir("mmkv", Context.MODE_PRIVATE)
        mmkv = MMKV(dir.absolutePath)
        // If you are using mmkv with encryption
        // mmkv = MMKV(dir.absolutePath, "88C51C536176AD8A8EE4A06F62EE897E")
    }
}

class MainActivity : AppCompatActivity() {

    private val mmkv: MMKV
        get() = MyApplication.mmkv

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)
        mmkv.putString("first_key", "first value")
        mmkv.putInt("second_key", 1024)
        mmkv.putBool("third_key", true)
        binding.string.text = mmkv.getString("first_key", "default")
        binding.integer.text = mmkv.getInt("second_key", 0).toString()
        binding.bool.text = mmkv.getBool("third_key", false).toString()
    }
}
```

Check the [android](https://github.com/yangkx1024/MMKV/tree/main/android) demo for more detail.

## Use in iOS project

1. Add this repo as swift package dependency to your Xcode project.

2. Init MMKV instance before use the API, for example:

   ```swift
   import Foundation
   import MMKV

   class MMKVManager {
       static var inst = initMMKV()
   }

   func initMMKV() -> MMKV {
       let paths = NSSearchPathForDirectoriesInDomains(.documentDirectory, .userDomainMask, true)
       let documentsDirectory = paths[0]
       let docURL = URL(string: documentsDirectory)!
       let dataPath = docURL.appendingPathComponent("mmkv")
       if !FileManager.default.fileExists(atPath: dataPath.path) {
           do {
               try FileManager.default.createDirectory(atPath: dataPath.path, withIntermediateDirectories: true, attributes: nil)
           } catch {
               print(error.localizedDescription)
           }
       }
       return MMKV(dataPath.path)
   }
   ```

3. Use `MMKV.shared` to access API directly:

    ```swift
    import SwiftUI
    import MMKV

    struct ContentView: View {
        @State var textContent: String = "Hello, world!"
        var body: some View {
            Text(textContent)
                .onTapGesture {
                    let value = MMKVManager.inst.getInt32(key: "int_key").unwrap(defalutValue: 0)
                    MMKVManager.inst.putInt32(key: "int_key", value: value + 1).unwrap(defalutValue: ())
                    textContent = MMKVManager.inst.getInt32(key: "int_key").unwrap(defalutValue: 0).formatted()
                }
        }
    }
    ```

Check the [ios](https://github.com/yangkx1024/MMKV/tree/main/ios) demo for more detail.

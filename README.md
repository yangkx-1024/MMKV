[![Crates.io](https://img.shields.io/crates/l/MMKV)](https://crates.io/crates/mmkv)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](https://github.com/yangkx1024/MMKV/pulls)
[![Crates.io](https://img.shields.io/crates/v/MMKV)](https://crates.io/crates/mmkv)
[![Crates.io](https://img.shields.io/crates/d/MMKV)](https://crates.io/crates/mmkv)

# Library uses file-based mmap to store key-values

This is a Rust version of [MMKV](https://github.com/Tencent/MMKV).

By default, this lib uses [CRC8](https://github.com/mrhooray/crc-rs) to check data integrity.

If include feature `encryption`, this lib will encrypt 
the data with [AES-EAX](https://github.com/RustCrypto/AEADs/tree/master/eax). 

MMKV is thread-safe but cannot guarantee cross-process data consistency (still under development). 
If you want to use it in a cross-process scenario, please ensure that there is no competing write.

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
    // Ok(1)
    println!("{:?}", MMKV::get_i32("key1"));
    MMKV::put_str("key1", "value");
    // Err(Error::KeyNotFound), cause "key1" was override by put_str
    println!("{:?}", MMKV::get_i32("key1"));
    // Ok("value")
    println!("{:?}", MMKV::get_str("key1"));
    MMKV::put_bool("key1", true);
    // Ok(true)
    println!("{:?}", MMKV::get_bool("key1"));
    // close the instance if you need to re-initialize with different config.
    // MMKV::close();
    // clear all related data to free disk space, this call will also close the instance.
    // MMKV::clear_data();
}
```

### Use with encryption feature
Add dependency:

`cargo add mmkv --features encryption`

Then init `MMKV` with an encryption credential:

`MMKV::initialize(".", "88C51C536176AD8A8EE4A06F62EE897E")`

Encryption will greatly reduce the efficiency of reading and writing, 
and will also increase the file size, use at your own risk!

# Use in Android projects
Add lib dependency to gradle:
```kotlin
dependencies {
    implementation("net.yangkx:mmkv:0.2.4")
    // Or another one with encryption feature
    // implementation("net.yangkx:mmkv-encrypt:0.2.4")
}
```
Use the kotlin API:
```kotlin
class MyApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        val dir = this.getDir("mmkv", Context.MODE_PRIVATE)
        MMKV.initialize(dir.absolutePath)
        // If you are using mmkv with encryption
        // MMKV.initialize(dir.absolutePath, "88C51C536176AD8A8EE4A06F62EE897E")
    }
}

class MainActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)
        MMKV.putString("first_key", "first value")
        MMKV.putInt("second_key", 1024)
        MMKV.putBool("third_key", true)
        binding.string.text = MMKV.getString("first_key", "default")
        binding.integer.text = MMKV.getInt("second_key", 0).toString()
        binding.bool.text = MMKV.getBool("third_key", false).toString()
    }
}
```
Check the [android](https://github.com/yangkx1024/MMKV/tree/main/android) demo for more detail.

# Use in iOS project
Since this repo is using git-lfs, and SwiftPM doesn't support git-lfs(check this [issue](https://github.com/git-lfs/git-lfs/issues/3587)), 
You need to clone this repo to your machine, and add `path-to-this-repo/ios/MMKV` as a Swift package to your project.
Then add code to your `App.swift`:
```swift
import SwiftUI
import MMKV

@main
struct MMKVDemoApp: App {
    init() {
        initMMKV()
    }
    var body: some Scene {
        ...
    }
}

func initMMKV() {
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
    MMKV.initialize(dir: dataPath.path)
}
```
And then you can use MMKV API directly:
```swift
import SwiftUI
import MMKV

struct ContentView: View {
    @State var textContent: String = "Hello, world!"
    var body: some View {
        Text(textContent)
            .onTapGesture {
                let value = MMKV.getInt32(key: "int_key").unwrap(defalutValue: 0)
                MMKV.putInt32(key: "int_key", value: value + 1).unwrap(defalutValue: ())
                textContent = MMKV.getInt32(key: "int_key").unwrap(defalutValue: 0).formatted()
            }
    }
}
```
Check the [ios](https://github.com/yangkx1024/MMKV/tree/main/ios) demo for more detail.
import Foundation
import RustMMKV

/**
 Wrapper of MMKV FFI call
 
 Usage:
 ```swift
 import MMKV
 
 // Init with a writable dir
 MMKV.shared.initialize(".")
 // Unwrap the result, if failed it will throw `MMKVError`
 try MMKV.shared.putString("key", "value").unwrap()
 // Or ignore the error with a default value, which is Void for put
 MMKV.shared.putString("key", "value").unwrap(())
 // Get value, if failed it will throw `MMKVError`
 try let value = MMKV.shared.getString("key").unwrap()
 // Or ignore the error with a default value
 let value = MMKV.shared.getString("key").unwrap("")
 // Close the instance if you wan't to init with different data dir
 MMKV.shared.close()
 // Must be reinitialized to continue using MMKV
 MMKV.shared.initialize("new_dir")
 // Clear all MMKV data in current dir, usually to free storage space
 MMKV.shared.clearData()
 ```
  */
public class MMKV {
    
    private init() {}
    /**
     Singleton of MMKV
     */
    public static let shared  = MMKV()
    
    /**
     Initialize the MMKV instance.
     
     All API calls before initialization will cause error.
     
     Calling ``initialize(_:)`` multiple times is allowed,
     the old instance will be closed (see ``close()``), the last call will take over.
     
     - Parameter dir: A writeable directory
     */
    public func initialize(_ dir: String) {
        RustMMKV.initialize(dir)
    }
    
    /**
     Set a log handler of MMKV log.
     
     - Parameter logger: See ``MMKVLogger``
     */
    public func setLogger(_ logger: MMKVLogger) {
        RustMMKV.set_logger(LogWrapper(logger: logger).toNativeLogger())
    }
    
    /**
     Filter log output of MMKV logger
     
     - Parameter logLevel: See ``LogLevel``
     */
    public func setLogLevel(_ logLevel: LogLevel) {
        let logLevelInt: Int32 = switch logLevel {
        case .off: 0
        case .error: 1
        case .warn: 2
        case .info: 3
        case .debug: 4
        case .trace: 5
        }
        RustMMKV.set_log_level(logLevelInt)
    }
    
    /**
     Close the instance to allow MMKV to initialize with different config.
     
     If you want to continue using the API, need to ``initialize(_:)`` again.
     */
    public func close() {
        RustMMKV.close_instance()
    }
    
    /**
     Clear all data and ``close()`` the instance.
     
     If you want to continue using the API, need to ``initialize(_:)`` again.
     */
    public func clearData() {
        RustMMKV.clear_data()
    }
    
    public func putString(_ key: String, _ value: String) -> ResultWrapper<Void> {
        RustMMKV.put_str(key, value).intoResultWrapper()
    }
    
    public func getString(_ key: String) -> ResultWrapper<String> {
        return RustMMKV.get_str(key).intoResultWrapper()
    }
    
    public func putBool(_ key: String, _ value: Bool) -> ResultWrapper<Void> {
        return RustMMKV.put_bool(key, value).intoResultWrapper()
    }
    
    public func getBool(_ key: String) -> ResultWrapper<Bool> {
        return RustMMKV.get_bool(key).intoResultWrapper()
    }
    
    public func putInt32(_ key: String, _ value: Int32) -> ResultWrapper<Void> {
        return RustMMKV.put_i32(key, value).intoResultWrapper()
    }
    
    public func getInt32(_ key: String) -> ResultWrapper<Int32> {
        return RustMMKV.get_i32(key).intoResultWrapper()
    }
    
    public func putInt64(_ key: String, _ value: Int64) -> ResultWrapper<Void> {
        return RustMMKV.put_i64(key, value).intoResultWrapper()
    }
    
    public func getInt64(_ key: String) -> ResultWrapper<Int64> {
        return RustMMKV.get_i64(key).intoResultWrapper()
    }
    
    public func putFloat32(_ key: String, _ value: Float32) -> ResultWrapper<Void> {
        return RustMMKV.put_f32(key, value).intoResultWrapper()
    }
    
    public func getFloat32(_ key: String) -> ResultWrapper<Float32> {
        return RustMMKV.get_f32(key).intoResultWrapper()
    }
    
    public func putFloat64(_ key: String, _ value: Float64) -> ResultWrapper<Void> {
        return RustMMKV.put_f64(key, value).intoResultWrapper()
    }
    
    public func getFloat64(_ key: String) -> ResultWrapper<Float64> {
        return RustMMKV.get_f64(key).intoResultWrapper()
    }
    
    public func putByteArray(_ key: String, _ value: Array<UInt8>) -> ResultWrapper<Void> {
        return RustMMKV.put_byte_array(key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public func getByteArray(_ key: String) -> ResultWrapper<[UInt8]> {
        return RustMMKV.get_byte_array(key).intoResultWrapper()
    }
    
    public func putInt32Array(_ key: String, _ value: Array<Int32>) -> ResultWrapper<Void> {
        return RustMMKV.put_i32_array(key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public func getInt32Array(_ key: String) -> ResultWrapper<[Int32]> {
        return RustMMKV.get_i32_array(key).intoResultWrapper()
    }
    
    public func putInt64Array(_ key: String, _ value: Array<Int64>) -> ResultWrapper<Void> {
        return RustMMKV.put_i64_array(key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public func getInt64Array(_ key: String) -> ResultWrapper<[Int64]> {
        return RustMMKV.get_i64_array(key).intoResultWrapper()
    }
    
    public func putFloat32Array(_ key: String, _ value: Array<Float32>) -> ResultWrapper<Void> {
        return RustMMKV.put_f32_array(key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public func getFloat32Array(_ key: String) -> ResultWrapper<[Float]> {
        return RustMMKV.get_f32_array(key).intoResultWrapper()
    }
    
    public func putFloat64Array(_ key: String, _ value: Array<Float64>) -> ResultWrapper<Void> {
        return RustMMKV.put_f64_array(key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public func getFloat64Array(_ key: String) -> ResultWrapper<[Float64]> {
        return RustMMKV.get_f64_array(key).intoResultWrapper()
    }
}

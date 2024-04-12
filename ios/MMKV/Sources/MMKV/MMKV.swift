import Foundation
import RustMMKV

/**
 Wrapper of MMKV FFI call
 
 Usage:
 ```swift
 import MMKV
 
 // Init with a writable dir
 let mmkv = MMKV(".")
 // Unwrap the result, if failed it will throw `MMKVError`
 try mmkv.putString("key", "value").unwrap()
 // Or ignore the error with a default value, which is Void for put
 mmkv.putString("key", "value").unwrap(())
 // Get value, if failed it will throw `MMKVError`
 try let value = mmkv.getString("key").unwrap()
 // Or ignore the error with a default value
 let value = mmkv.getString("key").unwrap("")
 // Delete key
 try mmkv.delete("key").unwrap()
 // Or ignore the error with a default value, which is Void for delete
 mmkv.delete("key").unwrap(())
 // Clear all MMKV data in current dir, usually to free storage space
 mmkv.clearData()
 ```
  */
public class MMKV {
    private let rawPointer: UnsafeRawPointer
    
    /**
     Initialize the MMKV instance.
     
     - Parameter dir: A writeable directory
     */
    public init(_ dir: String) {
        rawPointer = RustMMKV.new_instance(dir)
    }
    
    deinit {
        RustMMKV.close_instance(rawPointer)
    }
    
    /**
     Set a log handler of MMKV log.
     
     - Parameter logger: See ``MMKVLogger``
     */
    public static func setLogger(_ logger: MMKVLogger) {
        RustMMKV.set_logger(LogWrapper(logger: logger).toNativeLogger())
    }
    
    /**
     Filter log output of MMKV logger
     
     - Parameter logLevel: See ``LogLevel``
     */
    public static func setLogLevel(_ logLevel: LogLevel) {
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
     Clear all data.
    */
    public func clearData() {
        RustMMKV.clear_data(rawPointer)
    }
    
    public func putString(_ key: String, _ value: String) -> ResultWrapper<Void> {
        RustMMKV.put_str(rawPointer, key, value).intoResultWrapper()
    }
    
    public func getString(_ key: String) -> ResultWrapper<String> {
        return RustMMKV.get_str(rawPointer, key).intoResultWrapper()
    }
    
    public func putBool(_ key: String, _ value: Bool) -> ResultWrapper<Void> {
        return RustMMKV.put_bool(rawPointer, key, value).intoResultWrapper()
    }
    
    public func getBool(_ key: String) -> ResultWrapper<Bool> {
        return RustMMKV.get_bool(rawPointer, key).intoResultWrapper()
    }
    
    public func putInt32(_ key: String, _ value: Int32) -> ResultWrapper<Void> {
        return RustMMKV.put_i32(rawPointer, key, value).intoResultWrapper()
    }
    
    public func getInt32(_ key: String) -> ResultWrapper<Int32> {
        return RustMMKV.get_i32(rawPointer, key).intoResultWrapper()
    }
    
    public func putInt64(_ key: String, _ value: Int64) -> ResultWrapper<Void> {
        return RustMMKV.put_i64(rawPointer, key, value).intoResultWrapper()
    }
    
    public func getInt64(_ key: String) -> ResultWrapper<Int64> {
        return RustMMKV.get_i64(rawPointer, key).intoResultWrapper()
    }
    
    public func putFloat32(_ key: String, _ value: Float32) -> ResultWrapper<Void> {
        return RustMMKV.put_f32(rawPointer, key, value).intoResultWrapper()
    }
    
    public func getFloat32(_ key: String) -> ResultWrapper<Float32> {
        return RustMMKV.get_f32(rawPointer, key).intoResultWrapper()
    }
    
    public func putFloat64(_ key: String, _ value: Float64) -> ResultWrapper<Void> {
        return RustMMKV.put_f64(rawPointer, key, value).intoResultWrapper()
    }
    
    public func getFloat64(_ key: String) -> ResultWrapper<Float64> {
        return RustMMKV.get_f64(rawPointer, key).intoResultWrapper()
    }
    
    public func putByteArray(_ key: String, _ value: Array<UInt8>) -> ResultWrapper<Void> {
        return RustMMKV.put_byte_array(rawPointer, key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public func getByteArray(_ key: String) -> ResultWrapper<[UInt8]> {
        return RustMMKV.get_byte_array(rawPointer, key).intoResultWrapper()
    }
    
    public func putInt32Array(_ key: String, _ value: Array<Int32>) -> ResultWrapper<Void> {
        return RustMMKV.put_i32_array(rawPointer, key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public func getInt32Array(_ key: String) -> ResultWrapper<[Int32]> {
        return RustMMKV.get_i32_array(rawPointer, key).intoResultWrapper()
    }
    
    public func putInt64Array(_ key: String, _ value: Array<Int64>) -> ResultWrapper<Void> {
        return RustMMKV.put_i64_array(rawPointer, key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public func getInt64Array(_ key: String) -> ResultWrapper<[Int64]> {
        return RustMMKV.get_i64_array(rawPointer, key).intoResultWrapper()
    }
    
    public func putFloat32Array(_ key: String, _ value: Array<Float32>) -> ResultWrapper<Void> {
        return RustMMKV.put_f32_array(rawPointer, key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public func getFloat32Array(_ key: String) -> ResultWrapper<[Float]> {
        return RustMMKV.get_f32_array(rawPointer, key).intoResultWrapper()
    }
    
    public func putFloat64Array(_ key: String, _ value: Array<Float64>) -> ResultWrapper<Void> {
        return RustMMKV.put_f64_array(rawPointer, key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public func getFloat64Array(_ key: String) -> ResultWrapper<[Float64]> {
        return RustMMKV.get_f64_array(rawPointer, key).intoResultWrapper()
    }
    
    public func delete(_ key: String) -> ResultWrapper<Void> {
        return RustMMKV.delete(rawPointer, key).intoResultWrapper()
    }
}

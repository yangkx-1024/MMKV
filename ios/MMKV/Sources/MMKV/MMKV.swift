import Foundation
import RustMMKV

public class MMKV {
    
    private init() {}
    public static let shared  = MMKV()
    
    /**
     Initialize the MMKV instance.
     
     All API calls before initialization will cause error.
     
     Calling ``initialize(dir:)`` multiple times is allowed,
     the old instance will be closed (see ``close()``), the last call will take over.
     
     - Parameter dir: a writeable directory
     */
    public func initialize(dir: String) {
        RustMMKV.initialize(dir)
    }
    
    public func setLogger(logger: MMKVLogger) {
        RustMMKV.set_logger(LogWrapper(logger: logger).toNativeLogger())
    }
    
    /**
     Close the instance to allow MMKV to initialize with different config.
     
     If you want to continue using the API, need to ``initialize(dir:)`` again.
     */
    public func close() {
        RustMMKV.close_instance()
    }
    
    /**
     Clear all data and ``close()`` the instance.
     
     If you want to continue using the API, need to ``initialize(dir:)`` again.
     */
    public func clearData() {
        RustMMKV.clear_data()
    }
    
    public func putString(key: String, value: String) -> ResultWrapper<Void> {
        RustMMKV.put_str(key, value).intoResultWrapper()
    }
    
    public func getString(key: String) -> ResultWrapper<String> {
        return RustMMKV.get_str(key).intoResultWrapper()
    }
    
    public func putBool(key: String, value: Bool) -> ResultWrapper<Void> {
        return RustMMKV.put_bool(key, value).intoResultWrapper()
    }
    
    public func getBool(key: String) -> ResultWrapper<Bool> {
        return RustMMKV.get_bool(key).intoResultWrapper()
    }
    
    public func putInt32(key: String, value: Int32) -> ResultWrapper<Void> {
        return RustMMKV.put_i32(key, value).intoResultWrapper()
    }
    
    public func getInt32(key: String) -> ResultWrapper<Int32> {
        return RustMMKV.get_i32(key).intoResultWrapper()
    }
    
    public func putInt64(key: String, value: Int64) -> ResultWrapper<Void> {
        return RustMMKV.put_i64(key, value).intoResultWrapper()
    }
    
    public func getInt64(key: String) -> ResultWrapper<Int64> {
        return RustMMKV.get_i64(key).intoResultWrapper()
    }

    public func putFloat32(key: String, value: Float32) -> ResultWrapper<Void> {
        return RustMMKV.put_f32(key, value).intoResultWrapper()
    }
    
    public func getFloat32(key: String) -> ResultWrapper<Float32> {
        return RustMMKV.get_f32(key).intoResultWrapper()
    }
    
    public func putFloat64(key: String, value: Float64) -> ResultWrapper<Void> {
        return RustMMKV.put_f64(key, value).intoResultWrapper()
    }
    
    public func getFloat64(key: String) -> ResultWrapper<Float64> {
        return RustMMKV.get_f64(key).intoResultWrapper()
    }
    
    public func putByteArray(key: String, value: Array<UInt8>) -> ResultWrapper<Void> {
        return RustMMKV.put_byte_array(key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public func getByteArray(key: String) -> ResultWrapper<[UInt8]> {
        return RustMMKV.get_byte_array(key).intoResultWrapper()
    }
    
    public func putInt32Array(key: String, value: Array<Int32>) -> ResultWrapper<Void> {
        return RustMMKV.put_i32_array(key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public func getInt32Array(key: String) -> ResultWrapper<[Int32]> {
        return RustMMKV.get_i32_array(key).intoResultWrapper()
    }
    
    public func putInt64Array(key: String, value: Array<Int64>) -> ResultWrapper<Void> {
        return RustMMKV.put_i64_array(key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public func getInt64Array(key: String) -> ResultWrapper<[Int64]> {
        return RustMMKV.get_i64_array(key).intoResultWrapper()
    }
    
    public func putFloat32Array(key: String, value: Array<Float32>) -> ResultWrapper<Void> {
        return RustMMKV.put_f32_array(key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public func getFloat32Array(key: String) -> ResultWrapper<[Float]> {
        return RustMMKV.get_f32_array(key).intoResultWrapper()
    }
    
    public func putFloat64Array(key: String, value: Array<Float64>) -> ResultWrapper<Void> {
        return RustMMKV.put_f64_array(key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public func getFloat64Array(key: String) -> ResultWrapper<[Float64]> {
        return RustMMKV.get_f64_array(key).intoResultWrapper()
    }
}

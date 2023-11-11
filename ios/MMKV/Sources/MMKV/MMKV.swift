import Foundation
import RustMMKV
import Logging

private extension ByteSlice {
    private func asUnsafeBufferPointer() -> UnsafeBufferPointer<UInt8> {
        return UnsafeBufferPointer(start: self.bytes, count: Int(self.len))
    }
    
    func asString() -> String? {
        return String(bytes: asUnsafeBufferPointer(), encoding: String.Encoding.utf8)
    }
}

private extension UnsafePointer<ByteSlice> {
    func asString() -> String? {
        return self.pointee.asString()
    }
}

private extension UnsafePointer<RawTypedArray> {
    func readArray<T> (source: T.Type) -> Array<T> {
        let count = Int(self.pointee.len)
        var array = Array<T>()
        array.reserveCapacity(count)
        let ptr = self.pointee.array.assumingMemoryBound(to: source)
        let sourceArray = UnsafeBufferPointer(start: ptr, count: count)
        for i in 0...count - 1 {
            array.append(sourceArray[i])
        }
        return array
    }
}

private extension UnsafePointer<InternalError> {
    private func code() -> Int32 {
        return Int32(self.pointee.code)
    }
    
    private func reason() -> String? {
        if self.pointee.reason == nil {
            return nil
        }
        return self.pointee.reason.asString()
    }
    
    func asMMKVError() -> MMKVError {
        return MMKVError.native(code: code(), reason: reason())
    }
}

private extension UnsafeRawPointer {
    func bindType<T>(type: T.Type) -> UnsafePointer<T> {
        self.assumingMemoryBound(to: type)
    }
}

public class ResultWrapper<T> {
    private let rawBuffer: UnsafePointer<RawBuffer>
    
    init(rawBuffer: UnsafePointer<RawBuffer>) {
        self.rawBuffer = rawBuffer
    }
    
    deinit {
        RustMMKV.free_buffer(rawBuffer)
    }
    
    public func unwrapResult() -> Result<T, MMKVError> {
        if rawBuffer.pointee.err != nil {
            return Result.failure(rawBuffer.pointee.err.asMMKVError())
        }
        if rawBuffer.pointee.rawData == nil {
            return Result.success(() as! T)
        }
        let rawData = rawBuffer.pointee.rawData!
        let data: Any = switch rawBuffer.pointee.typeToken {
        case RustMMKV.I32:
            Int32(rawData.bindType(type: Int32.self).pointee)
        case RustMMKV.Str:
            rawData.bindType(type: ByteSlice.self).asString()!
        case RustMMKV.Bool:
            Bool(rawData.bindType(type: Bool.self).pointee)
        case RustMMKV.I64:
            Int64(rawData.bindType(type: Int64.self).pointee)
        case RustMMKV.F32:
            Float32(rawData.bindType(type: Float32.self).pointee)
        case RustMMKV.F64:
            Float64(rawData.bindType(type: Float64.self).pointee)
        case RustMMKV.ByteArray:
            rawData.bindType(type: RawTypedArray.self).readArray(source: UInt8.self)
        case RustMMKV.I32Array:
            rawData.bindType(type: RawTypedArray.self).readArray(source: Int32.self)
        case RustMMKV.I64Array:
            rawData.bindType(type: RawTypedArray.self).readArray(source: Int64.self)
        case RustMMKV.F32Array:
            rawData.bindType(type: RawTypedArray.self).readArray(source: Float32.self)
        case RustMMKV.F64Array:
            rawData.bindType(type: RawTypedArray.self).readArray(source: Float64.self)
        default:
            fatalError("should not happen")
        }
        return Result.success(data as! T)
    }
    
    public func unwrap(defalutValue: T) -> T {
        switch unwrapResult() {
        case .success(let value):
            return value
        case .failure(_):
            return defalutValue
        }
    }
    
    public func unwrap() throws -> T {
        switch unwrapResult() {
        case .failure(let e):
            throw e
        case .success(let value):
            return value
        }
    }
}

private extension UnsafePointer<RawBuffer> {
    func intoResultWrapper<T>() -> ResultWrapper<T> {
        return ResultWrapper(rawBuffer: self)
    }
}

public enum MMKVError: Error, Equatable {
    case native(code: Int32, reason: String?)
}

private class LogWrapper {
    
    private let logger: Logger
    
    init(logger: Logger) {
        self.logger = logger
    }
    
    func log(level: Int32, content: UnsafePointer<ByteSlice>) {
        let message = Logger.Message(stringLiteral: content.pointee.asString()!)
        switch level {
        case 1:
            logger.error(message)
        case 2:
            logger.warning(message)
        case 3:
            logger.info(message)
        case 4:
            logger.debug(message)
        default:
            logger.trace(message)
        }
    }
    
    func toNativeLogger() -> NativeLogger {
        let ownedPointer = Unmanaged.passRetained(self).toOpaque()
        let callback: (@convention(c) (UnsafeMutableRawPointer?, Int32, UnsafePointer<ByteSlice>?) -> Void) = {
            (obj, level, content) -> Void in
            let swiftObj: LogWrapper = Unmanaged.fromOpaque(obj!).takeUnretainedValue()
            swiftObj.log(level: level, content: content!)
        }
        return NativeLogger(obj: ownedPointer, callback: callback)
    }
}

public class MMKV {
    
    private init() {}
    private static let logger = Logger(label: "net.yangkx.MMKV")
    private static let logwrapper = LogWrapper(logger: logger)
    
    public static func initialize(dir: String) {
        RustMMKV.initialize(dir, logwrapper.toNativeLogger())
    }
    
    public static func close() {
        RustMMKV.close_instance()
    }
    
    public static func clearData() {
        RustMMKV.clear_data()
    }
    
    public static func putString(key: String, value: String) -> ResultWrapper<Void> {
        RustMMKV.put_str(key, value).intoResultWrapper()
    }
    
    public static func getString(key: String) -> ResultWrapper<String> {
        return RustMMKV.get_str(key).intoResultWrapper()
    }
    
    public static func putBool(key: String, value: Bool) -> ResultWrapper<Void> {
        return RustMMKV.put_bool(key, value).intoResultWrapper()
    }
    
    public static func getBool(key: String) -> ResultWrapper<Bool> {
        return RustMMKV.get_bool(key).intoResultWrapper()
    }
    
    public static func putInt32(key: String, value: Int32) -> ResultWrapper<Void> {
        return RustMMKV.put_i32(key, value).intoResultWrapper()
    }
    
    public static func getInt32(key: String) -> ResultWrapper<Int32> {
        return RustMMKV.get_i32(key).intoResultWrapper()
    }
    
    public static func putInt64(key: String, value: Int64) -> ResultWrapper<Void> {
        return RustMMKV.put_i64(key, value).intoResultWrapper()
    }
    
    public static func getInt64(key: String) -> ResultWrapper<Int64> {
        return RustMMKV.get_i64(key).intoResultWrapper()
    }

    public static func putFloat32(key: String, value: Float32) -> ResultWrapper<Void> {
        return RustMMKV.put_f32(key, value).intoResultWrapper()
    }
    
    public static func getFloat32(key: String) -> ResultWrapper<Float32> {
        return RustMMKV.get_f32(key).intoResultWrapper()
    }
    
    public static func putFloat64(key: String, value: Float64) -> ResultWrapper<Void> {
        return RustMMKV.put_f64(key, value).intoResultWrapper()
    }
    
    public static func getFloat64(key: String) -> ResultWrapper<Float64> {
        return RustMMKV.get_f64(key).intoResultWrapper()
    }
    
    public static func putByteArray(key: String, value: Array<UInt8>) -> ResultWrapper<Void> {
        return RustMMKV.put_byte_array(key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public static func getByteArray(key: String) -> ResultWrapper<[UInt8]> {
        return RustMMKV.get_byte_array(key).intoResultWrapper()
    }
    
    public static func putInt32Array(key: String, value: Array<Int32>) -> ResultWrapper<Void> {
        return RustMMKV.put_i32_array(key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public static func getInt32Array(key: String) -> ResultWrapper<[Int32]> {
        return RustMMKV.get_i32_array(key).intoResultWrapper()
    }
    
    public static func putInt64Array(key: String, value: Array<Int64>) -> ResultWrapper<Void> {
        return RustMMKV.put_i64_array(key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public static func getInt64Array(key: String) -> ResultWrapper<[Int64]> {
        return RustMMKV.get_i64_array(key).intoResultWrapper()
    }
    
    public static func putFloat32Array(key: String, value: Array<Float32>) -> ResultWrapper<Void> {
        return RustMMKV.put_f32_array(key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public static func getFloat32Array(key: String) -> ResultWrapper<[Float]> {
        return RustMMKV.get_f32_array(key).intoResultWrapper()
    }
    
    public static func putFloat64Array(key: String, value: Array<Float64>) -> ResultWrapper<Void> {
        return RustMMKV.put_f64_array(key, value, UInt(value.count)).intoResultWrapper()
    }
    
    public static func getFloat64Array(key: String) -> ResultWrapper<[Float64]> {
        return RustMMKV.get_f64_array(key).intoResultWrapper()
    }
}

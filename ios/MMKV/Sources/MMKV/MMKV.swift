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

public class ResultWrapper<T> {
    private let rawBuffer: UnsafePointer<RawBuffer>
    private let decode: (RawBuffer) -> T
    
    init(rawBuffer: UnsafePointer<RawBuffer>, decode: @escaping (RawBuffer) -> T) {
        self.rawBuffer = rawBuffer
        self.decode = decode
    }
    
    deinit {
        RustMMKV.free_buffer(rawBuffer)
    }
    
    public func unwrapResult() -> Result<T, MMKVError> {
        if rawBuffer.pointee.err != nil {
            return Result.failure(rawBuffer.pointee.err.asMMKVError())
        }
        return Result.success(decode(rawBuffer.pointee))
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
    func intoResultWrapper<T>(decoder: @escaping (RawBuffer) -> T) -> ResultWrapper<T> {
        return ResultWrapper(rawBuffer: self, decode: decoder)
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
    
    private static let logger = Logger(label: "net.yangkx.MMKV")
    private static let logwrapper = LogWrapper(logger: logger)
    
    public static func initialize(dir: String) {
        RustMMKV.initialize(dir, logwrapper.toNativeLogger())
    }
    
    public static func putString(key: String, value: String) -> ResultWrapper<Void> {
        RustMMKV.put_str(key, value).intoResultWrapper() {
            (_) -> Void in
        }
    }
    
    public static func getString(key: String) -> ResultWrapper<String> {
        return RustMMKV.get_str(key).intoResultWrapper() {
            (buffer) -> String in
            return buffer.rawData.assumingMemoryBound(to: ByteSlice.self).asString()!
        }
    }
    
    public static func putInt32(key: String, value: Int32) -> ResultWrapper<Void> {
        return RustMMKV.put_i32(key, value).intoResultWrapper() {
            (_) -> Void in
        }
    }
    
    public static func getInt32(key: String) -> ResultWrapper<Int32> {
        return RustMMKV.get_i32(key).intoResultWrapper() {
            (buffer) -> Int32 in
            let value = buffer.rawData.assumingMemoryBound(to: Int32.self)
            return Int32(value.pointee)
        }
    }
    
    public static func putInt32Array(key: String, value: Array<Int32>) -> ResultWrapper<Void> {
        return RustMMKV.put_i32_array(key, value, UInt(value.count)).intoResultWrapper() {
            (_) -> Void in
        }
    }
    
    public static func getInt32Array(key: String) -> ResultWrapper<[Int32]> {
        return RustMMKV.get_i32_array(key).intoResultWrapper(decoder: {
            (buffer) -> Array<Int32> in
            let ptr = buffer.rawData.assumingMemoryBound(to: RawTypedArray.self)
            let count = Int(ptr.pointee.len)
            var output = Array<Int32>(repeating: 0, count: count)
            let array = ptr.pointee.array.assumingMemoryBound(to: Int32.self)
            let buffer = UnsafeBufferPointer<Int32>(start: array, count: count)
            for i in 0...count - 1 {
                output[i] = buffer[i]
            }
            return output
        })
    }
}

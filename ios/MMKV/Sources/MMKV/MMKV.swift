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
    private func asUnsafeBufferPointer() -> UnsafeBufferPointer<UInt8> {
        return UnsafeBufferPointer(start: self.pointee.bytes, count: Int(self.pointee.len))
    }
    
    func asString() -> String? {
        return String(bytes: asUnsafeBufferPointer(), encoding: String.Encoding.utf8)
    }
}

private extension UnsafePointer<VoidResult> {
    func intoResult() -> Result<(), MMKVError> {
        defer {
            RustMMKV.destroy_void_result(self)
        }
        return if self.pointee.err != nil {
            Result.failure(self.pointee.err.asMMKVError())
        } else {
            Result.success(())
        }
    }
}

private extension UnsafePointer<Result_ByteSlice> {
    func intoResult() -> Result<String, MMKVError> {
        defer {
            RustMMKV.destroy_str_result(self)
        }
        if self.pointee.err != nil {
            return Result.failure(self.pointee.err.asMMKVError())
        }
        if self.pointee.rawData == nil {
            return Result.failure(MMKVError.unknown)
        }
        return Result.success(self.pointee.rawData.pointee.asString()!)
    }
}

private extension UnsafePointer<Result_i32> {
    func intoResult() -> Result<Int32, MMKVError> {
        defer {
            RustMMKV.destroy_i32_result(self)
        }
        if self.pointee.err != nil {
            return Result.failure(self.pointee.err.asMMKVError())
        }
        if self.pointee.rawData == nil {
            return Result.failure(MMKVError.unknown)
        }
        return Result.success(Int32(self.pointee.rawData.pointee))
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


public enum MMKVError: Error, Equatable {
    case unknown
    case native(code: Int32, reason: String?)
}

private class LogWrapper {
    
    private let logger: Logger
    
    init(logger: Logger) {
        self.logger = logger
    }
    
    func log(level: Int32, content: ByteSlice) {
        let message = Logger.Message(stringLiteral: content.asString()!)
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
        let callback: (@convention(c) (UnsafeMutableRawPointer?, Int32, ByteSlice) -> Void) = {
            (obj, level, content) -> Void in
            let swiftObj: LogWrapper = Unmanaged.fromOpaque(obj!).takeUnretainedValue()
            swiftObj.log(level: level, content: content)
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
    
    public static func putString(key: String, value: String) -> Result<(), MMKVError> {
        return RustMMKV.put_str(key, value).intoResult()
    }
    
    public static func getString(key: String) -> Result<String, MMKVError> {
        return RustMMKV.get_str(key).intoResult()
    }
    
    public static func putInt32(key: String, value: Int32) -> Result<(), MMKVError> {
        return RustMMKV.put_i32(key, value).intoResult()
    }
    
    public static func getInt32(key: String) -> Result<Int32, MMKVError> {
        return RustMMKV.get_i32(key).intoResult()
    }
}

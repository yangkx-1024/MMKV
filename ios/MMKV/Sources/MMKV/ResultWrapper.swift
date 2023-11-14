import Foundation
import RustMMKV

/**
 A wrapper of result of MMKV API call.
 */
public class ResultWrapper<T> {
    private let rawBuffer: UnsafePointer<RawBuffer>
    
    init(rawBuffer: UnsafePointer<RawBuffer>) {
        self.rawBuffer = rawBuffer
    }
    
    deinit {
        RustMMKV.free_buffer(rawBuffer)
    }
    
    /**
     Transform MMKV result into standard `Result`
     
     - returns: If failed, see  ``MMKVError``.
     */
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
    
    /**
     Unwrap the result, if failed, ignore the error and return a defalut value.
     */
    public func unwrap(defalutValue: T) -> T {
        switch unwrapResult() {
        case .success(let value):
            return value
        case .failure(_):
            return defalutValue
        }
    }
    
    /**
     Unwrap the result, if failed, throw the error.
     - throws: ``MMKVError``.
     */
    public func unwrap() throws -> T {
        switch unwrapResult() {
        case .failure(let e):
            throw e
        case .success(let value):
            return value
        }
    }
}

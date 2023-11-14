import Foundation
import RustMMKV

package extension ByteSlice {
    private func asUnsafeBufferPointer() -> UnsafeBufferPointer<UInt8> {
        return UnsafeBufferPointer(start: self.bytes, count: Int(self.len))
    }
    
    func asString() -> String? {
        return String(bytes: asUnsafeBufferPointer(), encoding: String.Encoding.utf8)
    }
}

package extension UnsafePointer<ByteSlice> {
    func asString() -> String? {
        return self.pointee.asString()
    }
}

package extension UnsafePointer<RawTypedArray> {
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

package extension UnsafePointer<InternalError> {
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

package extension UnsafePointer<RawBuffer> {
    func intoResultWrapper<T>() -> ResultWrapper<T> {
        return ResultWrapper(rawBuffer: self)
    }
}

package extension UnsafeRawPointer {
    func bindType<T>(type: T.Type) -> UnsafePointer<T> {
        self.assumingMemoryBound(to: type)
    }
}

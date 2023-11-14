import Foundation
import RustMMKV

/**
 MMKV error.
 */
public enum MMKVError: Error, Equatable {
    /**
     - Parameter code: MMKV error code.
     - Parameter reason: human readable reason.
     
     Error code list:
     - 0: key not found.
     - 1: decode failed.
     - 2: value type missmatch.
     - 3: data invalid.
     - 4: instance closed.
     - 5: encode failed.
     */
    case native(code: Int32, reason: String?)
}

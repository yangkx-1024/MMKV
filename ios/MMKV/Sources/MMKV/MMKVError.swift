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
     - 6: IO failed, for example because the disk is full.
     - 7: an internal lock was poisoned.
     - -1: the key is null or not valid UTF-8.
     - -2: the instance pointer is null.
     - -3: the value is null or, for strings, not valid UTF-8.
     */
    case native(code: Int32, reason: String?)
}

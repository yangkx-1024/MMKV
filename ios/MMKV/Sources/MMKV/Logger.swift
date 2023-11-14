import Foundation
import RustMMKV

public protocol MMKVLogger {
    func trace(_ message: String)
    func debug(_ message: String)
    func info(_ message: String)
    func warning(_ message: String)
    func error(_ message: String)
}

package class LogWrapper {
    
    private let logger: MMKVLogger
    
    init(logger: MMKVLogger) {
        self.logger = logger
    }
    
    deinit {
        logger.info("deinit logger \(self.logger)")
    }
    
    func log(level: Int32, content: UnsafePointer<ByteSlice>) {
        let message = content.pointee.asString()!
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
        let destroy: (@convention(c) (UnsafeMutableRawPointer?) -> Void) = {
            (obj) -> Void in
            let _: LogWrapper = Unmanaged.fromOpaque(obj!).takeRetainedValue()
        }
        return NativeLogger(obj: ownedPointer, callback: callback, destroy: destroy)
    }
}

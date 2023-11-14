import XCTest
@testable import MMKV
@testable import Logging

private class DefaultLogger: MMKVLogger {
    private let logger: Logger = Logger(label: "net.yangkx.MMKV")
    
    func trace(_ message: String) {
        logger.trace(Logger.Message(stringLiteral: message))
    }
    
    func debug(_ message: String) {
        logger.debug(Logger.Message(stringLiteral: message))
    }
    
    func info(_ message: String) {
        logger.info(Logger.Message(stringLiteral: message))
    }
    
    func warning(_ message: String) {
        logger.warning(Logger.Message(stringLiteral: message))
    }
    
    func error(_ message: String) {
        logger.error(Logger.Message(stringLiteral: message))
    }
}

// XCTest Documentation
// https://developer.apple.com/documentation/xctest
// Defining Test Cases and Test Methods
// https://developer.apple.com/documentation/xctest/defining_test_cases_and_test_methods
final class MMKVTests: XCTestCase {
    
    func testInitAndClose() throws {
        MMKV.shared.initialize(dir: ".")
        MMKV.shared.setLogger(logger: DefaultLogger())
        try MMKV.shared.putInt32(key: "init_i32", value: 111).unwrap()
        MMKV.shared.initialize(dir: ".")
        XCTAssertEqual(MMKV.shared.getInt32(key: "init_i32").unwrap(defalutValue: 0), 111)
        XCTAssertEqual(MMKV.shared.getInt32(key: "key_not_exists").unwrap(defalutValue: 0), 0)
        MMKV.shared.clearData()
        MMKV.shared.initialize(dir: ".")
        let emptyResult = MMKV.shared.getString(key: "init_i32").unwrapResult()
        switch emptyResult {
        case .failure(let err):
            assertError(err: err, exceptCode: 0)
        case .success(_):
            XCTFail("Should be failure")
        }
        MMKV.shared.close()
    }
    
    func testPutAndGetString() throws {
        MMKV.shared.initialize(dir: ".")
        try MMKV.shared.putString(key: "key", value: "test_value").unwrap()
        let str = MMKV.shared.getString(key: "key").unwrap(defalutValue: "")
        XCTAssertEqual(str, "test_value")
    }
    
    func testPutAndGetBool() throws {
        MMKV.shared.initialize(dir: ".")
        try MMKV.shared.putBool(key: "bool_key", value: true).unwrap()
        let value = MMKV.shared.getBool(key: "bool_key").unwrap(defalutValue: false)
        XCTAssertEqual(value, true)
    }
    
    func testPutAndGetInt32() throws {
        MMKV.shared.initialize(dir: ".")
        try MMKV.shared.putInt32(key: "key", value: 12).unwrap()
        XCTAssertEqual(MMKV.shared.getInt32(key: "key").unwrap(defalutValue: 0), 12)
        try MMKV.shared.putInt32(key: "key", value: Int32.max).unwrap()
        XCTAssertEqual(MMKV.shared.getInt32(key: "key").unwrap(defalutValue: 0), Int32.max)
        try MMKV.shared.putInt32(key: "key", value: Int32.min).unwrap()
        XCTAssertEqual(MMKV.shared.getInt32(key: "key").unwrap(defalutValue: 0), Int32.min)
    }
    
    func testPutAndGetInt64() throws {
        MMKV.shared.initialize(dir: ".")
        try MMKV.shared.putInt64(key: "key", value: 1231).unwrap()
        XCTAssertEqual(MMKV.shared.getInt64(key: "key").unwrap(defalutValue: 0), 1231)
        try MMKV.shared.putInt64(key: "key", value: Int64.max).unwrap()
        XCTAssertEqual(MMKV.shared.getInt64(key: "key").unwrap(defalutValue: 0), Int64.max)
        try MMKV.shared.putInt64(key: "key", value: Int64.min).unwrap()
        XCTAssertEqual(MMKV.shared.getInt64(key: "key").unwrap(defalutValue: 0), Int64.min)
    }
    
    func testPutAndGetF32() throws {
        MMKV.shared.initialize(dir: ".")
        try MMKV.shared.putFloat32(key: "key", value: 1.23).unwrap()
        XCTAssertEqual(MMKV.shared.getFloat32(key: "key").unwrap(defalutValue: 0), 1.23)
        try MMKV.shared.putFloat32(key: "key", value: Float.greatestFiniteMagnitude).unwrap()
        XCTAssertEqual(MMKV.shared.getFloat32(key: "key").unwrap(defalutValue: 0), Float.greatestFiniteMagnitude)
        try MMKV.shared.putFloat32(key: "key", value: -Float.greatestFiniteMagnitude).unwrap()
        XCTAssertEqual(MMKV.shared.getFloat32(key: "key").unwrap(defalutValue: 0), -Float.greatestFiniteMagnitude)
    }
    
    func testPutAndGetF64() throws {
        MMKV.shared.initialize(dir: ".")
        try MMKV.shared.putFloat64(key: "key", value: 1.2323).unwrap()
        XCTAssertEqual(MMKV.shared.getFloat64(key: "key").unwrap(defalutValue: 0), 1.2323)
        try MMKV.shared.putFloat64(key: "key", value: Float64.greatestFiniteMagnitude).unwrap()
        XCTAssertEqual(MMKV.shared.getFloat64(key: "key").unwrap(defalutValue: 0), Float64.greatestFiniteMagnitude)
        try MMKV.shared.putFloat64(key: "key", value: -Float64.greatestFiniteMagnitude).unwrap()
        XCTAssertEqual(MMKV.shared.getFloat64(key: "key").unwrap(defalutValue: 0), -Float64.greatestFiniteMagnitude)
    }
    
    func testPutAndGetByteArray() throws {
        MMKV.shared.initialize(dir: ".")
        let array: [UInt8] = [UInt8.min, 2, 3, 4, UInt8.max]
        try MMKV.shared.putByteArray(key: "key", value: array).unwrap()
        let value = MMKV.shared.getByteArray(key: "key").unwrap(defalutValue: [])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetInt32Array() throws {
        MMKV.shared.initialize(dir: ".")
        let array: [Int32] = [Int32.min, 2, 3, 4, Int32.max]
        try MMKV.shared.putInt32Array(key: "key", value: array).unwrap()
        let value = MMKV.shared.getInt32Array(key: "key").unwrap(defalutValue: [])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetInt64Array() throws {
        MMKV.shared.initialize(dir: ".")
        let array: [Int64] = [Int64.min, 2, 3, 4, Int64.max]
        try MMKV.shared.putInt64Array(key: "key", value: array).unwrap()
        let value = MMKV.shared.getInt64Array(key: "key").unwrap(defalutValue: [])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetFloat32Array() throws {
        MMKV.shared.initialize(dir: ".")
        let array: [Float32] = [-Float32.greatestFiniteMagnitude, 2.1, 3.2, Float32.pi, Float32.greatestFiniteMagnitude]
        try MMKV.shared.putFloat32Array(key: "key", value: array).unwrap()
        let value = MMKV.shared.getFloat32Array(key: "key").unwrap(defalutValue: [])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetFloat64Array() throws {
        MMKV.shared.initialize(dir: ".")
        let array: [Float64] = [-Float64.greatestFiniteMagnitude, 2.1, 3.2, Float64.pi, Float64.greatestFiniteMagnitude]
        try MMKV.shared.putFloat64Array(key: "key", value: array).unwrap()
        let value = MMKV.shared.getFloat64Array(key: "key").unwrap(defalutValue: [])
        XCTAssertEqual(value, array)
    }
    
    func assertError(err: MMKVError, exceptCode: Int32) {
        switch err {
        case MMKVError.native(let code, _):
            XCTAssertEqual(code, exceptCode)
        }
    }
}

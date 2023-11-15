import XCTest
@testable import MMKV

// XCTest Documentation
// https://developer.apple.com/documentation/xctest
// Defining Test Cases and Test Methods
// https://developer.apple.com/documentation/xctest/defining_test_cases_and_test_methods
final class MMKVTests: XCTestCase {
    
    func testInitAndClose() throws {
        MMKV.shared.initialize(".")
        MMKV.shared.setLogLevel(LogLevel.error)
        try MMKV.shared.putInt32("init_i32", 111).unwrap()
        MMKV.shared.initialize(".")
        XCTAssertEqual(MMKV.shared.getInt32("init_i32").unwrap(0), 111)
        XCTAssertEqual(MMKV.shared.getInt32("key_not_exists").unwrap(0), 0)
        MMKV.shared.clearData()
        MMKV.shared.initialize(".")
        let emptyResult = MMKV.shared.getString("init_i32").unwrapResult()
        switch emptyResult {
        case .failure(let err):
            assertError(err: err, exceptCode: 0)
        case .success(_):
            XCTFail("Should be failure")
        }
        MMKV.shared.close()
    }
    
    func testPutAndGetString() throws {
        MMKV.shared.initialize(".")
        try MMKV.shared.putString("key", "test_value").unwrap()
        let str = MMKV.shared.getString("key").unwrap("")
        XCTAssertEqual(str, "test_value")
    }
    
    func testPutAndGetBool() throws {
        MMKV.shared.initialize(".")
        try MMKV.shared.putBool("bool_key", true).unwrap()
        let value = MMKV.shared.getBool("bool_key").unwrap(false)
        XCTAssertEqual(value, true)
    }
    
    func testPutAndGetInt32() throws {
        MMKV.shared.initialize(".")
        try MMKV.shared.putInt32("key", 12).unwrap()
        XCTAssertEqual(MMKV.shared.getInt32("key").unwrap(0), 12)
        try MMKV.shared.putInt32("key", Int32.max).unwrap()
        XCTAssertEqual(MMKV.shared.getInt32("key").unwrap(0), Int32.max)
        try MMKV.shared.putInt32("key", Int32.min).unwrap()
        XCTAssertEqual(MMKV.shared.getInt32("key").unwrap(0), Int32.min)
    }
    
    func testPutAndGetInt64() throws {
        MMKV.shared.initialize(".")
        try MMKV.shared.putInt64("key", 1231).unwrap()
        XCTAssertEqual(MMKV.shared.getInt64("key").unwrap(0), 1231)
        try MMKV.shared.putInt64("key", Int64.max).unwrap()
        XCTAssertEqual(MMKV.shared.getInt64("key").unwrap(0), Int64.max)
        try MMKV.shared.putInt64("key", Int64.min).unwrap()
        XCTAssertEqual(MMKV.shared.getInt64("key").unwrap(0), Int64.min)
    }
    
    func testPutAndGetF32() throws {
        MMKV.shared.initialize(".")
        try MMKV.shared.putFloat32("key", 1.23).unwrap()
        XCTAssertEqual(MMKV.shared.getFloat32("key").unwrap(0), 1.23)
        try MMKV.shared.putFloat32("key", Float.greatestFiniteMagnitude).unwrap()
        XCTAssertEqual(MMKV.shared.getFloat32("key").unwrap(0), Float.greatestFiniteMagnitude)
        try MMKV.shared.putFloat32("key", -Float.greatestFiniteMagnitude).unwrap()
        XCTAssertEqual(MMKV.shared.getFloat32("key").unwrap(0), -Float.greatestFiniteMagnitude)
    }
    
    func testPutAndGetF64() throws {
        MMKV.shared.initialize(".")
        try MMKV.shared.putFloat64("key", 1.2323).unwrap()
        XCTAssertEqual(MMKV.shared.getFloat64("key").unwrap(0), 1.2323)
        try MMKV.shared.putFloat64("key", Float64.greatestFiniteMagnitude).unwrap()
        XCTAssertEqual(MMKV.shared.getFloat64("key").unwrap(0), Float64.greatestFiniteMagnitude)
        try MMKV.shared.putFloat64("key", -Float64.greatestFiniteMagnitude).unwrap()
        XCTAssertEqual(MMKV.shared.getFloat64("key").unwrap(0), -Float64.greatestFiniteMagnitude)
    }
    
    func testPutAndGetByteArray() throws {
        MMKV.shared.initialize(".")
        let array: [UInt8] = [UInt8.min, 2, 3, 4, UInt8.max]
        try MMKV.shared.putByteArray("key", array).unwrap()
        let value = MMKV.shared.getByteArray("key").unwrap([])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetInt32Array() throws {
        MMKV.shared.initialize(".")
        let array: [Int32] = [Int32.min, 2, 3, 4, Int32.max]
        try MMKV.shared.putInt32Array("key", array).unwrap()
        let value = MMKV.shared.getInt32Array("key").unwrap([])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetInt64Array() throws {
        MMKV.shared.initialize(".")
        let array: [Int64] = [Int64.min, 2, 3, 4, Int64.max]
        try MMKV.shared.putInt64Array("key", array).unwrap()
        let value = MMKV.shared.getInt64Array("key").unwrap([])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetFloat32Array() throws {
        MMKV.shared.initialize(".")
        let array: [Float32] = [-Float32.greatestFiniteMagnitude, 2.1, 3.2, Float32.pi, Float32.greatestFiniteMagnitude]
        try MMKV.shared.putFloat32Array("key", array).unwrap()
        let value = MMKV.shared.getFloat32Array("key").unwrap([])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetFloat64Array() throws {
        MMKV.shared.initialize(".")
        let array: [Float64] = [-Float64.greatestFiniteMagnitude, 2.1, 3.2, Float64.pi, Float64.greatestFiniteMagnitude]
        try MMKV.shared.putFloat64Array("key", array).unwrap()
        let value = MMKV.shared.getFloat64Array("key").unwrap([])
        XCTAssertEqual(value, array)
    }
    
    func assertError(err: MMKVError, exceptCode: Int32) {
        switch err {
        case MMKVError.native(let code, _):
            XCTAssertEqual(code, exceptCode)
        }
    }
}

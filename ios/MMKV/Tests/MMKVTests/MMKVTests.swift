import XCTest
@testable import MMKV

// XCTest Documentation
// https://developer.apple.com/documentation/xctest
// Defining Test Cases and Test Methods
// https://developer.apple.com/documentation/xctest/defining_test_cases_and_test_methods
final class MMKVTests: XCTestCase {
    
    func testInitAndClose() throws {
        MMKV.initialize(dir: ".")
        try MMKV.putInt32(key: "init_i32", value: 111).unwrap()
        MMKV.initialize(dir: ".")
        XCTAssertEqual(MMKV.getInt32(key: "init_i32").unwrap(defalutValue: 0), 111)
        XCTAssertEqual(MMKV.getInt32(key: "key_not_exists").unwrap(defalutValue: 0), 0)
        MMKV.clearData()
        MMKV.initialize(dir: ".")
        let emptyResult = MMKV.getString(key: "init_i32").unwrapResult()
        switch emptyResult {
        case .failure(let err):
            assertError(err: err, exceptCode: 0)
        case .success(_):
            XCTFail("Should be failure")
        }
        MMKV.close()
    }
    
    func testPutAndGetString() throws {
        MMKV.initialize(dir: ".")
        try MMKV.putString(key: "key", value: "test_value").unwrap()
        let str = MMKV.getString(key: "key").unwrap(defalutValue: "")
        XCTAssertEqual(str, "test_value")
    }
    
    func testPutAndGetBool() throws {
        MMKV.initialize(dir: ".")
        try MMKV.putBool(key: "bool_key", value: true).unwrap()
        let value = MMKV.getBool(key: "bool_key").unwrap(defalutValue: false)
        XCTAssertEqual(value, true)
    }
    
    func testPutAndGetInt32() throws {
        MMKV.initialize(dir: ".")
        try MMKV.putInt32(key: "key", value: 12).unwrap()
        XCTAssertEqual(MMKV.getInt32(key: "key").unwrap(defalutValue: 0), 12)
        try MMKV.putInt32(key: "key", value: Int32.max).unwrap()
        XCTAssertEqual(MMKV.getInt32(key: "key").unwrap(defalutValue: 0), Int32.max)
        try MMKV.putInt32(key: "key", value: Int32.min).unwrap()
        XCTAssertEqual(MMKV.getInt32(key: "key").unwrap(defalutValue: 0), Int32.min)
    }
    
    func testPutAndGetInt64() throws {
        MMKV.initialize(dir: ".")
        try MMKV.putInt64(key: "key", value: 1231).unwrap()
        XCTAssertEqual(MMKV.getInt64(key: "key").unwrap(defalutValue: 0), 1231)
        try MMKV.putInt64(key: "key", value: Int64.max).unwrap()
        XCTAssertEqual(MMKV.getInt64(key: "key").unwrap(defalutValue: 0), Int64.max)
        try MMKV.putInt64(key: "key", value: Int64.min).unwrap()
        XCTAssertEqual(MMKV.getInt64(key: "key").unwrap(defalutValue: 0), Int64.min)
    }
    
    func testPutAndGetF32() throws {
        MMKV.initialize(dir: ".")
        try MMKV.putFloat32(key: "key", value: 1.23).unwrap()
        XCTAssertEqual(MMKV.getFloat32(key: "key").unwrap(defalutValue: 0), 1.23)
        try MMKV.putFloat32(key: "key", value: Float.greatestFiniteMagnitude).unwrap()
        XCTAssertEqual(MMKV.getFloat32(key: "key").unwrap(defalutValue: 0), Float.greatestFiniteMagnitude)
        try MMKV.putFloat32(key: "key", value: -Float.greatestFiniteMagnitude).unwrap()
        XCTAssertEqual(MMKV.getFloat32(key: "key").unwrap(defalutValue: 0), -Float.greatestFiniteMagnitude)
    }
    
    func testPutAndGetF64() throws {
        MMKV.initialize(dir: ".")
        try MMKV.putFloat64(key: "key", value: 1.2323).unwrap()
        XCTAssertEqual(MMKV.getFloat64(key: "key").unwrap(defalutValue: 0), 1.2323)
        try MMKV.putFloat64(key: "key", value: Float64.greatestFiniteMagnitude).unwrap()
        XCTAssertEqual(MMKV.getFloat64(key: "key").unwrap(defalutValue: 0), Float64.greatestFiniteMagnitude)
        try MMKV.putFloat64(key: "key", value: -Float64.greatestFiniteMagnitude).unwrap()
        XCTAssertEqual(MMKV.getFloat64(key: "key").unwrap(defalutValue: 0), -Float64.greatestFiniteMagnitude)
    }
    
    func testPutAndGetByteArray() throws {
        MMKV.initialize(dir: ".")
        let array: [UInt8] = [UInt8.min, 2, 3, 4, UInt8.max]
        try MMKV.putByteArray(key: "key", value: array).unwrap()
        let value = MMKV.getByteArray(key: "key").unwrap(defalutValue: [])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetInt32Array() throws {
        MMKV.initialize(dir: ".")
        let array: [Int32] = [Int32.min, 2, 3, 4, Int32.max]
        try MMKV.putInt32Array(key: "key", value: array).unwrap()
        let value = MMKV.getInt32Array(key: "key").unwrap(defalutValue: [])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetInt64Array() throws {
        MMKV.initialize(dir: ".")
        let array: [Int64] = [Int64.min, 2, 3, 4, Int64.max]
        try MMKV.putInt64Array(key: "key", value: array).unwrap()
        let value = MMKV.getInt64Array(key: "key").unwrap(defalutValue: [])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetFloat32Array() throws {
        MMKV.initialize(dir: ".")
        let array: [Float32] = [-Float32.greatestFiniteMagnitude, 2.1, 3.2, Float32.pi, Float32.greatestFiniteMagnitude]
        try MMKV.putFloat32Array(key: "key", value: array).unwrap()
        let value = MMKV.getFloat32Array(key: "key").unwrap(defalutValue: [])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetFloat64Array() throws {
        MMKV.initialize(dir: ".")
        let array: [Float64] = [-Float64.greatestFiniteMagnitude, 2.1, 3.2, Float64.pi, Float64.greatestFiniteMagnitude]
        try MMKV.putFloat64Array(key: "key", value: array).unwrap()
        let value = MMKV.getFloat64Array(key: "key").unwrap(defalutValue: [])
        XCTAssertEqual(value, array)
    }
    
    func assertError(err: MMKVError, exceptCode: Int32) {
        switch err {
        case MMKVError.native(let code, _):
            XCTAssertEqual(code, exceptCode)
        }
    }
}

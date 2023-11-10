import XCTest
@testable import MMKV

final class MMKVTests: XCTestCase {
    func testPutAndGetString() throws {
        // XCTest Documentation
        // https://developer.apple.com/documentation/xctest
        MMKV.initialize(dir: ".")
        let emptyResult = MMKV.getString(key: "key_not_exists").unwrapResult()
        switch emptyResult {
        case .failure(let err):
            assertError(err: err, exceptCode: 0)
        case .success(_):
            XCTFail("Should be failure for key_not_exists")
        }
        try MMKV.putString(key: "key", value: "test_value").unwrap()
        let str = MMKV.getString(key: "key").unwrap(defalutValue: "")
        XCTAssertEqual(str, "test_value")
        // Defining Test Cases and Test Methods
        // https://developer.apple.com/documentation/xctest/defining_test_cases_and_test_methods
    }
    
    func testPutAndGetInt32() throws {
        MMKV.initialize(dir: ".")
        let emptyResult = MMKV.getInt32(key: "key_not_exists").unwrapResult()
        switch emptyResult {
        case .failure(let err):
            assertError(err: err, exceptCode: 0)
        case .success(_):
            XCTFail("Should be failure for key_not_exists")
        }
        try MMKV.putInt32(key: "key", value: 12).unwrap()
        let value = MMKV.getInt32(key: "key").unwrap(defalutValue: 0)
        XCTAssertEqual(value, 12)
    }
    
    func testPutAndGetInt32Array() throws {
        MMKV.initialize(dir: ".")
        let emptyResult = MMKV.getInt32Array(key: "key_not_exists").unwrapResult()
        switch emptyResult {
        case .failure(let err):
            assertError(err: err, exceptCode: 0)
        case .success(_):
            XCTFail("Should be failure for key_not_exists")
        }
        let array: [Int32] = [1, 2, 3, 4, 5]
        try MMKV.putInt32Array(key: "key", value: array).unwrap()
        let value = MMKV.getInt32Array(key: "key").unwrap(defalutValue: [])
        XCTAssertEqual(value, array)
    }
    
    func assertError(err: MMKVError, exceptCode: Int32) {
        switch err {
        case MMKVError.native(let code, _):
            XCTAssertEqual(code, exceptCode)
        }
    }
}

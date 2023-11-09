import XCTest
@testable import MMKV

final class MMKVTests: XCTestCase {
    func testPutAndGetString() throws {
        // XCTest Documentation
        // https://developer.apple.com/documentation/xctest
        MMKV.initialize(dir: ".")
        let emptyResult = MMKV.getString(key: "key_not_exists")
        switch emptyResult {
        case .failure(let err):
            assertError(err: err, exceptCode: 0)
        case .success(_):
            XCTFail("Should be failure for key_not_exists")
        }
        let ret = MMKV.putString(key: "key", value: "test_value")
        switch ret {
        case .failure(_):
            XCTFail()
        case .success(_):
            break
        }
        let strResult = MMKV.getString(key: "key")
        switch strResult {
        case .failure(_):
            XCTFail()
        case .success(let value):
            XCTAssertEqual(value, "test_value")
        }
        // Defining Test Cases and Test Methods
        // https://developer.apple.com/documentation/xctest/defining_test_cases_and_test_methods
    }
    
    func testPutAndGetInt32() throws {
        MMKV.initialize(dir: ".")
        let emptyResult = MMKV.getInt32(key: "key_not_exists")
        switch emptyResult {
        case .failure(let err):
            assertError(err: err, exceptCode: 0)
        case .success(_):
            XCTFail("Should be failure for key_not_exists")
        }
        let ret = MMKV.putInt32(key: "key", value: 12)
        switch ret {
        case .failure(_):
            XCTFail()
        case .success(_):
            break
        }
        let intResult = MMKV.getInt32(key: "key")
        switch intResult {
        case .failure(_):
            XCTFail()
        case .success(let value):
            XCTAssertEqual(value, 12)
        }
    }
    
    func assertError(err: MMKVError, exceptCode: Int32) {
        switch err {
        case MMKVError.unknown:
            XCTFail("Should not be unknown error")
        case MMKVError.native(let code, _):
            XCTAssertEqual(code, exceptCode)
        }
    }
}

import XCTest
@testable import MMKV

// XCTest Documentation
// https://developer.apple.com/documentation/xctest
// Defining Test Cases and Test Methods
// https://developer.apple.com/documentation/xctest/defining_test_cases_and_test_methods
final class MMKVTests: XCTestCase {

    func initSdk() {
        MMKV.shared.setLogLevel(LogLevel.debug)
        MMKV.shared.initialize(".")
    }
    
    func closeSdk() {
        MMKV.shared.close()
    }
    
    func clearData() {
        initSdk()
        MMKV.shared.clearData()
    }

    override func setUp() {
        NSSetUncaughtExceptionHandler { (exception) in
           let stack = exception.callStackReturnAddresses
           print("Stack trace: \(stack)")
        }
        clearData()
    }
    
    override func tearDown() {
        clearData()
    }
    
    override func tearDownWithError() throws {
        clearData()
    }
    
    func testInitAndClose() throws {
        initSdk()
        try MMKV.shared.putInt32("init_i32", 111).unwrap()
        closeSdk()
        initSdk()
        XCTAssertEqual(MMKV.shared.getInt32("init_i32").unwrap(0), 111)
        XCTAssertEqual(MMKV.shared.getInt32("key_not_exists").unwrap(0), 0)
        MMKV.shared.clearData()
        initSdk()
        let emptyResult = MMKV.shared.getString("init_i32").unwrapResult()
        switch emptyResult {
        case .failure(let err):
            assertError(err: err, exceptCode: 0)
        case .success(_):
            XCTFail("Should be failure")
        }
    }
    
    func testPutAndGetString() throws {
        initSdk()
        try MMKV.shared.putString("key_str", "test_value").unwrap()
        initSdk()
        let str = MMKV.shared.getString("key_str").unwrap("")
        XCTAssertEqual(str, "test_value")
    }
    
    func testPutAndGetBool() throws {
        initSdk()
        try MMKV.shared.putBool("key_bool", true).unwrap()
        initSdk()
        let value = MMKV.shared.getBool("key_bool").unwrap(false)
        XCTAssertEqual(value, true)
    }
    
    func testPutAndGetInt32() throws {
        initSdk()
        try MMKV.shared.putInt32("key_i32", 12).unwrap()
        try MMKV.shared.putInt32("key_i32_max", Int32.max).unwrap()
        try MMKV.shared.putInt32("key_i32_min", Int32.min).unwrap()
        initSdk()
        XCTAssertEqual(MMKV.shared.getInt32("key_i32").unwrap(0), 12)
        XCTAssertEqual(MMKV.shared.getInt32("key_i32_max").unwrap(0), Int32.max)
        XCTAssertEqual(MMKV.shared.getInt32("key_i32_min").unwrap(0), Int32.min)
    }
    
    func testPutAndGetInt64() throws {
        initSdk()
        try MMKV.shared.putInt64("key_i64", 1231).unwrap()
        try MMKV.shared.putInt64("key_i64_max", Int64.max).unwrap()
        try MMKV.shared.putInt64("key_i64_min", Int64.min).unwrap()
        initSdk()
        XCTAssertEqual(MMKV.shared.getInt64("key_i64").unwrap(0), 1231)
        XCTAssertEqual(MMKV.shared.getInt64("key_i64_max").unwrap(0), Int64.max)
        XCTAssertEqual(MMKV.shared.getInt64("key_i64_min").unwrap(0), Int64.min)
    }
    
    func testPutAndGetF32() throws {
        initSdk()
        try MMKV.shared.putFloat32("key_f32", 1.23).unwrap()
        try MMKV.shared.putFloat32("key_f32_max", Float.greatestFiniteMagnitude).unwrap()
        try MMKV.shared.putFloat32("key_f32_min", -Float.greatestFiniteMagnitude).unwrap()
        initSdk()
        XCTAssertEqual(MMKV.shared.getFloat32("key_f32").unwrap(0), 1.23)
        XCTAssertEqual(MMKV.shared.getFloat32("key_f32_max").unwrap(0), Float.greatestFiniteMagnitude)
        XCTAssertEqual(MMKV.shared.getFloat32("key_f32_min").unwrap(0), -Float.greatestFiniteMagnitude)
    }
    
    func testPutAndGetF64() throws {
        initSdk()
        try MMKV.shared.putFloat64("key_f64", 1.2323).unwrap()
        try MMKV.shared.putFloat64("key_f64_max", Float64.greatestFiniteMagnitude).unwrap()
        try MMKV.shared.putFloat64("key_f64_min", -Float64.greatestFiniteMagnitude).unwrap()
        initSdk()
        XCTAssertEqual(MMKV.shared.getFloat64("key_f64").unwrap(0), 1.2323)
        XCTAssertEqual(MMKV.shared.getFloat64("key_f64_max").unwrap(0), Float64.greatestFiniteMagnitude)
        XCTAssertEqual(MMKV.shared.getFloat64("key_f64_min").unwrap(0), -Float64.greatestFiniteMagnitude)
    }
    
    func testPutAndGetByteArray() throws {
        initSdk()
        let array: [UInt8] = [UInt8.min, 2, 3, 4, UInt8.max]
        try MMKV.shared.putByteArray("key_byte_array", array).unwrap()
        closeSdk()
        initSdk()
        let value = MMKV.shared.getByteArray("key_byte_array").unwrap([])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetInt32Array() throws {
        initSdk()
        let array: [Int32] = [Int32.min, 2, 3, 4, Int32.max]
        try MMKV.shared.putInt32Array("key_i32_array", array).unwrap()
        initSdk()
        let value = MMKV.shared.getInt32Array("key_i32_array").unwrap([])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetInt64Array() throws {
        initSdk()
        let array: [Int64] = [Int64.min, 2, 3, 4, Int64.max]
        try MMKV.shared.putInt64Array("key_i64_array", array).unwrap()
        initSdk()
        let value = MMKV.shared.getInt64Array("key_i64_array").unwrap([])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetFloat32Array() throws {
        initSdk()
        let array: [Float32] = [-Float32.greatestFiniteMagnitude, 2.1, 3.2, Float32.pi, Float32.greatestFiniteMagnitude]
        try MMKV.shared.putFloat32Array("key_f32_array", array).unwrap()
        initSdk()
        let value = MMKV.shared.getFloat32Array("key_f32_array").unwrap([])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetFloat64Array() throws {
        initSdk()
        let array: [Float64] = [-Float64.greatestFiniteMagnitude, 2.1, 3.2, Float64.pi, Float64.greatestFiniteMagnitude]
        try MMKV.shared.putFloat64Array("key_f64_array", array).unwrap()
        initSdk()
        let value = MMKV.shared.getFloat64Array("key_f64_array").unwrap([])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndDelete() throws {
        initSdk()
        try MMKV.shared.putInt32("key_to_delete", 1).unwrap()
        initSdk()
        try MMKV.shared.delete("key_to_delete").unwrap()
        let result = MMKV.shared.getInt32("key_to_delete").unwrapResult()
        switch result {
        case .failure(let err):
            assertError(err: err, exceptCode: 0)
        case .success(_):
            XCTFail("Should be failure")
        }
        initSdk()
        let newResult = MMKV.shared.getInt32("key_to_delete").unwrapResult()
        switch newResult {
        case .failure(let err):
            assertError(err: err, exceptCode: 0)
        case .success(_):
            XCTFail("Should be failure")
        }
    }
    
    func assertError(err: MMKVError, exceptCode: Int32) {
        switch err {
        case MMKVError.native(let code, _):
            XCTAssertEqual(code, exceptCode)
        }
    }

    func testMultiThread() throws {
        initSdk()
        let repeatCount: Int32 = 1000
        let dispatchGroup = DispatchGroup()
        DispatchQueue.global().async(group: dispatchGroup) {
            let key = "test_multi_thread_repeat_write_key"
            for i in 0...repeatCount - 1 {
                if (i % 2 == 0) {
                    MMKV.shared.putInt32(key, i).unwrap(())
                } else {
                    MMKV.shared.delete(key).unwrap(())
                }
            }
        }
        for i in 0...1 {
            DispatchQueue.global().async(group: dispatchGroup) {
                for j : Int32 in 0...repeatCount - 1 {
                    MMKV.shared.putInt32("task_\(i)_key_\(j)", j).unwrap(())
                }
            }
        }
        dispatchGroup.wait()
        closeSdk()
        initSdk()
        XCTAssertEqual(MMKV.shared.getInt32("test_multi_thread_repeat_write_key").unwrap(-1), -1)
        for i in 0...1 {
            DispatchQueue.global().async(group: dispatchGroup) {
                for j : Int32 in 0...repeatCount - 1 {
                    let value = MMKV.shared.getInt32("task_\(i)_key_\(j)").unwrap(0)
                    XCTAssertEqual(value, j)
                }
            }
        }
        dispatchGroup.wait()
    }
}

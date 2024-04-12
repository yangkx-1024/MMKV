import XCTest
@testable import MMKV

// XCTest Documentation
// https://developer.apple.com/documentation/xctest
// Defining Test Cases and Test Methods
// https://developer.apple.com/documentation/xctest/defining_test_cases_and_test_methods
final class MMKVTests: XCTestCase {

    var mmkv: MMKV? = nil
    let dirName = "mmkv_test"

    override func setUp() {
        NSSetUncaughtExceptionHandler { (exception) in
           let stack = exception.callStackReturnAddresses
           print("Stack trace: \(stack)")
        }
        do {
            try FileManager.default.createDirectory(atPath: dirName, withIntermediateDirectories: true)
        } catch {
            print(error.localizedDescription)
        }
        
        MMKV.setLogLevel(LogLevel.debug)
    }
    
    override func tearDown() {
        mmkv?.clearData()
        mmkv = nil
        
        do {
            try FileManager.default.removeItem(atPath: dirName)
        } catch {
            print(error.localizedDescription)
        }
    }
    
    override func tearDownWithError() throws {
        mmkv?.clearData()
        mmkv = nil
    }
    
    func testInitAndClose() throws {
        mmkv = MMKV(dirName)
        try mmkv!.putInt32("init_i32", 111).unwrap()
        mmkv = nil
        
        mmkv = MMKV(dirName)
        XCTAssertEqual(mmkv!.getInt32("init_i32").unwrap(0), 111)
        XCTAssertEqual(mmkv!.getInt32("key_not_exists").unwrap(0), 0)
        mmkv!.clearData()
        mmkv = nil
        
        mmkv = MMKV(dirName)
        let emptyResult = mmkv!.getString("init_i32").unwrapResult()
        switch emptyResult {
        case .failure(let err):
            assertError(err: err, exceptCode: 0)
        case .success(_):
            XCTFail("Should be failure")
        }
    }
    
    func testPutAndGetString() throws {
        mmkv = MMKV(dirName)
        try mmkv!.putString("key_str", "test_value").unwrap()
        mmkv = nil
        
        mmkv = MMKV(dirName)
        let str = mmkv!.getString("key_str").unwrap("")
        XCTAssertEqual(str, "test_value")
    }
    
    func testPutAndGetBool() throws {
        mmkv = MMKV(dirName)
        try mmkv!.putBool("key_bool", true).unwrap()
        mmkv = nil
        
        mmkv = MMKV(dirName)
        let value = mmkv!.getBool("key_bool").unwrap(false)
        XCTAssertEqual(value, true)
    }
    
    func testPutAndGetInt32() throws {
        mmkv = MMKV(dirName)
        try mmkv!.putInt32("key_i32", 12).unwrap()
        try mmkv!.putInt32("key_i32_max", Int32.max).unwrap()
        try mmkv!.putInt32("key_i32_min", Int32.min).unwrap()
        mmkv = nil
        
        mmkv = MMKV(dirName)
        XCTAssertEqual(mmkv!.getInt32("key_i32").unwrap(0), 12)
        XCTAssertEqual(mmkv!.getInt32("key_i32_max").unwrap(0), Int32.max)
        XCTAssertEqual(mmkv!.getInt32("key_i32_min").unwrap(0), Int32.min)
    }
    
    func testPutAndGetInt64() throws {
        mmkv = MMKV(dirName)
        try mmkv!.putInt64("key_i64", 1231).unwrap()
        try mmkv!.putInt64("key_i64_max", Int64.max).unwrap()
        try mmkv!.putInt64("key_i64_min", Int64.min).unwrap()
        mmkv = nil
        
        mmkv = MMKV(dirName)
        XCTAssertEqual(mmkv!.getInt64("key_i64").unwrap(0), 1231)
        XCTAssertEqual(mmkv!.getInt64("key_i64_max").unwrap(0), Int64.max)
        XCTAssertEqual(mmkv!.getInt64("key_i64_min").unwrap(0), Int64.min)
    }
    
    func testPutAndGetF32() throws {
        mmkv = MMKV(dirName)
        try mmkv!.putFloat32("key_f32", 1.23).unwrap()
        try mmkv!.putFloat32("key_f32_max", Float.greatestFiniteMagnitude).unwrap()
        try mmkv!.putFloat32("key_f32_min", -Float.greatestFiniteMagnitude).unwrap()
        mmkv = nil
        
        mmkv = MMKV(dirName)
        XCTAssertEqual(mmkv!.getFloat32("key_f32").unwrap(0), 1.23)
        XCTAssertEqual(mmkv!.getFloat32("key_f32_max").unwrap(0), Float.greatestFiniteMagnitude)
        XCTAssertEqual(mmkv!.getFloat32("key_f32_min").unwrap(0), -Float.greatestFiniteMagnitude)
    }
    
    func testPutAndGetF64() throws {
        mmkv = MMKV(dirName)
        try mmkv!.putFloat64("key_f64", 1.2323).unwrap()
        try mmkv!.putFloat64("key_f64_max", Float64.greatestFiniteMagnitude).unwrap()
        try mmkv!.putFloat64("key_f64_min", -Float64.greatestFiniteMagnitude).unwrap()
        mmkv = nil
        
        mmkv = MMKV(dirName)
        XCTAssertEqual(mmkv!.getFloat64("key_f64").unwrap(0), 1.2323)
        XCTAssertEqual(mmkv!.getFloat64("key_f64_max").unwrap(0), Float64.greatestFiniteMagnitude)
        XCTAssertEqual(mmkv!.getFloat64("key_f64_min").unwrap(0), -Float64.greatestFiniteMagnitude)
    }
    
    func testPutAndGetByteArray() throws {
        mmkv = MMKV(dirName)
        let array: [UInt8] = [UInt8.min, 2, 3, 4, UInt8.max]
        try mmkv!.putByteArray("key_byte_array", array).unwrap()
        mmkv = nil
        
        mmkv = MMKV(dirName)
        let value = mmkv!.getByteArray("key_byte_array").unwrap([])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetInt32Array() throws {
        mmkv = MMKV(dirName)
        let array: [Int32] = [Int32.min, 2, 3, 4, Int32.max]
        try mmkv!.putInt32Array("key_i32_array", array).unwrap()
        mmkv = nil
        
        mmkv = MMKV(dirName)
        let value = mmkv!.getInt32Array("key_i32_array").unwrap([])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetInt64Array() throws {
        mmkv = MMKV(dirName)
        let array: [Int64] = [Int64.min, 2, 3, 4, Int64.max]
        try mmkv!.putInt64Array("key_i64_array", array).unwrap()
        mmkv = nil
        
        mmkv = MMKV(dirName)
        let value = mmkv!.getInt64Array("key_i64_array").unwrap([])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetFloat32Array() throws {
        mmkv = MMKV(dirName)
        let array: [Float32] = [-Float32.greatestFiniteMagnitude, 2.1, 3.2, Float32.pi, Float32.greatestFiniteMagnitude]
        try mmkv!.putFloat32Array("key_f32_array", array).unwrap()
        mmkv = nil
        
        mmkv = MMKV(dirName)
        let value = mmkv!.getFloat32Array("key_f32_array").unwrap([])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndGetFloat64Array() throws {
        mmkv = MMKV(dirName)
        let array: [Float64] = [-Float64.greatestFiniteMagnitude, 2.1, 3.2, Float64.pi, Float64.greatestFiniteMagnitude]
        try mmkv!.putFloat64Array("key_f64_array", array).unwrap()
        mmkv = nil
        
        mmkv = MMKV(dirName)
        let value = mmkv!.getFloat64Array("key_f64_array").unwrap([])
        XCTAssertEqual(value, array)
    }
    
    func testPutAndDelete() throws {
        mmkv = MMKV(dirName)
        try mmkv!.putInt32("key_to_delete", 1).unwrap()
        mmkv = nil
        
        mmkv = MMKV(dirName)
        try mmkv!.delete("key_to_delete").unwrap()
        let result = mmkv!.getInt32("key_to_delete").unwrapResult()
        switch result {
        case .failure(let err):
            assertError(err: err, exceptCode: 0)
        case .success(_):
            XCTFail("Should be failure")
        }
        mmkv = nil
        
        mmkv = MMKV(dirName)
        let newResult = mmkv!.getInt32("key_to_delete").unwrapResult()
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
        mmkv = MMKV(dirName)
        let repeatCount: Int32 = 1000
        let dispatchGroup = DispatchGroup()
        DispatchQueue.global().async(group: dispatchGroup) {
            let key = "test_multi_thread_repeat_write_key"
            for i in 0...repeatCount - 1 {
                if (i % 2 == 0) {
                    self.mmkv!.putInt32(key, i).unwrap(())
                } else {
                    self.mmkv!.delete(key).unwrap(())
                }
            }
        }
        for i in 0...1 {
            DispatchQueue.global().async(group: dispatchGroup) {
                for j : Int32 in 0...repeatCount - 1 {
                    self.mmkv!.putInt32("task_\(i)_key_\(j)", j).unwrap(())
                }
            }
        }
        dispatchGroup.wait()
        mmkv = nil
        
        mmkv = MMKV(dirName)
        XCTAssertEqual(mmkv!.getInt32("test_multi_thread_repeat_write_key").unwrap(-1), -1)
        for i in 0...1 {
            DispatchQueue.global().async(group: dispatchGroup) {
                for j : Int32 in 0...repeatCount - 1 {
                    let value = self.mmkv!.getInt32("task_\(i)_key_\(j)").unwrap(0)
                    XCTAssertEqual(value, j)
                }
            }
        }
        dispatchGroup.wait()
    }
}

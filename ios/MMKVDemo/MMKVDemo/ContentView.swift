import SwiftUI
import MMKV

struct ContentView: View {
    
    var body: some View {
        VStack(spacing: 10) {
            MMVKView(content: "String value") { key in
                let value = MMKVManager.inst.getString(key).unwrap("")
                MMKVManager.inst.putString(key, value + "1").unwrap(())
                return MMKVManager.inst.getString(key).unwrap("")
            }
            MMVKView(content: "Bool value") { key in
                let value = MMKVManager.inst.getBool(key).unwrap(false)
                MMKVManager.inst.putBool(key, !value).unwrap(())
                return MMKVManager.inst.getBool(key).unwrap(false).description
            }
            MMVKView(content: "Int32 value") { key in
                let value = MMKVManager.inst.getInt32(key).unwrap(0)
                MMKVManager.inst.putInt32(key, value + 1).unwrap(())
                return MMKVManager.inst.getInt32(key).unwrap(0).description
            }
            MMVKView(content: "Int64 value") { key in
                let value = MMKVManager.inst.getInt64(key).unwrap(0)
                MMKVManager.inst.putInt64(key, value + 1).unwrap(())
                return MMKVManager.inst.getInt64(key).unwrap(0).description
            }
            MMVKView(content: "Float32 value") { key in
                let value = MMKVManager.inst.getFloat32(key).unwrap(0.1)
                MMKVManager.inst.putFloat32(key, value + 1).unwrap(())
                return MMKVManager.inst.getFloat32(key).unwrap(0).description
            }
            MMVKView(content: "Float64 value") { key in
                let value = MMKVManager.inst.getFloat64(key).unwrap(0.1)
                MMKVManager.inst.putFloat64(key, value + 1).unwrap(())
                return MMKVManager.inst.getFloat64(key).unwrap(0).description
            }
            MMVKView(content: "Byte array value") { key in
                let _ = MMKVManager.inst.getByteArray(key).unwrap([])
                MMKVManager.inst.putByteArray(key,[UInt8.random(in: 0...100), UInt8.random(in: 0...100), UInt8.random(in: 0...100)]).unwrap(())
                return MMKVManager.inst.getByteArray(key).unwrap([]).description
            }
            MMVKView(content: "Int32 array value") { key in
                let _ = MMKVManager.inst.getInt32Array(key).unwrap([])
                MMKVManager.inst.putInt32Array(key, [Int32.random(in: 0...100), Int32.random(in: 0...100), Int32.random(in: 0...100)]).unwrap(())
                return MMKVManager.inst.getInt32Array(key).unwrap([]).description
            }
            MMVKView(content: "Int64 array value") { key in
                let _ = MMKVManager.inst.getInt64Array(key).unwrap([])
                MMKVManager.inst.putInt64Array(key, [Int64.random(in: 0...100), Int64.random(in: 0...100), Int64.random(in: 0...100)]).unwrap(())
                return MMKVManager.inst.getInt64Array(key).unwrap([]).description
            }
            MMVKView(content: "Float32 array value") { key in
                let _ = MMKVManager.inst.getFloat32Array(key).unwrap([])
                MMKVManager.inst.putFloat32Array(key, [Float32.random(in: 0...100), Float32.random(in: 0...100), Float32.random(in: 0...100)]).unwrap(())
                return MMKVManager.inst.getFloat32Array(key).unwrap([]).description
            }
            MMVKView(content: "Float64 array value") { key in
                let _ = MMKVManager.inst.getFloat64Array(key).unwrap([])
                MMKVManager.inst.putFloat64Array(key, [Float64.random(in: 0...100), Float64.random(in: 0...100), Float64.random(in: 0...100)]).unwrap(())
                return MMKVManager.inst.getFloat64Array(key).unwrap([]).description
            }
            Spacer()
            Button(action: {
                MMKVManager.inst.clearData()
            }, label: {
                Text("Clear Data")
            })
            LogView(CustomLogger(LogLevel.debug, "MMKV log:"))
        }
    }
}

#Preview {
    ContentView()
}

struct MMVKView : View {
    @State private var content: String
    private let valueKey: String
    private let clickAction: (_ key: String) -> String
    
    init(content: String, clickAction: @escaping (_ key: String) -> String) {
        self.content = content
        self.valueKey = content
        self.clickAction = clickAction
    }
    var body: some View {
        Button(action: {
            content = clickAction(valueKey)
        }, label: {
            Text(content)
        })
    }
}

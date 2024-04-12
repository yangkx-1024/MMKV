import SwiftUI
import MMKV

struct ContentView: View {
    
    @State var logger = CustomLogger(LogLevel.trace, "")
    var body: some View {
        VStack(spacing: 10) {
            MMVKView(content: "String value") { key in
                let value = MMKVManager.mmkv.getString(key).unwrap("")
                MMKVManager.mmkv.putString(key, value + "1").unwrap(())
                return MMKVManager.mmkv.getString(key).unwrap("")
            }
            MMVKView(content: "Bool value") { key in
                let value = MMKVManager.mmkv.getBool(key).unwrap(false)
                MMKVManager.mmkv.putBool(key, !value).unwrap(())
                return MMKVManager.mmkv.getBool(key).unwrap(false).description
            }
            MMVKView(content: "Int32 value") { key in
                let value = MMKVManager.mmkv.getInt32(key).unwrap(0)
                MMKVManager.mmkv.putInt32(key, value + 1).unwrap(())
                return MMKVManager.mmkv.getInt32(key).unwrap(0).description
            }
            MMVKView(content: "Int64 value") { key in
                let value = MMKVManager.mmkv.getInt64(key).unwrap(0)
                MMKVManager.mmkv.putInt64(key, value + 1).unwrap(())
                return MMKVManager.mmkv.getInt64(key).unwrap(0).description
            }
            MMVKView(content: "Float32 value") { key in
                let value = MMKVManager.mmkv.getFloat32(key).unwrap(0.1)
                MMKVManager.mmkv.putFloat32(key, value + 1).unwrap(())
                return MMKVManager.mmkv.getFloat32(key).unwrap(0).description
            }
            MMVKView(content: "Float64 value") { key in
                let value = MMKVManager.mmkv.getFloat64(key).unwrap(0.1)
                MMKVManager.mmkv.putFloat64(key, value + 1).unwrap(())
                return MMKVManager.mmkv.getFloat64(key).unwrap(0).description
            }
            MMVKView(content: "Byte array value") { key in
                let _ = MMKVManager.mmkv.getByteArray(key).unwrap([])
                MMKVManager.mmkv.putByteArray(key,[UInt8.random(in: 0...100), UInt8.random(in: 0...100), UInt8.random(in: 0...100)]).unwrap(())
                return MMKVManager.mmkv.getByteArray(key).unwrap([]).description
            }
            MMVKView(content: "Int32 array value") { key in
                let _ = MMKVManager.mmkv.getInt32Array(key).unwrap([])
                MMKVManager.mmkv.putInt32Array(key, [Int32.random(in: 0...100), Int32.random(in: 0...100), Int32.random(in: 0...100)]).unwrap(())
                return MMKVManager.mmkv.getInt32Array(key).unwrap([]).description
            }
            MMVKView(content: "Int64 array value") { key in
                let _ = MMKVManager.mmkv.getInt64Array(key).unwrap([])
                MMKVManager.mmkv.putInt64Array(key, [Int64.random(in: 0...100), Int64.random(in: 0...100), Int64.random(in: 0...100)]).unwrap(())
                return MMKVManager.mmkv.getInt64Array(key).unwrap([]).description
            }
            MMVKView(content: "Float32 array value") { key in
                let _ = MMKVManager.mmkv.getFloat32Array(key).unwrap([])
                MMKVManager.mmkv.putFloat32Array(key, [Float32.random(in: 0...100), Float32.random(in: 0...100), Float32.random(in: 0...100)]).unwrap(())
                return MMKVManager.mmkv.getFloat32Array(key).unwrap([]).description
            }
            MMVKView(content: "Float64 array value") { key in
                let _ = MMKVManager.mmkv.getFloat64Array(key).unwrap([])
                MMKVManager.mmkv.putFloat64Array(key, [Float64.random(in: 0...100), Float64.random(in: 0...100), Float64.random(in: 0...100)]).unwrap(())
                return MMKVManager.mmkv.getFloat64Array(key).unwrap([]).description
            }
            Spacer()
            Button(action: {
                MMKVManager.mmkv.clearData()
                MMKVManager.reInit()
                logger = CustomLogger(LogLevel.debug, "MMKV log:")
            }, label: {
                Text("Clear Data")
            })
            LogView(logger)
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

import SwiftUI
import MMKV

struct ContentView: View {
    @State var logger = CustomLogger(LogLevel.debug, "")
    var body: some View {
        VStack(spacing: 10) {
            MMVKView(content: "String value") { key in
                let value = MMKV.shared.getString(key).unwrap("")
                MMKV.shared.putString(key, value + "1").unwrap(())
                return MMKV.shared.getString(key).unwrap("")
            }
            MMVKView(content: "Bool value") { key in
                let value = MMKV.shared.getBool(key).unwrap(false)
                MMKV.shared.putBool(key, !value).unwrap(())
                return MMKV.shared.getBool(key).unwrap(false).description
            }
            MMVKView(content: "Int32 value") { key in
                let value = MMKV.shared.getInt32(key).unwrap(0)
                MMKV.shared.putInt32(key, value + 1).unwrap(())
                return MMKV.shared.getInt32(key).unwrap(0).description
            }
            MMVKView(content: "Int64 value") { key in
                let value = MMKV.shared.getInt64(key).unwrap(0)
                MMKV.shared.putInt64(key, value + 1).unwrap(())
                return MMKV.shared.getInt64(key).unwrap(0).description
            }
            MMVKView(content: "Float32 value") { key in
                let value = MMKV.shared.getFloat32(key).unwrap(0.1)
                MMKV.shared.putFloat32(key, value + 1).unwrap(())
                return MMKV.shared.getFloat32(key).unwrap(0).description
            }
            MMVKView(content: "Float64 value") { key in
                let value = MMKV.shared.getFloat64(key).unwrap(0.1)
                MMKV.shared.putFloat64(key, value + 1).unwrap(())
                return MMKV.shared.getFloat64(key).unwrap(0).description
            }
            MMVKView(content: "Byte array value") { key in
                let _ = MMKV.shared.getByteArray(key).unwrap([])
                MMKV.shared.putByteArray(key,[UInt8.random(in: 0...100), UInt8.random(in: 0...100), UInt8.random(in: 0...100)]).unwrap(())
                return MMKV.shared.getByteArray(key).unwrap([]).description
            }
            MMVKView(content: "Int32 array value") { key in
                let _ = MMKV.shared.getInt32Array(key).unwrap([])
                MMKV.shared.putInt32Array(key, [Int32.random(in: 0...100), Int32.random(in: 0...100), Int32.random(in: 0...100)]).unwrap(())
                return MMKV.shared.getInt32Array(key).unwrap([]).description
            }
            MMVKView(content: "Int64 array value") { key in
                let _ = MMKV.shared.getInt64Array(key).unwrap([])
                MMKV.shared.putInt64Array(key, [Int64.random(in: 0...100), Int64.random(in: 0...100), Int64.random(in: 0...100)]).unwrap(())
                return MMKV.shared.getInt64Array(key).unwrap([]).description
            }
            MMVKView(content: "Float32 array value") { key in
                let _ = MMKV.shared.getFloat32Array(key).unwrap([])
                MMKV.shared.putFloat32Array(key, [Float32.random(in: 0...100), Float32.random(in: 0...100), Float32.random(in: 0...100)]).unwrap(())
                return MMKV.shared.getFloat32Array(key).unwrap([]).description
            }
            MMVKView(content: "Float64 array value") { key in
                let _ = MMKV.shared.getFloat64Array(key).unwrap([])
                MMKV.shared.putFloat64Array(key, [Float64.random(in: 0...100), Float64.random(in: 0...100), Float64.random(in: 0...100)]).unwrap(())
                return MMKV.shared.getFloat64Array(key).unwrap([]).description
            }
            Spacer()
            Button(action: {
                MMKV.shared.clearData()
                initMMKV()
                logger = CustomLogger(LogLevel.debug, logger.logStr)
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

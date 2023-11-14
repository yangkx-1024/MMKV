import SwiftUI
import MMKV

struct ContentView: View {
    @State var textContent: String = "Hello, world!"
    var body: some View {
        Text(textContent)
            .onTapGesture {
                let value = MMKV.shared.getInt32(key: "int_key").unwrap(defalutValue: 0)
                MMKV.shared.putInt32(key: "int_key", value: value + 1).unwrap(defalutValue: ())
                textContent = MMKV.shared.getInt32(key: "int_key").unwrap(defalutValue: 0).formatted()
            }
    }
}

#Preview {
    ContentView()
}

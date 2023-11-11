import SwiftUI
import MMKV

struct ContentView: View {
    @State var textContent: String = "Hello, world!"
    var body: some View {
        VStack {
            Image(systemName: "globe")
                .imageScale(.large)
                .foregroundStyle(.tint)
            Text(textContent)
                .onTapGesture {
                    let value = MMKV.getInt32(key: "int_key").unwrap(defalutValue: 0)
                    MMKV.putInt32(key: "int_key", value: value + 1).unwrap(defalutValue: ())
                    textContent = MMKV.getInt32(key: "int_key").unwrap(defalutValue: 0).formatted()
                }
        }
        .padding()
    }
}

#Preview {
    ContentView()
}

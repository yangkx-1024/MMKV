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
                    let value = MMKV.get_i32("key")
                    MMKV.put_i32("key", value + 1)
                    textContent = MMKV.get_i32("key").formatted()
                }
        }
        .padding()
    }
}

#Preview {
    ContentView()
}

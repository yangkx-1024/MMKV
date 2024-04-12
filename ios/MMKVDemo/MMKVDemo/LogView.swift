import SwiftUI
import MMKV

struct LogView: View {
    @ObservedObject private var logger: CustomLogger
    init(_ logger: CustomLogger) {
        self.logger = logger
    }
    
    var body: some View {
        HStack {
            ScrollView {
                ScrollViewReader { value in
                    LazyVStack(alignment: .leading) {
                        ForEach(logger.logs.indices, id: \.self) { index in
                            Text(logger.logs[index])
                                .foregroundColor(.green)
                                .font(.caption)
                                .lineLimit(nil)
                                .id(index)
                        }
                        Spacer().id(-1)
                    }
                    .onChange(of: logger.logs.count, initial: true) { _, _ in
                        withAnimation {
                            value.scrollTo(-1, anchor: .bottom)
                        }
                    }
                }
            }
            Spacer()
        }
        .background(.black)
        .frame(
            minWidth: /*@START_MENU_TOKEN@*/0/*@END_MENU_TOKEN@*/,
            maxWidth: .infinity,
            minHeight: 0,
            maxHeight: 200,
            alignment: .bottomLeading
        )
    }
    
}

#Preview {
    LogView(CustomLogger(LogLevel.trace, "MMKV Log"))
}

class CustomLogger: MMKVLogger, ObservableObject {
    
    @Published var logs = [String]()
    
    init(_ logLevel: LogLevel, _ content: String) {
        logs.append(content)
        MMKV.setLogLevel(logLevel)
        MMKV.setLogger(self)
    }
    
    func trace(_ message: String) {
        logs.append("Trace \(message)")
    }
    
    func info(_ message: String) {
        logs.append("Info \(message)")
    }
    
    func debug(_ message: String) {
        logs.append("Debug \(message)")
    }
    
    func warning(_ message: String) {
        logs.append("Warning \(message)")
    }
    
    func error(_ message: String) {
        logs.append("Error \(message)")
    }
}

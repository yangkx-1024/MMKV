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
                    VStack {
                        Text(logger.logStr)
                            .foregroundColor(.green)
                            .font(.caption)
                            .lineLimit(nil)
                            .padding()
                        Spacer()
                    }
                    .id(1)
                    .onChange(of: logger.logStr, initial: true) { _, _ in
                        value.scrollTo(1, anchor: .bottom)
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
    LogView(CustomLogger(LogLevel.trace, ""))
}

class CustomLogger: MMKVLogger, ObservableObject {
    @Published var logStr: String
    
    init(_ logLevel: LogLevel, _ content: String) {
        logStr = content
        MMKV.shared.setLogLevel(logLevel)
        MMKV.shared.setLogger(self)
    }
    
    func trace(_ message: String) {
        logStr.append("Trace - ")
        logStr.append(message)
        logStr.append("\n")
        
    }
    
    func info(_ message: String) {
        logStr.append("Info - ")
        logStr.append(message)
        logStr.append("\n")
    }
    
    func debug(_ message: String) {
        logStr.append("Debug - ")
        logStr.append(message)
        logStr.append("\n")
    }
    
    func warning(_ message: String) {
        logStr.append("Warning - ")
        logStr.append(message)
        logStr.append("\n")
    }
    
    func error(_ message: String) {
        logStr.append("Error - ")
        logStr.append(message)
        logStr.append("\n")
    }
}

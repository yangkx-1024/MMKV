import SwiftUI
import MMKV

@main
struct MMKVDemoApp: App {
    init() {
        initMMKV()
    }
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

func initMMKV() {
    let paths = NSSearchPathForDirectoriesInDomains(.documentDirectory, .userDomainMask, true)
    let documentsDirectory = paths[0]
    let docURL = URL(string: documentsDirectory)!
    let dataPath = docURL.appendingPathComponent("mmkv")
    if !FileManager.default.fileExists(atPath: dataPath.path) {
        do {
            try FileManager.default.createDirectory(atPath: dataPath.path, withIntermediateDirectories: true, attributes: nil)
        } catch {
            print(error.localizedDescription)
        }
    }
    MMKV.initialize(dataPath.path)
}

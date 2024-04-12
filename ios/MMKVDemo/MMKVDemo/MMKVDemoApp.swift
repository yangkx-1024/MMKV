import SwiftUI
import MMKV
@main
struct MMKVDemoApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

class MMKVManager {
    static var mmkv: MMKV {
        _mmkv!
    }
    
    private static var _mmkv: MMKV? = initMMKV()
    
    static func reInit() {
        _mmkv = nil
        _mmkv = initMMKV()
    }
}

func initMMKV() -> MMKV {
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
    return MMKV(dataPath.path)
}

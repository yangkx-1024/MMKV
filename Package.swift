// swift-tools-version: 5.9
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "MMKV",
    products: [
        // Products define the executables and libraries a package produces, making them visible to other packages.
        .library(
            name: "MMKV",
            targets: ["MMKV"]
        ),
    ],
    targets: [
        // Targets are the basic building blocks of a package, defining a module or a test suite.
        // Targets can depend on other targets in this package and products from dependencies.
        .binaryTarget(name: "RustMMKV", url: "https://github.com/yangkx-1024/MMKV/releases/download/0.7.1/RustMMKV.xcframework.zip", checksum: "3309d9b868ace68d0451ad44c6ec4d6cd11acdffa3dfc1e7c64f56deb18f259a"),
        .target(
            name: "MMKV",
            dependencies: ["RustMMKV"],
            path: "",
            exclude: [
                "android",
                "proc_macro_lib",
                "src",
                "target",
                "tests",
                "build_android.sh",
                "build_apple.sh",
                "build.rs",
                "build.sh",
                "Cargo.lock",
                "Cargo.toml",
                "cbindgen.toml",
                "README.md",
                "LICENSE-APACHE",
                "ios/MMKVDemo",
                "ios/MMKV/Tests",
                "LICENSE-MIT"
            ],
            sources: ["ios/MMKV/Sources/MMKV"]
        ),
        .testTarget(
            name: "MMKVTests",
            dependencies: ["MMKV"],
            path: "ios/MMKV/Tests/MMKVTests"
        ),
    ]
)

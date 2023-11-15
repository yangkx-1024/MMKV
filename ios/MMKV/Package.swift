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
        .binaryTarget(name: "RustMMKV", path: "Sources/RustMMKV.xcframework"),
        .target(
            name: "MMKV",
            dependencies: ["RustMMKV"]
        ),
        .testTarget(
            name: "MMKVTests",
            dependencies: ["MMKV"]
        ),
    ]
)

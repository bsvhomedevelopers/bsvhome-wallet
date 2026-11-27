// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "WowPayMpc",
    platforms: [.iOS(.v17), .macOS(.v15)],
    products: [.library(name: "WowPayMpc", targets: ["WowPayMpc"])],
    dependencies: [.package(path: "../Keychain")],
    targets: [
        .target(name: "WowPayMpcFFI", publicHeadersPath: "include"),
        .target(name: "WowPayMpc", dependencies: ["WowPayMpcFFI", "Keychain"]),
        .testTarget(name: "WowPayMpcTests", dependencies: ["WowPayMpc", "Keychain"], path: "Tests"),
    ],
)

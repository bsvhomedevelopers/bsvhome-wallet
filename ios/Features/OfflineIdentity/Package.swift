// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "OfflineIdentity",
    platforms: [.iOS(.v17), .macOS(.v15)],
    products: [.library(name: "OfflineIdentity", targets: ["OfflineIdentity"])],
    dependencies: [
        .package(path: "../../Packages/BsvHomeCore"),
        .package(path: "../../Packages/Keychain"),
        .package(path: "../../Packages/Components"),
        .package(path: "../../Packages/Style"),
        .package(path: "../../Packages/Localization"),
    ],
    targets: [
        .target(name: "OfflineIdentity", dependencies: ["BsvHomeCore", "Keychain", "Components", "Style", "Localization"], path: "Sources"),
        .testTarget(name: "OfflineIdentityTests", dependencies: ["OfflineIdentity", "BsvHomeCore", "Keychain"], path: "Tests", resources: [.copy("Fixtures")]),
    ],
)

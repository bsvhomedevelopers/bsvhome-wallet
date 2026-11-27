// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "PhaseOne",
    platforms: [.iOS(.v17), .macOS(.v15)],
    products: [.library(name: "PhaseOne", targets: ["PhaseOne"])],
    dependencies: [
        .package(path: "../../Packages/BsvHomeCore"),
        .package(path: "../../Packages/Components"),
        .package(path: "../../Packages/Style"),
        .package(path: "../../Packages/Localization"),
    ],
    targets: [
        .target(name: "PhaseOne", dependencies: ["BsvHomeCore", "Components", "Style", "Localization"]),
        .testTarget(name: "PhaseOneTests", dependencies: ["PhaseOne", "BsvHomeCore"], exclude: ["MpcWalletNavigationTests.swift"]),
    ],
)

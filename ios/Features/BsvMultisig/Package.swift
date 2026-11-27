// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "BsvMultisig",
    platforms: [.iOS(.v17), .macOS(.v15)],
    products: [.library(name: "BsvMultisig", targets: ["BsvMultisig"])],
    dependencies: [
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
        .package(name: "BigInt", path: "../../Submodules/BigInt"),
    ],
    targets: [
        .target(name: "BsvMultisig", dependencies: ["BsvHomeCore", "Components", "Style", "Localization", "Formatters", .product(name: "BigInt", package: "BigInt")], path: "Sources"),
        .testTarget(name: "BsvMultisigTests", dependencies: ["BsvMultisig", "BsvHomeCore", "Localization", "Components", "Style", .product(name: "BigInt", package: "BigInt"), .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives")], path: "Tests"),
    ],
)

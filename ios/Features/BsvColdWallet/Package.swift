// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "BsvColdWallet",
    platforms: [.iOS(.v17), .macOS(.v15)],
    products: [.library(name: "BsvColdWallet", targets: ["BsvColdWallet"])],
    dependencies: [
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "BigInt", path: "../../Submodules/BigInt"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "QRScanner", path: "../QRScanner"),
    ],
    targets: [
        .target(name: "BsvColdWallet", dependencies: ["BsvHomeCore", .product(name: "BigInt", package: "BigInt"), "Components", "Primitives", "PrimitivesComponents", "Formatters", "Localization", "Style", "QRScanner"], path: "Sources"),
        .testTarget(name: "BsvColdWalletTests", dependencies: ["BsvColdWallet", "Components", "BsvHomeCore", "BsvHomeCorePrimitives", "BsvHomeCoreServices", .product(name: "BsvHomeCoreServicesTestKit", package: "BsvHomeCoreServices")], path: "Tests"),
    ],
)

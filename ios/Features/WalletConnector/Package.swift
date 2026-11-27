// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "WalletConnector",
    platforms: [.iOS(.v17),
                .macOS(.v15)],
    products: [
        .library(
            name: "WalletConnector",
            targets: ["WalletConnector"],
        ),
    ],
    dependencies: [
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "Store", path: "../../Packages/Store"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "QRScanner", path: "../QRScanner"),
        .package(name: "FeatureServices", path: "../../Packages/FeatureServices"),
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
    ],
    targets: [
        .target(
            name: "WalletConnector",
            dependencies: [
                "BsvHomeCorePrimitives",
                "Primitives",
                .product(name: "WalletConnectorService", package: "FeatureServices"),
                "Components",
                "Localization",
                "Style",
                "Store",
                "PrimitivesComponents",
                "QRScanner",
                "BsvHomeCore",
                "Formatters",
            ],
            path: "Sources",
        ),
        .testTarget(
            name: "WalletConnectorTests",
            dependencies: [
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "StoreTestKit", package: "Store"),
                .product(name: "BsvHomeCoreServicesTestKit", package: "BsvHomeCoreServices"),
                .product(name: "WalletConnectorService", package: "FeatureServices"),
                .product(name: "WalletConnectorServiceTestKit", package: "FeatureServices"),
                "WalletConnector",
                "BsvHomeCore",
                "BsvHomeCorePrimitives",
                "Primitives",
                "PrimitivesComponents",
                "Store",
            ],
            resources: [.process("Resources")],
        ),
    ],
)

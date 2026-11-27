// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "WalletTab",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "WalletTab",
            targets: ["WalletTab"],
        ),
        .library(
            name: "WalletTabTestKit",
            targets: ["WalletTabTestKit"],
        ),
    ],
    dependencies: [
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "InfoSheet", path: "../InfoSheet"),

        .package(name: "Store", path: "../../Packages/Store"),
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "Perpetuals", path: "../Perpetuals"),
        .package(name: "Recents", path: "../Recents"),
        .package(name: "NFT", path: "../NFT"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
    ],
    targets: [
        .target(
            name: "WalletTab",
            dependencies: [
                "BsvHomeCore",
                "Primitives",
                "Localization",
                "Style",
                "Components",
                "PrimitivesComponents",
                "BsvHomeCorePrimitives",
                "InfoSheet",
                "Store",
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                "Perpetuals",
                "Recents",
                "NFT",
                "Formatters",
            ],
            path: "Sources",
        ),
        .target(
            name: "WalletTabTestKit",
            dependencies: [
                "BsvHomeCore",
                "BsvHomeCorePrimitives",
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                .product(name: "BsvHomeCoreServicesTestKit", package: "BsvHomeCoreServices"),
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                "WalletTab",
                "NFT",
                "Primitives",
                "Recents",
                .product(name: "StoreTestKit", package: "Store"),
            ],
            path: "TestKit",
        ),
        .testTarget(
            name: "WalletTabTests",
            dependencies: [
                "BsvHomeCore",
                .product(name: "BsvHomeCoreServicesTestKit", package: "BsvHomeCoreServices"),
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "StoreTestKit", package: "Store"),
                "NFT",
                "WalletTab",
                "WalletTabTestKit",
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                "BsvHomeCorePrimitives",
                "Primitives",
                "Store",
            ],
        ),
    ],
)

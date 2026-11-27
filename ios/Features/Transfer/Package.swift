// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Transfer",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "Transfer",
            targets: ["Transfer"],
        ),
        .library(
            name: "TransferTestKit",
            targets: ["TransferTestKit"],
        ),
    ],
    dependencies: [
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "Validators", path: "../../Packages/Validators"),
        .package(name: "Store", path: "../../Packages/Store"),

        .package(name: "Stake", path: "../Stake"),
        .package(name: "WalletConnector", path: "../WalletConnector"),
        .package(name: "InfoSheet", path: "../InfoSheet"),
        .package(name: "Swap", path: "../Swap"),
        .package(name: "Perpetuals", path: "../Perpetuals"),

        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "BigInt", path: "../../Submodules/BigInt"),
    ],
    targets: [
        .target(
            name: "Transfer",
            dependencies: [
                "BsvHomeCore",
                "Primitives",
                "Formatters",
                "Localization",
                "Style",
                "Components",
                "PrimitivesComponents",
                "BsvHomeCorePrimitives",
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                "Store",
                "Validators",
                "Stake",
                "WalletConnector",
                "InfoSheet",
                "Swap",
                "Perpetuals",
                .product(name: "BigInt", package: "BigInt"),
            ],
            path: "Sources",
        ),
        .target(
            name: "TransferTestKit",
            dependencies: [
                "Transfer",
                "Primitives",
                "WalletConnector",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                "BsvHomeCore",
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                "PrimitivesComponents",
            ],
            path: "TestKit",
        ),
        .testTarget(
            name: "TransferTests",
            dependencies: [
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                "Transfer",
                "TransferTestKit",
                "BsvHomeCore",
                "BsvHomeCorePrimitives",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "BsvHomeCoreServicesTestKit", package: "BsvHomeCoreServices"),
                .product(name: "StoreTestKit", package: "Store"),
                .product(name: "PrimitivesComponentsTestKit", package: "PrimitivesComponents"),
                .product(name: "BigInt", package: "BigInt"),
                "Components",
                "InfoSheet",
                "Localization",
                "Primitives",
                "PrimitivesComponents",
                "Store",
                "Style",
            ],
            path: "Tests",
        ),
    ],
)

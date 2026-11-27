// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Transactions",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "Transactions",
            targets: ["Transactions"],
        ),
    ],
    dependencies: [
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "Store", path: "../../Packages/Store"),
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "InfoSheet", path: "../InfoSheet"),
        .package(name: "BigInt", path: "../../Submodules/BigInt"),
    ],
    targets: [
        .target(
            name: "Transactions",
            dependencies: [
                "BsvHomeCorePrimitives",
                "BsvHomeCore",
                .product(name: "BigInt", package: "BigInt"),
                "Primitives",
                "Localization",
                "Formatters",
                "Store",
                "Style",
                "Components",
                "PrimitivesComponents",
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                "InfoSheet",
            ],
            path: "Sources",
        ),
        .testTarget(
            name: "TransactionsTests",
            dependencies: [
                .product(name: "StoreTestKit", package: "Store"),
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                "Transactions",
                "PrimitivesComponents",
                .product(name: "BigInt", package: "BigInt"),
                "Components",
                "Formatters",
                "BsvHomeCore",
                "BsvHomeCorePrimitives",
                "Localization",
                "Primitives",
                "Store",
                "Style",
            ],
        ),
    ],
)

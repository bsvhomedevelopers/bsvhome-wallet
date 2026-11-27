// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "MarketInsight",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "MarketInsight",
            targets: ["MarketInsight"],
        ),
    ],
    dependencies: [
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "Store", path: "../../Packages/Store"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "InfoSheet", path: "../InfoSheet"),
        .package(name: "Style", path: "../../Packages/Style"),
    ],
    targets: [
        .target(
            name: "MarketInsight",
            dependencies: [
                "BsvHomeCore",
                "Primitives",
                "Formatters",
                "BsvHomeCorePrimitives",
                "Localization",
                "Store",
                "PrimitivesComponents",
                "Components",
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                "InfoSheet",
                "Style",
            ],
            path: "Sources",
        ),
        .testTarget(
            name: "MarketInsightTests",
            dependencies: [
                "MarketInsight",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                "BsvHomeCore",
                "BsvHomeCorePrimitives",
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                "PrimitivesComponents",
                "Localization",
                "Primitives",
            ],
        ),
    ],
)

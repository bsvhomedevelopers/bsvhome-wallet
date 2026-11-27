// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "PriceAlerts",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "PriceAlerts",
            targets: ["PriceAlerts"],
        ),
    ],
    dependencies: [
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),

        .package(name: "Store", path: "../../Packages/Store"),
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
    ],
    targets: [
        .target(
            name: "PriceAlerts",
            dependencies: [
                "Primitives",
                "Components",
                "Style",
                "Localization",
                "PrimitivesComponents",
                "BsvHomeCore",
                "Store",
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                "Formatters",
            ],
            path: "Sources",
        ),
        .testTarget(
            name: "PriceAlertsTests",
            dependencies: [
                "PriceAlerts",
                .product(name: "BsvHomeCoreServicesTestKit", package: "BsvHomeCoreServices"),
                "BsvHomeCore",
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                "Primitives",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                "Store",
            ],
        ),
    ],
)

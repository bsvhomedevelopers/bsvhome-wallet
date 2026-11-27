// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Settings",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "Settings",
            targets: ["Settings"],
        ),
    ],
    dependencies: [
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
        .package(name: "QRScanner", path: "../QRScanner"),
        .package(name: "BigInt", path: "../../Submodules/BigInt"),
        .package(name: "Store", path: "../../Packages/Store"),
    ],
    targets: [
        .target(
            name: "Settings",
            dependencies: [
                "BsvHomeCore",
                "Primitives",
                "Components",
                "Style",
                "Localization",
                "PrimitivesComponents",
                "BsvHomeCorePrimitives",
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                "Formatters",
                "QRScanner",
                .product(name: "BigInt", package: "BigInt"),
                "Store",
            ],
            path: "Sources",
        ),
        .testTarget(
            name: "CurrencyTests",
            dependencies: [
                "Settings",
                "Components",
                "BsvHomeCore",
                "Localization",
                "Primitives",
                "BsvHomeCorePrimitives",
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                "BsvHomeCoreServices",
                "Store",
                .product(name: "StoreTestKit", package: "Store"),
            ],
        ),
    ],
)

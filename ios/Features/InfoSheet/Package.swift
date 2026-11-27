// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "InfoSheet",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "InfoSheet",
            targets: ["InfoSheet"],
        ),
    ],
    dependencies: [
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "BigInt", path: "../../Submodules/BigInt"),
    ],
    targets: [
        .target(
            name: "InfoSheet",
            dependencies: [
                "Primitives",
                "Style",
                "Localization",
                "Components",
                "Formatters",
                "BsvHomeCorePrimitives",
                "PrimitivesComponents",
                .product(name: "BigInt", package: "BigInt"),
            ],
            path: "Sources",
        ),
    ],
)

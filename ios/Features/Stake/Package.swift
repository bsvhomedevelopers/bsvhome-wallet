// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Stake",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "Stake",
            targets: ["Stake"],
        ),
        .library(
            name: "StakeTestKit",
            targets: ["StakeTestKit"],
        ),
    ],
    dependencies: [
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "Store", path: "../../Packages/Store"),
        .package(name: "InfoSheet", path: "../InfoSheet"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "BigInt", path: "../../Submodules/BigInt"),
    ],
    targets: [
        .target(
            name: "Stake",
            dependencies: [
                "BsvHomeCore",
                "Primitives",
                "Components",
                "BsvHomeCorePrimitives",
                "Localization",
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                "Store",
                "InfoSheet",
                "PrimitivesComponents",
                "Formatters",
                "Style",
                .product(name: "BigInt", package: "BigInt"),
            ],
            path: "Sources",
        ),
        .target(
            name: "StakeTestKit",
            dependencies: [
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                "Stake",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "BigInt", package: "BigInt"),
                "BsvHomeCore",
                "Primitives",
            ],
            path: "TestKit",
        ),
        .testTarget(
            name: "StakeTests",
            dependencies: [
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                "StakeTestKit",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "BsvHomeCoreServicesTestKit", package: "BsvHomeCoreServices"),
                "Stake",
                "BsvHomeCoreServices",
                "Localization",
                "Primitives",
                "Store",
            ],
            path: "Tests",
        ),
    ],
)

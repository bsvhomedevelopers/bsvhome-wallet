// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Swap",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "Swap",
            targets: ["Swap"],
        ),
    ],
    dependencies: [
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "Store", path: "../../Packages/Store"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "Validators", path: "../../Packages/Validators"),
        .package(name: "InfoSheet", path: "../InfoSheet"),
        .package(name: "BigInt", path: "../../Submodules/BigInt"),
        .package(name: "Style", path: "../../Packages/Style"),
    ],
    targets: [
        .target(
            name: "Swap",
            dependencies: [
                "Primitives",
                "Formatters",
                "Components",
                "BsvHomeCorePrimitives",
                "BsvHomeCore",
                "Localization",
                "Store",
                "PrimitivesComponents",
                "Validators",
                "InfoSheet",
                .product(name: "BigInt", package: "BigInt"),
                "Style",
            ],
            path: "Sources",
        ),
        .testTarget(
            name: "SwapTests",
            dependencies: [
                "BsvHomeCorePrimitives",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "BsvHomeCoreServicesTestKit", package: "BsvHomeCoreServices"),
                .product(name: "StoreTestKit", package: "Store"),
                "PrimitivesComponents",
                "Swap",
                .product(name: "BigInt", package: "BigInt"),
                "Components",
                "Formatters",
                "BsvHomeCore",
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                "Localization",
                "Primitives",
                "Store",
                "Style",
            ],
        ),
    ],
)

// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "FiatConnect",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "FiatConnect",
            targets: ["FiatConnect"],
        ),
    ],
    dependencies: [
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "Store", path: "../../Packages/Store"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "BigInt", path: "../../Submodules/BigInt"),
        .package(name: "Validators", path: "../../Packages/Validators"),
    ],
    targets: [
        .target(
            name: "FiatConnect",
            dependencies: [
                "BsvHomeCore",
                "Primitives",
                "Formatters",
                "Style",
                "Components",
                "Localization",
                "BsvHomeCorePrimitives",
                "Store",
                "PrimitivesComponents",
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                .product(name: "BigInt", package: "BigInt"),
                "Validators",
            ],
            path: "Sources",
        ),
        .testTarget(
            name: "FiatConnectTests",
            dependencies: [
                "FiatConnect",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "BsvHomeCoreServicesTestKit", package: "BsvHomeCoreServices"),
                .product(name: "BigInt", package: "BigInt"),
                "Formatters",
                "BsvHomeCore",
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                "Localization",
                "Primitives",
                "Store",
            ],
            path: "Tests",
        ),
    ],
)

// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Assets",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "Assets",
            targets: ["Assets"],
        ),
        .library(
            name: "AssetsTestKit",
            targets: ["AssetsTestKit"],
        ),
    ],
    dependencies: [
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "Store", path: "../../Packages/Store"),
        .package(name: "InfoSheet", path: "../InfoSheet"),
        .package(name: "QRScanner", path: "../QRScanner"),
        .package(name: "Recents", path: "../Recents"),
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "BigInt", path: "../../Submodules/BigInt"),
    ],
    targets: [
        .target(
            name: "Assets",
            dependencies: [
                "BsvHomeCore",
                "Formatters",
                "Primitives",
                "Localization",
                "Style",
                "Components",
                "PrimitivesComponents",
                "BsvHomeCorePrimitives",
                "Store",
                "InfoSheet",
                "QRScanner",
                "Recents",
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                .product(name: "BigInt", package: "BigInt"),
            ],
            path: "Sources",
        ),
        .target(
            name: "AssetsTestKit",
            dependencies: [
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "BsvHomeCoreServicesTestKit", package: "BsvHomeCoreServices"),
                "Components",
                "Assets",
                "BsvHomeCore",
                "BsvHomeCorePrimitives",
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                "BsvHomeCoreServices",
                "Primitives",
                "Store",
            ],
            path: "TestKit",
        ),
        .testTarget(
            name: "AssetsTests",
            dependencies: [
                .product(name: "StoreTestKit", package: "Store"),
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "BsvHomeCoreServicesTestKit", package: "BsvHomeCoreServices"),
                "Store",
                "AssetsTestKit",
                "Assets",
                .product(name: "BigInt", package: "BigInt"),
                "BsvHomeCore",
                "BsvHomeCorePrimitives",
                "Primitives",
                "PrimitivesComponents",
            ],
        ),
    ],
)

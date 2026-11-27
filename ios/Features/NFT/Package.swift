// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "NFT",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "NFT",
            targets: ["NFT"],
        ),
    ],
    dependencies: [
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "SystemServices", path: "../../Packages/SystemServices"),
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "Store", path: "../../Packages/Store"),
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
        .package(name: "InfoSheet", path: "../InfoSheet"),
    ],
    targets: [
        .target(
            name: "NFT",
            dependencies: [
                "BsvHomeCore",
                "Primitives",
                "Components",
                "PrimitivesComponents",
                "Style",
                "Localization",
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                "Store",
                .product(name: "ImageGalleryService", package: "SystemServices"),
                "BsvHomeCorePrimitives",
                "Formatters",
                "InfoSheet",
            ],
            path: "Sources",
        ),
        .testTarget(
            name: "NFTTests",
            dependencies: [
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "StoreTestKit", package: "Store"),
                .product(name: "BsvHomeCoreServicesTestKit", package: "BsvHomeCoreServices"),
                "NFT",
                "PrimitivesComponents",
                "Formatters",
                "Store",
                "BsvHomeCore",
                "Primitives",
            ],
        ),
    ],
)

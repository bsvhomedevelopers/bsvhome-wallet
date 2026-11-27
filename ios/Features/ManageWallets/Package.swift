// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "ManageWallets",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "ManageWallets",
            targets: ["ManageWallets"],
        ),
    ],
    dependencies: [
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "Store", path: "../../Packages/Store"),
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "Onboarding", path: "../Onboarding"),
    ],
    targets: [
        .target(
            name: "ManageWallets",
            dependencies: [
                "BsvHomeCorePrimitives",
                "BsvHomeCore",
                "Primitives",
                "Localization",
                "Style",
                "Components",
                "PrimitivesComponents",
                "Store",
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                "Onboarding",
            ],
            path: "Sources",
        ),
        .testTarget(
            name: "ManageWalletsTests",
            dependencies: [
                "ManageWallets",
                "Localization",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "BsvHomeCoreServicesTestKit", package: "BsvHomeCoreServices"),
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                .product(name: "StoreTestKit", package: "Store"),
                "BsvHomeCore",
                "BsvHomeCorePrimitives",
                "Primitives",
                "Store",
            ],
        ),
    ],
)

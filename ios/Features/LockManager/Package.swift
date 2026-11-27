// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "LockManager",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "LockManager",
            targets: ["LockManager"],
        ),
    ],
    dependencies: [
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "Primitives", path: "../../Packages/Primitives"),
    ],
    targets: [
        .target(
            name: "LockManager",
            dependencies: [
                "Style",
                "Components",
                "Localization",
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                "BsvHomeCore",
                "Primitives",
            ],
            path: "Sources",
        ),
        .testTarget(
            name: "LockManagerTests",
            dependencies: [
                "LockManager",
                "BsvHomeCore",
                "BsvHomeCoreServices",
                "Primitives",
            ],
        ),
    ],
)

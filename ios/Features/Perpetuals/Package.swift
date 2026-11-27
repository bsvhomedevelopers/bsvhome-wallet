// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Perpetuals",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "Perpetuals",
            targets: ["Perpetuals"],
        ),
        .library(
            name: "PerpetualsTestKit",
            targets: ["PerpetualsTestKit"],
        ),
    ],
    dependencies: [
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "Store", path: "../../Packages/Store"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
        .package(name: "InfoSheet", path: "../InfoSheet"),
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "Recents", path: "../Recents"),
        .package(name: "Validators", path: "../../Packages/Validators"),
    ],
    targets: [
        .target(
            name: "Perpetuals",
            dependencies: [
                "BsvHomeCore",
                "Primitives",
                "PrimitivesComponents",
                "BsvHomeCorePrimitives",
                "Components",
                "Style",
                "Localization",
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                "Store",
                "Formatters",
                "InfoSheet",
                "Recents",
                "Validators",
            ],
            path: "Sources",
        ),
        .target(
            name: "PerpetualsTestKit",
            dependencies: [
                "Perpetuals",
                "Primitives",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                "Formatters",
                "BsvHomeCore",
                "BsvHomeCorePrimitives",
            ],
            path: "TestKit",
        ),
        .testTarget(
            name: "PerpetualsTests",
            dependencies: [
                .product(name: "StoreTestKit", package: "Store"),
                "Perpetuals",
                "PerpetualsTestKit",
                "Formatters",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                "Components",
                "BsvHomeCore",
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                "BsvHomeCoreServices",
                .product(name: "BsvHomeCoreServicesTestKit", package: "BsvHomeCoreServices"),
                "Localization",
                "Primitives",
                "Style",
            ],
            path: "Tests",
        ),
    ],
    swiftLanguageModes: [.v6],
)

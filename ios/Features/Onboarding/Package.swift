// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Onboarding",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "Onboarding",
            targets: ["Onboarding"],
        ),
    ],
    dependencies: [
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "Formatters", path: "../../Packages/Formatters"),
        .package(name: "InfoSheet", path: "../InfoSheet"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "QRScanner", path: "../QRScanner"),
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "Store", path: "../../Packages/Store"),
    ],
    targets: [
        .target(
            name: "Onboarding",
            dependencies: [
                "BsvHomeCore",
                "Primitives",
                "BsvHomeCorePrimitives",
                "Components",
                "InfoSheet",
                "Style",
                "Localization",
                "PrimitivesComponents",
                "QRScanner",
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                "Formatters",
                "Store",
            ],
            path: "Sources",
        ),
        .testTarget(
            name: "OnboardingTest",
            dependencies: [
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                "Onboarding",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "BsvHomeCoreServicesTestKit", package: "BsvHomeCoreServices"),
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                .product(name: "StoreTestKit", package: "Store"),
                "BsvHomeCore",
                "BsvHomeCorePrimitives",
                "Primitives",
                "PrimitivesComponents",
                "Store",
            ],
            path: "Tests",
        ),
    ],
)

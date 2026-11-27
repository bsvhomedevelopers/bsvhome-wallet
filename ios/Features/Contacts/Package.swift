// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Contacts",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "Contacts",
            targets: ["Contacts"],
        ),
    ],
    dependencies: [
        .package(name: "BsvHomeCore", path: "../../Packages/BsvHomeCore"),
        .package(name: "Primitives", path: "../../Packages/Primitives"),
        .package(name: "PrimitivesComponents", path: "../../Packages/PrimitivesComponents"),
        .package(name: "Components", path: "../../Packages/Components"),
        .package(name: "Localization", path: "../../Packages/Localization"),
        .package(name: "Style", path: "../../Packages/Style"),
        .package(name: "Store", path: "../../Packages/Store"),
        .package(name: "Validators", path: "../../Packages/Validators"),
        .package(name: "BsvHomeCorePrimitives", path: "../../Packages/BsvHomeCorePrimitives"),
        .package(name: "BsvHomeCoreServices", path: "../../Packages/BsvHomeCoreServices"),
        .package(name: "QRScanner", path: "../QRScanner"),
    ],
    targets: [
        .target(
            name: "Contacts",
            dependencies: [
                "BsvHomeCore",
                "Primitives",
                "PrimitivesComponents",
                "Components",
                "Localization",
                "Style",
                "Store",
                "Validators",
                "BsvHomeCorePrimitives",
                "QRScanner",
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
            ],
            path: "Sources",
        ),
        .testTarget(
            name: "ContactsTests",
            dependencies: [
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                "Contacts",
                "Components",
                "Primitives",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "BsvHomeCoreServices", package: "BsvHomeCoreServices"),
                .product(name: "BsvHomeCoreServicesTestKit", package: "BsvHomeCoreServices"),
            ],
        ),
    ],
)

// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Validators",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "Validators",
            targets: ["Validators"],
        ),
    ],
    dependencies: [
        .package(name: "Primitives", path: "../Primitives"),
        .package(name: "BsvHomeCore", path: "../BsvHomeCore"),
        .package(name: "BsvHomeCorePrimitives", path: "../BsvHomeCorePrimitives"),
        .package(name: "Localization", path: "../Localization"),
        .package(name: "Formatters", path: "../Formatters"),
        .package(name: "BigInt", path: "../../Submodules/BigInt"),
    ],
    targets: [
        .target(
            name: "Validators",
            dependencies: [
                "Primitives",
                "BsvHomeCore",
                "Localization",
                "Formatters",
                .product(name: "BigInt", package: "BigInt"),
            ],
            path: "Sources",
        ),
        .testTarget(
            name: "ValidatorsTests",
            dependencies: [
                "Validators",
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "BigInt", package: "BigInt"),
                "Formatters",
                "BsvHomeCore",
                "Primitives",
            ],
        ),
    ],
)

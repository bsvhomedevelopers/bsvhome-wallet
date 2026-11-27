// swift-tools-version: 6.0
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "PrimitivesComponents",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "PrimitivesComponents",
            targets: ["PrimitivesComponents"],
        ),
        .library(
            name: "PrimitivesComponentsTestKit",
            targets: ["PrimitivesComponentsTestKit"],
        ),
    ],
    dependencies: [
        .package(name: "BsvHomeCore", path: "../BsvHomeCore"),
        .package(name: "Primitives", path: "../Primitives"),
        .package(name: "BsvHomeCorePrimitives", path: "../BsvHomeCorePrimitives"),
        .package(name: "Localization", path: "../Localization"),
        .package(name: "Components", path: "../Components"),
        .package(name: "Style", path: "../Style"),
        .package(name: "Validators", path: "../Validators"),
        .package(name: "Formatters", path: "../Formatters"),
        .package(name: "BigInt", path: "../../Submodules/BigInt"),
    ],
    targets: [
        .target(
            name: "PrimitivesComponents",
            dependencies: [
                "BsvHomeCore",
                "Primitives",
                "BsvHomeCorePrimitives",
                "Localization",
                "Components",
                "Style",
                "Validators",
                "Formatters",
                .product(name: "BigInt", package: "BigInt"),
            ],
            path: "Sources",
        ),
        .target(
            name: "PrimitivesComponentsTestKit",
            dependencies: [
                "PrimitivesComponents",
                "Formatters",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "BigInt", package: "BigInt"),
                "BsvHomeCore",
                "Primitives",
            ],
            path: "TestKit",
        ),
        .testTarget(
            name: "PrimitivesComponentsTests",
            dependencies: [
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                "PrimitivesComponents",
                "PrimitivesComponentsTestKit",
                "BsvHomeCorePrimitives",
                "Formatters",
                .product(name: "BigInt", package: "BigInt"),
                "Components",
                "BsvHomeCore",
                "Localization",
                "Primitives",
                "Style",
                "Validators",
            ],
        ),
    ],
)

// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Store",
    platforms: [.iOS(.v17), .macOS(.v15)],
    products: [
        .library(
            name: "Store",
            targets: ["Store"],
        ),
        .library(
            name: "StoreTestKit",
            targets: ["StoreTestKit"],
        ),
    ],
    dependencies: [
        .package(name: "Primitives", path: "../Primitives"),
        .package(name: "BsvHomeCorePrimitives", path: "../BsvHomeCorePrimitives"),
        .package(name: "BsvHomeCore", path: "../BsvHomeCore"),
        .package(name: "GRDB", path: "../../Submodules/GRDB"),
        .package(name: "BigInt", path: "../../Submodules/BigInt"),
    ],
    targets: [
        .target(
            name: "Store",
            dependencies: [
                "Primitives",
                "BsvHomeCorePrimitives",
                "BsvHomeCore",
                .product(name: "GRDB", package: "GRDB"),
                .product(name: "BigInt", package: "BigInt"),
            ],
            path: "Sources",
        ),
        .target(
            name: "StoreTestKit",
            dependencies: [
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                "Store",
                "Primitives",
            ],
            path: "TestKit",
        ),
        .testTarget(
            name: "StoreTests",
            dependencies: [
                "BsvHomeCore",
                "Store",
                "StoreTestKit",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "GRDB", package: "GRDB"),
                "Primitives",
            ],
            resources: [.process("Resources")],
        ),
    ],
)

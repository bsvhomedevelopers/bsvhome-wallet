// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "BsvHomeCorePrimitives",
    platforms: [.iOS(.v17), .macOS(.v15)],
    products: [
        .library(
            name: "BsvHomeCorePrimitives",
            targets: ["BsvHomeCorePrimitives"],
        ),
        .library(
            name: "BsvHomeCorePrimitivesTestKit",
            targets: ["BsvHomeCorePrimitivesTestKit"],
        ),
    ],
    dependencies: [
        .package(name: "BsvHomeCore", path: "../BsvHomeCore"),
        .package(name: "Primitives", path: "../Primitives"),
        .package(name: "BigInt", path: "../../Submodules/BigInt"),
    ],
    targets: [
        .target(
            name: "BsvHomeCorePrimitives",
            dependencies: [
                "BsvHomeCore",
                "Primitives",
                .product(name: "BigInt", package: "BigInt"),
            ],
            path: "Sources",
        ),
        .target(
            name: "BsvHomeCorePrimitivesTestKit",
            dependencies: [
                "BsvHomeCore",
                "BsvHomeCorePrimitives",
                "Primitives",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "BigInt", package: "BigInt"),
            ],
            path: "TestKit",
            resources: [.process("Resources")],
        ),
        .testTarget(
            name: "BsvHomeCorePrimitivesTests",
            dependencies: [
                "BsvHomeCorePrimitives",
                "BsvHomeCorePrimitivesTestKit",
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                "BsvHomeCore",
                "Primitives",
            ],
        ),
    ],
)

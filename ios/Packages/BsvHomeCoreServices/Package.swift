// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "BsvHomeCoreServices",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "BsvHomeCoreServices",
            targets: ["BsvHomeCoreServices"],
        ),
        .library(
            name: "BsvHomeCoreServicesTestKit",
            targets: ["BsvHomeCoreServicesTestKit"],
        ),
    ],
    dependencies: [
        .package(name: "Primitives", path: "../Primitives"),
        .package(name: "SwiftHTTPClient", path: "../SwiftHTTPClient"),
        .package(name: "SystemServices", path: "../SystemServices"),
        .package(name: "NativeProviderService", path: "../NativeProviderService"),
        .package(name: "Keychain", path: "../Keychain"),
        .package(name: "BsvHomeCore", path: "../BsvHomeCore"),
        .package(name: "BsvHomeCorePrimitives", path: "../BsvHomeCorePrimitives"),
        .package(name: "Store", path: "../Store"),
        .package(name: "BigInt", path: "../../Submodules/BigInt"),
    ],
    targets: [
        .target(
            name: "BsvHomeCoreServices",
            dependencies: [
                "Primitives",
                "BsvHomeCore",
                "BsvHomeCorePrimitives",
                "Store",
                .product(name: "LocalStore", package: "SystemServices"),
                "Keychain",
                "NativeProviderService",
                .product(name: "BigInt", package: "BigInt"),
                .product(name: "WebSocketClient", package: "SwiftHTTPClient"),
            ],
            path: "Sources",
        ),
        .target(
            name: "BsvHomeCoreServicesTestKit",
            dependencies: [
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                .product(name: "StoreTestKit", package: "Store"),
                .product(name: "NativeProviderService", package: "NativeProviderService"),
                "BsvHomeCore",
                "BsvHomeCoreServices",
                "Keychain",
                .product(name: "BigInt", package: "BigInt"),
                "BsvHomeCorePrimitives",
                "Primitives",
                "Store",
            ],
            path: "TestKit",
            resources: [.process("Resources")],
        ),
        .testTarget(
            name: "BsvHomeCoreServicesTests",
            dependencies: [
                .product(name: "BsvHomeCorePrimitivesTestKit", package: "BsvHomeCorePrimitives"),
                .product(name: "PrimitivesTestKit", package: "Primitives"),
                .product(name: "StoreTestKit", package: "Store"),
                "Primitives",
                .product(name: "Store", package: "Store"),
                "BsvHomeCore",
                "BsvHomeCoreServices",
                "BsvHomeCoreServicesTestKit",
                "BsvHomeCorePrimitives",
                "Keychain",
                .product(name: "WebSocketClient", package: "SwiftHTTPClient"),
            ],
            path: "Tests",
            resources: [.process("Resources")],
        ),
    ],
)

// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "NativeProviderService",
    platforms: [
        .iOS(.v17),
        .macOS(.v15),
    ],
    products: [
        .library(
            name: "NativeProviderService",
            targets: ["NativeProviderService"],
        ),
    ],
    dependencies: [
        .package(name: "Primitives", path: "../Primitives"),
        .package(name: "BsvHomeCore", path: "../BsvHomeCore"),
    ],
    targets: [
        .target(
            name: "NativeProviderService",
            dependencies: [
                "Primitives",
                "BsvHomeCore",
            ],
            path: "Sources",
        ),
        .testTarget(
            name: "NativeProviderServiceTests",
            dependencies: [
                "NativeProviderService",
                "BsvHomeCore",
            ],
        ),
    ],
)

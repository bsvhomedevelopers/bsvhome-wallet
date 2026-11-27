// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "BsvHomeAPI",
    platforms: [.iOS(.v17), .macOS(.v15)],
    products: [
        .library(
            name: "BsvHomeAPI",
            targets: ["BsvHomeAPI"],
        ),
    ],
    dependencies: [
        .package(name: "Primitives", path: "../Primitives"),
        .package(name: "SwiftHTTPClient", path: "../SwiftHTTPClient"),
    ],
    targets: [
        .target(
            name: "BsvHomeAPI",
            dependencies: [
                "Primitives",
                "SwiftHTTPClient",
            ],
            path: "Sources",
        ),
    ],
)

// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "BsvHomeTest",
    platforms: [
        .iOS(.v17), .macOS(.v15)
    ],
    products: [
        .library(
            name: "BsvHomeTest",
            targets: ["BsvHomeTest"]
        )
    ],
    dependencies: [
        .package(path: "../Packages/BsvHomeCore")
    ],
    targets: [
        .target(
            name: "BsvHomeTest",
            dependencies: [
                .product(name: "BsvHomeCore", package: "BsvHomeCore")
            ],
            path: "BsvHomeTest",
            exclude: [
                "Assets.xcassets",
                "BsvHomeTestApp.swift",
                "Preview Content"
            ],
            swiftSettings: [
                .swiftLanguageMode(.v5)
            ]
        ),
        .testTarget(
            name: "BsvHomeTestTests",
            dependencies: ["BsvHomeTest"],
            path: "BsvHomeTestTests",
            swiftSettings: [
                .swiftLanguageMode(.v5)
            ]
        )
    ]
)

// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "BsvHomeCore",
    platforms: [
        .iOS(.v17), .macOS(.v15)
    ],
    products: [
        .library(
            name: "BsvHomeCore",
            targets: ["BsvHomeCore"]
        )
    ],
    dependencies: [
    ],
    targets: [
        .target(
            name: "BsvHomeCore",
            dependencies: ["BsvHomeCoreFFI"],
            swiftSettings: [
                .swiftLanguageMode(.v5)
            ]
        ),
        .target(
            name: "BsvHomeCoreFFI",
            path: "Sources/BsvHomeCoreFFI",
            publicHeadersPath: "include"
        )
    ]
)

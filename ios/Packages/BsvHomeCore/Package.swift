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
        .package(name: "BigInt", path: "../../Submodules/BigInt"),
    ],
    targets: [
        .target(
            name: "BsvHomeCore",
            dependencies: [
                "BsvHomeCoreFFI",
                .product(name: "BigInt", package: "BigInt"),
            ],
            swiftSettings: [
                .swiftLanguageMode(.v5) // TODO: - remove when BsvHomeCoreFFI will support swift6 fully
            ]
        ),
        .target(
            name: "BsvHomeCoreFFI",
            path: "Sources/BsvHomeCoreFFI",
            publicHeadersPath: "include"
        )
    ]
)

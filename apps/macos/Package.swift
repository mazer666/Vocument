// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "VocumentMac",
    platforms: [
        .macOS(.v15)
    ],
    products: [
        .library(
            name: "VocumentMacKit",
            targets: ["VocumentMacKit"]
        )
    ],
    targets: [
        .target(
            name: "VocumentMacKit"
        ),
        .testTarget(
            name: "VocumentMacKitTests",
            dependencies: ["VocumentMacKit"]
        )
    ]
)

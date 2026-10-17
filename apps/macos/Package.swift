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
        ),
        .executable(
            name: "VocumentMacApp",
            targets: ["VocumentMacApp"]
        ),
        .executable(
            name: "vocument-mac-handshake",
            targets: ["VocumentMacHandshakeCLI"]
        )
    ],
    targets: [
        .target(
            name: "vocument_ffiFFI",
            path: "Sources/VocumentFFIHeaders",
            publicHeadersPath: "include"
        ),
        .target(
            name: "VocumentFFI",
            dependencies: ["vocument_ffiFFI"],
            path: "Sources/VocumentFFI",
            swiftSettings: [
                .swiftLanguageMode(.v5)
            ],
            linkerSettings: [
                .unsafeFlags([
                    "-L../../target/debug",
                    "-lvocument_ffi",
                    "-L/opt/homebrew/opt/mupdf/lib",
                    "-lmupdf",
                    "-Xlinker",
                    "-rpath",
                    "-Xlinker",
                    "/opt/homebrew/opt/mupdf/lib"
                ])
            ]
        ),
        .target(
            name: "VocumentMacKit",
            dependencies: ["VocumentFFI"],
            path: "Sources/VocumentMacKit"
        ),
        .executableTarget(
            name: "VocumentMacApp",
            dependencies: ["VocumentMacKit"],
            path: "Sources/VocumentMacApp"
        ),
        .executableTarget(
            name: "VocumentMacHandshakeCLI",
            dependencies: ["VocumentMacKit"],
            path: "Sources/VocumentMacHandshakeCLI"
        ),
        .testTarget(
            name: "VocumentMacKitTests",
            dependencies: ["VocumentMacKit"],
            path: "Tests/VocumentMacKitTests"
        )
    ]
)

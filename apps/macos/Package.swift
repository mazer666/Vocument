// swift-tools-version: 6.0
import PackageDescription
import Foundation

// The library of the FFI and the MuPDF that it needs are built next to the
// repository. On a Mac they come from the package manager of the platform; on
// every other system the environment says where they are — so that the data
// path of the application can be **compiled and tested** there as well.
//
// The macOS setting is unchanged: the same paths, the same flags, the same
// binary.
let environment = ProcessInfo.processInfo.environment
let mupdfDirectory = environment["MUPDF_DIR"] ?? "/opt/homebrew/opt/mupdf"
let repositoryRoot = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent()
    .deletingLastPathComponent()
    .deletingLastPathComponent()
    .path

var ffiLinkerFlags: [String] = [
    "-L\(repositoryRoot)/target/debug",
    "-lvocument_ffi",
    // The library of the FFI is found at the run time without an environment
    // variable: the path of the build directory stands in the binary.
    "-Xlinker",
    "-rpath",
    "-Xlinker",
    "\(repositoryRoot)/target/debug",
]
if FileManager.default.fileExists(atPath: "\(mupdfDirectory)/lib") {
    ffiLinkerFlags.append(contentsOf: [
        "-L\(mupdfDirectory)/lib",
        "-lmupdf",
        "-Xlinker",
        "-rpath",
        "-Xlinker",
        "\(mupdfDirectory)/lib",
    ])
}

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
                .unsafeFlags(ffiLinkerFlags)
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

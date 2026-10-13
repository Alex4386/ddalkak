// swift-tools-version:5.9
// Run `scripts/build-apple.sh` first: it builds ClicksCoreFFI.xcframework and
// generates Sources/ClicksCore/ClicksCore.swift from the Rust crate.
import PackageDescription

let package = Package(
    name: "ClicksKit",
    platforms: [.macOS(.v14), .iOS(.v17)],
    products: [
        .library(name: "ClicksKit", targets: ["ClicksKit"]),
        .library(name: "ClicksCore", targets: ["ClicksCore"]),
    ],
    targets: [
        .binaryTarget(name: "ClicksCoreFFI", path: "ClicksCoreFFI.xcframework"),
        .target(name: "ClicksCore", dependencies: ["ClicksCoreFFI"]),
        .target(name: "ClicksKit", dependencies: ["ClicksCore"]),
    ]
)

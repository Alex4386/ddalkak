#!/usr/bin/env bash
# Builds swift/ClicksCoreFFI.xcframework and the generated Swift bindings.
# Requires macOS with Xcode and the Rust targets below (`rustup target add ...`).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

MACOS=(aarch64-apple-darwin x86_64-apple-darwin)
IOS=(aarch64-apple-ios)
IOS_SIM=(aarch64-apple-ios-sim x86_64-apple-ios)
PROFILE=release
LIB=libclicks_ffi.a

for t in "${MACOS[@]}" "${IOS[@]}" "${IOS_SIM[@]}"; do
  cargo build -p clicks-ffi --profile "$PROFILE" --target "$t"
done

BUILD="$ROOT/target/apple"
rm -rf "$BUILD" && mkdir -p "$BUILD"/{macos,ios-sim,headers}

lipo -create $(printf "target/%s/$PROFILE/$LIB " "${MACOS[@]}") -output "$BUILD/macos/$LIB"
lipo -create $(printf "target/%s/$PROFILE/$LIB " "${IOS_SIM[@]}") -output "$BUILD/ios-sim/$LIB"

# Bindings are generated from the host dylib; the FFI surface is identical on every target.
cargo build -p clicks-ffi --features bindgen --profile "$PROFILE"
cargo run -q -p clicks-ffi --features bindgen --profile "$PROFILE" --bin uniffi-bindgen -- \
  generate --library "target/$PROFILE/libclicks_ffi.dylib" --language swift --out-dir "$BUILD/gen"

mkdir -p swift/Sources/ClicksCore
cp "$BUILD/gen/ClicksCore.swift" swift/Sources/ClicksCore/
cp "$BUILD/gen/ClicksCoreFFI.h" "$BUILD/headers/"
cp "$BUILD/gen/ClicksCoreFFI.modulemap" "$BUILD/headers/module.modulemap"

rm -rf swift/ClicksCoreFFI.xcframework
xcodebuild -create-xcframework \
  -library "$BUILD/macos/$LIB" -headers "$BUILD/headers" \
  -library "target/${IOS[0]}/$PROFILE/$LIB" -headers "$BUILD/headers" \
  -library "$BUILD/ios-sim/$LIB" -headers "$BUILD/headers" \
  -output swift/ClicksCoreFFI.xcframework

echo "swift/ClicksCoreFFI.xcframework ready"

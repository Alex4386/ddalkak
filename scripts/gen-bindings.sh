#!/usr/bin/env bash
# Generates Swift and Kotlin bindings into target/bindings (any host OS).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p clicks-ffi --features bindgen
case "$(uname -s)" in
  Darwin) LIB=target/debug/libclicks_ffi.dylib ;;
  MINGW*|MSYS*|CYGWIN*) LIB=target/debug/clicks_ffi.dll ;;
  *) LIB=target/debug/libclicks_ffi.so ;;
esac
for lang in swift kotlin; do
  cargo run -q -p clicks-ffi --features bindgen --bin uniffi-bindgen -- \
    generate --library "$LIB" --language "$lang" --out-dir "target/bindings/$lang"
done
echo "bindings in target/bindings/"

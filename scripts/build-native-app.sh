#!/bin/zsh
set -euo pipefail

ROOT_DIR=${0:A:h:h}
BUILD_DIR="$ROOT_DIR/build/native"
APP_PATH="$BUILD_DIR/Mass Print.app"
SOURCES=("$ROOT_DIR"/NativeApp/*.swift)
TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)

for target in $TARGETS; do
  rustup target add "$target"
  cargo build --manifest-path "$ROOT_DIR/Cargo.toml" -p mass-print-ffi --release --target "$target"
done

rm -rf -- "$APP_PATH"
mkdir -p -- "$APP_PATH/Contents/MacOS" "$APP_PATH/Contents/Resources" "$BUILD_DIR/bin"
cp "$ROOT_DIR/NativeApp/Info.plist" "$APP_PATH/Contents/Info.plist"
cp "$ROOT_DIR/NativeApp/Resources/icon.icns" "$APP_PATH/Contents/Resources/icon.icns"

for architecture in arm64 x86_64; do
  if [[ "$architecture" == arm64 ]]; then
    rust_target=aarch64-apple-darwin
  else
    rust_target=x86_64-apple-darwin
  fi
  xcrun swiftc \
    -parse-as-library \
    -O \
    -target "$architecture-apple-macos13.0" \
    "${SOURCES[@]}" \
    "$ROOT_DIR/target/$rust_target/release/libmass_print_ffi.a" \
    -framework AppKit \
    -framework SwiftUI \
    -framework UniformTypeIdentifiers \
    -framework CoreServices \
    -o "$BUILD_DIR/bin/Mass Print-$architecture"
done

lipo -create \
  "$BUILD_DIR/bin/Mass Print-arm64" \
  "$BUILD_DIR/bin/Mass Print-x86_64" \
  -output "$APP_PATH/Contents/MacOS/Mass Print"

chmod +x "$APP_PATH/Contents/MacOS/Mass Print"
plutil -lint "$APP_PATH/Contents/Info.plist"
codesign --force --deep --sign - "$APP_PATH"
codesign --verify --deep --strict "$APP_PATH"
file "$APP_PATH/Contents/MacOS/Mass Print"
print "$APP_PATH"

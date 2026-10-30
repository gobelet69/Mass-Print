#!/bin/zsh
set -euo pipefail

ROOT_DIR=${0:A:h:h}
BUILD_DIR="$ROOT_DIR/build/native"
APP_PATH="$BUILD_DIR/Mass Print.app"
CACHE_KEY=$(print -n "$ROOT_DIR" | cksum | awk '{print $1}')
USER_CACHE_ROOT=$(getconf DARWIN_USER_CACHE_DIR)
CACHE_DIR="${USER_CACHE_ROOT%/}/com.massprint.native-build/$CACHE_KEY"
CACHE_APP="$CACHE_DIR/Mass Print.app"
STAGE_DIR=$(mktemp -d "${TMPDIR:-/tmp}/mass-print-native.XXXXXX")
STAGED_APP="$STAGE_DIR/Mass Print.app"
SOURCES=("$ROOT_DIR"/NativeApp/*.swift)
TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
SIGNING_IDENTITY=${MASS_PRINT_CODESIGN_IDENTITY:--}
trap 'rm -rf -- "$STAGE_DIR"' EXIT

for target in $TARGETS; do
  rustup target add "$target"
  cargo build --manifest-path "$ROOT_DIR/Cargo.toml" -p mass-print-ffi --release --target "$target"
done

mkdir -p -- "$STAGED_APP/Contents/MacOS" "$STAGED_APP/Contents/Resources" "$BUILD_DIR/bin"
cp "$ROOT_DIR/NativeApp/Info.plist" "$STAGED_APP/Contents/Info.plist"
cp "$ROOT_DIR/NativeApp/Resources/icon.icns" "$STAGED_APP/Contents/Resources/icon.icns"

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
  -output "$STAGED_APP/Contents/MacOS/Mass Print"

chmod +x "$STAGED_APP/Contents/MacOS/Mass Print"
plutil -lint "$STAGED_APP/Contents/Info.plist"
xattr -cr "$STAGED_APP"
if [[ "$SIGNING_IDENTITY" == "-" ]]; then
  codesign --force --deep --sign - "$STAGED_APP"
else
  codesign --force --deep --options runtime --timestamp --sign "$SIGNING_IDENTITY" "$STAGED_APP"
fi
codesign --verify --deep --strict "$STAGED_APP"

mkdir -p -- "$CACHE_DIR"
rm -rf -- "$CACHE_APP" "$APP_PATH"
ditto "$STAGED_APP" "$CACHE_APP"
xattr -cr "$CACHE_APP"
codesign --verify --deep --strict "$CACHE_APP"
ln -s "$CACHE_APP" "$APP_PATH"
codesign --verify --deep --strict "$APP_PATH"
file "$APP_PATH/Contents/MacOS/Mass Print"
print "$APP_PATH"

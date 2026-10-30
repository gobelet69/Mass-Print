#!/bin/zsh
set -euo pipefail

ROOT_DIR=${0:A:h:h}
CARGO_VERSION=$(awk '
  /^\[workspace\.package\]$/ { in_package = 1; next }
  /^\[/ { in_package = 0 }
  in_package && /^version = / {
    gsub(/"/, "", $3)
    print $3
    exit
  }
' "$ROOT_DIR/Cargo.toml")
APP_VERSION=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$ROOT_DIR/NativeApp/Info.plist")
BUILD_VERSION=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$ROOT_DIR/NativeApp/Info.plist")
CHANGELOG_VERSION=$(awk '
  match($0, /^## \[[0-9]+\.[0-9]+\.[0-9]+\]/) {
    version = substr($0, RSTART + 4, RLENGTH - 5)
    print version
    exit
  }
' "$ROOT_DIR/CHANGELOG.md")

if [[ -z "$CARGO_VERSION" || -z "$APP_VERSION" || -z "$BUILD_VERSION" || -z "$CHANGELOG_VERSION" ]]; then
  print -u2 "release version metadata is incomplete"
  exit 1
fi

if [[ "$CARGO_VERSION" != "$APP_VERSION" || "$CARGO_VERSION" != "$BUILD_VERSION" || "$CARGO_VERSION" != "$CHANGELOG_VERSION" ]]; then
  print -u2 "version mismatch: Cargo=$CARGO_VERSION app=$APP_VERSION build=$BUILD_VERSION changelog=$CHANGELOG_VERSION"
  exit 1
fi

if [[ -n "${GITHUB_REF_NAME:-}" && "$GITHUB_REF_NAME" == v* && "$GITHUB_REF_NAME" != "v$CARGO_VERSION" ]]; then
  print -u2 "tag mismatch: tag=$GITHUB_REF_NAME version=$CARGO_VERSION"
  exit 1
fi

print "Mass Print release version $CARGO_VERSION is consistent"

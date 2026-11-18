#!/bin/zsh
set -euo pipefail

ROOT_DIR=${0:A:h:h}
APP_LINK="$ROOT_DIR/build/native/Mass Print.app"
APP_PATH=${APP_LINK:A}
DIST_DIR="$ROOT_DIR/dist-native"
VERSION=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP_PATH/Contents/Info.plist")

rm -rf -- "$DIST_DIR"
mkdir -p -- "$DIST_DIR"
STAGE_DIR=$(mktemp -d "${TMPDIR:-/tmp}/mass-print-dmg.XXXXXX")
trap 'rm -rf -- "$STAGE_DIR"' EXIT
ditto "$APP_PATH" "$STAGE_DIR/Mass Print.app"
ln -s /Applications "$STAGE_DIR/Applications"
ditto -c -k --sequesterRsrc --keepParent "$APP_PATH" "$DIST_DIR/Mass.Print_${VERSION}_universal.app.zip"
hdiutil create \
  -volname "Mass Print" \
  -srcfolder "$STAGE_DIR" \
  -ov \
  -format UDZO \
  "$DIST_DIR/Mass.Print_${VERSION}_universal.dmg"

(cd "$DIST_DIR" && shasum -a 256 Mass.Print* > SHA256SUMS.txt && cat SHA256SUMS.txt)
